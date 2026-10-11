//! What a closed list takes: the pure part of the closed combobox. The
//! component (slot.rs) decides when to ask; this module decides what typed
//! text means against the entries a list offers, and says why it was not
//! taken.
use prose_core::profile::Wording;

use super::config::{Labels, Suggestion, fill};

/// The entry whose term, label or IRI is `typed`, ignoring case and a
/// leading `odrl:`. `same_iri` says whether two written forms name one IRI.
pub(crate) fn exact<'a>(
    options: &'a [Suggestion],
    typed: &str,
    same_iri: &dyn Fn(&str, &str) -> bool,
) -> Option<&'a Suggestion> {
    let _ = (options, typed, same_iri);
    None
}

/// The entry nearest to `typed` by edit distance on the local name, when it
/// is near enough to be a slip of the keys and not a different word.
pub(crate) fn closest<'a>(options: &'a [Suggestion], typed: &str) -> Option<&'a Suggestion> {
    let _ = (options, typed);
    None
}

/// The one entry left when `typed` is filtered, if exactly one is.
pub(crate) fn only<'a>(options: &'a [Suggestion], typed: &str) -> Option<&'a Suggestion> {
    let _ = (options, typed);
    None
}

/// "Research Data Profile 0.1.0, Other 2", or "ODRL" with no profile.
pub(crate) fn profile_names(w: &Wording) -> String {
    let _ = w;
    String::new()
}

/// Why `typed` was not taken, with the nearest entry when there is one.
pub(crate) fn not_allowed_message(
    labels: &Labels,
    typed: &str,
    field: &str,
    profile: &str,
    near: Option<&Suggestion>,
) -> String {
    let _ = (labels, typed, field, profile, near);
    String::new()
}

/// What a polite live region says when a closed field goes back to `kept`.
pub(crate) fn kept_message(labels: &Labels, kept: &str, typed: &str) -> String {
    let _ = (labels, kept, typed, fill);
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use prose_core::profile::ProfileVocab;

    fn s(value: &str, label: &str) -> Suggestion {
        Suggestion::new(value, label, None)
    }

    fn actions() -> Vec<Suggestion> {
        vec![
            s("ex:distill", "Distill"),
            s("ex:train", "Train"),
            s("use", "use"),
            s("odrl:read", "read"),
            s(
                "https://example.org/odrl/profile/research/fineTune",
                "Fine-tune",
            ),
        ]
    }

    fn never(_: &str, _: &str) -> bool {
        false
    }

    #[test]
    fn exact_matches_term_label_and_iri_ignoring_case() {
        let o = actions();
        let value = |t: &str| exact(&o, t, &never).map(|s| s.value.to_string());
        assert_eq!(value("ex:distill").as_deref(), Some("ex:distill"));
        assert_eq!(value("Distill").as_deref(), Some("ex:distill"));
        assert_eq!(value("DISTILL").as_deref(), Some("ex:distill"));
        assert_eq!(value("  distill ").as_deref(), Some("ex:distill"));
        assert_eq!(value("Fine-tune").as_deref(), Some(o[4].value.as_str()));
        // A leading odrl: is not part of the name, on either side.
        assert_eq!(value("odrl:use").as_deref(), Some("use"));
        assert_eq!(value("read").as_deref(), Some("odrl:read"));
        // Not a prefix, not a substring.
        assert_eq!(value("dist"), None);
        assert_eq!(value("distil"), None);
        assert_eq!(value(""), None);
    }

    #[test]
    fn exact_matches_an_iri_written_another_way() {
        let o = actions();
        let same = |a: &str, b: &str| {
            let full = |x: &str| x.replace("ex:", "https://example.org/odrl/profile/research/");
            full(a) == full(b)
        };
        let got = exact(
            &o,
            "https://example.org/odrl/profile/research/distill",
            &same,
        );
        assert_eq!(got.map(|s| s.value.as_str()), Some("ex:distill"));
        // Case matters inside an IRI, so only the term and label are folded.
        let got = exact(
            &o,
            "https://example.org/odrl/profile/research/Distill",
            &same,
        );
        assert_eq!(got, None);
    }

    #[test]
    fn closest_is_a_slip_not_a_different_word() {
        let o = actions();
        let near = |t: &str| closest(&o, t).map(|s| s.value.to_string());
        assert_eq!(near("distil").as_deref(), Some("ex:distill"));
        assert_eq!(near("ex:distl").as_deref(), Some("ex:distill"));
        assert_eq!(near("Trian").as_deref(), Some("ex:train"));
        assert_eq!(near("fine tune").as_deref(), Some(o[4].value.as_str()));
        // Too far, too short to tell, empty, too long to compare.
        assert_eq!(near("transfer"), None);
        assert_eq!(near("u"), None);
        assert_eq!(near(""), None);
        assert_eq!(near(&"distill".repeat(40)), None);
        // An exact entry is not "near" anything else.
        assert_eq!(near("use").as_deref(), Some("use"));
    }

    #[test]
    fn closest_prefers_the_smaller_distance_then_the_earlier_entry() {
        let o = vec![s("ab", "ab"), s("abcd", "abcd"), s("abce", "abce")];
        assert_eq!(closest(&o, "abcf").map(|s| s.value.as_str()), Some("abcd"));
        assert_eq!(closest(&o, "abcd").map(|s| s.value.as_str()), Some("abcd"));
    }

    #[test]
    fn only_is_the_single_filtered_entry() {
        let o = actions();
        let one = |t: &str| only(&o, t).map(|s| s.value.to_string());
        assert_eq!(one("dist").as_deref(), Some("ex:distill"));
        assert_eq!(one("fine").as_deref(), Some(o[4].value.as_str()));
        // Several, none, empty.
        assert_eq!(one("t"), None);
        assert_eq!(one("zzz"), None);
        assert_eq!(one(""), None);
    }

    #[test]
    fn profile_names_are_titles_with_versions() {
        let text = include_str!("../../../prose-core/tests/profiles/research-closed.jsonld");
        let w = Wording::new().with_profile(ProfileVocab::parse(text).unwrap());
        assert_eq!(profile_names(&w), "Research Data Profile 0.1.0");
        assert_eq!(profile_names(&Wording::new()), "ODRL");
        let other = include_str!("../../../prose-core/tests/profiles/other.jsonld");
        let two = w.with_profile(ProfileVocab::parse(other).unwrap());
        let names = profile_names(&two);
        assert!(
            names.starts_with("Research Data Profile 0.1.0, "),
            "{names}"
        );
    }

    #[test]
    fn the_message_names_the_field_the_profile_and_the_near_entry() {
        let l = Labels::default();
        let o = actions();
        let msg = not_allowed_message(
            &l,
            "distil",
            "action",
            "Research Data Profile 0.1.0",
            Some(&o[0]),
        );
        assert_eq!(
            msg,
            "\"distil\" is not an allowed action in Research Data Profile 0.1.0. Did you mean Distill?"
        );
        let msg = not_allowed_message(&l, "zzz", "unit", "ODRL", None);
        assert_eq!(msg, "\"zzz\" is not an allowed unit in ODRL.");
    }

    #[test]
    fn the_announcement_keeps_the_old_value_or_says_the_field_stays_empty() {
        let l = Labels::default();
        assert_eq!(
            kept_message(&l, "use", "distil"),
            "Kept \"use\"; \"distil\" is not in the profile"
        );
        assert_eq!(
            kept_message(&l, "", "distil"),
            "\"distil\" is not in the profile; the field stays empty"
        );
    }
}
