use crate::{MediaError, MediaResult, PlaybackWakeMode};
use gpui_util::android::AndroidRuntime;
use jni::{
    JNIEnv, NativeMethod,
    objects::{GlobalRef, JClass, JString, JValue},
    sys::{jint, jlong},
};
use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, AtomicI64, Ordering},
    },
};

const CLASS: &str = "dev.gpui.android.AudioPlayback";
static REGISTERED: Mutex<bool> = Mutex::new(false);
static NEXT_ID: AtomicI64 = AtomicI64::new(1);
type Callback = dyn Fn(PlatformEvent) + Send + Sync;
static CALLBACKS: OnceLock<Mutex<HashMap<i64, Weak<Callback>>>> = OnceLock::new();

pub(super) enum PlatformEvent {
    Playing,
    Paused,
    Duck(bool),
    Error(String),
}

pub(super) struct AndroidAudioControl {
    id: i64,
    object: GlobalRef,
    _callback: Arc<Callback>,
    closed: AtomicBool,
}

fn callbacks() -> &'static Mutex<HashMap<i64, Weak<Callback>>> {
    CALLBACKS.get_or_init(Default::default)
}
fn error(error: impl std::fmt::Display) -> MediaError {
    MediaError::backend(format!("Android audio policy: {error}"))
}

impl AndroidAudioControl {
    pub fn open(callback: impl Fn(PlatformEvent) + Send + Sync + 'static) -> MediaResult<Self> {
        let runtime = AndroidRuntime::get().map_err(error)?;
        let mut registered = REGISTERED.lock().map_err(error)?;
        if !*registered {
            runtime
                .with_env(|env| {
                    let class = runtime.load_class(env, CLASS)?;
                    env.register_native_methods(
                        class,
                        &[NativeMethod {
                            name: "nativeEvent".into(),
                            sig: "(JILjava/lang/String;)V".into(),
                            fn_ptr: event as *mut c_void,
                        }],
                    )?;
                    Ok(())
                })
                .map_err(|cause| {
                    error(format!(
                        "{cause}; enable audio-playback in the Android host and synchronize it"
                    ))
                })?;
            *registered = true;
        }
        drop(registered);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let callback: Arc<Callback> = Arc::new(callback);
        callbacks()
            .lock()
            .map_err(error)?
            .insert(id, Arc::downgrade(&callback));
        let object = runtime.with_env(|env| {
            let class = runtime.load_class(env, CLASS)?;
            let object = env.new_object(
                class,
                "(Landroid/content/Context;J)V",
                &[JValue::Object(runtime.context()), JValue::Long(id)],
            )?;
            Ok(env.new_global_ref(object)?)
        });
        match object {
            Ok(object) => Ok(Self {
                id,
                object,
                _callback: callback,
                closed: AtomicBool::new(false),
            }),
            Err(cause) => {
                if let Ok(mut map) = callbacks().lock() {
                    map.remove(&id);
                }
                Err(error(cause))
            }
        }
    }

    fn result(&self, method: &str, signature: &str, args: &[JValue]) -> MediaResult<()> {
        AndroidRuntime::get()
            .map_err(error)?
            .with_env(|env| {
                let result = env
                    .call_method(self.object.as_obj(), method, signature, args)?
                    .l()?;
                if !result.is_null() {
                    anyhow::bail!(String::from(env.get_string(&JString::from(result))?));
                }
                Ok(())
            })
            .map_err(error)
    }
    pub fn play(&self) -> MediaResult<()> {
        self.result("play", "()Ljava/lang/String;", &[])
    }
    pub fn pause(&self) -> MediaResult<()> {
        AndroidRuntime::get()
            .map_err(error)?
            .with_env(|env| {
                env.call_method(self.object.as_obj(), "pause", "()V", &[])?;
                Ok(())
            })
            .map_err(error)
    }
    pub fn set_focus_enabled(&self, enabled: bool) -> MediaResult<()> {
        self.result(
            "setFocusEnabled",
            "(Z)Ljava/lang/String;",
            &[JValue::Bool(enabled.into())],
        )
    }
    pub fn set_wake_mode(&self, mode: PlaybackWakeMode) -> MediaResult<()> {
        let mode = match mode {
            PlaybackWakeMode::None => 0,
            PlaybackWakeMode::Local => 1,
            PlaybackWakeMode::Network => 2,
        };
        self.result("setWakeMode", "(I)Ljava/lang/String;", &[JValue::Int(mode)])
    }
    pub fn close(&self) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Ok(mut map) = callbacks().lock() {
            map.remove(&self.id);
        }
        let result = AndroidRuntime::get().and_then(|runtime| {
            runtime.with_env(|env| {
                env.call_method(self.object.as_obj(), "close", "()V", &[])?;
                Ok(())
            })
        });
        if let Err(error) = result {
            log::warn!("Could not release Android audio policy: {error:#}");
        }
    }
}
impl Drop for AndroidAudioControl {
    fn drop(&mut self) {
        self.close();
    }
}

extern "system" fn event(mut env: JNIEnv, _: JClass, id: jlong, kind: jint, message: JString) {
    let callback = callbacks()
        .lock()
        .ok()
        .and_then(|map| map.get(&id).and_then(Weak::upgrade));
    let Some(callback) = callback else { return };
    let event = match kind {
        0 => PlatformEvent::Playing,
        1 => PlatformEvent::Paused,
        2 => PlatformEvent::Duck(true),
        3 => PlatformEvent::Duck(false),
        4 => PlatformEvent::Error(
            env.get_string(&message)
                .map(String::from)
                .unwrap_or_else(|_| "Audio policy failed".into()),
        ),
        _ => return,
    };
    callback(event);
}
