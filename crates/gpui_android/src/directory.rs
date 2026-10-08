use crate::file::{Document, selected_file};
use anyhow::{Result, ensure};
use futures::future::LocalBoxFuture;
use gpui::gpui_io::{
    CreateOptions, FileHandle, IoExecutor, LocationBookmark, LocationHandle, PlatformLocation,
};
use jni::{
    JavaVM,
    objects::{GlobalRef, JString, JValue},
};
use std::sync::Arc;

pub(crate) fn selected_directory(
    vm: Arc<JavaVM>,
    object: GlobalRef,
    executor: IoExecutor,
) -> Result<LocationHandle> {
    let document = Arc::new(Document { vm, object });
    document.void("validate")?;
    Ok(LocationHandle::new(Directory { document, executor }))
}

pub(crate) fn bookmark_uri(bookmark: &LocationBookmark) -> Result<&str> {
    let reference = bookmark
        .provider()
        .ok_or_else(|| anyhow::anyhow!("expected a directory provider bookmark"))?;
    ensure!(
        reference.provider() == "android-directory",
        "unsupported directory bookmark provider"
    );
    ensure!(
        (1..=16384).contains(&reference.data().len()),
        "invalid directory bookmark"
    );
    Ok(std::str::from_utf8(reference.data())?)
}

struct Directory {
    document: Arc<Document>,
    executor: IoExecutor,
}

impl PlatformLocation for Directory {
    fn persist(&self) -> LocalBoxFuture<'static, Result<LocationBookmark>> {
        let document = self.document.clone();
        self.executor.run(move || {
            let uri: String = document.call(|env| {
                let value = env
                    .call_method(
                        document.object.as_obj(),
                        "persist",
                        "()Ljava/lang/String;",
                        &[],
                    )?
                    .l()?;
                Ok(env.get_string(&JString::from(value))?.into())
            })?;
            Ok(LocationBookmark::new("android-directory", uri.into_bytes()))
        })
    }

    fn create_file(
        &self,
        relative_path: String,
        options: CreateOptions,
    ) -> LocalBoxFuture<'static, Result<FileHandle>> {
        let document = self.document.clone();
        let executor = self.executor.clone();
        self.executor.run(move || {
            let object = document.call(|env| {
                let path = env.new_string(relative_path)?;
                let mime = env.new_string(
                    options
                        .mime_type
                        .as_deref()
                        .unwrap_or("application/octet-stream"),
                )?;
                let file = env
                    .call_method(
                        document.object.as_obj(),
                        "create",
                        "(Ljava/lang/String;Ljava/lang/String;)Ldev/gpui/android/SelectedDocument;",
                        &[JValue::Object(path.as_ref()), JValue::Object(mime.as_ref())],
                    )?
                    .l()?;
                Ok(env.new_global_ref(file)?)
            })?;
            selected_file(document.vm.clone(), object, executor)
        })
    }
}
