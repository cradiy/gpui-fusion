mod deferred;

use crate::{AndroidPlatform, surface::NativeWindow};
use anyhow::{Context, Result};
use gpui::{AppLifecyclePhase, ApplicationHandle, TouchPhase};
use jni::{
    JNIEnv, JavaVM, NativeMethod,
    objects::{GlobalRef, JClass, JIntArray, JObject, JObjectArray, JString, JValue},
    sys::{jboolean, jfloat, jint, jlong, jobject},
};
use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
    process::ExitCode,
    rc::Rc,
    sync::{Arc, OnceLock},
    time::Duration,
};

type Entry = fn() -> ExitCode;
static ENTRY: OnceLock<Entry> = OnceLock::new();
static VM: OnceLock<Arc<JavaVM>> = OnceLock::new();

thread_local! {
    static SESSIONS: RefCell<HashMap<i64, Rc<Session>>> = RefCell::default();
    #[allow(clippy::missing_const_for_thread_local, reason = "Android's TLS expansion contains generated non-const initializers")]
    static NEXT_ID: std::cell::Cell<i64> = const { std::cell::Cell::new(1) };
    #[allow(clippy::missing_const_for_thread_local, reason = "Android's TLS expansion contains generated non-const initializers")]
    static CURRENT: RefCell<Option<LaunchContext>> = const { RefCell::new(None) };
}

/// Returns the host platform while the Android application's main function is running.
/// Android applications must be created by a `GpuiSession` with a live Surface.
pub fn current_platform() -> Rc<AndroidPlatform> {
    CURRENT
        .with(|current| {
            current
                .borrow()
                .as_ref()
                .map(|context| context.platform.clone())
        })
        .expect("create the GPUI application inside its main function")
}

struct LaunchContext {
    platform: Rc<AndroidPlatform>,
    application: Option<ApplicationHandle>,
}

pub(crate) fn retain_application(application: ApplicationHandle) {
    CURRENT.with(|current| {
        let mut current = current.borrow_mut();
        let context = current
            .as_mut()
            .expect("application must run during Android launch");
        assert!(
            context.application.is_none(),
            "a GpuiSession can run only one application"
        );
        context.application = Some(application);
    });
}

struct PlatformScope {
    platform: Rc<AndroidPlatform>,
    completed: bool,
}
impl Drop for PlatformScope {
    fn drop(&mut self) {
        let context = CURRENT.with(|current| current.borrow_mut().take());
        if !self.completed {
            self.platform.close();
        }
        drop(context);
    }
}

struct Session {
    _app: ApplicationHandle,
    platform: Rc<AndroidPlatform>,
}

#[derive(Clone)]
pub(crate) struct Host {
    vm: Arc<JavaVM>,
    object: GlobalRef,
}
impl Host {
    pub fn update_autofill(&self, fields: &str) -> Result<()> {
        self.with_env(|env| {
            let fields = env.new_string(fields)?;
            env.call_method(
                self.object.as_obj(),
                "updateAutofill",
                "(Ljava/lang/String;)V",
                &[JValue::Object(fields.as_ref())],
            )?;
            Ok(())
        })
    }

    pub fn finish_autofill(&self, commit: bool) -> Result<()> {
        self.with_env(|env| {
            env.call_method(
                self.object.as_obj(),
                "finishAutofill",
                "(Z)V",
                &[JValue::Bool(commit.into())],
            )?;
            Ok(())
        })
    }
    pub fn raise_accessibility_events(&self, events: accesskit_android::QueuedEvents) {
        if let Err(error) = self.with_env(|env| {
            let view = env
                .call_method(
                    self.object.as_obj(),
                    "accessibilityView",
                    "()Landroid/view/View;",
                    &[],
                )?
                .l()?;
            if !view.is_null() {
                events.raise(env, &view);
            }
            Ok(())
        }) {
            log::error!("Android accessibility event failed: {error:#}");
        }
    }

    pub fn background_operation(&self, operation: &str, payload: &str) -> Result<()> {
        self.with_env(|env| {
            let operation = env.new_string(operation)?;
            let payload = env.new_string(payload)?;
            let error = env
                .call_method(
                    self.object.as_obj(),
                    "backgroundOperation",
                    "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
                    &[
                        JValue::Object(operation.as_ref()),
                        JValue::Object(payload.as_ref()),
                    ],
                )?
                .l()?;
            if !error.is_null() {
                anyhow::bail!(String::from(env.get_string(&JString::from(error))?));
            }
            Ok(())
        })
    }
    pub fn notification_operation(&self, operation: &str, payload: &str) -> Result<String> {
        self.with_env(|env| {
            let operation = env.new_string(operation)?;
            let payload = env.new_string(payload)?;
            let result = env
                .call_method(
                    self.object.as_obj(),
                    "notificationOperation",
                    "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
                    &[
                        JValue::Object(operation.as_ref()),
                        JValue::Object(payload.as_ref()),
                    ],
                )?
                .l()?;
            Ok(env.get_string(&JString::from(result))?.into())
        })
    }
    pub fn credential_store(&self) -> Result<(Arc<JavaVM>, GlobalRef)> {
        self.with_env(|env| {
            let store = env
                .call_method(
                    self.object.as_obj(),
                    "credentialStore",
                    "()Ljava/lang/Object;",
                    &[],
                )?
                .l()?;
            Ok((self.vm.clone(), env.new_global_ref(store)?))
        })
    }
    pub fn file_store(&self) -> Result<(Arc<JavaVM>, GlobalRef)> {
        self.with_env(|env| {
            let store = env
                .call_method(
                    self.object.as_obj(),
                    "fileStore",
                    "()Ljava/lang/Object;",
                    &[],
                )?
                .l()?;
            Ok((self.vm.clone(), env.new_global_ref(store)?))
        })
    }

    pub fn request_files(
        &self,
        token: u64,
        multiple: bool,
        writable: bool,
        mime_types: &[String],
    ) -> Result<()> {
        self.with_env(|env| {
            let types =
                env.new_object_array(mime_types.len() as i32, "java/lang/String", JObject::null())?;
            for (index, mime) in mime_types.iter().enumerate() {
                let mime = env.new_string(mime)?;
                env.set_object_array_element(&types, index as i32, mime)?;
            }
            env.call_method(
                self.object.as_obj(),
                "requestFiles",
                "(JZZ[Ljava/lang/String;)V",
                &[
                    JValue::Long(token as i64),
                    JValue::Bool(multiple as u8),
                    JValue::Bool(writable as u8),
                    JValue::Object(types.as_ref()),
                ],
            )?;
            Ok(())
        })
    }
    pub fn request_directory(&self, token: u64) -> Result<()> {
        self.with_env(|env| {
            env.call_method(
                self.object.as_obj(),
                "requestDirectory",
                "(J)V",
                &[JValue::Long(token as i64)],
            )?;
            Ok(())
        })
    }
    pub fn request_file_save(&self, token: u64, options: &gpui::FileSaveOptions) -> Result<()> {
        self.with_env(|env| {
            let name = env.new_string(&options.suggested_name)?;
            let mime = env.new_string(&options.mime_type)?;
            env.call_method(
                self.object.as_obj(),
                "requestFileSave",
                "(JLjava/lang/String;Ljava/lang/String;)V",
                &[
                    JValue::Long(token as i64),
                    JValue::Object(name.as_ref()),
                    JValue::Object(mime.as_ref()),
                ],
            )?;
            Ok(())
        })
    }
    pub fn request_frame(&self) {
        let host = self.clone();
        deferred::post(move || host.request_frame_now());
    }

    fn request_frame_now(&self) {
        if let Err(error) = self.with_env(|env| {
            env.call_method(self.object.as_obj(), "requestFrame", "()V", &[])?;
            Ok(())
        }) {
            log::error!("Android frame scheduling failed: {error}");
        }
    }

    pub fn request_frame_after(&self, delay: Duration) {
        if let Err(error) = self.with_env(|env| {
            env.call_method(
                self.object.as_obj(),
                "requestFrameAfter",
                "(J)V",
                &[JValue::Long(
                    delay.as_nanos().div_ceil(1_000_000).min(i64::MAX as u128) as i64,
                )],
            )?;
            Ok(())
        }) {
            log::error!("Android delayed frame scheduling failed: {error}");
        }
    }

    pub fn set_cursor(&self, style: i32) -> Result<()> {
        self.with_env(|env| {
            env.call_method(
                self.object.as_obj(),
                "setCursor",
                "(I)V",
                &[JValue::Int(style)],
            )?;
            Ok(())
        })
    }
    pub fn set_screen_orientation(&self, orientation: gpui::ScreenOrientation) -> Result<bool> {
        let value = match orientation {
            gpui::ScreenOrientation::Automatic => 0,
            gpui::ScreenOrientation::Portrait => 1,
            gpui::ScreenOrientation::Landscape => 2,
            gpui::ScreenOrientation::ReversePortrait => 3,
            gpui::ScreenOrientation::ReverseLandscape => 4,
            gpui::ScreenOrientation::Locked => 5,
        };
        self.with_env(|env| {
            Ok(env
                .call_method(
                    self.object.as_obj(),
                    "setScreenOrientation",
                    "(I)Z",
                    &[JValue::Int(value)],
                )?
                .z()?)
        })
    }
    pub fn set_fullscreen(&self, enabled: bool) -> Result<bool> {
        self.with_env(|env| {
            Ok(env
                .call_method(
                    self.object.as_obj(),
                    "setFullscreen",
                    "(Z)Z",
                    &[JValue::Bool(enabled.into())],
                )?
                .z()?)
        })
    }
    pub fn set_system_bar_appearance(&self, appearance: gpui::SystemBarAppearance) -> Result<bool> {
        let encode = |style| match style {
            gpui::SystemBarStyle::Automatic => 0,
            gpui::SystemBarStyle::Light => 1,
            gpui::SystemBarStyle::Dark => 2,
        };
        self.with_env(|env| {
            Ok(env
                .call_method(
                    self.object.as_obj(),
                    "setSystemBarAppearance",
                    "(II)Z",
                    &[
                        JValue::Int(encode(appearance.status)),
                        JValue::Int(encode(appearance.navigation)),
                    ],
                )?
                .z()?)
        })
    }
    pub fn supports_picture_in_picture(&self) -> Result<bool> {
        self.with_env(|env| {
            Ok(env
                .call_method(self.object.as_obj(), "supportsPictureInPicture", "()Z", &[])?
                .z()?)
        })
    }

    pub fn enter_picture_in_picture(&self, ratio: gpui::Size<u32>) -> Result<()> {
        let width = i32::try_from(ratio.width)?;
        let height = i32::try_from(ratio.height)?;
        anyhow::ensure!(
            width > 0 && height > 0,
            "invalid picture-in-picture aspect ratio"
        );
        self.with_env(|env| {
            env.call_method(
                self.object.as_obj(),
                "enterPictureInPicture",
                "(II)V",
                &[JValue::Int(width), JValue::Int(height)],
            )?;
            Ok(())
        })
    }

    pub fn set_picture_in_picture_source_bounds(
        &self,
        bounds: Option<gpui::Bounds<gpui::Pixels>>,
    ) -> Result<()> {
        let valid = bounds.is_some();
        let bounds = bounds.unwrap_or_default();
        self.with_env(|env| {
            env.call_method(
                self.object.as_obj(),
                "setPictureInPictureSourceBounds",
                "(FFFFZ)V",
                &[
                    JValue::Float(bounds.left().into()),
                    JValue::Float(bounds.top().into()),
                    JValue::Float(bounds.right().into()),
                    JValue::Float(bounds.bottom().into()),
                    JValue::Bool(valid.into()),
                ],
            )?;
            Ok(())
        })
    }

    pub fn window_appearance(&self) -> Result<gpui::WindowAppearance> {
        self.with_env(|env| {
            let dark = env
                .call_method(self.object.as_obj(), "darkAppearance", "()Z", &[])?
                .z()?;
            Ok(if dark {
                gpui::WindowAppearance::Dark
            } else {
                gpui::WindowAppearance::Light
            })
        })
    }

    pub fn scaled_font_size(&self, base_size: f32) -> Result<f32> {
        self.with_env(|env| {
            Ok(env
                .call_method(
                    self.object.as_obj(),
                    "scaledFontSize",
                    "(F)F",
                    &[JValue::Float(base_size)],
                )?
                .f()?)
        })
    }

    pub fn prefers_reduced_motion(&self) -> Result<bool> {
        self.with_env(|env| {
            Ok(env
                .call_method(self.object.as_obj(), "prefersReducedMotion", "()Z", &[])?
                .z()?)
        })
    }

    pub fn system_font_paths(&self) -> Result<Vec<std::path::PathBuf>> {
        self.with_env(|env| {
            let paths = JObjectArray::from(
                env.call_method(
                    self.object.as_obj(),
                    "systemFontPaths",
                    "()[Ljava/lang/String;",
                    &[],
                )?
                .l()?,
            );
            let mut result = Vec::new();
            for index in 0..env.get_array_length(&paths)? {
                let path = env.get_object_array_element(&paths, index)?;
                let path = env.auto_local(path);
                let path: &JString = path.as_ref().into();
                result.push(String::from(env.get_string(path)?).into());
            }
            Ok(result)
        })
    }

    pub fn permission_status(&self, permission: &str) -> Result<i32> {
        self.with_env(|env| {
            let permission = env.new_string(permission)?;
            Ok(env
                .call_method(
                    self.object.as_obj(),
                    "permissionStatus",
                    "(Ljava/lang/String;)I",
                    &[JValue::Object(permission.as_ref())],
                )?
                .i()?)
        })
    }

    pub fn network_status(&self) -> Result<i32> {
        self.with_env(|env| {
            Ok(env
                .call_method(self.object.as_obj(), "networkStatus", "()I", &[])?
                .i()?)
        })
    }

    pub fn thermal_status(&self) -> Result<i32> {
        self.with_env(|env| {
            Ok(env
                .call_method(self.object.as_obj(), "thermalStatus", "()I", &[])?
                .i()?)
        })
    }

    pub fn observe_network(&self, token: u64, enable: bool) -> Result<()> {
        self.with_env(|env| {
            env.call_method(
                self.object.as_obj(),
                "observeNetwork",
                "(JZ)V",
                &[JValue::Long(token as i64), JValue::Bool(enable.into())],
            )?;
            Ok(())
        })
    }

    pub fn open_app_settings(&self, token: u64, page: gpui::AppSettings) -> Result<()> {
        self.with_env(|env| {
            let page = match page {
                gpui::AppSettings::Application => 0,
                gpui::AppSettings::Notifications => 1,
            };
            env.call_method(
                self.object.as_obj(),
                "openAppSettings",
                "(JI)V",
                &[JValue::Long(token as i64), JValue::Int(page)],
            )?;
            Ok(())
        })
    }

    pub fn request_permission(&self, permission: &str, token: u64) -> Result<()> {
        self.with_env(|env| {
            let permission = env.new_string(permission)?;
            env.call_method(
                self.object.as_obj(),
                "requestPermission",
                "(Ljava/lang/String;J)V",
                &[
                    JValue::Object(permission.as_ref()),
                    JValue::Long(token as i64),
                ],
            )?;
            Ok(())
        })
    }

    pub fn cancel_permission(&self, token: u64) -> Result<()> {
        self.with_env(|env| {
            env.call_method(
                self.object.as_obj(),
                "cancelPermission",
                "(J)V",
                &[JValue::Long(token as i64)],
            )?;
            Ok(())
        })
    }

    pub fn set_keyboard_visible(&self, visible: bool) -> Result<()> {
        self.with_env(|env| {
            env.call_method(
                self.object.as_obj(),
                "setKeyboardVisible",
                "(Z)V",
                &[JValue::Bool(visible as u8)],
            )?;
            Ok(())
        })
    }

    pub fn perform_haptic_feedback(&self, feedback: gpui::HapticFeedback) -> Result<bool> {
        let kind = match feedback {
            gpui::HapticFeedback::Selection => 0,
            gpui::HapticFeedback::Confirm => 1,
            gpui::HapticFeedback::Reject => 2,
            gpui::HapticFeedback::LongPress => 3,
            gpui::HapticFeedback::GestureStart => 4,
            gpui::HapticFeedback::GestureEnd => 5,
        };
        self.with_env(|env| {
            Ok(env
                .call_method(
                    self.object.as_obj(),
                    "performHaptic",
                    "(I)Z",
                    &[JValue::Int(kind)],
                )?
                .z()?)
        })
    }

    pub fn set_back_enabled(&self, enabled: bool) -> Result<()> {
        self.with_env(|env| {
            env.call_method(
                self.object.as_obj(),
                "setBackEnabled",
                "(Z)V",
                &[JValue::Bool(enabled as u8)],
            )?;
            Ok(())
        })
    }

    fn with_env<T>(&self, call: impl FnOnce(&mut JNIEnv) -> Result<T>) -> Result<T> {
        let mut env = self.vm.attach_current_thread()?;
        env.with_local_frame(16, |env| {
            let result = call(env);
            if env.exception_check()? {
                env.exception_describe()?;
                env.exception_clear()?;
                anyhow::bail!(
                    "Android system service rejected the request; see logcat for details"
                );
            }
            result
        })
    }

    pub fn read_clipboard(&self) -> Result<Option<String>> {
        self.with_env(|env| {
            let value = env
                .call_method(
                    self.object.as_obj(),
                    "readClipboard",
                    "()Ljava/lang/String;",
                    &[],
                )?
                .l()?;
            if value.is_null() {
                return Ok(None);
            }
            let text: String = env.get_string(&JString::from(value))?.into();
            Ok(Some(text))
        })
    }

    pub fn clipboard_object(&self, method: &str) -> Result<Option<crate::file::Document>> {
        self.with_env(|env| {
            let object = env
                .call_method(self.object.as_obj(), method, "()Ljava/lang/Object;", &[])?
                .l()?;
            if object.is_null() {
                return Ok(None);
            }
            Ok(Some(crate::file::Document {
                vm: self.vm.clone(),
                object: env.new_global_ref(object)?,
            }))
        })
    }

    pub fn publish_clipboard_image(&self, image: &GlobalRef) -> Result<()> {
        self.with_env(|env| {
            env.call_method(
                self.object.as_obj(),
                "publishClipboardImage",
                "(Ljava/lang/Object;)V",
                &[JValue::Object(image.as_obj())],
            )?;
            Ok(())
        })
    }

    pub fn write_clipboard(&self, text: &str) -> Result<()> {
        self.with_env(|env| {
            let text = env.new_string(text)?;
            env.call_method(
                self.object.as_obj(),
                "writeClipboard",
                "(Ljava/lang/String;)V",
                &[JValue::Object(text.as_ref())],
            )?;
            Ok(())
        })
    }

    pub fn open_url(&self, url: &str) -> Result<()> {
        self.with_env(|env| {
            let url = env.new_string(url)?;
            env.call_method(
                self.object.as_obj(),
                "openUrl",
                "(Ljava/lang/String;)V",
                &[JValue::Object(url.as_ref())],
            )?;
            Ok(())
        })
    }

    pub fn open_file_intent(&self, intent: &GlobalRef) -> Result<()> {
        self.with_env(|env| {
            let error = env
                .call_method(
                    self.object.as_obj(),
                    "openFileIntent",
                    "(Landroid/content/Intent;)Ljava/lang/String;",
                    &[JValue::Object(intent.as_obj())],
                )?
                .l()?;
            if !error.is_null() {
                anyhow::bail!(String::from(env.get_string(&JString::from(error))?));
            }
            Ok(())
        })
    }

    pub fn share(
        &self,
        text: Option<&str>,
        title: Option<&str>,
        files: &[GlobalRef],
    ) -> Result<()> {
        self.with_env(|env| {
            let text = match text {
                Some(text) => env.new_string(text)?.into(),
                None => JObject::null(),
            };
            let title = match title {
                Some(title) => env.new_string(title)?.into(),
                None => JObject::null(),
            };
            let intents = env.new_object_array(
                i32::try_from(files.len())?,
                "android/content/Intent",
                JObject::null(),
            )?;
            for (index, file) in files.iter().enumerate() {
                env.set_object_array_element(&intents, index as i32, file.as_obj())?;
            }
            let error = env
                .call_method(
                    self.object.as_obj(),
                    "share",
                    "(Ljava/lang/String;Ljava/lang/String;[Landroid/content/Intent;)Ljava/lang/String;",
                    &[JValue::Object(&text), JValue::Object(&title), JValue::Object(intents.as_ref())],
                )?
                .l()?;
            if !error.is_null() {
                anyhow::bail!(String::from(env.get_string(&JString::from(error))?));
            }
            Ok(())
        })
    }

    pub fn schedule(&self, token: u64, delay: Duration) {
        let host = self.clone();
        let requested_at = std::time::Instant::now();
        deferred::post(move || {
            host.schedule_now(token, delay.saturating_sub(requested_at.elapsed()))
        });
    }

    fn schedule_now(&self, token: u64, delay: Duration) {
        let result = (|| -> jni::errors::Result<()> {
            let mut env = self.vm.attach_current_thread()?;
            let result = env.call_method(
                self.object.as_obj(),
                "scheduleTask",
                "(JJ)V",
                &[
                    JValue::Long(token as i64),
                    JValue::Long(delay.as_millis().min(i64::MAX as u128) as i64),
                ],
            );
            if result.is_err() && env.exception_check()? {
                env.exception_describe()?;
                env.exception_clear()?;
            }
            result.map(|_| ())
        })();
        if let Err(error) = result {
            eprintln!("GPUI Android task scheduling failed: {error}");
        }
    }
    pub fn request_close(&self) {
        if let Ok(mut env) = self.vm.attach_current_thread() {
            let _ = env.call_method(self.object.as_obj(), "requestClose", "()V", &[]);
        }
    }
}

/// Registers the Android host's JNI methods. Called by the generated entry point
/// from the application's library loader, before constructing a `GpuiSession`.
pub fn initialize(vm: JavaVM, entry: Entry) -> Result<()> {
    crate::logging::initialize();
    let vm = Arc::new(vm);
    let mut env = vm.get_env()?;
    let methods = [
        method(
            "nativeAccessibilityNode",
            "(JLandroid/view/View;IZ)Landroid/view/accessibility/AccessibilityNodeInfo;",
            crate::accessibility::node as *mut c_void,
        ),
        method(
            "nativeAccessibilityAction",
            "(JLandroid/view/View;IILandroid/os/Bundle;)Z",
            crate::accessibility::action as *mut c_void,
        ),
        method(
            "nativeAccessibilityHover",
            "(JLandroid/view/View;IFF)Z",
            crate::accessibility::hover as *mut c_void,
        ),
        method(
            "nativeAccessibilityReset",
            "(J)V",
            crate::accessibility::reset as *mut c_void,
        ),
        method(
            "nativeBackgroundEvent",
            "(JLjava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
            background_event as *mut c_void,
        ),
        method(
            "nativeNotificationEvent",
            "(JLjava/lang/String;)Z",
            notification_event as *mut c_void,
        ),
        method(
            "nativeMediaCommand",
            "(JLjava/lang/String;)V",
            media_command as *mut c_void,
        ),
        method(
            "nativeCreate",
            "(Ldev/gpui/android/GpuiSession;Landroid/view/Surface;IIF)J",
            create as *mut c_void,
        ),
        method(
            "nativeAttach",
            "(JLandroid/view/Surface;IIF)V",
            attach as *mut c_void,
        ),
        method("nativeDetach", "(J)V", detach as *mut c_void),
        method("nativeFrame", "(J)Z", frame as *mut c_void),
        method("nativeViewport", "(JIIF[I)V", viewport as *mut c_void),
        method(
            "nativeInputState",
            "(J)Ldev/gpui/android/TextInputState;",
            input_state as *mut c_void,
        ),
        method(
            "nativeEdit",
            "(JJILjava/lang/String;III)Z",
            edit as *mut c_void,
        ),
        method("nativeKey", "(JLjava/lang/String;IZ)Z", key as *mut c_void),
        method(
            "nativeAutofill",
            "(JLjava/lang/String;Ljava/lang/String;)V",
            crate::autofill::fill as *mut c_void,
        ),
        method("nativeInputAction", "(JJI)Z", input_action as *mut c_void),
        method("nativeInputIndex", "(JJFF)I", input_index as *mut c_void),
        method("nativeScrollInput", "(JJFF)Z", scroll_input as *mut c_void),
        method("nativeLifecycle", "(JI)V", lifecycle as *mut c_void),
        method("nativeTrimMemory", "(JI)V", trim_memory as *mut c_void),
        method(
            "nativeThermalStateChanged",
            "(JI)V",
            thermal_state_changed as *mut c_void,
        ),
        method(
            "nativePictureInPictureChanged",
            "(JZ)V",
            picture_in_picture_changed as *mut c_void,
        ),
        method(
            "nativePictureInPictureResult",
            "(JLjava/lang/String;)V",
            picture_in_picture_result as *mut c_void,
        ),
        method("nativeFocus", "(JZ)V", focus as *mut c_void),
        method("nativeAppearance", "(JZ)V", appearance as *mut c_void),
        method(
            "nativeReducedMotionChanged",
            "(JZ)V",
            reduced_motion_changed as *mut c_void,
        ),
        method(
            "nativeFontSizeChanged",
            "(J)V",
            font_size_changed as *mut c_void,
        ),
        method("nativeBack", "(J)Z", system_back as *mut c_void),
        method("nativeBackGesture", "(JIFI)V", back_gesture as *mut c_void),
        method("nativeTouch", "(JIIFF)Z", touch as *mut c_void),
        method("nativePinch", "(JIFFF)V", pinch as *mut c_void),
        method("nativeLongPress", "(JFF)Z", long_press as *mut c_void),
        method("nativeTap", "(JFF)V", tap as *mut c_void),
        method(
            "nativeOpenUrl",
            "(JLjava/lang/String;)V",
            receive_url as *mut c_void,
        ),
        method(
            "nativeReceiveShare",
            "(JLjava/lang/String;Ljava/lang/String;[Ljava/lang/Object;Ljava/lang/String;)V",
            receive_share as *mut c_void,
        ),
        method(
            "nativeFocusTextInput",
            "(JFF)Z",
            focus_text_input as *mut c_void,
        ),
        method("nativeScroll", "(JIFFFF)V", scroll as *mut c_void),
        method("nativeMouse", "(JIFFIIIIFF)V", mouse as *mut c_void),
        method("nativeRunTask", "(JJ)V", run_task as *mut c_void),
        method("nativeClose", "(J)V", close as *mut c_void),
        method("nativeRedraw", "(J)V", redraw as *mut c_void),
        method(
            "nativePermissionResult",
            "(JJI)V",
            permission_result as *mut c_void,
        ),
        method(
            "nativeNetworkChanged",
            "(JJI)V",
            network_changed as *mut c_void,
        ),
        method(
            "nativeSettingsResult",
            "(JJLjava/lang/String;)V",
            settings_result as *mut c_void,
        ),
        method(
            "nativeFileResult",
            "(JJ[Ljava/lang/Object;Ljava/lang/String;)V",
            file_result as *mut c_void,
        ),
    ];
    if let Err(error) = env.register_native_methods("dev/gpui/android/GpuiSession", &methods) {
        if env.exception_check()? {
            env.exception_describe()?;
            env.exception_clear()?;
        }
        return Err(error).context(
            "Android host JNI registration failed; rebuild GPUiForge and regenerate the Android host to match gpui_android",
        );
    }
    ENTRY
        .set(entry)
        .map_err(|_| anyhow::anyhow!("GPUI Android entry is already registered"))?;
    VM.set(vm)
        .map_err(|_| anyhow::anyhow!("GPUI Android VM is already registered"))?;
    Ok(())
}

fn method(name: &str, signature: &str, pointer: *mut c_void) -> NativeMethod {
    NativeMethod {
        name: name.into(),
        sig: signature.into(),
        fn_ptr: pointer,
    }
}

extern "system" fn file_result(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    token: jlong,
    documents: JObjectArray,
    error: JString,
) {
    call(&mut env, |env| {
        let result = (|| -> Result<Option<Vec<GlobalRef>>> {
            if !error.is_null() {
                anyhow::bail!(String::from(env.get_string(&error)?));
            }
            if documents.is_null() {
                return Ok(None);
            }
            let mut files = Vec::new();
            for index in 0..env.get_array_length(&documents)? {
                let object = env.get_object_array_element(&documents, index)?;
                files.push(env.new_global_ref(&object)?);
                env.delete_local_ref(object)?;
            }
            Ok(Some(files))
        })();
        session(id)?.platform.files.complete(
            token as u64,
            VM.get().context("Android VM unavailable")?.clone(),
            result,
        );
        Ok(())
    });
}

fn session(id: i64) -> Result<Rc<Session>> {
    SESSIONS
        .with(|sessions| sessions.borrow().get(&id).cloned())
        .context("GpuiSession is closed or called from the wrong thread")
}

pub(crate) fn window(id: i64) -> Result<Rc<crate::window::AndroidWindow>> {
    Ok(session(id)?.platform.window.clone())
}

pub(crate) fn call<T: Default>(env: &mut JNIEnv, f: impl FnOnce(&mut JNIEnv) -> Result<T>) -> T {
    let scope = deferred::Scope::enter();
    let outcome = catch_unwind(AssertUnwindSafe(|| f(env)));
    drop(scope);
    match outcome {
        Ok(Ok(value)) => value,
        outcome => {
            let error = match outcome {
                Ok(Err(error)) => format!("{error:#}"),
                _ => "GPUI Android callback panicked".to_owned(),
            };
            let _ = env.throw_new("java/lang/IllegalStateException", error);
            T::default()
        }
    }
}

extern "system" fn create(
    mut env: JNIEnv,
    _: JClass,
    host: JObject,
    surface: JObject,
    width: jint,
    height: jint,
    density: jfloat,
) -> jlong {
    call(&mut env, |env| {
        anyhow::ensure!(
            CURRENT.with(|current| current.borrow().is_none()),
            "nested Android launch"
        );
        let context = env
            .call_method(&host, "requireContext", "()Landroid/content/Context;", &[])?
            .l()?;
        gpui_util::android::AndroidRuntime::initialize(env, &context)?;
        #[cfg(feature = "network")]
        crate::tls::initialize(env)?;
        let host = Arc::new(Host {
            vm: VM.get().context("JNI VM not initialized")?.clone(),
            object: env.new_global_ref(host)?,
        });
        let native = NativeWindow::from_surface(env, &surface)?;
        let platform = AndroidPlatform::new(host, native, width, height, density)?;
        CURRENT.with(|current| {
            *current.borrow_mut() = Some(LaunchContext {
                platform: platform.clone(),
                application: None,
            })
        });
        let mut scope = PlatformScope {
            platform: platform.clone(),
            completed: false,
        };
        let result = ENTRY.get().context("application entry missing")?();
        anyhow::ensure!(
            result == ExitCode::SUCCESS,
            "application main returned an error"
        );
        let app = CURRENT
            .with(|current| {
                current
                    .borrow_mut()
                    .as_mut()
                    .and_then(|context| context.application.take())
            })
            .context("application main must call Application::run")?;
        scope.completed = true;
        drop(scope);
        let id = NEXT_ID.with(|next| {
            let id = next.get();
            next.set(id.checked_add(1).expect("session id exhausted"));
            id
        });
        SESSIONS.with(|sessions| {
            sessions.borrow_mut().insert(
                id,
                Rc::new(Session {
                    _app: app,
                    platform,
                }),
            )
        });
        Ok(id)
    })
}
extern "system" fn attach(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    surface: JObject,
    width: jint,
    height: jint,
    density: jfloat,
) {
    call(&mut env, |env| {
        let session = session(id)?;
        let native = NativeWindow::from_surface(env, &surface)?;
        session
            .platform
            .window
            .attach(native, width, height, density, &session.platform.context)
    });
}
extern "system" fn detach(mut env: JNIEnv, _: JClass, id: jlong) {
    call(&mut env, |_| {
        session(id)?.platform.window.detach();
        Ok(())
    });
}
extern "system" fn frame(mut env: JNIEnv, _: JClass, id: jlong) -> jboolean {
    call(&mut env, |_| {
        let session = session(id)?;
        session.platform.dispatch_open_urls();
        session.platform.shares.dispatch();
        session.platform.window.frame()?;
        Ok(session.platform.window.input_dirty.replace(false) as u8)
    })
}

extern "system" fn receive_url(mut env: JNIEnv, _: JClass, id: jlong, url: JString) {
    call(&mut env, |env| {
        let url: String = env.get_string(&url)?.into();
        session(id)?.platform.receive_url(url);
        Ok(())
    });
}

extern "system" fn receive_share(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    text: JString,
    mime_type: JString,
    documents: JObjectArray,
    error: JString,
) {
    call(&mut env, |env| {
        let result = (|| -> Result<crate::share::IncomingShare> {
            if !error.is_null() {
                anyhow::bail!(String::from(env.get_string(&error)?));
            }
            let text = (!text.is_null())
                .then(|| env.get_string(&text).map(String::from))
                .transpose()?;
            let mime_type = (!mime_type.is_null())
                .then(|| env.get_string(&mime_type).map(String::from))
                .transpose()?;
            let mut files = Vec::new();
            for index in 0..env.get_array_length(&documents)? {
                let object = env.get_object_array_element(&documents, index)?;
                files.push(env.new_global_ref(&object)?);
                env.delete_local_ref(object)?;
            }
            Ok(crate::share::IncomingShare {
                text,
                mime_type,
                documents: files,
                vm: VM.get().context("Android VM unavailable")?.clone(),
            })
        })();
        session(id)?.platform.shares.receive(result);
        Ok(())
    });
}

extern "system" fn redraw(mut env: JNIEnv, _: JClass, id: jlong) {
    call(&mut env, |_| session(id)?.platform.window.redraw());
}

extern "system" fn viewport(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    width: jint,
    height: jint,
    density: jfloat,
    insets: JIntArray,
) {
    call(&mut env, |env| {
        anyhow::ensure!(
            density.is_finite() && density > 0.,
            "invalid viewport density"
        );
        anyhow::ensure!(
            env.get_array_length(&insets)? == 12,
            "invalid viewport insets"
        );
        let mut values = [0; 12];
        env.get_int_array_region(&insets, 0, &mut values)?;
        anyhow::ensure!(
            values.iter().all(|value| *value >= 0),
            "negative viewport inset"
        );
        let edges = |offset| gpui::Edges {
            left: gpui::px(values[offset] as f32 / density),
            top: gpui::px(values[offset + 1] as f32 / density),
            right: gpui::px(values[offset + 2] as f32 / density),
            bottom: gpui::px(values[offset + 3] as f32 / density),
        };
        session(id)?.platform.window.set_viewport(
            width,
            height,
            density,
            gpui::WindowInsets {
                safe_area: edges(0),
                ime: edges(4),
                consumed: edges(8),
            },
        )
    });
}

extern "system" fn input_state(mut env: JNIEnv, _: JClass, id: jlong) -> jobject {
    call(&mut env, |env| {
        let Some(state) = session(id)?.platform.window.input_state() else {
            return Ok(std::ptr::null_mut());
        };
        let text = match state.text {
            Some(text) => JObject::from(env.new_string(text)?),
            None => JObject::null(),
        };
        fn bounds_array<'local>(
            env: &mut JNIEnv<'local>,
            bounds: Option<gpui::Bounds<gpui::Pixels>>,
        ) -> Result<JObject<'local>> {
            let Some(bounds) = bounds else {
                return Ok(JObject::null());
            };
            let array = env.new_float_array(4)?;
            env.set_float_array_region(
                &array,
                0,
                &[
                    bounds.left().into(),
                    bounds.top().into(),
                    bounds.right().into(),
                    bounds.bottom().into(),
                ],
            )?;
            Ok(array.into())
        }
        let caret = bounds_array(env, state.caret_bounds)?;
        let editor = bounds_array(env, state.editor_bounds)?;
        let anchor = bounds_array(env, state.anchor_bounds)?;
        let head = bounds_array(env, state.head_bounds)?;
        Ok(env
            .new_object(
                "dev/gpui/android/TextInputState",
                "(JLjava/lang/String;IIIIIZZZIZZI[F[F[F[F)V",
                &[
                    JValue::Long(state.epoch as i64),
                    JValue::Object(&text),
                    JValue::Int(state.offset as i32),
                    JValue::Int(state.anchor as i32),
                    JValue::Int(state.head as i32),
                    JValue::Int(state.marked.as_ref().map_or(-1, |r| r.start as i32)),
                    JValue::Int(state.marked.as_ref().map_or(-1, |r| r.end as i32)),
                    JValue::Bool(state.hit as u8),
                    JValue::Bool((state.mode == gpui::TextInputMode::Multiline) as u8),
                    JValue::Bool((state.mode == gpui::TextInputMode::Password) as u8),
                    JValue::Int(match state.purpose {
                        gpui::TextInputPurpose::Text => 0,
                        gpui::TextInputPurpose::Email => 1,
                        gpui::TextInputPurpose::Url => 2,
                        gpui::TextInputPurpose::Phone => 3,
                        gpui::TextInputPurpose::Number { .. } => 4,
                    }),
                    JValue::Bool(matches!(
                        state.purpose,
                        gpui::TextInputPurpose::Number { decimal: true, .. }
                    ) as u8),
                    JValue::Bool(matches!(
                        state.purpose,
                        gpui::TextInputPurpose::Number { signed: true, .. }
                    ) as u8),
                    JValue::Int(crate::input::action_code(state.action)),
                    JValue::Object(&caret),
                    JValue::Object(&editor),
                    JValue::Object(&anchor),
                    JValue::Object(&head),
                ],
            )?
            .into_raw())
    })
}

extern "system" fn input_index(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    epoch: jlong,
    x: jfloat,
    y: jfloat,
) -> jint {
    call(&mut env, |_| {
        Ok(session(id)?
            .platform
            .window
            .input_index(epoch as u64, x, y)
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(-1))
    })
}

extern "system" fn scroll_input(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    epoch: jlong,
    dx: jfloat,
    dy: jfloat,
) -> jboolean {
    call(&mut env, |_| {
        Ok(session(id)?
            .platform
            .window
            .scroll_input(epoch as u64, dx, dy) as u8)
    })
}

extern "system" fn input_action(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    epoch: jlong,
    action: jint,
) -> jboolean {
    call(&mut env, |_| {
        Ok(session(id)?
            .platform
            .window
            .perform_input_action(epoch as u64, action) as u8)
    })
}

extern "system" fn edit(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    epoch: jlong,
    operation: jint,
    text: JString,
    a: jint,
    b: jint,
    cursor: jint,
) -> jboolean {
    call(&mut env, |env| {
        let text: String = env.get_string(&text)?.into();
        Ok(session(id)?
            .platform
            .window
            .edit(epoch as u64, operation, &text, a, b, cursor) as u8)
    })
}

extern "system" fn key(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    name: JString,
    modifiers: jint,
    down: jboolean,
) -> jboolean {
    call(&mut env, |env| {
        use gpui::{KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, PlatformInput};
        let key: String = env.get_string(&name)?.into();
        let keystroke = Keystroke {
            key,
            key_char: None,
            modifiers: Modifiers {
                shift: modifiers & 1 != 0,
                control: modifiers & 2 != 0,
                alt: modifiers & 4 != 0,
                platform: modifiers & 8 != 0,
                ..Default::default()
            },
        };
        let input = if down != 0 {
            PlatformInput::KeyDown(KeyDownEvent {
                keystroke,
                is_held: false,
                prefer_character_input: false,
            })
        } else {
            PlatformInput::KeyUp(KeyUpEvent { keystroke })
        };
        Ok(session(id)?.platform.window.input(input).default_prevented as u8)
    })
}
extern "system" fn lifecycle(mut env: JNIEnv, _: JClass, id: jlong, phase: jint) {
    call(&mut env, |_| {
        let phase = match phase {
            0 => AppLifecyclePhase::Foreground,
            1 => AppLifecyclePhase::Active,
            2 => AppLifecyclePhase::Inactive,
            3 => AppLifecyclePhase::Background,
            _ => anyhow::bail!("invalid lifecycle phase"),
        };
        session(id)?.platform.lifecycle(phase);
        Ok(())
    });
}
extern "system" fn picture_in_picture_changed(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    enabled: jboolean,
) {
    call(&mut env, |_| {
        session(id)?
            .platform
            .window
            .picture_in_picture_changed(enabled != 0);
        Ok(())
    });
}

extern "system" fn picture_in_picture_result(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    error: JString,
) {
    call(&mut env, |env| {
        let result = if error.is_null() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(String::from(env.get_string(&error)?)))
        };
        session(id)?
            .platform
            .window
            .picture_in_picture_result(result);
        Ok(())
    });
}

extern "system" fn appearance(mut env: JNIEnv, _: JClass, id: jlong, dark: jboolean) {
    call(&mut env, |_| {
        session(id)?.platform.window.set_appearance(if dark != 0 {
            gpui::WindowAppearance::Dark
        } else {
            gpui::WindowAppearance::Light
        });
        Ok(())
    });
}

extern "system" fn font_size_changed(mut env: JNIEnv, _: JClass, id: jlong) {
    call(&mut env, |_| {
        session(id)?.platform.window.font_size_changed();
        Ok(())
    });
}

extern "system" fn reduced_motion_changed(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    reduced: jboolean,
) {
    call(&mut env, |_| {
        session(id)?
            .platform
            .window
            .reduced_motion_changed(reduced != 0);
        Ok(())
    });
}

extern "system" fn focus(mut env: JNIEnv, _: JClass, id: jlong, active: jboolean) {
    call(&mut env, |_| {
        session(id)?.platform.window.set_active(active != 0);
        Ok(())
    });
}
extern "system" fn system_back(mut env: JNIEnv, _: JClass, id: jlong) -> jboolean {
    call(&mut env, |_| {
        Ok(session(id)?.platform.window.system_back() as u8)
    })
}

extern "system" fn back_gesture(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    phase: jint,
    progress: jfloat,
    edge: jint,
) {
    call(&mut env, |_| {
        anyhow::ensure!(
            progress.is_finite() && (0.0..=1.0).contains(&progress),
            "invalid Back gesture progress"
        );
        let phase = match phase {
            0 => gpui::TouchPhase::Started,
            1 => gpui::TouchPhase::Moved,
            2 => gpui::TouchPhase::Ended,
            3 => gpui::TouchPhase::Cancelled,
            _ => anyhow::bail!("invalid Back gesture phase"),
        };
        let edge = match edge {
            0 => gpui::BackGestureEdge::Left,
            1 => gpui::BackGestureEdge::Right,
            2 => gpui::BackGestureEdge::None,
            _ => anyhow::bail!("invalid Back gesture edge"),
        };
        session(id)?
            .platform
            .window
            .back_gesture(gpui::BackGestureEvent {
                phase,
                progress,
                edge,
            });
        Ok(())
    });
}

extern "system" fn touch(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    pointer: jint,
    phase: jint,
    x: jfloat,
    y: jfloat,
) -> jboolean {
    call(&mut env, |_| {
        let phase = match phase {
            0 => TouchPhase::Started,
            1 => TouchPhase::Moved,
            2 => TouchPhase::Ended,
            3 => TouchPhase::Cancelled,
            _ => anyhow::bail!("invalid touch phase"),
        };
        Ok(session(id)?.platform.window.touch(pointer, phase, x, y) as u8)
    })
}
extern "system" fn focus_text_input(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    x: jfloat,
    y: jfloat,
) -> jboolean {
    call(&mut env, |_| {
        Ok(session(id)?.platform.window.focus_text_input(x, y) as u8)
    })
}

extern "system" fn pinch(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    phase: jint,
    x: jfloat,
    y: jfloat,
    delta: jfloat,
) {
    call(&mut env, |_| {
        let phase = match phase {
            0 => TouchPhase::Started,
            1 => TouchPhase::Moved,
            2 => TouchPhase::Ended,
            3 => TouchPhase::Cancelled,
            _ => anyhow::bail!("invalid pinch phase"),
        };
        anyhow::ensure!(
            x.is_finite() && y.is_finite() && delta.is_finite() && delta > -1.,
            "invalid pinch geometry"
        );
        session(id)?.platform.window.pinch(phase, x, y, delta);
        Ok(())
    });
}

extern "system" fn tap(mut env: JNIEnv, _: JClass, id: jlong, x: jfloat, y: jfloat) {
    call(&mut env, |_| {
        session(id)?.platform.window.tap(x, y);
        Ok(())
    });
}
extern "system" fn long_press(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    x: jfloat,
    y: jfloat,
) -> jboolean {
    call(&mut env, |_| {
        anyhow::ensure!(
            x.is_finite() && y.is_finite(),
            "invalid long-press position"
        );
        Ok(session(id)?.platform.window.long_press(x, y) as u8)
    })
}
extern "system" fn scroll(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    phase: jint,
    x: jfloat,
    y: jfloat,
    dx: jfloat,
    dy: jfloat,
) {
    call(&mut env, |_| {
        let phase = match phase {
            0 => TouchPhase::Started,
            1 => TouchPhase::Moved,
            2 => TouchPhase::Ended,
            3 => TouchPhase::Cancelled,
            _ => anyhow::bail!("invalid scroll phase"),
        };
        session(id)?.platform.window.scroll(phase, x, y, dx, dy);
        Ok(())
    });
}

extern "system" fn mouse(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    kind: jint,
    x: jfloat,
    y: jfloat,
    button: jint,
    pressed: jint,
    click_count: jint,
    modifiers: jint,
    dx: jfloat,
    dy: jfloat,
) {
    call(&mut env, |_| {
        use gpui::*;
        let session = session(id)?;
        let window = &session.platform.window;
        let scale = window.display.scale_factor();
        let position = point(px(x / scale), px(y / scale));
        let modifiers = Modifiers {
            shift: modifiers & 1 != 0,
            control: modifiers & 2 != 0,
            alt: modifiers & 4 != 0,
            platform: modifiers & 8 != 0,
            ..Default::default()
        };
        let decode_button = |mask| match mask {
            1 => Some(MouseButton::Left),
            2 => Some(MouseButton::Right),
            4 => Some(MouseButton::Middle),
            8 => Some(MouseButton::Navigate(NavigationDirection::Back)),
            16 => Some(MouseButton::Navigate(NavigationDirection::Forward)),
            _ => None,
        };
        let pressed_button = decode_button(pressed);
        let click_count = click_count.clamp(1, 3) as usize;
        let event = match kind {
            0 => PlatformInput::MouseMove(MouseMoveEvent {
                position,
                pressed_button,
                modifiers,
            }),
            1 => PlatformInput::MouseDown(MouseDownEvent {
                position,
                modifiers,
                click_count,
                button: decode_button(button).context("invalid mouse button")?,
                ..Default::default()
            }),
            2 | 5 => {
                let event = MouseUpEvent {
                    position,
                    modifiers,
                    click_count,
                    button: decode_button(button).context("invalid mouse button")?,
                };
                if kind == 5 {
                    PlatformInput::MouseCancelled(event)
                } else {
                    PlatformInput::MouseUp(event)
                }
            }
            3 => PlatformInput::MouseExited(MouseExitEvent {
                position,
                pressed_button,
                modifiers,
            }),
            4 => PlatformInput::ScrollWheel(ScrollWheelEvent {
                position,
                modifiers,
                delta: ScrollDelta::Pixels(point(px(dx / scale), px(dy / scale))),
                touch_phase: TouchPhase::Moved,
            }),
            _ => anyhow::bail!("invalid mouse event"),
        };
        window.pointer.set(position);
        let hovered = kind != 3 && kind != 5 && window.display.bounds().contains(&position);
        window.mouse(event, hovered, modifiers);
        Ok(())
    });
}
extern "system" fn run_task(mut env: JNIEnv, _: JClass, id: jlong, token: jlong) {
    call(&mut env, |_| {
        if let Ok(session) = session(id) {
            session.platform.dispatcher.run(token as u64);
        }
        Ok(())
    });
}
extern "system" fn close(mut env: JNIEnv, _: JClass, id: jlong) {
    call(&mut env, |_| {
        let session = SESSIONS.with(|sessions| sessions.borrow_mut().remove(&id));
        if let Some(session) = session {
            session.platform.background.close();
            session.platform.close();
        }
        Ok(())
    });
}

extern "system" fn background_event(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    token: JString,
    event: JString,
    error: JString,
) {
    call(&mut env, |env| {
        if let Ok(session) = session(id) {
            let token: String = env.get_string(&token)?.into();
            let event: String = env.get_string(&event)?.into();
            let error: String = env.get_string(&error)?.into();
            session.platform.background.event(&token, &event, &error);
        }
        Ok(())
    });
}

extern "system" fn notification_event(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    event: JString,
) -> jboolean {
    let mut accepted = false;
    call(&mut env, |env| {
        let json: String = env.get_string(&event)?.into();
        accepted = crate::notifications::deliver(&session(id)?.platform.host, &json)?;
        Ok(())
    });
    accepted.into()
}

extern "system" fn media_command(mut env: JNIEnv, _: JClass, id: jlong, event: JString) {
    call(&mut env, |env| {
        let _ = session(id)?;
        let json: String = env.get_string(&event)?.into();
        crate::notifications::deliver_media(&json)
    });
}

extern "system" fn permission_result(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    token: jlong,
    status: jint,
) {
    call(&mut env, |_| {
        if let Ok(session) = session(id) {
            session.platform.permissions.complete(token as u64, status);
        }
        Ok(())
    });
}

extern "system" fn trim_memory(mut env: JNIEnv, _: JClass, id: jlong, level: jint) {
    call(&mut env, |_| {
        if let Ok(session) = session(id) {
            session.platform.trim_memory(level);
        }
        Ok(())
    });
}

extern "system" fn thermal_state_changed(mut env: JNIEnv, _: JClass, id: jlong, status: jint) {
    call(&mut env, |_| {
        if let Ok(session) = session(id) {
            session.platform.thermal_state_changed(status);
        }
        Ok(())
    });
}

extern "system" fn network_changed(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    token: jlong,
    status: jint,
) {
    call(&mut env, |_| {
        if let Ok(session) = session(id) {
            session
                .platform
                .services
                .network_changed(token as u64, status)?;
        }
        Ok(())
    });
}

extern "system" fn settings_result(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    token: jlong,
    error: JString,
) {
    call(&mut env, |env| {
        if let Ok(session) = session(id) {
            let error = if error.is_null() {
                None
            } else {
                Some(env.get_string(&error)?.into())
            };
            session
                .platform
                .services
                .settings_result(token as u64, error);
        }
        Ok(())
    });
}
