use crate::{AndroidPlatform, surface::NativeWindow};
use anyhow::{Context, Result};
use gpui::{AppLifecyclePhase, ApplicationHandle, TouchPhase};
use jni::{
    JNIEnv, JavaVM, NativeMethod,
    objects::{GlobalRef, JClass, JObject, JObjectArray, JString, JValue},
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

pub(crate) struct Host {
    vm: Arc<JavaVM>,
    object: GlobalRef,
}
impl Host {
    pub fn request_frame(&self) {
        if let Err(error) = self.with_env(|env| {
            env.call_method(self.object.as_obj(), "requestFrame", "()V", &[])?;
            Ok(())
        }) {
            log::error!("Android frame scheduling failed: {error}");
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

    pub fn schedule(&self, token: u64, delay: Duration) {
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
        method("nativeViewport", "(JIIF)V", viewport as *mut c_void),
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
        method("nativeInputAction", "(JJI)Z", input_action as *mut c_void),
        method("nativeInputIndex", "(JJFF)I", input_index as *mut c_void),
        method("nativeScrollInput", "(JJFF)Z", scroll_input as *mut c_void),
        method("nativeLifecycle", "(JI)V", lifecycle as *mut c_void),
        method("nativeFocus", "(JZ)V", focus as *mut c_void),
        method("nativeAppearance", "(JZ)V", appearance as *mut c_void),
        method("nativeBack", "(J)Z", system_back as *mut c_void),
        method("nativeTouch", "(JIIFF)Z", touch as *mut c_void),
        method("nativeTap", "(JFF)V", tap as *mut c_void),
        method(
            "nativeOpenUrl",
            "(JLjava/lang/String;)V",
            receive_url as *mut c_void,
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

fn session(id: i64) -> Result<Rc<Session>> {
    SESSIONS
        .with(|sessions| sessions.borrow().get(&id).cloned())
        .context("GpuiSession is closed or called from the wrong thread")
}

fn call<T: Default>(env: &mut JNIEnv, f: impl FnOnce(&mut JNIEnv) -> Result<T>) -> T {
    match catch_unwind(AssertUnwindSafe(|| f(env))) {
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
) {
    call(&mut env, |_| {
        session(id)?
            .platform
            .window
            .set_viewport(width, height, density)
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

extern "system" fn tap(mut env: JNIEnv, _: JClass, id: jlong, x: jfloat, y: jfloat) {
    call(&mut env, |_| {
        session(id)?.platform.window.tap(x, y);
        Ok(())
    });
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
            session.platform.close();
        }
        Ok(())
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
