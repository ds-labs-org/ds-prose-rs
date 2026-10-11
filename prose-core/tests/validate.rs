//! `prose_core::validate`: the advisory mode is `check_profile`, exactly;
//! the rule catalogue and how options resolve a rule's level.
mod common;
mod corpus;
use corpus::*;
use prose_core::edit::*;
use prose_core::profile::Wording;
use prose_core::validate::*;
use std::sync::Arc;

// --- T1: advisory equals check_profile --------------------------------------

#[test]
fn advisory_equals_check_profile_on_every_fixture() {
    let mut compared = 0;
    for (name, text) in policy_corpus() {
        let Ok(doc) = read_model(&text) else {
            continue;
        };
        for (label, w) in wordings() {
            let want = check_profile(&doc, &w);
            let report = validate(&doc, &w, &Options::advisory());
            assert_eq!(report.issues(), want, "{name} under {label}");
            assert_eq!(report.mode, Mode::Advisory);
            let v = Validator::new(Arc::new(w.clone()), Options::advisory());
            assert_eq!(v.validate(&doc).issues(), want, "{name} under {label}");
            compared += 1;
        }
    }
    assert!(compared >= 300, "{compared}");
}

#[test]
fn advisory_report_matches_the_goldens() {
    // The same goldens check_profile is pinned by, built from the report.
    for (name, text) in policy_corpus() {
        let Ok(doc) = read_model(&text) else {
            continue;
        };
        let mut got = String::new();
        for (label, w) in wordings() {
            got.push_str(&format!("## {label}\n"));
            for i in validate(&doc, &w, &Options::advisory()).issues() {
                got.push_str(&issue_line(&i));
                got.push('\n');
            }
        }
        compare_golden(&format!("advisory/{name}.txt"), &got);
    }
}

#[test]
fn empty_wording_empty_report() {
    let w = Wording::default();
    for (name, text) in policy_corpus() {
        let Ok(doc) = read_model(&text) else {
            continue;
        };
        let r = validate(&doc, &w, &Options::advisory());
        assert!(r.findings.is_empty(), "{name}: {:?}", r.findings);
        assert!(r.gaps.is_empty(), "{name}");
        assert_eq!(r.verdict, Verdict::Conforms, "{name}");
        assert!(r.conforms(), "{name}");
    }
}

#[test]
fn options_default_is_advisory() {
    assert_eq!(Options::default(), Options::advisory());
    assert_eq!(Options::default().mode(), Mode::Advisory);
    assert_eq!(Options::strict().mode(), Mode::Strict);
    assert_ne!(Options::strict(), Options::advisory());
}

#[test]
fn validator_equality_is_by_wording_pointer_and_options() {
    let w = Arc::new(wording_of(&["research.jsonld"]));
    let a = Validator::new(w.clone(), Options::strict());
    let b = Validator::new(w.clone(), Options::strict());
    assert!(a == b);
    assert!(a.clone() == a);
    // Equal content, another allocation: a different validator.
    let c = Validator::new(Arc::new((*w).clone()), Options::strict());
    assert!(a != c);
    assert!(a != Validator::new(w, Options::advisory()));
    assert_eq!(a.options(), &Options::strict());
}

#[test]
fn validate_json_advisory_equals_validate_on_the_model() {
    let w = wording_of(&["research.jsonld"]);
    let text = profile_text("research-policy.json");
    let doc = read_model(&text).unwrap();
    assert_eq!(
        validate_json(&text, &w, &Options::advisory()),
        validate(&doc, &w, &Options::advisory())
    );
}
