use std::{path::PathBuf, time::Duration};

use ashpd::desktop::settings::Settings;

pub(crate) fn family() -> Option<String> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let is_desktop = |name: &str| {
        desktop
            .split(':')
            .any(|part| part.eq_ignore_ascii_case(name))
    };
    let kde = is_desktop("KDE");
    let family = pollster::block_on(smol::future::or(
        read_settings(kde, is_desktop("XFCE")),
        async {
            #[expect(
                clippy::disallowed_methods,
                reason = "bounds startup before the platform executor exists"
            )]
            smol::Timer::after(Duration::from_millis(200)).await;
            None
        },
    ));
    family.or_else(|| configured_family(kde))
}

async fn read_settings(kde: bool, xfce: bool) -> Option<String> {
    if xfce {
        if let Some(family) = read_xfconf().await {
            return Some(family);
        }
    }
    let settings = Settings::new().await.ok()?;
    let mut keys = [
        ("org.gnome.desktop.interface", "font-name", false),
        ("org.kde.kdeglobals.General", "font", true),
    ];
    if kde {
        keys.swap(0, 1);
    }
    for (namespace, key, qt) in keys {
        if let Ok(value) = settings.read::<String>(namespace, key).await {
            if let Some(family) = parse_family(&value, qt) {
                return Some(family);
            }
        }
    }
    None
}

async fn read_xfconf() -> Option<String> {
    let bus = ashpd::zbus::Connection::session().await.ok()?;
    let proxy = ashpd::zbus::Proxy::new(
        &bus,
        "org.xfce.Xfconf",
        "/org/xfce/Xfconf",
        "org.xfce.Xfconf",
    )
    .await
    .ok()?;
    let value: ashpd::zvariant::OwnedValue = proxy
        .call("GetProperty", &("xsettings", "/Gtk/FontName"))
        .await
        .ok()?;
    parse_family(&String::try_from(value).ok()?, false)
}

fn configured_family(kde: bool) -> Option<String> {
    let user = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));
    let system = std::env::var_os("XDG_CONFIG_DIRS")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/etc/xdg".into());
    let roots = user
        .into_iter()
        .chain(std::env::split_paths(&system).filter(|path| path.is_absolute()));
    let mut files = vec![
        ("gtk-4.0/settings.ini", "Settings", "gtk-font-name", false),
        ("gtk-3.0/settings.ini", "Settings", "gtk-font-name", false),
    ];
    if kde {
        files.insert(0, ("kdeglobals", "General", "font", true));
    }
    for root in roots {
        for (file, section, key, qt) in &files {
            if let Ok(contents) = std::fs::read_to_string(root.join(file)) {
                if let Some(family) =
                    ini_value(&contents, section, key).and_then(|value| parse_family(value, *qt))
                {
                    return Some(family);
                }
            }
        }
    }
    None
}

fn ini_value<'a>(contents: &'a str, section: &str, key: &str) -> Option<&'a str> {
    let mut current = "";
    let mut result = None;
    for line in contents.lines().map(str::trim) {
        if line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            current = name;
        } else if current == section {
            if let Some((name, value)) = line.split_once('=') {
                if name.trim() == key {
                    result = Some(value.trim());
                }
            }
        }
    }
    result
}

fn parse_family(description: &str, qt: bool) -> Option<String> {
    let mut description = description.trim();
    if qt {
        description = description.split(',').next()?.trim();
    } else {
        // Pango descriptions append style, size, variation and feature fields.
        for marker in [" #", " @"] {
            description = description.split(marker).next()?.trim_end();
        }
        if let Some(last) = description.split_whitespace().next_back() {
            let size = last.strip_suffix("px").unwrap_or(last);
            if size
                .parse::<f64>()
                .is_ok_and(|size| size.is_finite() && size >= 0.0)
            {
                description = description[..description.len() - last.len()].trim_end();
            }
        }
        while let Some(last) = description.split_whitespace().next_back() {
            let normalized = last.to_ascii_lowercase().replace('-', "");
            if !matches!(
                normalized.as_str(),
                "normal"
                    | "roman"
                    | "oblique"
                    | "italic"
                    | "smallcaps"
                    | "allsmallcaps"
                    | "petitecaps"
                    | "allpetitecaps"
                    | "unicase"
                    | "titlecaps"
                    | "thin"
                    | "ultralight"
                    | "extralight"
                    | "light"
                    | "semilight"
                    | "demilight"
                    | "book"
                    | "regular"
                    | "medium"
                    | "semibold"
                    | "demibold"
                    | "bold"
                    | "ultrabold"
                    | "extrabold"
                    | "heavy"
                    | "black"
                    | "ultrablack"
                    | "extrablack"
                    | "ultracondensed"
                    | "extracondensed"
                    | "condensed"
                    | "semicondensed"
                    | "semiexpanded"
                    | "expanded"
                    | "extraexpanded"
                    | "ultraexpanded"
                    | "notrotated"
                    | "south"
                    | "upsidedown"
                    | "north"
                    | "rotatedleft"
                    | "east"
                    | "rotatedright"
                    | "west"
                    | "withcolor"
                    | "withoutcolor"
            ) {
                break;
            }
            description = description[..description.len() - last.len()].trim_end();
        }
        description = description.split(',').next()?.trim();
    }
    (!description.is_empty()).then(|| description.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_font_encodings_preserve_family_names() {
        for (description, qt, expected) in [
            (
                "Liberation Sans Semi-Bold 10.5",
                false,
                Some("Liberation Sans"),
            ),
            (
                "Noto Sans CJK SC 13px @wght=450 #tnum=1",
                false,
                Some("Noto Sans CJK SC"),
            ),
            ("Example Heavy, 12", false, Some("Example Heavy")),
            ("Example 2,11,-1,5,400,0,0,0,0,0", true, Some("Example 2")),
            ("Example 2", true, Some("Example 2")),
            ("", false, None),
            ("Bold 12", false, None),
        ] {
            assert_eq!(
                parse_family(description, qt).as_deref(),
                expected,
                "{description}"
            );
        }
    }

    #[test]
    fn settings_files_use_the_requested_section_and_last_value() {
        let ini = "[Other]\ngtk-font-name=Wrong\n[Settings]\n# gtk-font-name=Ignored\ngtk-font-name=First 9\ngtk-font-name = Last 11\n[Other]\ngtk-font-name=Wrong Again\n";
        assert_eq!(ini_value(ini, "Settings", "gtk-font-name"), Some("Last 11"));
        assert_eq!(ini_value(ini, "General", "font"), None);
    }
}
