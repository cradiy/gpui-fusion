use super::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen(module = "/src/web.js")]
extern "C" {
    #[wasm_bindgen(catch)]
    fn create(
        app: &str,
        worker: Option<&str>,
        callback: &js_sys::Function,
    ) -> Result<js_sys::Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn permission(request: bool) -> Result<js_sys::Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn show(center: &JsValue, json: &str) -> Result<js_sys::Promise, JsValue>;
    #[wasm_bindgen(catch)]
    fn remove(center: &JsValue, id: &str) -> Result<js_sys::Promise, JsValue>;
    fn close(center: &JsValue);
    fn max_actions(center: &JsValue) -> usize;
}

fn error(value: JsValue) -> anyhow::Error {
    anyhow::anyhow!("browser notification: {value:?}")
}
struct WebNotifications {
    center: JsValue,
    _callback: Closure<dyn FnMut(String)>,
}

pub async fn create_center(options: NotificationOptions) -> Result<NotificationCenter> {
    let (events, rx) = async_channel::unbounded();
    let callback = Closure::new(move |json: String| {
        if let Ok(event) = serde_json::from_str(&json) {
            let _ = events.try_send(event);
        }
    });
    let center = JsFuture::from(
        create(
            &options.app_id,
            options.web_service_worker.as_deref(),
            callback.as_ref().unchecked_ref(),
        )
        .map_err(error)?,
    )
    .await
    .map_err(error)?;
    Ok(NotificationCenter::from_backend(
        Rc::new(WebNotifications {
            center,
            _callback: callback,
        }),
        rx,
    ))
}
impl NotificationBackend for WebNotifications {
    fn capabilities(&self) -> NotificationCapabilities {
        NotificationCapabilities {
            max_actions: max_actions(&self.center),
            inline_reply: false,
            dismissal_events: true,
            progress: false,
            resource_icons: false,
            image_icons: true,
        }
    }
    fn permission(&self, request: bool) -> LocalBoxFuture<'static, Result<NotificationPermission>> {
        let promise = permission(request).map_err(error);
        Box::pin(async move {
            let value = JsFuture::from(promise?).await.map_err(error)?.as_string();
            Ok(match value.as_deref() {
                Some("granted") => NotificationPermission::Granted,
                Some("denied") => NotificationPermission::Denied,
                _ => NotificationPermission::NotDetermined,
            })
        })
    }
    fn show(&self, notification: Notification) -> LocalBoxFuture<'_, Result<()>> {
        Box::pin(async move {
            JsFuture::from(
                show(&self.center, &serde_json::to_string(&notification)?).map_err(error)?,
            )
            .await
            .map_err(error)?;
            Ok(())
        })
    }
    fn remove(&self, id: String) -> LocalBoxFuture<'_, Result<()>> {
        Box::pin(async move {
            JsFuture::from(remove(&self.center, &id).map_err(error)?)
                .await
                .map_err(error)?;
            Ok(())
        })
    }
}
impl Drop for WebNotifications {
    fn drop(&mut self) {
        close(&self.center);
    }
}
