use crate::{AndroidPlatform, surface::NativeWindow};
use anyhow::{Context, Result};
use gpui::{AppLifecyclePhase, ApplicationHandle, TouchPhase};
use jni::{
    JNIEnv, JavaVM, NativeMethod,
    objects::{GlobalRef, JClass, JObject, JValue},
    sys::{jboolean, jfloat, jint, jlong},
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
        method("nativeFrame", "(J)V", frame as *mut c_void),
        method("nativeLifecycle", "(JI)V", lifecycle as *mut c_void),
        method("nativeFocus", "(JZ)V", focus as *mut c_void),
        method("nativeTouch", "(JIIFF)Z", touch as *mut c_void),
        method("nativeTap", "(JFF)V", tap as *mut c_void),
        method("nativeScroll", "(JIFFFF)V", scroll as *mut c_void),
        method("nativeRunTask", "(JJ)V", run_task as *mut c_void),
        method("nativeClose", "(J)V", close as *mut c_void),
    ];
    env.register_native_methods("dev/gpui/android/GpuiSession", &methods)?;
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
extern "system" fn frame(mut env: JNIEnv, _: JClass, id: jlong) {
    call(&mut env, |_| session(id)?.platform.window.frame());
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
extern "system" fn focus(mut env: JNIEnv, _: JClass, id: jlong, active: jboolean) {
    call(&mut env, |_| {
        session(id)?.platform.window.set_active(active != 0);
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
