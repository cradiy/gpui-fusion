use crate::{bridge::Host, file::Document};
use anyhow::{Result, ensure};
use gpui::{ClipboardEntry, ClipboardItem, ForegroundExecutor, Image, ImageFormat, Task};
use jni::objects::{JByteArray, JString, JValue};
use std::sync::Arc;

const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;

pub(crate) fn read(
    host: &Host,
    foreground: &ForegroundExecutor,
) -> Task<Result<Option<ClipboardItem>>> {
    let snapshot = host.clipboard_object("clipboardSnapshot");
    foreground.spawn(async move {
        let Some(snapshot) = snapshot? else {
            return Ok(None);
        };
        crate::dispatcher::io_executor()
            .run(move || {
                let count = snapshot.call(|env| {
                    Ok(env
                        .call_method(snapshot.object.as_obj(), "count", "()I", &[])?
                        .i()?)
                })?;
                ensure!(count <= 128, "too many clipboard entries");
                let mut entries = Vec::new();
                let mut remaining = MAX_IMAGE_BYTES;
                let mut text_seen = false;
                for index in 0..count {
                    snapshot.call(|env| {
                        let text = env
                            .call_method(
                                snapshot.object.as_obj(),
                                "text",
                                "(I)Ljava/lang/String;",
                                &[JValue::Int(index)],
                            )?
                            .l()?;
                        if !text.is_null() {
                            let mut text: String = env.get_string(&JString::from(text))?.into();
                            if text_seen {
                                text.insert(0, '\n');
                            }
                            text_seen = true;
                            entries.extend(ClipboardItem::new_string(text).entries);
                        }
                        let mime = env
                            .call_method(
                                snapshot.object.as_obj(),
                                "imageType",
                                "(I)Ljava/lang/String;",
                                &[JValue::Int(index)],
                            )?
                            .l()?;
                        if mime.is_null() {
                            return Ok(());
                        }
                        let mime: String = env.get_string(&JString::from(mime))?.into();
                        let format = match mime.as_str() {
                            "image/x-icon" | "image/vnd.microsoft.icon" => Some(ImageFormat::Ico),
                            "image/x-ms-bmp" => Some(ImageFormat::Bmp),
                            _ => ImageFormat::from_mime_type(&mime),
                        }
                        .ok_or_else(|| {
                            anyhow::anyhow!("unsupported clipboard image type: {mime}")
                        })?;
                        let bytes = env
                            .call_method(
                                snapshot.object.as_obj(),
                                "image",
                                "(II)[B",
                                &[JValue::Int(index), JValue::Int(remaining as i32)],
                            )?
                            .l()?;
                        let bytes = env.convert_byte_array(JByteArray::from(bytes))?;
                        ensure!(
                            !bytes.is_empty() && bytes.len() <= remaining,
                            "invalid clipboard image size"
                        );
                        remaining -= bytes.len();
                        entries.push(ClipboardEntry::Image(Image::from_bytes(format, bytes)));
                        Ok(())
                    })?;
                }
                Ok((!entries.is_empty()).then_some(ClipboardItem { entries }))
            })
            .await
    })
}

struct Export(Document);
impl Drop for Export {
    fn drop(&mut self) {
        if let Err(error) = self.0.void("close") {
            log::warn!("Clipboard export cleanup failed: {error:#}");
        }
    }
}

pub(crate) fn write_image(
    host: Arc<Host>,
    foreground: &ForegroundExecutor,
    image: Image,
) -> Task<Result<()>> {
    let export = host.clipboard_object("clipboardImage");
    foreground.spawn(async move {
        let export =
            Export(export?.ok_or_else(|| anyhow::anyhow!("clipboard export unavailable"))?);
        let export = crate::dispatcher::io_executor()
            .run(move || {
                ensure!(
                    !image.bytes.is_empty() && image.bytes.len() <= MAX_IMAGE_BYTES,
                    "clipboard image must contain 1 byte to 32 MiB"
                );
                let extension = match image.format {
                    ImageFormat::Png => "png",
                    ImageFormat::Jpeg => "jpg",
                    ImageFormat::Webp => "webp",
                    ImageFormat::Gif => "gif",
                    ImageFormat::Svg => "svg",
                    ImageFormat::Bmp => "bmp",
                    ImageFormat::Tiff => "tiff",
                    ImageFormat::Ico => "ico",
                    ImageFormat::Pnm => "pnm",
                };
                export.0.call(|env| {
                    let bytes = env.byte_array_from_slice(&image.bytes)?;
                    let mime = env.new_string(image.format.mime_type())?;
                    let extension = env.new_string(extension)?;
                    env.call_method(
                        export.0.object.as_obj(),
                        "prepare",
                        "([BLjava/lang/String;Ljava/lang/String;)V",
                        &[
                            JValue::Object(bytes.as_ref()),
                            JValue::Object(mime.as_ref()),
                            JValue::Object(extension.as_ref()),
                        ],
                    )?;
                    Ok(())
                })?;
                Ok(export)
            })
            .await?;
        host.publish_clipboard_image(&export.0.object)?;
        if let Err(error) = crate::dispatcher::io_executor()
            .run(move || export.0.void("cleanOld"))
            .await
        {
            log::warn!("Clipboard cache cleanup failed: {error:#}");
        }
        Ok(())
    })
}
