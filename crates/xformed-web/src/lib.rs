//! The same conversion engine as the native API, without HTTP or filesystem access.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn convert_json(request: &str) -> Result<String, JsValue> {
    let convert = || -> anyhow::Result<String> {
        let request = serde_json::from_str::<converters::ConvertRequest>(request)?;
        let response = converters::handle_convert(request)?;
        Ok(serde_json::to_string(&response)?)
    };
    convert().map_err(|error| JsValue::from_str(&error.to_string()))
}
