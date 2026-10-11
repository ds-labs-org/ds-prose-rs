//! Wording with loaded profiles: a term a loaded profile declares reads with
//! the profile's label; every other term, and every ODRL term, reads as the
//! `words` module phrases it.
use crate::profile::{Prefixes, Wording};
use crate::words::{self, local_name};

/// The wording one reading uses. Without a profile it is `words`, exactly.
#[allow(dead_code)] // the profile fields are read once profile wording lands
pub(crate) struct WordCtx<'a> {
    /// `None` when no profile is loaded.
    wording: Option<&'a Wording>,
    prefixes: Prefixes,
}

impl<'a> WordCtx<'a> {
    /// The built-in wording.
    pub(crate) fn plain() -> WordCtx<'static> {
        WordCtx {
            wording: None,
            prefixes: Prefixes::default(),
        }
    }

    /// `w`, with a policy's compact terms expanded by `prefixes`.
    pub(crate) fn new(w: &'a Wording, prefixes: Prefixes) -> WordCtx<'a> {
        WordCtx {
            wording: (!w.is_empty()).then_some(w),
            prefixes,
        }
    }

    /// An action as a base-form verb phrase.
    pub(crate) fn action(&self, raw: &str) -> String {
        words::action(raw)
    }

    /// A left operand as a noun phrase.
    pub(crate) fn operand(&self, raw: &str) -> String {
        words::operand(raw)
    }

    /// An operator as the verb phrase that follows `left`.
    pub(crate) fn operator_for(&self, left: &str, op: &str) -> String {
        words::operator_for(left, op)
    }

    /// A unit, after its value.
    pub(crate) fn unit(&self, raw: &str) -> String {
        words::humanise(local_name(raw))
    }
}
