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
//! values or arrays), and ODRL property keys written as `odrl:permission` or
//! as full IRIs, the way a JSON-LD processor compacts them against a context
//! that declares the `odrl` prefix but not the terms. It does not run a JSON-LD processor, so a custom
//! `@context` that renames ODRL terms is not followed; unknown terms are
//! reported in [`Document::warnings`] instead of guessed at.
pub mod edit;
mod model;
mod read;
mod words;

/// The wording helpers the reader uses, public so an editor can show the
/// same phrases for the same terms.
pub mod wording {
    pub use crate::words::{action, humanise, join, local_name, operand, operator, operator_for};

    /// The (term, phrase) table [`operand`] draws on.
    pub fn known_left_operands() -> &'static [(&'static str, &'static str)] {
        crate::words::OPERANDS
    }
}

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

#[cfg(doctest)]
mod non_exhaustive_pins;
