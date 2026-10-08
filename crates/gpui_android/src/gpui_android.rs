//! Android View hosting for GPUI applications.
#![cfg(target_os = "android")]

mod accessibility;
mod autofill;
mod background;
mod bridge;
mod clipboard;
mod credentials;
mod directory;
mod dispatcher;
mod file;
mod file_dialog;
mod file_system;
mod input;
mod logging;
mod notifications;
mod permissions;
mod platform;
mod share;
mod surface;
mod system_services;
mod window;

pub use background::{
    AndroidBackgroundExecution, BackgroundExecution, BackgroundStopReason, DataSyncNotification,
};
pub use bridge::{current_platform, initialize};
pub use jni;
pub use permissions::{AndroidPermissions, PermissionStatus};
pub use platform::AndroidPlatform;

#[doc(hidden)]
#[macro_export]
macro_rules! android_entry {
    ($entry:path) => {
        #[unsafe(no_mangle)]
        #[allow(
            clippy::main_recursion,
            reason = "Android invokes main from the host library loader"
        )]
        pub extern "system" fn JNI_OnLoad(
            vm: $crate::jni::JavaVM,
            _: *mut ::std::ffi::c_void,
        ) -> $crate::jni::sys::jint {
            match $crate::initialize(vm, || ::std::process::Termination::report($entry())) {
                Ok(()) => $crate::jni::sys::JNI_VERSION_1_6,
                Err(error) => {
                    eprintln!("GPUI Android initialization failed: {error:#}");
                    $crate::jni::sys::JNI_ERR
                }
            }
        }
    };
}
