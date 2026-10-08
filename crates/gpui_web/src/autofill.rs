use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/src/autofill.js")]
extern "C" {
    pub(crate) type WebAutofill;
    #[wasm_bindgen(constructor)]
    pub(crate) fn new(
        canvas: &web_sys::HtmlCanvasElement,
        ime: &web_sys::HtmlInputElement,
        fill: &js_sys::Function,
        focus: &js_sys::Function,
    ) -> WebAutofill;
    #[wasm_bindgen(method)]
    pub(crate) fn update(this: &WebAutofill, json: &str, width: f32, height: f32);
    #[wasm_bindgen(method)]
    pub(crate) fn listen(this: &WebAutofill, name: &str, handler: &js_sys::Function);
    #[wasm_bindgen(method)]
    pub(crate) fn focus(this: &WebAutofill);
    #[wasm_bindgen(method)]
    pub(crate) fn selection(this: &WebAutofill, start: usize, end: usize);
    #[wasm_bindgen(method)]
    pub(crate) fn finish(this: &WebAutofill, commit: bool);
    #[wasm_bindgen(method)]
    pub(crate) fn dispose(this: &WebAutofill);
}

pub(crate) fn encode(fields: Vec<gpui::AutofillField>) -> String {
    serde_json::to_string(&fields.into_iter().map(|field| serde_json::json!({
        "id": field.id.to_string(), "name": field.name.as_ref(), "hint": field.hint.autocomplete(),
        "value": field.value.as_ref(), "focused": field.focused,
        "x": f32::from(field.bounds.left()), "y": f32::from(field.bounds.top()),
        "width": f32::from(field.bounds.size.width), "height": f32::from(field.bounds.size.height),
    })).collect::<Vec<_>>()).expect("autofill fields serialize")
}
