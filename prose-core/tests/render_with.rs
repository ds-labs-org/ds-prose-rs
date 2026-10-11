//! `render_with` and `sentences_with` with no profile loaded: the output is
//! the v0.1 reading, byte for byte, and the glossary is empty.
mod common;
use common::*;
use prose_core::edit::{read_model, sentences, sentences_with};
use prose_core::profile::Wording;
use prose_core::{render, render_value, render_value_with, render_with};

#[test]
fn default_wording_identical_for_all_v01_fixtures() {
    let w = Wording::default();
    for name in FIXTURES {
        let text = fixture(name);
        let plain = format!("{:#?}", render(&text));
        let with = format!("{:#?}", render_with(&text, &w).map(|r| r.document));
        assert_eq!(with, plain, "render_with differs for {name}");
        let v = fixture_value(name);
        let plain = format!("{:#?}", render_value(&v));
        let with = format!("{:#?}", render_value_with(&v, &w).map(|r| r.document));
        assert_eq!(with, plain, "render_value_with differs for {name}");
    }
}

#[test]
fn errors_are_the_same_with_and_without_wording() {
    let w = Wording::default();
    for text in ["not json", "{}", "[]", r#"{"@graph": [{"uid": "x"}]}"#] {
        assert_eq!(
            render_with(text, &w).map(|r| r.document),
            render(text),
            "{text}"
        );
    }
}

#[test]
fn empty_glossary_without_profile() {
    let w = Wording::default();
    for name in FIXTURES {
        let reading = render_with(&fixture(name), &w).unwrap();
        assert_eq!(
            reading.glossary.len(),
            reading.document.policies.len(),
            "one glossary per policy in {name}"
        );
        assert!(
            reading.glossary.iter().all(|g| g.terms.is_empty()),
            "{name}: {:?}",
            reading.glossary
        );
    }
}

#[test]
fn sentences_with_default_equals_sentences_all_fixtures() {
    let w = Wording::default();
    for name in FIXTURES {
        let doc = read_model(&fixture(name)).unwrap();
        assert_eq!(
            format!("{:#?}", sentences_with(&doc, &w)),
            format!("{:#?}", sentences(&doc)),
            "sentences_with differs for {name}"
        );
    }
}
