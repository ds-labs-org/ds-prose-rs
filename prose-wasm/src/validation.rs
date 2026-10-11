//! Validation for JavaScript hosts (stub).
use prose_core::profile::Wording;
use prose_core::validate::{Options, Validator};
use serde_json::Value;

pub const TYPES_D_TS: &str = "";

pub fn options_from_value(_v: Option<&Value>) -> Result<Options, String> {
    todo!()
}
pub fn profile_list(_v: &Value) -> Result<Vec<String>, String> {
    todo!()
}
pub fn wording(_texts: &[String]) -> Result<Wording, String> {
    todo!()
}
pub fn validator(_texts: &[String], _o: Options) -> Result<Validator, String> {
    todo!()
}
pub fn validate_text(_json: &str, _v: &Validator) -> String {
    todo!()
}
pub fn rules_json() -> String {
    todo!()
}
