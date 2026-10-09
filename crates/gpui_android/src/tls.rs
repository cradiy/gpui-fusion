use anyhow::{Context, Result};
use gpui_util::android::AndroidRuntime;
use jni::JNIEnv;

pub(crate) fn initialize(env: &mut JNIEnv) -> Result<()> {
    let runtime = AndroidRuntime::get()?;
    let context = env.new_local_ref(runtime.context())?.into_raw();
    // SAFETY: the current native callback owns this thread's JNI environment. The
    // 0.21 wrapper is not used while the verifier's 0.22 wrapper borrows it.
    let mut verifier_env = unsafe { jni22::EnvUnowned::from_raw(env.get_raw().cast()) };
    let outcome = verifier_env
        .with_env(|env| {
            // SAFETY: this local reference belongs to the current JNI frame and
            // wraps the application context retained by AndroidRuntime.
            let context = unsafe { jni22::objects::JObject::from_raw(env, context.cast()) };
            rustls_platform_verifier::android::init_with_env(env, context)
        })
        .into_outcome();
    match outcome {
        jni22::Outcome::Ok(()) => Ok(()),
        jni22::Outcome::Err(error) => Err(error).context("initializing Android TLS verification"),
        jni22::Outcome::Panic(payload) => std::panic::resume_unwind(payload),
    }
}
