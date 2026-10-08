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
