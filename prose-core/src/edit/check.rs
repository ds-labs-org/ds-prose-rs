//! What the loaded profiles say about a document, as advisory issues. The
//! reducer never consults them: a profile widens and annotates, it never
//! refuses an edit.
use super::issue::Issue;
use super::model::EditDoc;
use crate::profile::Wording;
use crate::validate::{Options, validate};

/// What the loaded profiles in `wording` say about `doc`, in document
/// order: advisory [`Issue`]s, never a refusal. A term undeclared in a
/// loaded profile's namespace or deprecated by it is a warning on its slot,
/// a declared profile that is not loaded a warning on the policy; an
/// operator or literal outside the left operand's `prose:` hints is info.
/// Empty when no profile is loaded.
///
/// This is [`crate::validate::validate`] in [`Options::advisory`], as
/// issues.
pub fn check_profile(doc: &EditDoc, wording: &Wording) -> Vec<Issue> {
    if wording.is_empty() {
        return Vec::new();
    }
    validate(doc, wording, &Options::advisory()).issues()
}
