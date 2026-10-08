use crate::bridge::Host;
use anyhow::{Result, anyhow};
use futures::channel::oneshot;
use gpui::{ForegroundExecutor, Task};
use jni::{
    JNIEnv,
    objects::{JByteArray, JObject, JString, JValue},
};

fn dispatch<T: Send + 'static>(
    host: &Host,
    foreground: ForegroundExecutor,
    operation: &'static str,
    work: impl FnOnce(&mut JNIEnv, &JObject) -> Result<T> + Send + 'static,
) -> Task<Result<T>> {
    let (vm, object) = match host.credential_store() {
        Ok(store) => store,
        Err(error) => return Task::ready(Err(error)),
    };
    let (tx, rx) = oneshot::channel();
    crate::dispatcher::io_executor().dispatch(move || {
        let result = (|| {
            let mut env = vm.attach_current_thread()?;
            let result = env.with_local_frame(16, |env| {
                let result = work(env, object.as_obj());
                if env.exception_check()? {
                    let exception = env.exception_occurred()?;
                    env.exception_clear()?;
                    // Report only the exception type; messages can contain credential data.
                    let class = env.get_object_class(exception)?;
                    let name = env
                        .call_method(class, "getName", "()Ljava/lang/String;", &[])?
                        .l()?;
                    let name: String = env.get_string(&JString::from(name))?.into();
                    return Err(anyhow!("Android credential {operation} failed ({name})"));
                }
                result
            });
            drop(object);
            result
        })();
        let _ = tx.send(result);
    });
    foreground.spawn(async move { rx.await.map_err(|_| anyhow!("credential worker stopped"))? })
}

pub(crate) fn write(
    host: &Host,
    foreground: ForegroundExecutor,
    url: &str,
    username: &str,
    password: &[u8],
) -> Task<Result<()>> {
    if username
        .len()
        .saturating_add(password.len())
        .saturating_add(4)
        > 1024 * 1024
    {
        return Task::ready(Err(anyhow!("credential exceeds the 1 MiB size limit")));
    }
    let url = url.to_owned();
    let username = username.to_owned();
    let mut password = password.to_vec();
    dispatch(host, foreground, "write", move |env, store| {
        let url = env.new_string(url)?;
        let username = env.new_string(username)?;
        let bytes = env.byte_array_from_slice(&password);
        password.fill(0);
        let bytes = bytes?;
        env.call_method(
            store,
            "write",
            "(Ljava/lang/String;Ljava/lang/String;[B)V",
            &[
                JValue::Object(url.as_ref()),
                JValue::Object(username.as_ref()),
                JValue::Object(bytes.as_ref()),
            ],
        )?;
        Ok(())
    })
}

pub(crate) fn read(
    host: &Host,
    foreground: ForegroundExecutor,
    url: &str,
) -> Task<Result<Option<(String, Vec<u8>)>>> {
    let url = url.to_owned();
    dispatch(host, foreground, "read", move |env, store| {
        let url = env.new_string(url)?;
        let record = env
            .call_method(
                store,
                "read",
                "(Ljava/lang/String;)Ldev/gpui/android/StoredCredential;",
                &[JValue::Object(url.as_ref())],
            )?
            .l()?;
        if record.is_null() {
            return Ok(None);
        }
        let result = (|| {
            let username = env
                .get_field(&record, "username", "Ljava/lang/String;")?
                .l()?;
            let username = env.get_string(&JString::from(username))?.into();
            let password = env.get_field(&record, "password", "[B")?.l()?;
            Ok(Some((
                username,
                env.convert_byte_array(JByteArray::from(password))?,
            )))
        })();
        if !env.exception_check()? {
            env.call_method(&record, "clear", "()V", &[])?;
        }
        result
    })
}

pub(crate) fn delete(host: &Host, foreground: ForegroundExecutor, url: &str) -> Task<Result<()>> {
    let url = url.to_owned();
    dispatch(host, foreground, "delete", move |env, store| {
        let url = env.new_string(url)?;
        env.call_method(
            store,
            "delete",
            "(Ljava/lang/String;)V",
            &[JValue::Object(url.as_ref())],
        )?;
        Ok(())
    })
}
