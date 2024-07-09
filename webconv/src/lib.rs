use wasm_bindgen::prelude::*;

use serde::{Deserialize, Serialize};
use serde_json as json;
use ron as ron;

use desynced_exchange::{
    error::LoadError,
    dumper::dump_blueprint as dump,
    loader::load_blueprint as load,
    value::Value,
    blueprint::{
        dump_blueprint, load_blueprint,
        Exchange,
        Blueprint, Behavior,
    }
};

#[wasm_bindgen]
extern "C" {
    pub type DecodeParameters;

    #[wasm_bindgen(method, getter, js_name="decodeFormat")]
    fn decode_format_s(this: &DecodeParameters) -> String;

    #[wasm_bindgen(method, getter, js_name="decodeStyle")]
    fn decode_style_s(this: &DecodeParameters) -> String;

    #[wasm_bindgen(method, getter, js_name="interRepr")]
    fn inter_repr_s(this: &DecodeParameters) -> String;

}

#[wasm_bindgen]
extern "C" {
    pub type EncodeParameters;

    #[wasm_bindgen(method, getter, js_name="decodeFormat")]
    fn decode_format_s(this: &EncodeParameters) -> String;

    #[wasm_bindgen(method, getter, js_name="interRepr")]
    fn inter_repr_s(this: &EncodeParameters) -> String;

}

enum DecodeFormat {
    Ron,
    Json,
}

impl TryFrom<&str> for DecodeFormat {
    type Error = JsError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            "ron"  => Self::Ron,
            "json" => Self::Json,
            other => return Err(JsError::new(
                &format!("unrecognized decode format {other:?}") )),
        })
    }
}

enum DecodeStyle {
    Pretty,
    Compact,
}

impl TryFrom<&str> for DecodeStyle {
    type Error = JsError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            "pretty"  => Self::Pretty,
            "compact" => Self::Compact,
            other => return Err(JsError::new(
                &format!("unrecognized decode style {other:?}") )),
        })
    }
}

enum InterRepr {
    Struct,
    MapTree,
}

impl TryFrom<&str> for InterRepr {
    type Error = JsError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            "struct"  => Self::Struct,
            "map_tree"    => Self::MapTree,
            other => return Err(JsError::new(
                &format!("unrecognized intermediate repr {other:?}") )),
        })
    }
}

impl DecodeParameters {
    fn decode_format(&self) -> Result<DecodeFormat, JsError> {
        self.decode_format_s().as_str().try_into()
    }
    fn decode_style(&self) -> Result<DecodeStyle, JsError> {
        self.decode_style_s().as_str().try_into()
    }
    fn inter_repr(&self) -> Result<InterRepr, JsError> {
        self.inter_repr_s().as_str().try_into()
    }
}

impl EncodeParameters {
    fn decode_format(&self) -> Result<DecodeFormat, JsError> {
        self.decode_format_s().as_str().try_into()
    }
    fn inter_repr(&self) -> Result<InterRepr, JsError> {
        self.inter_repr_s().as_str().try_into()
    }
}

#[wasm_bindgen]
pub fn decode(encoded: &str, params: &DecodeParameters)
-> Result<String, JsError>
{
    match params.inter_repr()? {
        InterRepr::Struct =>
            serialize::<Exchange<Blueprint, Behavior>>(
                load_blueprint(encoded)?,
                params ),
        InterRepr::MapTree => {
            let value = load::<_,_,LoadError>(encoded)?
                .transpose().ok_or_else(|| JsError::new(
                    "Blueprint or behavior should not \
                    be represented with nil" ))?;
            serialize::<Exchange<Value>>(value, params)
        }
    }
}

fn serialize<V>(value: V, params: &DecodeParameters)
-> Result<String, JsError>
where V: Serialize
{
    match params.decode_format()? {
        DecodeFormat::Ron => serialize_into_ron(value, params),
        DecodeFormat::Json => serialize_into_json(value, params),
    }
}

fn serialize_into_ron<V>(value: V, params: &DecodeParameters)
-> Result<String, JsError>
where V: Serialize
{
    Ok(match params.decode_style()? {
        DecodeStyle::Pretty =>
            ron::ser::to_string_pretty( &value,
                ron::ser::PrettyConfig::default() )?,
        DecodeStyle::Compact =>
            ron::ser::to_string(&value)?,
    })
}

fn serialize_into_json<V>(value: V, params: &DecodeParameters)
-> Result<String, JsError>
where V: Serialize,
{
    Ok(match params.decode_style()? {
        DecodeStyle::Pretty =>
            json::ser::to_string_pretty(&value)?,
        DecodeStyle::Compact =>
            json::ser::to_string(&value)?,
    })
}

#[wasm_bindgen]
pub fn encode(decoded: &str, params: &EncodeParameters)
-> Result<String, JsError>
{
    Ok(match params.inter_repr()? {
        InterRepr::Struct =>
            dump_blueprint(deserialise(decoded, params)?)?,
        InterRepr::MapTree => {
            dump(
                deserialise::<Exchange<Value>>(decoded, params)?
                    .map_mono(Some)
            )?
        },
    })
}

fn deserialise<'de, V>(decoded: &'de str, params: &EncodeParameters)
-> Result<V, JsError>
where V: Deserialize<'de>,
{
    Ok(match params.decode_format()? {
        DecodeFormat::Ron => deserialize_from_ron(decoded, params)?,
        DecodeFormat::Json => deserialize_from_json(decoded, params)?,
    })
}

fn deserialize_from_ron<'de, V>(decoded: &'de str, _params: &EncodeParameters)
-> Result<V, JsError>
where V: Deserialize<'de>,
{
    Ok(ron::from_str(decoded)?)
}

fn deserialize_from_json<'de, V>(decoded: &'de str, _params: &EncodeParameters)
-> Result<V, JsError>
where V: Deserialize<'de>,
{
    Ok(json::from_str(decoded)?)
}

