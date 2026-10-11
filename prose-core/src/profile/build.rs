//! Triples to a vocabulary: what each subject is, as ODRL sees it.
use super::triples::Parsed;
use super::{ProfileError, ProfileLimits, ProfileVocab};

pub(crate) fn vocab(parsed: Parsed, limits: &ProfileLimits) -> Result<ProfileVocab, ProfileError> {
    let _ = (parsed, limits);
    Err(ProfileError::NoTerms)
}
