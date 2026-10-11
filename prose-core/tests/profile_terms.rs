//! Term semantics: what each subject of a profile is, as ODRL sees it, and
//! which of its texts and links are kept.
use prose_core::profile::{ProfileError, ProfileLimits, ProfileVocab, Term, TermKind};
use serde_json::{Value, json};

const ODRL: &str = "http://www.w3.org/ns/odrl/2/";
const EX: &str = "https://example.org/odrl/profile/research/";
const RESEARCH: &str = "https://example.org/odrl/profile/research";

fn profile_text(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/profiles/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn parse(name: &str) -> ProfileVocab {
    ProfileVocab::parse(&profile_text(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn from(v: Value) -> Result<ProfileVocab, ProfileError> {
    ProfileVocab::from_value(&v, &ProfileLimits::default())
}

fn term<'a>(v: &'a ProfileVocab, iri: &str) -> &'a Term {
    v.term(iri).unwrap_or_else(|| {
        let all: Vec<_> = v.terms().map(|t| &t.iri).collect();
        panic!("no term {iri} in {all:?}")
    })
}

fn ex(local: &str) -> String {
    format!("{EX}{local}")
}

fn odrl(local: &str) -> String {
    format!("{ODRL}{local}")
}

fn en() -> Vec<String> {
    vec!["en".to_string()]
}

#[test]
fn research_kinds_counts() {
    let v = parse("research.jsonld");
    let n = |k| v.terms_of(k).count();
    assert_eq!(n(TermKind::Action), 3);
    assert_eq!(n(TermKind::LeftOperand), 3);
    assert_eq!(n(TermKind::Operator), 1);
    assert_eq!(n(TermKind::RightOperand), 3);
    assert_eq!(n(TermKind::ConflictTerm), 1);
    assert_eq!(n(TermKind::PartyFunction), 1);
    assert_eq!(n(TermKind::PolicyClass), 1);
    assert_eq!(n(TermKind::AssetClass), 1);
    assert_eq!(v.terms().count(), 14);
    assert_eq!(
        term(&v, &ex("dataSteward")).kinds,
        vec![TermKind::PartyFunction]
    );
    assert_eq!(
        term(&v, &ex("ResearchAgreement")).kinds,
        vec![TermKind::PolicyClass]
    );
    assert_eq!(term(&v, &ex("Dataset")).kinds, vec![TermKind::AssetClass]);

    let s = v.summary();
    assert_eq!(s.iri.as_deref(), Some(RESEARCH));
    assert_eq!(s.version.as_deref(), Some("0.1.0"));
    assert_eq!(s.terms, 14);
    assert_eq!(s.count(TermKind::Action), 3);
    assert_eq!(s.count(TermKind::RightOperand), 3);
    assert_eq!(s.count(TermKind::RuleClass), 0);
    assert_eq!(s.warnings, 0);
}

#[test]
fn profile_metadata() {
    let v = parse("research.jsonld");
    assert_eq!(v.title().pick(&en()), Some("Research Data Profile"));
    assert!(
        v.description()
            .pick(&en())
            .is_some_and(|d| d.starts_with("Actions, constraints and purposes"))
    );
    assert_eq!(v.version(), Some("0.1.0"));
    assert!(v.imports().is_empty());

    let v = from(json!({
        "@context": {"odrl": ODRL, "owl": "http://www.w3.org/2002/07/owl#",
                     "prof": "http://www.w3.org/ns/dx/prof/", "dct": "http://purl.org/dc/terms/"},
        "@graph": [
            {"@id": "https://example.org/p", "@type": "prof:Profile",
             "dct:title": "P", "dct:hasVersion": "2",
             "owl:imports": {"@id": "https://example.org/base"},
             "prof:isProfileOf": {"@id": "http://www.w3.org/ns/odrl/2/core"}},
            {"@id": "https://example.org/p/a", "@type": "odrl:Action"}
        ]
    }))
    .unwrap();
    assert_eq!(v.title().pick(&en()), Some("P"));
    assert_eq!(v.version(), Some("2"));
    assert_eq!(
        v.imports(),
        &[
            "https://example.org/base",
            "http://www.w3.org/ns/odrl/2/core"
        ]
    );
}

#[test]
fn skos_concept_alone_is_not_a_term() {
    let v = parse("research.jsonld");
    assert!(v.term(&ex("archive")).is_none());
    assert!(v.term(&ex("actions")).is_none());
    assert!(v.term(RESEARCH).is_none());
}

#[test]
fn profile_iri_from_ontology_or_prof_or_odrl_profile_or_isdefinedby() {
    assert_eq!(parse("research.jsonld").iri(), Some(RESEARCH));
    let typed = |t: &str| {
        from(json!({
            "@context": {"odrl": ODRL},
            "@graph": [
                {"@id": "https://example.org/p", "@type": t},
                {"@id": "https://example.org/a", "@type": "odrl:Action"}
            ]
        }))
        .unwrap()
        .iri()
        .map(String::from)
    };
    let p = Some("https://example.org/p".to_string());
    assert_eq!(typed("http://www.w3.org/ns/dx/prof/Profile"), p);
    assert_eq!(typed("odrl:Profile"), p);
    assert_eq!(typed("http://www.w3.org/2002/07/owl#Ontology"), p);

    let defined_by = from(json!({
        "@context": {"odrl": ODRL, "rdfs": "http://www.w3.org/2000/01/rdf-schema#"},
        "@graph": [
            {"@id": "https://example.org/a", "@type": "odrl:Action", "rdfs:isDefinedBy": {"@id": "https://example.org/other"}},
            {"@id": "https://example.org/b", "@type": "odrl:Action", "rdfs:isDefinedBy": {"@id": "https://example.org/p"}},
            {"@id": "https://example.org/c", "@type": "odrl:Action", "rdfs:isDefinedBy": {"@id": "https://example.org/p"}}
        ]
    }))
    .unwrap();
    assert_eq!(defined_by.iri(), Some("https://example.org/p"));

    let none = from(
        json!({"@id": "https://example.org/a", "@type": "http://www.w3.org/ns/odrl/2/Action"}),
    )
    .unwrap();
    assert_eq!(none.iri(), None);
}

#[test]
fn label_chain_rdfs_preflabel_dct_title_humanised() {
    let v = parse("research.jsonld");
    assert_eq!(term(&v, &ex("train")).display_label(&en()), "Train");
    assert_eq!(
        term(&v, &ex("researchPurpose")).display_label(&en()),
        "Research purpose"
    );
    let v = from(json!({
        "@context": {"odrl": ODRL, "dct": "http://purl.org/dc/terms/",
                     "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
                     "skos": "http://www.w3.org/2004/02/skos/core#"},
        "@graph": [
            {"@id": "https://example.org/t", "@type": "odrl:Action", "dct:title": "Titled"},
            {"@id": "https://example.org/all", "@type": "odrl:Action",
             "dct:title": "C", "skos:prefLabel": "B", "rdfs:label": "A"},
            {"@id": "https://example.org/dataSet", "@type": "odrl:Action"}
        ]
    }))
    .unwrap();
    assert_eq!(
        term(&v, "https://example.org/t").display_label(&en()),
        "Titled"
    );
    assert_eq!(
        term(&v, "https://example.org/all").display_label(&en()),
        "A"
    );
    let bare = term(&v, "https://example.org/dataSet");
    assert!(bare.label.is_empty());
    assert_eq!(bare.display_label(&en()), "data set");
}

#[test]
fn definition_chain_skos_comment_description() {
    let v = parse("research.jsonld");
    assert_eq!(
        term(&v, &ex("fineTune")).definition.pick(&en()),
        Some("To adapt an already trained model using the Asset.")
    );
    let v = from(json!({
        "@context": {"odrl": ODRL, "dct": "http://purl.org/dc/terms/",
                     "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
                     "skos": "http://www.w3.org/2004/02/skos/core#"},
        "@graph": [
            {"@id": "https://example.org/d", "@type": "odrl:Action", "dct:description": "Described"},
            {"@id": "https://example.org/all", "@type": "odrl:Action",
             "dct:description": "C", "rdfs:comment": "B", "skos:definition": "A"}
        ]
    }))
    .unwrap();
    assert_eq!(
        term(&v, "https://example.org/d").definition.pick(&en()),
        Some("Described")
    );
    assert_eq!(
        term(&v, "https://example.org/all").definition.pick(&en()),
        Some("A")
    );
}

#[test]
fn untagged_comment_used() {
    let v = parse("research.jsonld");
    assert_eq!(
        term(&v, &ex("researchPurpose")).definition.pick(&en()),
        Some("The research purpose the action of the Rule is exercised for.")
    );
}

#[test]
fn de_label_with_de_preference() {
    let v = parse("research.jsonld");
    let de = vec!["de-CH".to_string()];
    assert_eq!(term(&v, &ex("train")).display_label(&de), "Trainieren");
    assert_eq!(term(&v, &ex("fineTune")).display_label(&de), "Fine-tune");
    assert_eq!(v.title().pick(&de), Some("Forschungsdatenprofil"));
}

#[test]
fn html_note_is_plain_text() {
    let v = parse("research.jsonld");
    assert_eq!(
        term(&v, &ex("dataSubjectCount")).note.pick(&en()),
        Some("Only the gteq and gt operators SHOULD be used. Small counts risk re-identification.")
    );
    // ODRL22's own notes carry HTML too, untagged.
    let v = parse("odrl22-excerpt.json");
    let note = term(&v, &odrl("elapsedTime")).note.pick(&en()).unwrap();
    assert!(
        note.contains("See also Metered Time. Example: elpasedTime eq P60M"),
        "{note}"
    );
    // "Non-Normative" is a marker, not a note; skos:note is read first.
    assert_eq!(
        term(&v, &odrl("purpose")).note.pick(&en()),
        Some("Example: Educational use.")
    );
}

#[test]
fn scope_note_and_example_fall_back() {
    let v = from(json!({
        "@context": {"odrl": ODRL, "skos": "http://www.w3.org/2004/02/skos/core#"},
        "@graph": [
            {"@id": "https://example.org/a", "@type": "odrl:Action",
             "skos:scopeNote": ["Non-Normative", "Scoped."], "skos:example": "Ex."},
            {"@id": "https://example.org/b", "@type": "odrl:Action",
             "skos:scopeNote": "Non-Normative", "skos:example": "Ex."},
            {"@id": "https://example.org/c", "@type": "odrl:Action", "skos:scopeNote": "Non-Normative"}
        ]
    }))
    .unwrap();
    assert_eq!(
        term(&v, "https://example.org/a").note.pick(&en()),
        Some("Scoped.")
    );
    assert_eq!(
        term(&v, "https://example.org/b").note.pick(&en()),
        Some("Ex.")
    );
    assert!(term(&v, "https://example.org/c").note.is_empty());
}

#[test]
fn includedin_and_broader_expanded() {
    let v = parse("research.jsonld");
    assert_eq!(term(&v, &ex("fineTune")).included_in, vec![ex("train")]);
    assert_eq!(term(&v, &ex("train")).included_in, vec![odrl("use")]);
    assert_eq!(term(&v, &ex("clinical")).broader, vec![ex("academic")]);
    assert_eq!(
        term(&v, &ex("researchPurpose")).broader,
        vec![odrl("purpose")]
    );
    assert_eq!(
        term(&v, &ex("ResearchAgreement")).broader,
        vec![odrl("Agreement")]
    );
    assert_eq!(term(&v, &ex("dataSteward")).broader, vec![odrl("function")]);
    assert!(term(&v, &ex("train")).broader.is_empty());
}

#[test]
fn implies_is_not_inclusion() {
    let v = parse("research.jsonld");
    let t = term(&v, &ex("fineTune"));
    assert_eq!(t.implies, vec![odrl("read")]);
    assert!(!t.included_in.contains(&odrl("read")));
    assert!(!t.broader.contains(&odrl("read")));
}

#[test]
fn deprecated_and_exact_match() {
    let v = parse("odrl22-excerpt.json");
    let c = term(&v, &odrl("commercialize"));
    assert!(c.deprecated);
    assert_eq!(
        c.exact_match,
        vec!["http://creativecommons.org/ns#CommercialUse"]
    );
    assert_eq!(
        term(&v, &odrl("systemDevice")).exact_match,
        vec![odrl("system"), odrl("device")]
    );
    assert!(!term(&v, &odrl("use")).deprecated);
    assert_eq!(term(&v, &odrl("distribute")).included_in, vec![odrl("use")]);
    assert_eq!(
        term(&v, &odrl("spatialCoordinates")).broader,
        vec![odrl("spatial")]
    );
    // Agreement is a subclass of Policy in the same document.
    assert_eq!(
        term(&v, &odrl("Agreement")).kinds,
        vec![TermKind::PolicyClass]
    );
}

#[test]
fn subclass_kinds_are_transitive_within_the_document() {
    let v = from(json!({
        "@context": {"odrl": ODRL, "rdfs": "http://www.w3.org/2000/01/rdf-schema#"},
        "@graph": [
            {"@id": "https://example.org/Special", "rdfs:subClassOf": {"@id": "https://example.org/Licence"}},
            {"@id": "https://example.org/Licence", "rdfs:subClassOf": {"@id": "odrl:Offer"}},
            {"@id": "https://example.org/Grant", "rdfs:subClassOf": {"@id": "odrl:Permission"}},
            {"@id": "https://example.org/Team", "rdfs:subClassOf": {"@id": "odrl:Party"}},
            {"@id": "https://example.org/rel", "rdfs:subPropertyOf": {"@id": "odrl:relation"}},
            {"@id": "https://example.org/op", "rdfs:subPropertyOf": {"@id": "odrl:operand"}}
        ]
    }))
    .unwrap();
    let k = |iri: &str| term(&v, iri).kinds.clone();
    assert_eq!(
        k("https://example.org/Special"),
        vec![TermKind::PolicyClass]
    );
    assert_eq!(
        k("https://example.org/Licence"),
        vec![TermKind::PolicyClass]
    );
    assert_eq!(k("https://example.org/Grant"), vec![TermKind::RuleClass]);
    assert_eq!(k("https://example.org/Team"), vec![TermKind::PartyClass]);
    assert_eq!(k("https://example.org/rel"), vec![TermKind::AssetRelation]);
    assert_eq!(k("https://example.org/op"), vec![TermKind::LogicalOperand]);
}

#[test]
fn untyped_includedin_subject_is_action_with_warning() {
    let v = from(json!({
        "@context": {"odrl": ODRL, "ex": EX},
        "@graph": [
            {"@id": "ex:x", "odrl:includedIn": {"@id": "odrl:use"}},
            {"@id": "ex:y", "@type": "http://www.w3.org/2004/02/skos/core#Concept",
             "odrl:includedIn": {"@id": "odrl:use"}}
        ]
    }))
    .unwrap();
    assert_eq!(term(&v, &ex("x")).kinds, vec![TermKind::Action]);
    // Typed, but not as an action: not guessed.
    assert!(v.term(&ex("y")).is_none());
    assert!(
        v.warnings()
            .iter()
            .any(|w| w.contains("includedIn") && w.contains(&ex("x"))),
        "{:?}",
        v.warnings()
    );
}

#[test]
fn ex_purpose_and_odrl_purpose_distinct() {
    let v = from(json!({
        "@context": {"odrl": ODRL, "ex": EX, "rdfs": "http://www.w3.org/2000/01/rdf-schema#"},
        "@graph": [
            {"@id": "ex:purpose", "@type": "odrl:LeftOperand", "rdfs:label": "Study purpose"},
            {"@id": "odrl:purpose", "@type": "odrl:LeftOperand", "rdfs:label": "Purpose"}
        ]
    }))
    .unwrap();
    assert_eq!(
        term(&v, &ex("purpose")).display_label(&en()),
        "Study purpose"
    );
    assert_eq!(term(&v, &odrl("purpose")).display_label(&en()), "Purpose");
    assert!(v.term("purpose").is_none());
    assert!(v.term("ex:purpose").is_none());
}
