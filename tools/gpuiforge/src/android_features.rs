use crate::config::{AndroidFeature, Platform};
use anyhow::{Context, Result, ensure};
use std::path::Path;

pub fn included(path: &str, platform: &Platform) -> bool {
    use AndroidFeature::*;
    let feature = match Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
    {
        "FilePickerHost.kt"
        | "SelectedDocument.kt"
        | "DocumentGrants.kt"
        | "FileStore.kt"
        | "GpuiFileProvider.kt"
        | "gpui_file_paths.xml" => Files,
        "IncomingShare.kt" | "ShareIntent.kt" => Sharing,
        "CredentialStore.kt" => Credentials,
        "MediaSession.kt" | "MediaFrames.kt" | "MediaAudioTracks.kt" | "SystemMediaControls.kt" => Media,
        "NotificationStore.kt" => Notifications,
        "MediaNotification.kt" => MediaNotifications,
        "DataSyncService.kt" => DataSync,
        "MediaPlaybackService.kt" => BackgroundMedia,
        _ => return true,
    };
    platform.feature(feature)
}

/// Resolve feature blocks before expanding ordinary template variables.
pub fn render(bytes: &[u8], platform: &Platform) -> Result<Vec<u8>> {
    let Ok(source) = std::str::from_utf8(bytes) else {
        return Ok(bytes.to_vec());
    };
    let mut stack = Vec::<(bool, bool, bool)>::new();
    let mut enabled = true;
    let mut output = String::new();
    for line in source.split_inclusive('\n') {
        let directive = line
            .trim()
            .strip_prefix("// gpuiforge:")
            .or_else(|| line.trim().strip_prefix("# gpuiforge:"))
            .or_else(|| {
                line.trim()
                    .strip_prefix("<!-- gpuiforge:")
                    .and_then(|s| s.strip_suffix(" -->"))
            });
        if let Some(directive) = directive {
            if let Some(name) = directive.strip_prefix("if ") {
                let feature: AndroidFeature =
                    serde::Deserialize::deserialize(serde::de::value::StrDeserializer::<
                        serde::de::value::Error,
                    >::new(name))
                    .context("unknown host feature")?;
                let selected = platform.feature(feature);
                stack.push((enabled, selected, false));
                enabled &= selected;
            } else if directive == "else" {
                let (parent, selected, seen_else) =
                    stack.last_mut().context("unmatched feature else")?;
                ensure!(!*seen_else, "duplicate feature else");
                *seen_else = true;
                enabled = *parent && !*selected;
            } else if directive == "endif" {
                enabled = stack.pop().context("unmatched feature endif")?.0;
            } else {
                anyhow::bail!("invalid host feature directive {directive}");
            }
        } else if enabled {
            output.push_str(line);
        }
    }
    ensure!(stack.is_empty(), "unterminated host feature block");
    Ok(output.into_bytes())
}
