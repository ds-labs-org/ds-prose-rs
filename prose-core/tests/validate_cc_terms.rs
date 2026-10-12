//! The Creative Commons actions of the ODRL Common Vocabulary
//! (`http://creativecommons.org/ns#`), which ODRL 2.2 lists as its own
//! (Vocabulary & Expression 2.2, 3.1 and appendix A).
//!
//! The W3C `ODRL22.json` spells one of them `cc:CommericalUse`; the Turtle
//! `ODRL22.ttl`, the vocabulary text and Creative Commons all spell it
//! `cc:CommercialUse`. The table follows the correct spelling.
mod common;
mod corpus;
use corpus::*;
use prose_core::profile::{OdrlVocab, ProfileVocab, Wording, odrl_term, odrl_terms};
use prose_core::validate::*;
use serde_json::{Value, json};
use std::collections::BTreeSet;

const ODRL: &str = "http://www.w3.org/ns/odrl/2/";
const CC: &str = "http://creativecommons.org/ns#";

fn policy(action: &str) -> String {
    json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Set", "uid": "urn:p",
        "permission": [{"target": "urn:a", "action": action}]
    })
    .to_string()
}

/// The W3C excerpt with `cc:` Action nodes added, as `ODRL22.ttl` has them
/// (`typo` writes the one `ODRL22.json` misspells as it does).
fn odrl_with_cc(typo: bool) -> Wording {
    let mut doc: Value = serde_json::from_str(&profile_text("odrl22-excerpt.json")).unwrap();
    for local in [
        "Attribution",
        if typo {
            "CommericalUse"
        } else {
            "CommercialUse"
        },
    ] {
        doc.as_array_mut().unwrap().push(json!({
            "@id": format!("{CC}{local}"),
            "@type": [format!("{ODRL}Action"),
                      "http://www.w3.org/2004/02/skos/core#Concept"],
            "http://www.w3.org/2000/01/rdf-schema#isDefinedBy": [{"@id": ODRL}]
        }));
    }
    Wording::new().with_profile(ProfileVocab::parse(&doc.to_string()).unwrap())
}

#[test]
fn the_table_spells_commercial_use_correctly() {
    assert!(odrl_term(&format!("{CC}CommercialUse"), OdrlVocab::Common).is_some());
    assert!(odrl_term(&format!("{CC}CommericalUse"), OdrlVocab::Common).is_none());
}

#[test]
fn commercial_use_conforms_without_a_profile() {
    for action in [
        "cc:CommercialUse",
        "http://creativecommons.org/ns#CommercialUse",
    ] {
        let r = validate_json(&policy(action), &Wording::new(), &Options::strict());
        assert!(r.conforms(), "{action}: {:?}", r.findings);
    }
}

#[test]
fn cc_actions_need_no_declaration_when_the_odrl_vocabulary_is_loaded() {
    // The W3C vocabulary loaded as a profile: the cc: actions it lists are
    // ODRL's own, so the policy declares nothing.
    let w = odrl_with_cc(false);
    for action in ["cc:Attribution", "cc:CommercialUse"] {
        for opts in [Options::strict(), Options::advisory()] {
            let r = validate_json(&policy(action), &w, &opts);
            assert!(
                r.conforms() && r.findings.is_empty(),
                "{action}: {:?}",
                r.findings
            );
        }
    }
}

#[test]
fn the_json_vocabulary_misspelling_does_not_flag_the_right_spelling() {
    // ODRL22.json declares cc:CommericalUse and not cc:CommercialUse.
    let w = odrl_with_cc(true);
    for opts in [Options::strict(), Options::advisory()] {
        let r = validate_json(&policy("cc:CommercialUse"), &w, &opts);
        assert!(r.findings.is_empty(), "{:?}", r.findings);
    }
}

/// The IRIs the excerpt defines in the ODRL and Common Vocabulary namespaces.
fn defined(doc: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for node in doc.as_array().unwrap() {
        let id = node["@id"].as_str().unwrap();
        let ontology = node["@type"].as_array().is_some_and(|t| {
            t.iter()
                .any(|t| t.as_str().is_some_and(|t| t.ends_with("#Ontology")))
        });
        if !ontology && id != ODRL && (id.starts_with(ODRL) || id.starts_with(CC)) {
            out.insert(id.to_string());
        }
    }
    out
}

#[test]
fn the_table_covers_the_excerpt() {
    let doc: Value = serde_json::from_str(&profile_text("odrl22-excerpt.json")).unwrap();
    let ids = defined(&doc);
    assert!(ids.len() > 10);
    for id in &ids {
        assert!(odrl_term(id, OdrlVocab::Common).is_some(), "{id}");
    }
    // Every cc: IRI the excerpt names, as a definition or a match.
    let text = doc.to_string();
    for part in text.split(CC).skip(1) {
        let local: String = part.chars().take_while(|c| c.is_alphanumeric()).collect();
        assert!(
            odrl_term(&format!("{CC}{local}"), OdrlVocab::Common).is_some(),
            "{local}"
        );
    }
}

/// With `ODRL22_TTL` set to a copy of
/// <https://www.w3.org/ns/odrl/2/ODRL22.ttl>: every subject of the file is a
/// table term where the table claims its kind (actions, operands, operators,
/// and conflict values; the deprecated party-scope and undefined-term
/// values have no kind in the table), and every table term is a subject of
/// the file.
#[test]
fn the_table_matches_odrl22_ttl() {
    let Ok(path) = std::env::var("ODRL22_TTL") else {
        eprintln!("ODRL22_TTL not set; skipping");
        return;
    };
    let ttl = std::fs::read_to_string(path).unwrap();
    let lines: Vec<&str> = ttl.lines().collect();
    let (mut subjects, mut valued) = (BTreeSet::new(), BTreeSet::new());
    for (i, line) in lines.iter().enumerate() {
        let Some((ns, rest)) = [(":", ODRL), ("odrl:", ODRL), ("cc:", CC)]
            .iter()
            .find_map(|(p, ns)| line.strip_prefix(p).map(|r| (*ns, r)))
        else {
            continue;
        };
        if rest.is_empty() || !rest.chars().all(char::is_alphanumeric) {
            continue;
        }
        let iri = format!("{ns}{rest}");
        let typ = lines[i + 1].trim_start();
        if [
            "a :Action",
            "a :LeftOperand",
            "a :Operator",
            "a :RightOperand",
            "a :ConflictTerm",
        ]
        .iter()
        .any(|t| typ.starts_with(t))
        {
            valued.insert(iri.clone());
        }
        subjects.insert(iri);
    }
    assert!(valued.len() > 100, "{}", valued.len());
    let have: BTreeSet<String> = odrl_terms().iter().map(|t| t.iri.to_string()).collect();
    let missing: Vec<_> = valued.difference(&have).collect();
    let extra: Vec<_> = have.difference(&subjects).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "missing {missing:?}, extra {extra:?}"
    );
}
