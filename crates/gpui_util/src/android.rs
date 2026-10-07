//! JVM access for Android platform services without a window dependency.

use anyhow::{Context, Result};
use jni::{
    JNIEnv, JavaVM,
    objects::{GlobalRef, JClass, JObject, JValue},
};
use std::sync::{Arc, OnceLock};

pub mod hardware_buffer;

static RUNTIME: OnceLock<AndroidRuntime> = OnceLock::new();

pub struct AndroidRuntime {
    vm: Arc<JavaVM>,
    context: GlobalRef,
    loader: GlobalRef,
}

impl AndroidRuntime {
    /// Retains the application context, never an Activity or View.
    pub fn initialize(env: &mut JNIEnv, context: &JObject) -> Result<()> {
        if RUNTIME.get().is_some() {
            return Ok(());
        }
        let application = env
            .call_method(
                context,
                "getApplicationContext",
                "()Landroid/content/Context;",
                &[],
            )?
            .l()?;
        let loader = env
            .call_method(
                &application,
                "getClassLoader",
                "()Ljava/lang/ClassLoader;",
                &[],
            )?
            .l()?;
        let runtime = Self {
            vm: Arc::new(env.get_java_vm()?),
            context: env.new_global_ref(application)?,
            loader: env.new_global_ref(loader)?,
        };
        let _ = RUNTIME.set(runtime);
        Ok(())
    }

    pub fn get() -> Result<&'static Self> {
        RUNTIME
            .get()
            .context("Android application context is not initialized")
    }

    pub fn context(&self) -> &JObject<'static> {
        self.context.as_obj()
    }

    /// Loads application classes on both Java and native worker threads.
    pub fn load_class<'local>(
        &self,
        env: &mut JNIEnv<'local>,
        name: &str,
    ) -> Result<JClass<'local>> {
        let name = env.new_string(name)?;
        Ok(env
            .call_method(
                self.loader.as_obj(),
                "loadClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                &[JValue::Object(&name)],
            )?
            .l()?
            .into())
    }

    pub fn with_env<T>(&self, operation: impl FnOnce(&mut JNIEnv) -> Result<T>) -> Result<T> {
        let mut env = self.vm.attach_current_thread()?;
        let result = env.with_local_frame(32, operation);
        if env.exception_check()? {
            // Do not leave a pending Java exception on an attached Rust thread.
            env.exception_clear()?;
            anyhow::bail!("Android platform service threw an exception");
        }
        result
    }
}
