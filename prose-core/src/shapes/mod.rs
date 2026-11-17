//! Companion SHACL shapes: the narrowing a dataspace already publishes for
//! its connectors, read by the editor so that what it offers is what the
//! connector checks.
//!
//! This is not a SHACL engine. Only this subset is read, and everything else
//! is ignored with a warning ([`ShapesGraph::warnings`]):
//!
//! - A property shape on `odrl:rightOperand` inside a constraint shape
//!   selected by `odrl:leftOperand sh:hasValue X` (directly, under `sh:or` /
//!   `sh:and`, or under `sh:qualifiedValueShape`): `sh:in` is the closed list
//!   of values (IRIs and literals; a literal member is matched on its
//!   lexical form, and a number by value, whatever its datatype),
//!   `sh:class` and `sh:node` name a class whose
//!   members in the loaded profiles, with their `skos:broader` descendants,
//!   are the allowed values, `sh:datatype` is the datatype, and `sh:pattern`
//!   (with `sh:flags`), `sh:minInclusive` and `sh:maxInclusive` are lexical
//!   checks.
//! - `sh:in` on `odrl:operator` and `odrl:unit` in the same constraint
//!   shape: the operators and units of that left operand (IRIs only: a
//!   literal in one of these lists ignores the list, with a warning).
//! - `sh:in` on `odrl:action` or `odrl:leftOperand` (the latter without
//!   `sh:hasValue`) closes the term list for the whole policy (IRIs only).
//!   The validator binds on these two lists in strict mode only, as it does
//!   for every unknown term; the editor offers the closed action and left
//!   operand lists only when strict validation is on.
//!
//! Several shapes for one left operand (alternatives under `sh:or`) are
//! read as a union of their lists. A shape under `sh:not` or `sh:xone` is
//! not read.
//!
//! Shapes only add hints to terms the loaded profiles declare, through
//! [`crate::profile::Wording::with_shapes`]; they never add a term or
//! relabel one. Every syntax goes through the same private triple list as
//! a profile's, and so does its [`ProfileLimits`](crate::profile::ProfileLimits).
//!
//! ```
//! use prose_core::profile::{ProfileVocab, Wording};
//! use prose_core::shapes::ShapesGraph;
//!
//! let profile = ProfileVocab::parse(r#"{
//!   "@context": { "odrl": "http://www.w3.org/ns/odrl/2/", "ex": "https://example.org/" },
//!   "@graph": [
//!     { "@id": "ex:purpose", "@type": "odrl:LeftOperand" },
//!     { "@id": "ex:research", "@type": "odrl:RightOperand" }
//!   ]
//! }"#)?;
//! let shapes = ShapesGraph::parse(r#"{
//!   "@context": { "sh": "http://www.w3.org/ns/shacl#", "odrl": "http://www.w3.org/ns/odrl/2/",
//!                 "ex": "https://example.org/" },
//!   "@id": "ex:PurposeShape",
//!   "sh:property": [
//!     { "sh:path": { "@id": "odrl:leftOperand" }, "sh:hasValue": { "@id": "ex:purpose" } },
//!     { "sh:path": { "@id": "odrl:rightOperand" }, "sh:in": { "@list": [{ "@id": "ex:research" }] } }
//!   ]
//! }"#)?;
//! let wording = Wording::new().with_profile(profile).with_shapes(shapes);
//! let (term, _) = wording.lookup("https://example.org/purpose").unwrap();
//! let hints = wording.operand_hints(term);
//! assert!(hints.closed);
//! assert_eq!(hints.values, ["https://example.org/research"]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
use crate::profile::{ProfileError, ProfileLimits};
use serde_json::Value;

mod bounds;
mod pattern;
mod read;

pub(crate) use bounds::{Bound, outside_bounds};
pub(crate) use pattern::Pattern;

/// What the shapes say about one left operand's right operand, operator and
/// unit, as read; not yet laid over a profile's hints.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConstraintShape {
    /// `sh:in` on `odrl:rightOperand`: the allowed values that are IRIs.
    pub values: Vec<String>,
    /// `sh:in` on `odrl:rightOperand`: the allowed values that are
    /// literals, as their lexical forms (the datatype is not kept: a value
    /// is matched on its text, and numbers by value).
    pub literals: Vec<String>,
    /// `sh:class` and `sh:node` on `odrl:rightOperand`.
    pub classes: Vec<String>,
    /// `sh:datatype`.
    pub datatype: Option<String>,
    /// `sh:pattern`, with its `sh:flags` (`""` or `"i"`).
    pub pattern: Option<(String, String)>,
    /// `sh:minInclusive`, as written.
    pub min_inclusive: Option<String>,
    /// `sh:maxInclusive`, as written.
    pub max_inclusive: Option<String>,
    /// `sh:in` on `odrl:operator`.
    pub operators: Vec<String>,
    /// `sh:in` on `odrl:unit`.
    pub units: Vec<String>,
}

impl ConstraintShape {
    /// The lists sorted, so shapes compare as the graph they are.
    fn normalised(&self) -> ConstraintShape {
        let sorted = |v: &[String]| {
            let mut v = v.to_vec();
            v.sort_unstable();
            v
        };
        ConstraintShape {
            values: sorted(&self.values),
            literals: sorted(&self.literals),
            classes: sorted(&self.classes),
            operators: sorted(&self.operators),
            units: sorted(&self.units),
            ..self.clone()
        }
    }
}

/// The SHACL subset of one shapes document; see the [module](self).
///
/// Equality compares the shapes as a graph, not how they were written: the
/// order of the lists and the parse warnings are left out, so a JSON-LD
/// document and its Turtle twin are equal.
#[derive(Clone, Debug, Default)]
pub struct ShapesGraph {
    constraints: Vec<(String, ConstraintShape)>,
    actions: Option<Vec<String>>,
    left_operands: Option<Vec<String>>,
    warnings: Vec<String>,
}

impl PartialEq for ShapesGraph {
    fn eq(&self, other: &ShapesGraph) -> bool {
        fn sorted(v: &Option<Vec<String>>) -> Option<Vec<&str>> {
            v.as_ref().map(|v| {
                let mut v: Vec<&str> = v.iter().map(String::as_str).collect();
                v.sort_unstable();
                v
            })
        }
        self.constraints.len() == other.constraints.len()
            && self.constraints.iter().all(|(left, c)| {
                other
                    .constraint(left)
                    .is_some_and(|d| c.normalised() == d.normalised())
            })
            && sorted(&self.actions) == sorted(&other.actions)
            && sorted(&self.left_operands) == sorted(&other.left_operands)
    }
}

impl ShapesGraph {
    /// Reads shapes written as JSON-LD (or, with the `turtle` feature, as
    /// Turtle), with the default limits. The syntax is told from the first
    /// characters, as for a profile.
    pub fn parse(text: &str) -> Result<ShapesGraph, ShapesError> {
        ShapesGraph::parse_with(text, &ProfileLimits::default())
    }

    /// As [`ShapesGraph::parse`], with explicit limits.
    pub fn parse_with(text: &str, limits: &ProfileLimits) -> Result<ShapesGraph, ShapesError> {
        read::interpret(
            crate::profile::ProfileVocab::read_triples(text, None, limits)?,
            limits,
        )
    }

    /// As [`ShapesGraph::parse`], for a value that is already parsed.
    pub fn from_value(v: &Value, limits: &ProfileLimits) -> Result<ShapesGraph, ShapesError> {
        read::interpret(crate::profile::jsonld_triples(v, limits)?, limits)
    }

    /// The left operands some shape selects, with what it says about each,
    /// in first-seen order.
    pub fn constraints(&self) -> impl Iterator<Item = (&str, &ConstraintShape)> {
        self.constraints.iter().map(|(l, c)| (l.as_str(), c))
    }

    /// What the shapes say about the left operand with this expanded IRI.
    pub fn constraint(&self, left: &str) -> Option<&ConstraintShape> {
        self.constraints
            .iter()
            .find(|(l, _)| l == left)
            .map(|(_, c)| c)
    }

    /// `sh:in` on `odrl:action`: the actions the policy may use, as expanded
    /// IRIs; `None` when no shape lists them.
    pub fn closed_actions(&self) -> Option<&[String]> {
        self.actions.as_deref()
    }

    /// `sh:in` on `odrl:leftOperand`: the left operands the policy may use.
    pub fn closed_left_operands(&self) -> Option<&[String]> {
        self.left_operands.as_deref()
    }

    /// What was not read, or read with a guess: lower-case fragments, each
    /// once.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Whether the shapes say nothing the editor can use.
    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty() && self.actions.is_none() && self.left_operands.is_none()
    }
}

/// Why a text could not be read as shapes.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShapesError {
    TooLarge {
        bytes: usize,
        limit: usize,
    },
    Json(String),
    LooksLikeHtml,
    UnsupportedFormat,
    LimitExceeded(&'static str),
    /// The text was read as Turtle (the `turtle` feature) and is not valid
    /// Turtle.
    Turtle(String),
    /// The text was read, and is not SHACL shapes; the reason says what it
    /// is instead.
    NotShapes(String),
}

impl From<ProfileError> for ShapesError {
    fn from(e: ProfileError) -> ShapesError {
        match e {
            ProfileError::TooLarge { bytes, limit } => ShapesError::TooLarge { bytes, limit },
            ProfileError::Json(e) => ShapesError::Json(e),
            ProfileError::LooksLikeHtml => ShapesError::LooksLikeHtml,
            ProfileError::UnsupportedFormat => ShapesError::UnsupportedFormat,
            ProfileError::LimitExceeded(what) => ShapesError::LimitExceeded(what),
            ProfileError::Turtle(e) => ShapesError::Turtle(e),
            ProfileError::ContextOnly => ShapesError::NotShapes(
                "this document holds only a @context: it maps short names to IRIs but holds no SHACL shapes".to_string(),
            ),
            ProfileError::JsonSchema => ShapesError::NotShapes(
                "this is a JSON Schema, not SHACL shapes".to_string(),
            ),
            ProfileError::LooksLikePolicy => ShapesError::NotShapes(
                "this is an ODRL policy, not SHACL shapes".to_string(),
            ),
            _ => ShapesError::NotShapes("this is not SHACL shapes".to_string()),
        }
    }
}

impl std::fmt::Display for ShapesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShapesError::TooLarge { bytes, limit } => write!(
                f,
                "the shapes are {bytes} bytes, more than the limit of {limit}; load smaller shapes, or raise ProfileLimits::max_bytes for shapes you trust"
            ),
            ShapesError::Json(e) => write!(
                f,
                "not valid JSON: {e}. Shapes are read as JSON-LD, or as Turtle in the build that reads it"
            ),
            ShapesError::LooksLikeHtml => write!(
                f,
                "this looks like an HTML page, not shapes: the server probably sent its landing page. Download the shapes as JSON-LD or Turtle instead"
            ),
            ShapesError::UnsupportedFormat => write!(
                f,
                "this is not JSON-LD, and this build does not read it as Turtle: convert the shapes to JSON-LD, or use the build that reads Turtle"
            ),
            ShapesError::Turtle(e) => write!(f, "not valid Turtle: {e}"),
            ShapesError::LimitExceeded(what) => write!(
                f,
                "the shapes go past the reader's limit {what}, so none of them were read; raise ProfileLimits::{what} for shapes you trust"
            ),
            ShapesError::NotShapes(why) => write!(
                f,
                "{why}. Shapes are SHACL (sh:property, sh:path, sh:in ...) over ODRL policies; load the profile as a profile"
            ),
        }
    }
}

impl std::error::Error for ShapesError {}
