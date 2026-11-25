//! The operator overlay (issue #76, section B): a small annotation-only
//! graph of `prose:` triples about terms declared elsewhere. It merges its
//! hints onto the terms the loaded profiles declare, and never adds or
//! relabels one. Precedence: profile < overlay < shapes; a finding names the
//! source that closed the slot.
mod common;
mod corpus;
use corpus::*;
use prose_core::edit::*;
use prose_core::profile::{HintSource, OperandHints, Overlay, OverlayError, ProfileVocab, Wording};
use prose_core::shapes::ShapesGraph;
use prose_core::validate::*;
use serde_json::{Value, json};

const DX: &str = "https://example.org/dataspace/";
const PROFILE: &str = "https://example.org/dataspace/profile";
const PROSE: &str = "https://ds-labs-org.github.io/ds-prose-rs/profile#";

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/overlay/{name}", tests_dir())).unwrap()
}

fn dataspace_profile() -> ProfileVocab {
    let text = std::fs::read_to_string(format!("{}/shapes/dataspace-profile.jsonld", tests_dir()))
        .unwrap();
    ProfileVocab::parse(&text).unwrap()
}

fn fixture_overlay() -> Overlay {
    Overlay::parse(&fixture("dataspace-overlay.jsonld")).unwrap()
}

fn overlaid() -> Wording {
    Wording::new()
        .with_profile(dataspace_profile())
        .with_overlay(fixture_overlay())
}

fn plain() -> Wording {
    Wording::new().with_profile(dataspace_profile())
}

/// An overlay written as a JSON-LD graph, the prefixes bound.
fn overlay(nodes: Value) -> Overlay {
    let doc = json!({
        "@context": {"prose": PROSE, "odrl": "http://www.w3.org/ns/odrl/2/",
                     "xsd": "http://www.w3.org/2001/XMLSchema#", "dx": DX},
        "@graph": nodes
    });
    Overlay::parse(&doc.to_string()).unwrap()
}

fn shapes(nodes: Value) -> ShapesGraph {
    let doc = json!({
        "@context": {"sh": "http://www.w3.org/ns/shacl#", "odrl": "http://www.w3.org/ns/odrl/2/",
                     "xsd": "http://www.w3.org/2001/XMLSchema#", "dx": DX},
        "@graph": nodes
    });
    ShapesGraph::parse(&doc.to_string()).unwrap()
}

fn hints<'a>(w: &'a Wording, left: &str) -> &'a OperandHints {
    let (t, _) = w.lookup(&format!("{DX}{left}")).unwrap();
    w.operand_hints(t)
}

fn policy(constraints: Value) -> String {
    json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"dx": DX}],
        "@type": "Set", "uid": "urn:policy:p", "profile": PROFILE,
        "permission": [{"target": "urn:asset:1", "action": "dx:train", "constraint": constraints}]
    })
    .to_string()
}

fn run(text: &str, w: &Wording) -> Report {
    validate_json(text, w, &Options::strict())
}

// --- reading -----------------------------------------------------------------

#[test]
fn the_fixture_overlay_is_read_without_warnings() {
    let o = fixture_overlay();
    assert_eq!(o.warnings(), &[] as &[String]);
    assert_eq!(o.terms().count(), 3);
    assert!(!o.is_empty());
}

#[test]
fn a_text_without_prose_triples_is_not_an_overlay() {
    let e = Overlay::parse(
        r#"{"@context": {"ex": "https://example.org/"}, "@id": "ex:a", "ex:b": "c"}"#,
    )
    .unwrap_err();
    assert!(matches!(e, OverlayError::NotOverlay(_)), "{e:?}");
    assert!(e.to_string().contains("prose"), "{e}");
    assert!(matches!(
        Overlay::parse("<html></html>"),
        Err(OverlayError::LooksLikeHtml)
    ));
    assert!(matches!(
        Overlay::parse("{ not json"),
        Err(OverlayError::Json(_))
    ));
}

// --- merging -----------------------------------------------------------------

#[test]
fn the_overlay_merges_its_hints_onto_the_declared_term() {
    let w = overlaid();
    let h = hints(&w, "purpose");
    assert!(h.closed);
    assert_eq!(
        h.values,
        [format!("{DX}research"), format!("{DX}marketing")]
    );
    // An overlay lists exactly the values it means, like sh:in: no descendants.
    let tree: Vec<&str> = h.value_tree.iter().map(|(v, _)| v.as_str()).collect();
    assert_eq!(tree, [format!("{DX}research"), format!("{DX}marketing")]);
    assert_eq!(
        h.operators,
        [
            "http://www.w3.org/ns/odrl/2/eq",
            "http://www.w3.org/ns/odrl/2/isAnyOf"
        ]
    );
    assert_eq!(h.sources.values, HintSource::Overlay);
    assert_eq!(h.sources.operators, HintSource::Overlay);
    let c = hints(&w, "count");
    assert_eq!(c.units, [format!("{DX}people")]);
    assert_eq!(
        c.datatype.as_deref(),
        Some("http://www.w3.org/2001/XMLSchema#integer")
    );
    assert_eq!(c.sources.datatype, HintSource::Overlay);
}

#[test]
fn an_overlay_is_not_a_second_declaration() {
    // The overlay names IRIs the profile declares, and that is no conflict.
    assert!(overlaid().conflicts().is_empty());
    // A term the overlay does not mention is as the profile has it.
    assert_eq!(hints(&overlaid(), "price"), hints(&plain(), "price"));
}

#[test]
fn an_overlay_never_adds_or_relabels_a_term() {
    let o = overlay(json!([
        {"@id": "dx:purpose", "http://www.w3.org/2000/01/rdf-schema#label": "Renamed",
         "@type": ["http://www.w3.org/ns/odrl/2/LeftOperand"], "prose:closed": true,
         "prose:rightOperand": {"@id": "dx:research"}},
        {"@id": "dx:ghost", "prose:closed": true, "prose:rightOperand": {"@id": "dx:research"}}
    ]));
    let w = plain().with_overlay(o.clone());
    let (t, _) = w.lookup(&format!("{DX}purpose")).unwrap();
    assert_eq!(t.display_label(&[]), "Purpose");
    assert!(w.lookup(&format!("{DX}ghost")).is_none());
    let warnings = w.overlay_warnings(&o);
    assert!(
        warnings.iter().any(|m| m.contains("ghost")),
        "an undeclared term is ignored with a warning: {warnings:?}"
    );
    assert!(
        !warnings
            .iter()
            .any(|m| m.contains("purpose") && m.contains("ignored term")),
        "{warnings:?}"
    );
    assert!(
        o.warnings()
            .iter()
            .any(|m| m.contains("label") || m.contains("type")),
        "a label or a type in an overlay is ignored with a warning: {:?}",
        o.warnings()
    );
}

#[test]
fn only_left_operands_take_hints() {
    let o = overlay(json!([
        {"@id": "dx:research", "prose:closed": true, "prose:rightOperand": {"@id": "dx:marketing"}}
    ]));
    let w = plain().with_overlay(o.clone());
    assert!(
        w.overlay_warnings(&o)
            .iter()
            .any(|m| m.contains("research")),
        "{:?}",
        w.overlay_warnings(&o)
    );
}

#[test]
fn the_order_of_profile_and_overlay_does_not_matter() {
    let a = Wording::new()
        .with_overlay(fixture_overlay())
        .with_profile(dataspace_profile());
    assert_eq!(hints(&a, "purpose"), hints(&overlaid(), "purpose"));
    assert!(hints(&a, "purpose").closed);
    assert_eq!(a, overlaid());
}

#[test]
fn overlays_are_part_of_the_wording() {
    assert_ne!(overlaid(), plain());
    assert_eq!(overlaid().overlays().count(), 1);
    assert_eq!(plain().overlays().count(), 0);
}

#[test]
fn without_prose_closed_the_overlay_only_suggests() {
    let o = overlay(json!([
        {"@id": "dx:purpose", "prose:rightOperand": {"@id": "dx:research"}}
    ]));
    let w = plain().with_overlay(o);
    let h = hints(&w, "purpose");
    assert!(!h.closed);
    assert_eq!(h.values, [format!("{DX}research")]);
    assert_eq!(h.sources.values, HintSource::Overlay);
    // Suggestions are not errors, even in strict mode.
    let text = policy(json!([
        {"leftOperand": "dx:purpose", "operator": "eq", "rightOperand": {"@id": "dx:marketing"}}
    ]));
    let r = run(&text, &w);
    assert!(
        r.findings.iter().all(|f| f.severity != Severity::Error),
        "{:?}",
        r.findings
    );
}

#[test]
fn a_later_overlay_wins_per_facet() {
    let first = overlay(json!([
        {"@id": "dx:purpose", "prose:closed": true,
         "prose:rightOperand": {"@id": "dx:research"}, "prose:operator": {"@id": "odrl:eq"}}
    ]));
    let second = overlay(json!([
        {"@id": "dx:purpose", "prose:closed": true, "prose:rightOperand": {"@id": "dx:marketing"}}
    ]));
    let w = plain().with_overlay(first).with_overlay(second);
    let h = hints(&w, "purpose");
    assert_eq!(h.values, [format!("{DX}marketing")]);
    assert_eq!(h.operators, ["http://www.w3.org/ns/odrl/2/eq"]);
}

// --- precedence --------------------------------------------------------------

#[test]
fn shapes_win_over_the_overlay_per_facet_and_the_overlay_over_the_profile() {
    // The profile closes nothing here; the overlay gives operators and
    // values; the shapes give only values.
    let o = overlay(json!([
        {"@id": "dx:purpose", "prose:closed": true,
         "prose:operator": {"@id": "odrl:eq"},
         "prose:rightOperand": [{"@id": "dx:research"}, {"@id": "dx:marketing"}]}
    ]));
    let s = shapes(json!([{
        "@id": "dx:S",
        "sh:property": [
            {"sh:path": {"@id": "odrl:leftOperand"}, "sh:hasValue": {"@id": "dx:purpose"}},
            {"sh:path": {"@id": "odrl:rightOperand"}, "sh:in": {"@list": [{"@id": "dx:marketing"}]}}
        ]
    }]));
    let w = plain().with_overlay(o).with_shapes(s);
    let h = hints(&w, "purpose");
    assert_eq!(h.values, [format!("{DX}marketing")]);
    assert_eq!(h.sources.values, HintSource::Shapes);
    // The overlay's closed operators stay, and are still the overlay's.
    assert_eq!(h.operators, ["http://www.w3.org/ns/odrl/2/eq"]);
    assert_eq!(h.sources.operators, HintSource::Overlay);
    assert!(h.closed);
}

#[test]
fn the_overlay_replaces_a_facet_the_profile_gave() {
    // research-closed closes ex:dataSubjectCount (operators gteq/gt, integer).
    let profile = ProfileVocab::parse(&profile_text("research-closed.jsonld")).unwrap();
    let ex = "https://example.org/odrl/profile/research/";
    let o = Overlay::parse(
        &json!({
            "@context": {"prose": PROSE, "odrl": "http://www.w3.org/ns/odrl/2/", "ex": ex},
            "@id": "ex:dataSubjectCount", "prose:operator": {"@id": "odrl:gteq"}
        })
        .to_string(),
    )
    .unwrap();
    let w = Wording::new().with_profile(profile).with_overlay(o);
    let (t, _) = w.lookup(&format!("{ex}dataSubjectCount")).unwrap();
    let h = w.operand_hints(t);
    assert_eq!(h.operators, ["http://www.w3.org/ns/odrl/2/gteq"]);
    assert_eq!(h.sources.operators, HintSource::Overlay);
    // Still closed by the profile, and its datatype still the profile's.
    assert!(h.closed);
    assert_eq!(
        h.datatype.as_deref(),
        Some("http://www.w3.org/2001/XMLSchema#integer")
    );
    assert_eq!(h.sources.datatype, HintSource::Profile);
}

// --- A and B -----------------------------------------------------------------

/// `h` with every facet's provenance reset, for comparing what two sources
/// say rather than who said it.
fn ignoring_provenance(h: &OperandHints) -> OperandHints {
    let mut h = h.clone();
    h.sources = Default::default();
    h
}

#[test]
fn a_shape_and_its_overlay_twin_give_equal_operand_hints() {
    let s = shapes(json!([
        {"@id": "dx:P", "sh:property": [
            {"sh:path": {"@id": "odrl:leftOperand"}, "sh:hasValue": {"@id": "dx:purpose"}},
            {"sh:path": {"@id": "odrl:operator"},
             "sh:in": {"@list": [{"@id": "odrl:eq"}, {"@id": "odrl:isAnyOf"}]}},
            {"sh:path": {"@id": "odrl:rightOperand"},
             "sh:in": {"@list": [{"@id": "dx:research"}, {"@id": "dx:marketing"}]}}
        ]},
        {"@id": "dx:R", "sh:property": [
            {"sh:path": {"@id": "odrl:leftOperand"}, "sh:hasValue": {"@id": "dx:region"}},
            {"sh:path": {"@id": "odrl:rightOperand"},
             "sh:in": {"@list": [{"@id": "dx:eu"}, {"@id": "dx:us"}]}}
        ]},
        {"@id": "dx:C", "sh:property": [
            {"sh:path": {"@id": "odrl:leftOperand"}, "sh:hasValue": {"@id": "dx:count"}},
            {"sh:path": {"@id": "odrl:rightOperand"}, "sh:datatype": {"@id": "xsd:integer"}},
            {"sh:path": {"@id": "odrl:unit"}, "sh:in": {"@list": [{"@id": "dx:people"}]}}
        ]}
    ]));
    let a = plain().with_shapes(s);
    let b = overlaid();
    for left in ["purpose", "region", "count"] {
        let (ha, hb) = (hints(&a, left), hints(&b, left));
        assert_ne!(
            ha, hb,
            "{left}: provenance differs, or the test proves nothing"
        );
        assert_eq!(
            ignoring_provenance(ha),
            ignoring_provenance(hb),
            "{left}: a shape and its overlay twin must give equal hints"
        );
    }
    // The two also judge a policy alike; only the named source differs.
    let text = policy(json!([
        {"leftOperand": "dx:purpose", "operator": "eq", "rightOperand": {"@id": "dx:clinical"}}
    ]));
    let (ra, rb) = (run(&text, &a), run(&text, &b));
    assert_eq!(ra.findings.len(), 1);
    assert_eq!(rb.findings.len(), 1);
    assert_eq!(ra.findings[0].rule, rb.findings[0].rule);
    assert_eq!(
        ra.findings[0].expected.as_ref().unwrap().values,
        rb.findings[0].expected.as_ref().unwrap().values
    );
}

// --- validation --------------------------------------------------------------

#[test]
fn a_value_outside_the_overlay_is_an_error_naming_the_overlay() {
    let text = policy(json!([
        {"leftOperand": "dx:purpose", "operator": "eq", "rightOperand": {"@id": "dx:clinical"}}
    ]));
    let r = run(&text, &overlaid());
    assert_eq!(r.findings.len(), 1, "{:?}", r.findings);
    let f = &r.findings[0];
    assert_eq!(f.rule.to_string(), "prose.value-not-allowed");
    assert_eq!(f.severity, Severity::Error);
    assert!(f.message.contains("the overlay allows"), "{}", f.message);
    let e = f.expected.as_ref().unwrap();
    assert!(e.closed);
    assert_eq!(e.source, Some(HintSource::Overlay));
    assert_eq!(
        r.to_json()["findings"][0]["expected"]["source"],
        json!("overlay")
    );
    // Without the overlay the policy is clean.
    assert!(run(&text, &plain()).findings.is_empty());
}

#[test]
fn an_operator_and_a_unit_outside_the_overlay_name_it() {
    let text = policy(json!([
        {"leftOperand": "dx:purpose", "operator": "neq", "rightOperand": {"@id": "dx:research"}},
        {"leftOperand": "dx:count", "operator": "eq", "rightOperand": 5, "unit": "dx:kilo"}
    ]));
    let r = run(&text, &overlaid());
    let named: Vec<_> = r
        .findings
        .iter()
        .filter(|f| {
            f.expected
                .as_ref()
                .is_some_and(|e| e.source == Some(HintSource::Overlay))
        })
        .collect();
    assert_eq!(named.len(), 2, "{:?}", r.findings);
    assert!(
        named
            .iter()
            .any(|f| f.message.contains("the overlay allows only the operators"))
    );
    assert!(
        named
            .iter()
            .any(|f| f.message.contains("the overlay allows only the units"))
    );
}

// --- the source's name -------------------------------------------------------

#[test]
fn the_source_has_a_name() {
    assert_eq!(HintSource::Overlay.as_str(), "overlay");
    assert_eq!(HintSource::Overlay.noun(), "the overlay");
    assert!(HintSource::Profile < HintSource::Overlay && HintSource::Overlay < HintSource::Shapes);
}

// --- Turtle ------------------------------------------------------------------

#[cfg(feature = "turtle")]
#[test]
fn turtle_overlay_equals_its_jsonld_twin() {
    let ttl = Overlay::parse(&fixture("dataspace-overlay.ttl")).unwrap();
    assert_eq!(ttl, fixture_overlay());
    assert_eq!(
        hints(&plain().with_overlay(ttl), "purpose"),
        hints(&overlaid(), "purpose")
    );
}
