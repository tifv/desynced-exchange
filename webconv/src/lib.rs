use wasm_bindgen::prelude::*;

use exchange_desynced::blueprint;

#[wasm_bindgen]
pub fn exchange_to_ron(exchange: &str) -> Result<String, JsError> {
    Ok(ron::ser::to_string_pretty(
        &blueprint::load_blueprint(exchange)?,
        ron::ser::PrettyConfig::default(),
    )?)
}

#[wasm_bindgen]
pub fn ron_to_exchange(ron_data: &str) -> Result<String, JsError> {
    blueprint::dump_blueprint(
        ron::from_str(ron_data).unwrap()
    ).map_err(JsError::from)
}

