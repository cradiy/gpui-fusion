use std::ffi::{CString, c_char};

#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(priority: i32, tag: *const c_char, text: *const c_char) -> i32;
}

struct AndroidLogger;
static LOGGER: AndroidLogger = AndroidLogger;
const MAX_LEVEL: log::LevelFilter = if cfg!(debug_assertions) {
    log::LevelFilter::Debug
} else {
    log::LevelFilter::Info
};

impl log::Log for AndroidLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Info
            || (cfg!(debug_assertions)
                && metadata.level() <= log::Level::Debug
                && matches!(
                    metadata.target(),
                    "wgpu_hal::vulkan::instance" | "wgpu_hal::vulkan::adapter"
                ))
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
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
        log::set_max_level(MAX_LEVEL);
    }
}
