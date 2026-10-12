//! What a closed list takes: the pure part of the closed combobox. The
//! component (slot.rs) decides when to ask; this module decides what typed
//! text means against the entries a list offers, and says why it was not
//! taken.
use prose_core::profile::Wording;

use super::config::{Labels, Suggestion, fill};
use super::suggest::{filter_suggestions, strip};

/// Typed text longer than this is never compared for nearness: the work is
/// quadratic in the length.
const LONGEST_COMPARED: usize = 64;
/// At most this many entries are compared for nearness.
const MOST_COMPARED: usize = 1000;

/// The entry whose term, label or IRI is `typed`, ignoring case and a
/// leading `odrl:`. `same_iri` says whether two written forms name one IRI.
pub(crate) fn exact<'a>(
    options: &'a [Suggestion],
    typed: &str,
    same_iri: &dyn Fn(&str, &str) -> bool,
) -> Option<&'a Suggestion> {
    let typed = typed.trim();
    if typed.is_empty() {
        return None;
    }
    let folded = strip(typed);
    options
        .iter()
        .find(|o| strip(&o.value) == folded || strip(&o.label) == folded)
        .or_else(|| options.iter().find(|o| same_iri(&o.value, typed)))
}

/// What follows the last `:`, `/` or `#`, in lower case.
fn local(s: &str) -> String {
    let s = s.trim();
    let at = s.rfind([':', '/', '#']).map_or(0, |i| i + 1);
    s[at..].to_lowercase()
}

/// Edit distance between two short texts, in characters.
fn distance(a: &[char], b: &[char]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diag = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let up = row[j + 1];
            row[j + 1] = (diag + usize::from(ca != cb)).min(up + 1).min(row[j] + 1);
            diag = up;
        }
    }
    row[b.len()]
}

/// The entry nearest to `typed` by edit distance on the local name, when it
/// is near enough to be a slip of the keys and not a different word.
pub(crate) fn closest<'a>(options: &'a [Suggestion], typed: &str) -> Option<&'a Suggestion> {
    let t: Vec<char> = local(typed).chars().collect();
    if t.len() < 2 || t.len() > LONGEST_COMPARED {
        return None;
    }
    let limit = ((t.len() + 1) / 3).max(1);
    let mut best: Option<(usize, &Suggestion)> = None;
    for o in options.iter().take(MOST_COMPARED) {
        for name in [local(&o.value), local(&o.label)] {
            let n: Vec<char> = name.chars().collect();
            if n.len() > LONGEST_COMPARED {
                continue;
            }
            let d = distance(&t, &n);
            if d <= limit && d < t.len() && best.is_none_or(|(b, _)| d < b) {
                best = Some((d, o));
            }
        }
    }
    best.map(|(_, o)| o)
}

/// The one entry left when `typed` is filtered, if exactly one is.
pub(crate) fn only<'a>(options: &'a [Suggestion], typed: &str) -> Option<&'a Suggestion> {
    if typed.trim().is_empty() {
        return None;
    }
    let mut left = filter_suggestions(options, typed, 2);
    match left.len() {
        1 => left.pop(),
        _ => None,
    }
}

/// "Research Data Profile 0.1.0, Other 2", or "ODRL" with no profile.
pub(crate) fn profile_names(w: &Wording) -> String {
    let names: Vec<String> = w
        .profiles()
        .map(|p| {
            let name = p
                .title()
                .pick(w.languages())
                .or(p.iri())
                .unwrap_or("unnamed profile");
            match p.version() {
                Some(v) => format!("{name} {v}"),
                None => name.to_string(),
            }
        })
        .collect();
    if names.is_empty() {
        "ODRL".to_string()
    } else {
        names.join(", ")
    }
}

/// Why `typed` was not taken, with the nearest entry when there is one.
pub(crate) fn not_allowed_message(
    labels: &Labels,
    typed: &str,
    field: &str,
    profile: &str,
    near: Option<&Suggestion>,
) -> String {
    let mut msg = fill(
        &labels.not_allowed_term,
        &[("typed", typed), ("field", field), ("profile", profile)],
    );
    if let Some(n) = near {
        msg.push(' ');
        msg.push_str(&fill(&labels.did_you_mean, &[("suggestion", &n.label)]));
    }
    msg
}

/// What a polite live region says when a closed field goes back to `kept`.
pub(crate) fn kept_message(labels: &Labels, kept: &str, typed: &str) -> String {
    if kept.is_empty() {
        fill(&labels.kept_empty, &[("typed", typed)])
    } else {
        fill(&labels.kept_previous, &[("kept", kept), ("typed", typed)])
    }
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
        // Three slips in five letters is a different word.
        assert_eq!(near("txxyn"), None);
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
