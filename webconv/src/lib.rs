use wasm_bindgen::prelude::*;

use desynced_exchange::blueprint;

#[wasm_bindgen]
pub fn exchange_to_ron(exchange: &str) -> Result<String, JsError> {
    Ok(ron::ser::to_string_pretty(
        &blueprint::load_blueprint(exchange)?,
        ron::ser::PrettyConfig::default(),
    )?)
}

#[wasm_bindgen]
pub fn ron_to_exchange(ron_data: &str) -> Result<String, JsError> {
    Ok(blueprint::dump_blueprint(
        ron::from_str(ron_data)?
    )?)
}

#[wasm_bindgen]
pub fn exchange_to_json(exchange: &str) -> Result<String, JsError> {
    Ok(serde_json::ser::to_string_pretty(
        &blueprint::load_blueprint(exchange)?,
    )?)
}

#[wasm_bindgen]
pub fn json_to_exchange(ron_data: &str) -> Result<String, JsError> {
    Ok(blueprint::dump_blueprint(
        serde_json::from_str(ron_data)?
    )?)
}

