//! WebAssembly exports for the browser front end, via wasm-bindgen. Results
//! cross as plain JavaScript objects; see `json` for their shape.

use serde::Serialize;
use wasm_bindgen::prelude::*;

fn to_js<T: Serialize>(v: &T) -> Result<JsValue, JsError> {
    v.serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|e| JsError::new(&e.to_string()))
}

/// Run a deck. Errors in the deck are part of the result (`ok: false`).
#[wasm_bindgen]
pub fn run(deck: &str) -> Result<JsValue, JsError> {
    to_js(&crate::json::run(deck))
}

/// Make a deck from data: see `json::ImportRequest`.
#[wasm_bindgen(js_name = importData)]
pub fn import_data(request: JsValue) -> Result<JsValue, JsError> {
    let req: crate::json::ImportRequest = serde_wasm_bindgen::from_value(request)?;
    to_js(&crate::json::import(req))
}
