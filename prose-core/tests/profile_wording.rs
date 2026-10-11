//! `Wording` (the loaded profiles plus language preferences) and `Prefixes`
//! (how a policy's compact terms expand): terms are matched by expanded IRI,
//! with the policy's own prefixes first, then the profiles'.
use prose_core::edit::{EditDoc, read_model};
use prose_core::profile::{Prefixes, ProfileLimits, ProfileVocab, Wording};
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

fn research() -> ProfileVocab {
    ProfileVocab::parse(&profile_text("research.jsonld")).unwrap()
}

fn other() -> ProfileVocab {
    ProfileVocab::from_value(
        &json!({
            "@context": {"odrl": ODRL, "ex": EX, "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
                         "owl": "http://www.w3.org/2002/07/owl#"},
            "@graph": [
                {"@id": "https://example.org/other", "@type": "owl:Ontology"},
                {"@id": "ex:train", "@type": "odrl:Action", "rdfs:label": "Train (other)"},
                {"@id": "ex:distill", "@type": "odrl:Action", "rdfs:label": "Distill"}
            ]
        }),
        &ProfileLimits::default(),
    )
    .unwrap()
}

fn en() -> Vec<String> {
    vec!["en".to_string()]
}

#[test]
fn default_wording_is_empty() {
    assert!(Wording::default().is_empty());
    assert_eq!(Wording::new(), Wording::default());
    assert_eq!(Wording::default().profiles().count(), 0);
    assert!(Wording::default().conflicts().is_empty());
    assert!(Wording::default().lookup(&format!("{EX}train")).is_none());
    let w = Wording::new().with_languages(vec!["de".into()]);
    assert_eq!(w.languages(), &["de".to_string()]);
    assert!(w.is_empty());
}

#[test]
fn first_loaded_wins_conflict_reported() {
    let w = Wording::new()
        .with_profile(research())
        .with_profile(other());
    assert!(!w.is_empty());
    assert_eq!(w.profiles().count(), 2);
    let (term, from) = w.lookup(&format!("{EX}train")).unwrap();
    assert_eq!(term.display_label(&en()), "Train");
    assert_eq!(from.iri(), Some(RESEARCH));
    let (term, from) = w.lookup(&format!("{EX}distill")).unwrap();
    assert_eq!(term.display_label(&en()), "Distill");
    assert_eq!(from.iri(), Some("https://example.org/other"));
    assert_eq!(w.conflicts().len(), 1, "{:?}", w.conflicts());
    let c = &w.conflicts()[0];
    assert!(
        c.contains(&format!("{EX}train"))
            && c.contains(RESEARCH)
            && c.contains("https://example.org/other"),
        "{c}"
    );

    // The other order: the other profile's label wins.
    let w = Wording::new()
        .with_profile(other())
        .with_profile(research());
    let (term, _) = w.lookup(&format!("{EX}train")).unwrap();
    assert_eq!(term.display_label(&en()), "Train (other)");
}

#[test]
fn covers_ignores_trailing_hash_slash() {
    let w = Wording::new().with_profile(research());
    assert!(w.covers(RESEARCH));
    assert!(w.covers(&format!("{RESEARCH}/")));
    assert!(w.covers(&format!("{RESEARCH}#")));
    assert!(!w.covers("https://example.org/other"));
    assert!(!w.covers("https://example.org/odrl/profile/research-2"));
    assert!(!Wording::default().covers(RESEARCH));
}

#[test]
fn odrl_core_always_covered() {
    let w = Wording::default();
    assert!(w.covers("http://www.w3.org/ns/odrl/2/core"));
    assert!(w.covers("http://www.w3.org/ns/odrl/2/"));
    assert!(w.covers("http://www.w3.org/ns/odrl/2"));
    assert!(
        Wording::new()
            .with_profile(research())
            .covers("http://www.w3.org/ns/odrl/2/core")
    );
}

#[test]
fn policy_prefix_before_profile_prefix() {
    let w = Wording::new().with_profile(research());
    let policy = Prefixes::of_document(&json!({
        "@context": {"ex": "https://example.org/policy-ns/"},
        "permission": []
    }));
    assert_eq!(
        w.expand(&policy, "ex:train"),
        "https://example.org/policy-ns/train"
    );
    assert!(w.lookup_written(&policy, "ex:train").is_none());
    // Unbound in the policy: the profile's own prefix is used.
    let none = Prefixes::default();
    assert_eq!(w.expand(&none, "ex:train"), format!("{EX}train"));
    let (term, _) = w.lookup_written(&none, "ex:train").unwrap();
    assert_eq!(term.iri, format!("{EX}train"));
    // Bound nowhere: kept as written.
    assert_eq!(w.expand(&none, "zz:train"), "zz:train");
    // A full IRI is never rewritten.
    assert_eq!(
        w.expand(&policy, &format!("{EX}train")),
        format!("{EX}train")
    );
}

#[test]
fn turtle_profile_prefixes_are_used_too() {
    #[cfg(feature = "turtle")]
    {
        let w = Wording::new()
            .with_profile(ProfileVocab::parse(&profile_text("research.ttl")).unwrap());
        assert_eq!(
            w.expand(&Prefixes::default(), "ex:fineTune"),
            format!("{EX}fineTune")
        );
    }
}

#[test]
fn bare_and_odrl_prefix_map_to_core() {
    let p = Prefixes::default();
    assert_eq!(p.expand("use"), format!("{ODRL}use"));
    assert_eq!(p.expand("odrl:use"), format!("{ODRL}use"));
    assert_eq!(p.expand(&format!("{ODRL}use")), format!("{ODRL}use"));
    assert_eq!(p.expand("urn:uuid:1234"), "urn:uuid:1234");
    assert_eq!(p.expand("ex:train"), "ex:train");

    // An author's binding of odrl elsewhere is respected.
    let rebound =
        Prefixes::of_document(&json!({"@context": {"odrl": "https://example.org/not-odrl/"}}));
    assert_eq!(
        rebound.expand("odrl:use"),
        "https://example.org/not-odrl/use"
    );
    // A term the policy's context defines.
    let defined = Prefixes::of_document(&json!({
        "@context": {"ex": EX, "fineTune": "ex:fineTune", "tune": {"@id": "ex:fineTune"}}
    }));
    assert_eq!(defined.expand("fineTune"), format!("{EX}fineTune"));
    assert_eq!(defined.expand("tune"), format!("{EX}fineTune"));
    assert_eq!(defined.expand("use"), format!("{ODRL}use"));

    let w = Wording::default();
    assert_eq!(w.expand(&Prefixes::default(), "use"), format!("{ODRL}use"));
}

#[test]
fn prefixes_of_document_single_graph_array_context() {
    let policy: Value = serde_json::from_str(&profile_text("research-policy.json")).unwrap();
    let p = Prefixes::of_document(&policy);
    assert_eq!(p.expand("ex:fineTune"), format!("{EX}fineTune"));
    // The W3C ODRL context binds the usual prefixes.
    assert_eq!(p.expand("dct:title"), "http://purl.org/dc/terms/title");

    let graph = Prefixes::of_document(&json!({
        "@context": {"ex": EX},
        "@graph": [{"@context": {"zz": "https://example.org/zz/"}, "permission": []}]
    }));
    assert_eq!(graph.expand("ex:a"), format!("{EX}a"));
    assert_eq!(graph.expand("zz:a"), "https://example.org/zz/a");

    let array = Prefixes::of_document(&json!([
        {"@context": {"ex": "https://example.org/first/"}, "permission": []},
        {"@context": {"ex": "https://example.org/second/", "zz": "https://example.org/zz/"}, "permission": []}
    ]));
    assert_eq!(array.expand("ex:a"), "https://example.org/first/a");
    assert_eq!(array.expand("zz:a"), "https://example.org/zz/a");

    // Within one @context array the later entry wins, as in JSON-LD.
    let later = Prefixes::of_document(&json!({
        "@context": [{"ex": "https://example.org/first/"}, {"ex": "https://example.org/second/"}]
    }));
    assert_eq!(later.expand("ex:a"), "https://example.org/second/a");

    assert_eq!(
        Prefixes::of_document(&json!("not a document")),
        Prefixes::default()
    );
}

#[test]
fn edit_prefixes_reads_origins() {
    let doc = read_model(&profile_text("research-policy.json")).unwrap();
    let p = prose_core::edit::prefixes(&doc);
    assert_eq!(p.expand("ex:fineTune"), format!("{EX}fineTune"));

    let graph = json!({
        "@context": {"ex": EX},
        "@graph": [{"@type": "Set", "permission": [{"action": "ex:train"}]}]
    });
    let doc = read_model(&graph.to_string()).unwrap();
    assert_eq!(
        prose_core::edit::prefixes(&doc).expand("ex:a"),
        format!("{EX}a")
    );

    let built = EditDoc::new(vec![]);
    assert_eq!(prose_core::edit::prefixes(&built), Prefixes::default());
}
