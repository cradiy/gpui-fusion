use crate::config::{Management, Project, Variables, expand, relative};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

const STATE: &str = ".gpuiforge-generated.json";

#[derive(Serialize, Deserialize)]
struct State {
    owner: PathBuf,
    files: BTreeMap<PathBuf, String>,
}
struct File {
    bytes: Vec<u8>,
    permissions: Option<fs::Permissions>,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn collect(
    source: &Path,
    destination: &Path,
    vars: Option<&Variables>,
    files: &mut BTreeMap<PathBuf, File>,
) -> Result<()> {
    let metadata =
        fs::symlink_metadata(source).with_context(|| format!("reading {}", source.display()))?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "template/copy sources cannot be symlinks: {}",
        source.display()
    );
    if metadata.is_dir() {
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            collect(
                &entry.path(),
                &destination.join(entry.file_name()),
                vars,
                files,
            )?;
        }
    } else {
        ensure!(
            metadata.is_file(),
            "not a regular file: {}",
            source.display()
        );
        let mut destination = destination.to_owned();
        let mut bytes = fs::read(source)?;
        if source.extension().is_some_and(|e| e == "tmpl")
            && let Some(vars) = vars
        {
            destination.set_extension("");
            bytes = expand(std::str::from_utf8(&bytes)?, vars)?.into_bytes();
        }
        relative(&destination)?;
        ensure!(
            destination != Path::new(STATE),
            "reserved generated filename"
        );
        ensure!(
            files
                .insert(
                    destination.clone(),
                    File {
                        bytes,
                        permissions: Some(metadata.permissions())
                    }
                )
                .is_none(),
            "duplicate generated path: {}",
            destination.display()
        );
    }
    Ok(())
}

fn no_symlinks(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        if let Ok(metadata) = fs::symlink_metadata(ancestor) {
            ensure!(
                !metadata.file_type().is_symlink(),
                "refusing generated writes through symlink: {}",
                ancestor.display()
            );
        }
    }
    Ok(())
}

pub fn generate(project: &Project, name: &str) -> Result<PathBuf> {
    synchronize(project, name, false)
}

pub fn check(project: &Project, name: &str) -> Result<PathBuf> {
    synchronize(project, name, true)
}

fn synchronize(project: &Project, name: &str, check: bool) -> Result<PathBuf> {
    let platform = project.platform(name)?;
    ensure!(
        platform.management == Management::Managed,
        "{name} is manually managed; generation is disabled"
    );
    let directory = project.directory(name)?;
    no_symlinks(&directory)?;
    let vars = project.variables(name, false, None)?;
    let mut files = BTreeMap::new();
    if platform.bundled {
        for &(name, bytes) in crate::ANDROID_FILES {
            if !crate::android_features::included(name, platform) {
                continue;
            }
            let bytes = crate::android_features::render(bytes, platform)?;
            let (path, bytes) = if let Some(path) = name.strip_suffix(".tmpl") {
                (
                    path,
                    expand(std::str::from_utf8(&bytes)?, &vars)?.into_bytes(),
                )
            } else {
                (name, bytes)
            };
            #[cfg(unix)]
            let permissions = {
                use std::os::unix::fs::PermissionsExt;
                Some(fs::Permissions::from_mode(
                    if matches!(path, "gradlew" | "build-rust.sh") {
                        0o755
                    } else {
                        0o644
                    },
                ))
            };
            #[cfg(not(unix))]
            let permissions = None;
            files.insert(PathBuf::from(path), File { bytes, permissions });
        }
    } else {
        let template = platform
            .template
            .as_ref()
            .context("this platform has no project template")?;
        collect(template, Path::new(""), Some(&vars), &mut files)?;
    }
    for copy in &platform.copies {
        collect(&copy.from, &copy.to, None, &mut files)?;
    }
    if platform.bundled {
        if platform.cleartext_traffic == Some(true) || !platform.cleartext_domains.is_empty() {
            let resource = PathBuf::from("app/src/main/res/xml/network_security_config.xml");
            ensure!(
                !files.contains_key(&resource),
                "cleartext settings conflict with a copied network_security_config.xml"
            );
            let path = if platform.merge_network_config() {
                PathBuf::from("app/network-security-config.xml")
            } else {
                resource
            };
            let mut xml = format!(
                "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<network-security-config>\n    <base-config cleartextTrafficPermitted=\"{}\" />\n",
                platform.cleartext_traffic == Some(true)
            );
            if !platform.cleartext_domains.is_empty() {
                xml.push_str("    <domain-config cleartextTrafficPermitted=\"true\">\n");
                for host in &platform.cleartext_domains {
                    xml.push_str(&format!(
                        "        <domain includeSubdomains=\"false\">{host}</domain>\n"
                    ));
                }
                xml.push_str("    </domain-config>\n");
            }
            xml.push_str("</network-security-config>\n");
            ensure!(
                files
                    .insert(
                        path,
                        File {
                            bytes: xml.into_bytes(),
                            permissions: None
                        }
                    )
                    .is_none(),
                "cleartext settings conflict with a copied network-security-config.xml"
            );
        }
        if platform.notification_icon.is_some() {
            let path = PathBuf::from("app/src/main/res/raw/gpui_notification_keep.xml");
            ensure!(
                !files.contains_key(&path),
                "duplicate generated path: {}",
                path.display()
            );
            files.insert(path, File {
                bytes: br#"<resources xmlns:tools="http://schemas.android.com/tools" tools:keep="@drawable/gpui_notification_icon" />"#.to_vec(),
                permissions: None,
            });
        }
        for (setting, source, resource) in [
            ("icon", &platform.icon, "gpui_app_icon"),
            (
                "notification-icon",
                &platform.notification_icon,
                "gpui_notification_icon",
            ),
        ] {
            if let Some(source) = source {
                ensure!(
                    source.is_file(),
                    "platforms.android.{setting} must point to an existing file: {}",
                    source.display()
                );
                let extension = source
                    .extension()
                    .and_then(|s| s.to_str())
                    .context("Android icon needs a file extension")?;
                ensure!(
                    matches!(extension, "png" | "webp" | "xml"),
                    "Android icons must be PNG, WebP or Android drawable XML: {}",
                    source.display()
                );
                collect(
                    source,
                    &PathBuf::from(format!(
                        "app/src/main/res/drawable-nodpi/{resource}.{extension}"
                    )),
                    None,
                    &mut files,
                )?;
            }
        }
    }
    ensure!(!files.is_empty(), "template contains no files");
    let state_path = directory.join(STATE);
    no_symlinks(&state_path)?;
    let previous = if state_path.exists() {
        let state: State = serde_json::from_slice(&fs::read(&state_path)?)?;
        ensure!(
            state.owner == project.config_path,
            "generated directory belongs to a different project"
        );
        state
    } else {
        ensure!(
            !directory.exists() || fs::read_dir(&directory)?.next().is_none(),
            "refusing to adopt nonempty directory {}; choose another project-dir",
            directory.display()
        );
        State {
            owner: project.config_path.clone(),
            files: BTreeMap::new(),
        }
    };
    for (path, hash) in &previous.files {
        relative(path)?;
        let path = directory.join(path);
        no_symlinks(&path)?;
        ensure!(
            path.is_file() && digest(&fs::read(&path)?) == *hash,
            "generated file was edited or removed: {}; restore it or use 'platform eject {name}' to preserve edits",
            path.display()
        );
    }
    for path in files.keys() {
        let dest = directory.join(path);
        no_symlinks(&dest)?;
        ensure!(
            !dest.exists() || previous.files.contains_key(path),
            "refusing to overwrite untracked file {}",
            dest.display()
        );
    }
    if check {
        let mut changes = Vec::new();
        if !state_path.exists() {
            changes.push(format!("create {STATE}"));
        }
        for (path, file) in &files {
            let action = match previous.files.get(path) {
                None => Some("create"),
                Some(hash) if *hash != digest(&file.bytes) => Some("update"),
                Some(_) if permissions_differ(&directory.join(path), file)? => Some("permissions"),
                Some(_) => None,
            };
            if let Some(action) = action {
                changes.push(format!("{action} {}", path.display()));
            }
        }
        for path in previous
            .files
            .keys()
            .filter(|path| !files.contains_key(*path))
        {
            changes.push(format!("remove {}", path.display()));
        }
        ensure!(
            changes.is_empty(),
            "{name} needs synchronization; run `gpuiforge sync {name}`:\n{}",
            changes.join("\n")
        );
        return Ok(directory);
    }
    fs::create_dir_all(&directory)?;
    for (path, file) in &files {
        let dest = directory.join(path);
        fs::create_dir_all(dest.parent().unwrap())?;
        if fs::read(&dest).ok().as_deref() != Some(&file.bytes) {
            fs::write(&dest, &file.bytes)?;
        }
        if let Some(permissions) = &file.permissions {
            fs::set_permissions(&dest, permissions.clone())?;
        }
    }
    for path in previous
        .files
        .keys()
        .filter(|path| !files.contains_key(*path))
    {
        fs::remove_file(directory.join(path))?;
    }
    let state = State {
        owner: project.config_path.clone(),
        files: files
            .into_iter()
            .map(|(path, file)| (path, digest(&file.bytes)))
            .collect(),
    };
    fs::write(state_path, serde_json::to_vec_pretty(&state)?)?;
    Ok(directory)
}

fn permissions_differ(path: &Path, file: &File) -> Result<bool> {
    let Some(expected) = &file.permissions else {
        return Ok(false);
    };
    let actual = fs::metadata(path)?.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok(actual.mode() & 0o777 != expected.mode() & 0o777)
    }
    #[cfg(not(unix))]
    {
        Ok(actual.readonly() != expected.readonly())
    }
}

pub fn eject(project: &Project, name: &str, destination: &Path) -> Result<PathBuf> {
    relative(destination)?;
    let platform = project.platform(name)?;
    ensure!(
        platform.management == Management::Managed,
        "{name} is already manually managed"
    );
    let output = project.root.join(destination);
    no_symlinks(&output)?;
    ensure!(
        !output.exists(),
        "destination already exists: {}",
        output.display()
    );
    let source = project.directory(name)?;
    ensure!(
        !output.starts_with(&source) && !source.starts_with(&output),
        "export and generated directories must not overlap"
    );
    if !source.join(STATE).exists() {
        generate(project, name)?;
    }
    no_symlinks(&source.join(STATE))?;
    let state: State = serde_json::from_slice(&fs::read(source.join(STATE))?)?;
    ensure!(
        state.owner == project.config_path,
        "generated directory belongs to a different project"
    );
    // Export source files, including user edits and additions, but no build caches.
    let mut files = BTreeMap::new();
    collect_export(&source, Path::new(""), &mut files)?;
    let mut document: serde_json::Value = fs::read_to_string(&project.config_path)?.parse()?;
    document["platforms"][name]["management"] = serde_json::json!("manual");
    document["platforms"][name]["project-dir"] =
        serde_json::json!(destination.to_str().context("project-dir must be UTF-8")?);
    fs::create_dir_all(&output)?;
    for (path, file) in files {
        let target = output.join(path);
        fs::create_dir_all(target.parent().unwrap())?;
        fs::write(&target, file.bytes)?;
        if let Some(permissions) = file.permissions {
            fs::set_permissions(target, permissions)?;
        }
    }
    fs::write(
        &project.config_path,
        serde_json::to_string_pretty(&document)? + "\n",
    )?;
    Ok(output)
}

fn collect_export(
    source: &Path,
    destination: &Path,
    files: &mut BTreeMap<PathBuf, File>,
) -> Result<()> {
    no_symlinks(source)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        if matches!(
            entry.file_name().to_str(),
            Some(
                "build" | ".gradle" | ".kotlin" | ".gpuiforge-generated.json" | "local.properties"
            )
        ) {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.is_dir() {
            collect_export(&entry.path(), &destination.join(entry.file_name()), files)?;
        } else {
            collect(
                &entry.path(),
                &destination.join(entry.file_name()),
                None,
                files,
            )?;
        }
    }
    Ok(())
}
