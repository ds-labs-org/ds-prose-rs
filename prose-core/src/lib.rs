//! PROSE -- Prose Renderer for ODRL Semantic Encoding.
//!
//! Reads an ODRL policy written as JSON-LD and builds a [`Document`]: a
//! structured, plain-language reading of it. The structure (policy, rules,
//! conditions, follow-up duties) is kept as data so a UI can lay it out as
//! headings and lists; every string in it is already a finished sentence or
//! clause.
//!
//! Scope: this reads the JSON the way ODRL authors write it (compact IRIs
//! such as `odrl:use`, full ODRL IRIs, `@list`, `@value`/`@type`, single
//! values or arrays). It does not run a JSON-LD processor, so a custom
//! `@context` that renames ODRL terms is not followed; unknown terms are
//! reported in [`Document::warnings`] instead of guessed at.
mod model;
mod read;
mod words;

pub use model::{Condition, Document, FollowUp, Policy, PolicyKind, Rule, RuleKind};

/// Why a document could not be read at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProseError {
    /// The input is not valid JSON.
    Json(String),
    /// The JSON is valid but contains no ODRL policy object.
    NoPolicy,
}

impl std::fmt::Display for ProseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProseError::Json(e) => write!(f, "not valid JSON: {e}"),
            ProseError::NoPolicy => write!(
                f,
                "no ODRL policy found (expected an object with a Set, Offer or Agreement @type, or a permission, prohibition or obligation)"
            ),
        }
    }
}

impl std::error::Error for ProseError {}

/// Parse `json` and write it as prose.
pub fn render(json: &str) -> Result<Document, ProseError> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| ProseError::Json(e.to_string()))?;
    render_value(&value)
}

/// As [`render`], for a value that is already parsed. Accepts a single
/// policy, an array of policies, or an object whose `@graph` holds them.
pub fn render_value(value: &serde_json::Value) -> Result<Document, ProseError> {
    read::document(value)
}
