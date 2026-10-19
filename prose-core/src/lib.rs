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
//! that declares the `odrl` prefix but not the terms. It does not run a
//! JSON-LD processor, so a custom `@context` that renames ODRL terms is not
//! followed; unknown terms are reported in [`Document::warnings`] instead of
//! guessed at.
//!
//! Profiles: [`render_with`] reads the policy with the labels of the ODRL
//! profiles a [`profile::Wording`] holds, and returns a [`Reading`]: the
//! [`Document`] plus a glossary per policy. With `Wording::default()` the
//! document is exactly what [`render`] returns. See [`profile`].
//!
//! Validation: [`validate`] checks a policy against the loaded profiles,
//! in the advisory mode [`edit::check_profile`] has always used, or in a
//! strict mode that reads ODRL plus the loaded profiles as a closed
//! vocabulary and reports a verdict.
//!
//! Features: `turtle` (off by default) also reads profiles written in
//! Turtle, through oxttl.
pub mod edit;
mod model;
pub mod profile;
mod read;
pub mod validate;
mod words;
mod words_ctx;

/// The wording helpers the reader uses, public so an editor can show the
/// same phrases for the same terms.
pub mod wording {
    pub use crate::words::ValueShape;
    pub use crate::words::{action, humanise, join, local_name, operand, operator, short_iri};
    pub use crate::words_ctx::OperatorQuery;

    /// The one entry point for operator wording: the phrase that follows the
    /// left operand of `query`. See [`OperatorQuery`].
    pub fn operator_phrase(query: &OperatorQuery<'_>) -> String {
        crate::words_ctx::operator_phrase(query)
    }

    /// The (term, phrase) table [`operand`] draws on.
    pub fn known_left_operands() -> &'static [(&'static str, &'static str)] {
        crate::words::OPERANDS
    }

    /// How the loaded profiles explain `written`, a term as a policy whose
    /// prefixes are `prefixes` writes it; `None` when no loaded profile
    /// declares it.
    pub fn gloss(
        wording: &crate::profile::Wording,
        prefixes: &crate::profile::Prefixes,
        written: &str,
    ) -> Option<crate::Gloss> {
        crate::words_ctx::gloss(wording, prefixes, written)
    }

    /// The phrase that follows `left`: a date and time reads "are before"
    /// rather than "are less than", a duration stays a quantity, and the verb
    /// agrees in number with the operand's noun phrase.
    #[deprecated(since = "0.5.0", note = "use operator_phrase(&OperatorQuery)")]
    pub fn operator_for(left: &str, op: &str) -> String {
        operator_phrase(&OperatorQuery::new(left, op))
    }

    /// [`operator_for`] when the one right operand is a date without a time
    /// of day: a bound on a whole day says whether it means the start or the
    /// end.
    #[deprecated(since = "0.5.0", note = "use operator_phrase(&OperatorQuery)")]
    pub fn operator_for_date(left: &str, op: &str) -> String {
        operator_phrase(&OperatorQuery::new(left, op).value(ValueShape::DateOnly))
    }

    /// [`operator_phrase`] with the loaded profiles: an operator a profile
    /// declares (outside ODRL's namespace) reads with its label, e.g. "is
    /// within region".
    #[deprecated(since = "0.5.0", note = "use operator_phrase(&OperatorQuery)")]
    pub fn operator_with(
        wording: &crate::profile::Wording,
        prefixes: &crate::profile::Prefixes,
        left: &str,
        op: &str,
    ) -> String {
        operator_phrase(&OperatorQuery::new(left, op).wording(wording, prefixes))
    }

    /// [`operator_with`] when the one right operand is a date without a
    /// time of day.
    #[deprecated(since = "0.5.0", note = "use operator_phrase(&OperatorQuery)")]
    pub fn operator_with_date(
        wording: &crate::profile::Wording,
        prefixes: &crate::profile::Prefixes,
        left: &str,
        op: &str,
    ) -> String {
        operator_phrase(
            &OperatorQuery::new(left, op)
                .value(ValueShape::DateOnly)
                .wording(wording, prefixes),
        )
    }
}

pub use model::{
    Condition, ConditionNames, Document, FollowUp, Gloss, Mention, NameSpan, Policy,
    PolicyGlossary, PolicyKind, PolicyNames, Reading, Rule, RuleKind, RuleNames,
};

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
    read::document_with(value, &profile::Wording::default()).map(|r| r.document)
}

/// As [`render`], worded with the profiles `wording` holds: profile terms
/// read with their labels and a glossary explains them. With no profile
/// loaded, `document` is exactly what [`render`] returns.
pub fn render_with(json: &str, wording: &profile::Wording) -> Result<Reading, ProseError> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| ProseError::Json(e.to_string()))?;
    render_value_with(&value, wording)
}

/// As [`render_with`], for a value that is already parsed.
pub fn render_value_with(
    value: &serde_json::Value,
    wording: &profile::Wording,
) -> Result<Reading, ProseError> {
    read::document_with(value, wording)
}

#[cfg(doctest)]
mod non_exhaustive_pins;
