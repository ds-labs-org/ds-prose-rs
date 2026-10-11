//! `render_with` with a loaded profile: profile terms read with the
//! profile's labels, ODRL terms keep their built-in phrasing, each policy
//! gets a glossary, and the reader warns about what the profile does not
//! cover.
use prose_core::edit::{Segment, read_model, sentences_with};
use prose_core::profile::{Prefixes, ProfileLimits, ProfileVocab, TermKind, Wording};
use prose_core::{Reading, render, render_with, wording};
use serde_json::{Value, json};

const ODRL: &str = "http://www.w3.org/ns/odrl/2/";
const EX: &str = "https://example.org/odrl/profile/research/";
const RESEARCH: &str = "https://example.org/odrl/profile/research";
const UNIVERSITY: &str = "http://example.com/org:university";
const COHORT: &str = "http://example.com/data:cohort-2026";
const HOSPITAL: &str = "http://example.com/org:hospital";

fn profile_text(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/profiles/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn research() -> Wording {
    Wording::new().with_profile(ProfileVocab::parse(&profile_text("research.jsonld")).unwrap())
}

fn vocab(v: Value) -> ProfileVocab {
    ProfileVocab::from_value(&v, &ProfileLimits::default()).unwrap()
}

fn read(policy: &str, w: &Wording) -> Reading {
    render_with(policy, w).unwrap()
}

fn research_reading() -> Reading {
    read(&profile_text("research-policy.json"), &research())
}

/// A policy with the research prefix bound, one permission holding `rule`.
fn policy_with(rule: Value) -> String {
    json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"ex": EX}],
        "@type": "Set",
        "uid": "urn:policy:t",
        "permission": [rule]
    })
    .to_string()
}

fn condition(left: &str, op: &str, right: Value) -> String {
    let p = policy_with(json!({
        "target": "urn:asset:1",
        "action": "use",
        "constraint": [{"leftOperand": left, "operator": op, "rightOperand": right}]
    }));
    read(&p, &research()).document.policies[0].rules[0].conditions[0]
        .text
        .clone()
}

fn written(r: &Reading, policy: usize) -> Vec<&str> {
    r.glossary[policy]
        .terms
        .iter()
        .map(|g| g.written.as_str())
        .collect()
}

#[test]
fn action_label() {
    let r = research_reading();
    let rule = &r.document.policies[0].rules[0];
    assert_eq!(
        rule.sentence,
        format!("{UNIVERSITY} may fine-tune {COHORT}, as permitted by {HOSPITAL}.")
    );
    assert_eq!(
        rule.refinements[0].text,
        "The action \"fine-tune\" is limited to cases where"
    );
    let ob = &r.document.policies[0].rules[3];
    assert_eq!(
        ob.sentence,
        format!("{UNIVERSITY} must pseudonymise {COHORT}, as required by {HOSPITAL}.")
    );
}

#[test]
fn left_operand_label_with_article() {
    let r = research_reading();
    let rule = &r.document.policies[0].rules[0];
    assert_eq!(
        rule.refinements[0].children[0].text,
        "the number of data subjects is at least 50"
    );
    assert_eq!(
        rule.conditions[1].text,
        "the retention period is at most P2Y"
    );
    // A label that already starts with an article gets no second one.
    let w = Wording::new().with_profile(vocab(json!({
        "@context": {"odrl": ODRL, "ex": EX, "rdfs": "http://www.w3.org/2000/01/rdf-schema#"},
        "@graph": [{"@id": "ex:size", "@type": "odrl:LeftOperand", "rdfs:label": "The cohort size"}]
    })));
    let p = policy_with(json!({
        "target": "urn:asset:1", "action": "use",
        "constraint": [{"leftOperand": "ex:size", "operator": "lt", "rightOperand": 9}]
    }));
    assert_eq!(
        read(&p, &w).document.policies[0].rules[0].conditions[0].text,
        "the cohort size is less than 9"
    );
}

#[test]
fn right_operand_iri_labels() {
    let r = research_reading();
    assert_eq!(
        r.document.policies[0].rules[0].conditions[0].text,
        "the research purpose is any of clinical research or academic research"
    );
    // A plain string is a literal, not a term: it reads as written.
    assert_eq!(
        condition("ex:researchPurpose", "eq", json!("ex:clinical")),
        "the research purpose is equal to ex:clinical"
    );
}

#[test]
fn operator_label_is_phrase() {
    assert_eq!(
        condition("spatial", "ex:withinRegion", json!("EU")),
        "the location is within region \"EU\""
    );
    // A label that already reads as a verb phrase is kept.
    let w = Wording::new().with_profile(vocab(json!({
        "@context": {"odrl": ODRL, "ex": EX, "rdfs": "http://www.w3.org/2000/01/rdf-schema#"},
        "@graph": [{"@id": "ex:overlaps", "@type": "odrl:Operator", "rdfs:label": "Has overlap with"}]
    })));
    let p = policy_with(json!({
        "target": "urn:asset:1", "action": "use",
        "constraint": [{"leftOperand": "spatial", "operator": "ex:overlaps", "rightOperand": "EU"}]
    }));
    assert_eq!(
        read(&p, &w).document.policies[0].rules[0].conditions[0].text,
        "the location has overlap with \"EU\""
    );
}

#[test]
fn unit_label_as_written() {
    let w = Wording::new().with_profile(vocab(json!({
        "@context": {"odrl": ODRL, "ex": EX, "rdfs": "http://www.w3.org/2000/01/rdf-schema#"},
        "@graph": [{"@id": "ex:eur", "@type": "odrl:RightOperand", "rdfs:label": "EUR"}]
    })));
    let p = policy_with(json!({
        "target": "urn:asset:1", "action": "use",
        "constraint": [{"leftOperand": "payAmount", "operator": "eq",
                        "rightOperand": 10, "unit": "ex:eur"}]
    }));
    assert_eq!(
        read(&p, &w).document.policies[0].rules[0].conditions[0].text,
        "the payment amount is equal to 10 EUR"
    );
}

#[test]
fn policy_class_heading() {
    let p = json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"ex": EX}],
        "@type": "ex:ResearchAgreement",
        "uid": "urn:policy:ra",
        "permission": [{"target": "urn:asset:1", "action": "use"}]
    })
    .to_string();
    assert_eq!(
        read(&p, &research()).document.policies[0].heading,
        "Research agreement urn:policy:ra"
    );
    assert_eq!(
        render(&p).unwrap().policies[0].heading,
        "Policy urn:policy:ra"
    );
}

#[test]
fn party_function_rendered_not_warned() {
    let p = policy_with(json!({
        "target": "urn:asset:1",
        "action": "use",
        "ex:dataSteward": "http://example.com/person:carol"
    }));
    let r = read(&p, &research());
    assert_eq!(
        r.document.policies[0].rules[0].sentence,
        "Anyone may use urn:asset:1. Data steward: http://example.com/person:carol."
    );
    assert!(r.document.warnings.is_empty(), "{:?}", r.document.warnings);
    // Without a profile the warning stays exactly as today.
    let plain = render(&p).unwrap();
    assert_eq!(
        plain.policies[0].rules[0].sentence,
        "Anyone may use urn:asset:1."
    );
    assert_eq!(
        plain.warnings,
        vec!["ignored unknown property \"ex:dataSteward\" on a rule".to_string()]
    );
}

#[test]
fn core_terms_keep_words_phrasing() {
    let w = Wording::new()
        .with_profile(ProfileVocab::parse(&profile_text("hostile/relabel-core.jsonld")).unwrap());
    let p = policy_with(json!({"target": "urn:asset:1", "action": "distribute"}));
    let r = read(&p, &w);
    assert_eq!(
        r.document.policies[0].rules[0].sentence,
        "Anyone may distribute urn:asset:1."
    );
    // The profile's text only reaches the glossary.
    let g = &r.glossary[0].terms[0];
    assert_eq!(
        (g.written.as_str(), g.iri.as_str()),
        ("distribute", &*format!("{ODRL}distribute"))
    );
    assert_eq!(g.label, "never distribute");
}

#[test]
fn core_profile_changes_no_sentence() {
    let w = Wording::new()
        .with_profile(ProfileVocab::parse(&profile_text("odrl22-excerpt.json")).unwrap());
    let text = std::fs::read_to_string(format!(
        "{}/tests/fixtures/agreement.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let r = read(&text, &w);
    assert_eq!(r.document, render(&text).unwrap());
    assert_eq!(
        written(&r, 0),
        vec!["Agreement", "odrl:distribute", "purpose", "spatial"]
    );
}

#[test]
fn glossary_per_policy_first_use_dedup() {
    let r = research_reading();
    assert_eq!(r.glossary.len(), 1);
    assert_eq!(
        written(&r, 0),
        vec![
            "ex:fineTune",
            "ex:dataSubjectCount",
            "ex:researchPurpose",
            "ex:clinical",
            "ex:academic",
            "ex:retentionPeriod",
            "ex:pseudonymise",
        ]
    );
    let two = json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"ex": EX}],
        "@graph": [
            {"@type": "Set", "uid": "urn:p:1", "permission": [
                {"target": "urn:a", "action": ["ex:train", "ex:fineTune"]},
                {"target": "urn:b", "action": "ex:train"}]},
            {"@type": "Set", "uid": "urn:p:2", "permission": [
                {"target": "urn:a", "action": "ex:train"}]}
        ]
    })
    .to_string();
    let r = read(&two, &research());
    assert_eq!(written(&r, 0), vec!["ex:train", "ex:fineTune"]);
    assert_eq!(written(&r, 1), vec!["ex:train"]);
}

#[test]
fn glossary_entry_fields() {
    let r = research_reading();
    let g = &r.glossary[0].terms[0];
    assert_eq!(g.iri, format!("{EX}fineTune"));
    assert_eq!(g.kind, TermKind::Action);
    assert_eq!(g.label, "Fine-tune");
    assert_eq!(
        g.definition.as_deref(),
        Some("To adapt an already trained model using the Asset.")
    );
    assert_eq!(
        g.note.as_deref(),
        Some("Fine-tuning implies that the Asset may be read.")
    );
    assert_eq!(g.profile.as_deref(), Some("Research Data Profile"));
    assert!(!g.deprecated);
    // The note's markup was stripped when the profile was read.
    let count = &r.glossary[0].terms[1];
    assert_eq!(count.kind, TermKind::LeftOperand);
    assert!(!count.note.as_deref().unwrap().contains('<'), "{count:?}");
}

#[test]
fn glossary_broader_and_implies_labels() {
    let r = research_reading();
    let by = |w: &str| r.glossary[0].terms.iter().find(|g| g.written == w).unwrap();
    let fine = by("ex:fineTune");
    assert_eq!(
        fine.broader,
        vec![(format!("{EX}train"), "Train".to_string())]
    );
    assert_eq!(
        fine.implies,
        vec![(format!("{ODRL}read"), "read".to_string())]
    );
    let clinical = by("ex:clinical");
    assert_eq!(clinical.kind, TermKind::RightOperand);
    assert_eq!(
        clinical.broader,
        vec![(format!("{EX}academic"), "Academic research".to_string())]
    );
    let purpose = by("ex:researchPurpose");
    assert_eq!(
        purpose.broader,
        vec![(format!("{ODRL}purpose"), "purpose".to_string())]
    );
}

#[test]
fn label_language_follows_wording() {
    let w = research().with_languages(vec!["de-CH".into()]);
    let p = policy_with(json!({"target": "urn:asset:1", "action": "ex:train"}));
    let r = read(&p, &w);
    assert_eq!(
        r.document.policies[0].rules[0].sentence,
        "Anyone may trainieren urn:asset:1."
    );
    assert_eq!(r.glossary[0].terms[0].label, "Trainieren");
}

#[test]
fn hostile_label_is_plain_in_sentence() {
    let w = Wording::new().with_profile(
        ProfileVocab::parse(&profile_text("hostile/bidi-html-label.jsonld")).unwrap(),
    );
    let p = json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"ex": "https://example.org/hostile/"}],
        "@type": "Set",
        "permission": [{"target": "urn:asset:1", "action": "ex:fineTune"}]
    })
    .to_string();
    let r = read(&p, &w);
    let s = &r.document.policies[0].rules[0].sentence;
    assert!(
        !s.contains(['<', '>', '\u{202e}', '\u{200b}', '\u{0}']),
        "{s:?}"
    );
    // Only the first letter is lower-cased, and only that of the label.
    assert!(s.starts_with("Anyone may exe.gnp "), "{s:?}");
    assert!(s.contains("Fine-tune & train"), "{s:?}");
}

#[test]
fn profile_note_names_the_loaded_profile() {
    let r = research_reading();
    assert_eq!(
        r.document.policies[0].notes[0],
        format!("Written to the profile Research Data Profile ({RESEARCH}, version 0.1.0).")
    );
    let plain = render(&profile_text("research-policy.json")).unwrap();
    assert_eq!(
        plain.policies[0].notes[0],
        format!("Written to the profile {RESEARCH}.")
    );
}

#[test]
fn undeclared_in_profile_namespace_warns() {
    let r = research_reading();
    assert_eq!(
        r.document.warnings,
        vec![format!(
            "the action \"ex:distill\" is not declared by the loaded profile <{RESEARCH}>"
        )]
    );
    // Still read, with the built-in wording.
    assert_eq!(
        r.document.policies[0].rules[1].sentence,
        format!("{UNIVERSITY} may distill {COHORT}, as permitted by {HOSPITAL}.")
    );
    let p = policy_with(json!({
        "target": "urn:asset:1", "action": "use",
        "constraint": [{"leftOperand": "ex:typo", "operator": "ex:near",
                        "rightOperand": {"@id": "ex:nowhere"}}]
    }));
    let warnings = read(&p, &research()).document.warnings;
    for (what, term) in [
        ("left operand", "ex:typo"),
        ("operator", "ex:near"),
        ("right operand", "ex:nowhere"),
    ] {
        let msg =
            format!("the {what} \"{term}\" is not declared by the loaded profile <{RESEARCH}>");
        assert!(warnings.contains(&msg), "{msg} not in {warnings:?}");
    }
}

#[test]
fn foreign_namespace_silent() {
    let p = policy_with(json!({
        "target": "urn:asset:1",
        "action": ["https://other.example/ns/distill", "commercialize"],
        "constraint": [{"leftOperand": "https://other.example/ns/size",
                        "operator": "lt", "rightOperand": 3}]
    }));
    let r = read(&p, &research());
    assert!(r.document.warnings.is_empty(), "{:?}", r.document.warnings);
    assert!(r.glossary[0].terms.is_empty());
}

#[test]
fn deprecated_term_warns() {
    let w = Wording::new().with_profile(vocab(json!({
        "@context": {"odrl": ODRL, "ex": EX, "owl": "http://www.w3.org/2002/07/owl#"},
        "@graph": [
            {"@id": "https://example.org/odrl/profile/research", "@type": "owl:Ontology"},
            {"@id": "ex:old", "@type": "odrl:Action", "owl:deprecated": true}
        ]
    })));
    let p = policy_with(json!({"target": "urn:asset:1", "action": "ex:old"}));
    let r = read(&p, &w);
    assert_eq!(
        r.document.warnings,
        vec![format!(
            "\"ex:old\" is deprecated in the profile <{RESEARCH}>"
        )]
    );
    assert!(r.glossary[0].terms[0].deprecated);
}

#[test]
fn declared_profile_not_loaded_warns_once() {
    let two = json!({
        "@graph": [
            {"@type": "Set", "uid": "urn:p:1", "profile": "https://example.org/other-profile",
             "permission": [{"target": "urn:a", "action": "use"}]},
            {"@type": "Set", "uid": "urn:p:2",
             "profile": ["https://example.org/other-profile", "http://www.w3.org/ns/odrl/2/core",
                         "https://example.org/odrl/profile/research/"],
             "permission": [{"target": "urn:a", "action": "use"}]}
        ]
    })
    .to_string();
    let r = read(&two, &research());
    assert_eq!(
        r.document.warnings,
        vec![
            "the policy declares the profile <https://example.org/other-profile>, which is not loaded"
                .to_string()
        ]
    );
    // Without a profile loaded, nothing is said about it.
    assert!(render(&two).unwrap().warnings.is_empty());
    assert!(read(&two, &Wording::default()).document.warnings.is_empty());
}

#[test]
fn collision_policy_labels_only_ex_term() {
    let w = Wording::new().with_profile(vocab(json!({
        "@context": {"odrl": ODRL, "ex": EX, "rdfs": "http://www.w3.org/2000/01/rdf-schema#"},
        "@graph": [{"@id": "ex:purpose", "@type": "odrl:LeftOperand", "rdfs:label": "Study purpose"}]
    })));
    let r = read(&profile_text("hostile/collision-policy.json"), &w);
    let texts: Vec<&str> = r.document.policies[0].rules[0]
        .conditions
        .iter()
        .map(|c| c.text.as_str())
        .collect();
    assert_eq!(
        texts,
        vec![
            "the purpose is equal to \"education\"",
            "the study purpose is equal to \"education\"",
            "the purpose is equal to \"education\"",
        ]
    );
    assert_eq!(written(&r, 0), vec!["ex:purpose"]);
}

#[test]
fn public_gloss_of_a_written_term() {
    let w = research();
    let policy: Value = serde_json::from_str(&profile_text("research-policy.json")).unwrap();
    let p = Prefixes::of_document(&policy);
    let g = wording::gloss(&w, &p, "ex:train").unwrap();
    assert_eq!((g.label.as_str(), g.kind), ("Train", TermKind::Action));
    assert_eq!(g.broader, vec![(format!("{ODRL}use"), "use".to_string())]);
    assert!(wording::gloss(&w, &p, "ex:distill").is_none());
    assert!(wording::gloss(&Wording::default(), &p, "ex:train").is_none());
}

#[test]
fn edit_sentences_use_the_same_labels() {
    let doc = read_model(&profile_text("research-policy.json")).unwrap();
    let prose = sentences_with(&doc, &research());
    let slots: Vec<(String, String)> = prose[0].rules[0]
        .sentence
        .segments
        .iter()
        .filter_map(|s| match s {
            Segment::List(l) => Some(
                l.items
                    .iter()
                    .map(|i| (i.slot.raw.clone(), i.slot.display.clone())),
            ),
            _ => None,
        })
        .flatten()
        .collect();
    assert!(
        slots.contains(&("ex:fineTune".to_string(), "fine-tune".to_string())),
        "{slots:?}"
    );
    assert_eq!(prose[0].rules[0].refinements[0].action_name, "fine-tune");
}

/// Goldens for profile readings, with the v0.1 bless rule: written only
/// with `PROSE_BLESS_GOLDEN` set, and only when missing.
#[test]
fn profile_goldens() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let cases = [
        (
            "research",
            "profiles/research-policy.json",
            "research.jsonld",
        ),
        (
            "agreement_odrl22",
            "fixtures/agreement.json",
            "odrl22-excerpt.json",
        ),
    ];
    for (name, policy, profile) in cases {
        let text = std::fs::read_to_string(format!("{dir}/tests/{policy}")).unwrap();
        let w = Wording::new().with_profile(ProfileVocab::parse(&profile_text(profile)).unwrap());
        let got = format!("{:#?}\n", render_with(&text, &w));
        let path = format!("{dir}/tests/golden_profile/{name}.txt");
        if std::env::var_os("PROSE_BLESS_GOLDEN").is_some() && !std::path::Path::new(&path).exists()
        {
            std::fs::create_dir_all(format!("{dir}/tests/golden_profile")).unwrap();
            std::fs::write(&path, &got).unwrap();
        }
        let want =
            std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing golden {path}"));
        assert_eq!(got, want, "golden mismatch for {name}");
    }
}
