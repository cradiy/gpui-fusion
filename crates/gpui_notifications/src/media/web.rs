use super::*;
use wasm_bindgen::prelude::*;
#[wasm_bindgen(module = "/src/media/web.js")]
extern "C" {
    #[wasm_bindgen(catch)]
    fn create_media(callback: &js_sys::Function) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch)]
    fn update_media(session: &JsValue, state: &str) -> Result<(), JsValue>;
    fn close_media(session: &JsValue);
}
struct WebMedia {
    session: JsValue,
    _callback: Closure<dyn FnMut(String)>,
    commands: async_channel::Sender<MediaCommand>,
}
fn error(error: JsValue) -> anyhow::Error {
    anyhow::anyhow!("browser media session: {error:?}")
}
pub async fn create(_: MediaSessionOptions) -> Result<SystemMediaSession> {
    let (commands, rx) = async_channel::unbounded();
    let sender = commands.clone();
    let callback = Closure::new(move |json: String| {
        if let Ok(command) = serde_json::from_str(&json) {
            let _ = sender.try_send(command);
        }
    });
    let session = create_media(callback.as_ref().unchecked_ref()).map_err(error)?;
    Ok(SystemMediaSession::from_backend(
        Box::new(WebMedia {
            session,
            _callback: callback,
            commands,
        }),
        rx,
    ))
}
impl MediaSessionBackend for WebMedia {
    fn update(&self, state: MediaSessionState) -> Result<()> {
        update_media(&self.session, &serde_json::to_string(&state)?).map_err(error)
    }
}
impl Drop for WebMedia {
    fn drop(&mut self) {
        close_media(&self.session);
        self.commands.close();
    }
}
