/// Whether primary editing shortcuts use Command instead of Control.
///
/// Web applications follow the browser's host platform rather than the WASM target.
pub fn uses_command_modifier() -> bool {
    #[cfg(target_family = "wasm")]
    {
        use std::cell::OnceCell;

        thread_local! {
            static USES_COMMAND: OnceCell<bool> = const { OnceCell::new() };
        }
        USES_COMMAND.with(|value| {
            *value.get_or_init(|| {
                let Some(window) = web_sys::window() else {
                    return false;
                };
                let navigator = window.navigator();
                #[allow(deprecated)]
                let platform = navigator.platform().unwrap_or_default();
                let platform = if platform.is_empty() {
                    navigator.user_agent().unwrap_or_default()
                } else {
                    platform
                };
                ["Mac", "iPhone", "iPad", "iPod"]
                    .iter()
                    .any(|name| platform.contains(name))
            })
        })
    }
    #[cfg(not(target_family = "wasm"))]
    {
        cfg!(target_os = "macos")
    }
}

/// Converts application shortcuts to the host's editing modifier.
///
/// `ctrl`, `cmd`, and `secondary` become `cmd` on macOS and browsers running
/// on Apple platforms, and `ctrl` elsewhere.
/// Space-separated sequences, other modifiers, keys, and test input characters
/// are preserved. Chords containing both `ctrl` and `cmd` remain literal.
/// Use literal shortcuts directly when a binding requires a physical modifier.
///
/// For example, `platform_shortcut("ctrl-shift-p ctrl-a")` produces
/// `"cmd-shift-p cmd-a"` on macOS and `"ctrl-shift-p ctrl-a"` on Linux/Windows.
pub fn platform_shortcut(shortcut: &str) -> String {
    shortcut_for_host(shortcut, uses_command_modifier())
}

fn shortcut_for_host(shortcut: &str, command: bool) -> String {
    let is_modifier = |part: &str| {
        [
            "ctrl",
            "cmd",
            "secondary",
            "alt",
            "shift",
            "fn",
            "super",
            "win",
        ]
        .iter()
        .any(|modifier| part.eq_ignore_ascii_case(modifier))
    };
    let mut result = String::with_capacity(shortcut.len());
    for segment in shortcut.split_inclusive(char::is_whitespace) {
        let chord = segment.trim_end_matches(char::is_whitespace);
        let modifiers = chord.split('-').take_while(|part| is_modifier(part));
        let control = modifiers
            .clone()
            .any(|part| part.eq_ignore_ascii_case("ctrl"));
        let platform = modifiers
            .clone()
            .any(|part| part.eq_ignore_ascii_case("cmd"));
        let mut in_modifiers = true;
        for (index, part) in chord.split('-').enumerate() {
            if index > 0 {
                result.push('-');
            }
            in_modifiers &= is_modifier(part);
            if in_modifiers
                && !(control && platform)
                && ["ctrl", "cmd", "secondary"]
                    .iter()
                    .any(|modifier| part.eq_ignore_ascii_case(modifier))
            {
                result.push_str(if command { "cmd" } else { "ctrl" });
            } else {
                result.push_str(part);
            }
        }
        result.push_str(&segment[chord.len()..]);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_shortcuts_follow_the_host_in_both_directions() {
        for (input, apple, other) in [
            ("ctrl-a", "cmd-a", "ctrl-a"),
            ("cmd-c", "cmd-c", "ctrl-c"),
            ("secondary-v", "cmd-v", "ctrl-v"),
            (
                "CTRL-shift-p cmd-a",
                "cmd-shift-p cmd-a",
                "ctrl-shift-p ctrl-a",
            ),
            (
                "\tctrl--  cmd-x\n",
                "\tcmd--  cmd-x\n",
                "\tctrl--  ctrl-x\n",
            ),
            ("ctrl-a->ctrl", "cmd-a->ctrl", "ctrl-a->ctrl"),
            ("ctrl-你", "cmd-你", "ctrl-你"),
            ("ctrl", "cmd", "ctrl"),
        ] {
            assert_eq!(shortcut_for_host(input, true), apple);
            assert_eq!(shortcut_for_host(input, false), other);
        }
    }

    #[test]
    fn literal_chords_and_key_characters_are_preserved() {
        for input in [
            "",
            "enter",
            "alt-left",
            "ctrl-cmd-space",
            "shift-cmd-ctrl-p",
            "a->cmd",
            "super-q",
        ] {
            assert_eq!(shortcut_for_host(input, true), input);
            assert_eq!(shortcut_for_host(input, false), input);
        }
    }
}
