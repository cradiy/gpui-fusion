use std::ffi::{CString, c_char};

#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(priority: i32, tag: *const c_char, text: *const c_char) -> i32;
}

struct AndroidLogger;
static LOGGER: AndroidLogger = AndroidLogger;
impl log::Log for AndroidLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let text = format!("{}: {}", record.target(), record.args()).replace('\0', "�");
        let text = CString::new(text).expect("log message has no NUL bytes");
        let priority = match record.level() {
            log::Level::Error => 6,
            log::Level::Warn => 5,
            log::Level::Info => 4,
            log::Level::Debug => 3,
            log::Level::Trace => 2,
        };
        // SAFETY: both strings remain valid and NUL-terminated for this call.
        unsafe {
            __android_log_write(priority, c"GPUI".as_ptr(), text.as_ptr());
        }
    }

    fn flush(&self) {}
}

pub(crate) fn initialize() {
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Trace);
    }
    static PANIC_HOOK: std::sync::Once = std::sync::Once::new();
    PANIC_HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            log::error!(target: "gpui_android::panic", "{info}");
            for line in std::backtrace::Backtrace::force_capture()
                .to_string()
                .lines()
            {
                log::error!(target: "gpui_android::panic", "{line}");
            }
            previous(info);
        }));
    });
}
