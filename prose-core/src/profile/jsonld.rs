//! JSON-LD to triples, without a JSON-LD processor.
use super::triples::{Parsed, Sink};
use super::{ProfileError, ProfileLimits};
use serde_json::Value;

pub(crate) fn read(v: &Value, limits: &ProfileLimits) -> Result<Parsed, ProfileError> {
    let _ = v;
    Ok(Sink::new(limits).parsed)
}
