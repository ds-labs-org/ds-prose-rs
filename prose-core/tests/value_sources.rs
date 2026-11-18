//! Runtime value sources (issue #76, section C): values the host takes from
//! the dataspace's registry or catalog (participant DIDs, credential types,
//! asset ids, purposes) and passes per left operand, or per party role. The
//! editor never fetches. Precedence: profile < overlay < shapes < host.
//! A value removed from the source later is kept in the document and
//! flagged, never dropped.
mod common;
mod corpus;
use corpus::*;
use prose_core::edit::{EntityRole, Severity, read_model, write_jsonld};
use prose_core::profile::{
    HintSource, OperandHints, Overlay, ProfileVocab, SourceValue, ValueSource, ValueSources,
    Wording,
};
use prose_core::shapes::ShapesGraph;
use prose_core::validate::*;
use serde_json::{Value, json};

const DX: &str = "https://example.org/dataspace/";
const PROFILE: &str = "https://example.org/dataspace/profile";
const PROSE: &str = "https://ds-labs-org.github.io/ds-prose-rs/profile#";

fn dataspace_profile() -> ProfileVocab {
    let text = std::fs::read_to_string(format!("{}/shapes/dataspace-profile.jsonld", tests_dir()))
        .unwrap();
    ProfileVocab::parse(&text).unwrap()
}

fn plain() -> Wording {
    Wording::new().with_profile(dataspace_profile())
}

fn member(op: &str) -> String {
    format!("{DX}{op}")
}

fn dids(closed: bool) -> ValueSource {
    ValueSource::new(
        vec![
            SourceValue::new("did:web:alpha.example")
                .with_label("Alpha Corp")
                .with_group("Members"),
            SourceValue::new("did:web:beta.example")
                .with_label("Beta Ltd")
                .with_group("Members"),
            SourceValue::new("did:web:gamma.example")
                .with_label("Gamma")
                .with_group("Observers"),
        ],
        closed,
    )
}

fn with_source(key: &str, source: ValueSource) -> Wording {
    let mut s = ValueSources::new();
    s.set_operand(&member(key), source);
    plain().with_value_sources(s)
}

fn hints<'a>(w: &'a Wording, left: &str) -> &'a OperandHints {
    let (t, _) = w.lookup(&member(left)).unwrap();
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

fn eq_did(did: &str) -> String {
    policy(json!([
        {"leftOperand": "dx:participantType", "operator": "eq", "rightOperand": {"@id": did}}
    ]))
}

// --- the hints ---------------------------------------------------------------

#[test]
fn a_host_source_fills_the_operand_hints() {
    let w = with_source("participantType", dids(true));
    let h = hints(&w, "participantType");
    assert!(h.closed);
    assert_eq!(h.sources.values, HintSource::Host);
    assert_eq!(
        h.values,
        [
            "did:web:alpha.example",
            "did:web:beta.example",
            "did:web:gamma.example"
        ]
    );
    let tree: Vec<(&str, usize)> = h.value_tree.iter().map(|(v, d)| (v.as_str(), *d)).collect();
    assert_eq!(
        tree,
        [
            ("did:web:alpha.example", 0),
            ("did:web:beta.example", 0),
            ("did:web:gamma.example", 0)
        ]
    );
    // The labels and groups the host gave travel with the values.
    assert_eq!(h.source_values.len(), 3);
    assert_eq!(h.source_values[0].value(), "did:web:alpha.example");
    assert_eq!(h.source_values[0].label(), Some("Alpha Corp"));
    assert_eq!(h.source_values[2].group(), Some("Observers"));
}

#[test]
fn without_a_source_the_hints_are_the_profile_s() {
    let w = plain();
    assert!(!hints(&w, "participantType").closed);
    assert!(hints(&w, "participantType").source_values.is_empty());
}

#[test]
fn a_source_replaces_the_whole_set_and_an_empty_set_clears_it() {
    let w = with_source("participantType", dids(true));
    let cleared = w.with_value_sources(ValueSources::new());
    assert!(!hints(&cleared, "participantType").closed);
    assert_eq!(
        hints(&cleared, "participantType"),
        hints(&plain(), "participantType")
    );
}

#[test]
fn an_empty_source_is_no_source() {
    // A registry fetch that came back empty must not flag every value.
    let w = with_source("participantType", ValueSource::new(vec![], true));
    assert_eq!(
        hints(&w, "participantType"),
        hints(&plain(), "participantType")
    );
    let r = run(&eq_did("did:web:alpha.example"), &w);
    assert!(r.findings.is_empty(), "{:?}", r.findings);
}

#[test]
fn the_order_of_profile_and_sources_does_not_matter() {
    let mut s = ValueSources::new();
    s.set_operand(&member("participantType"), dids(true));
    let a = Wording::new()
        .with_value_sources(s.clone())
        .with_profile(dataspace_profile());
    let b = plain().with_value_sources(s);
    assert_eq!(hints(&a, "participantType"), hints(&b, "participantType"));
}

#[test]
fn a_source_for_a_term_no_profile_declares_as_a_left_operand_warns() {
    let mut s = ValueSources::new();
    s.set_operand(&member("participantType"), dids(true));
    s.set_operand(&member("nope"), dids(true));
    s.set_operand(&member("research"), dids(true));
    let w = plain().with_value_sources(s);
    let warnings = w.value_source_warnings();
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(
        warnings
            .iter()
            .any(|m| m.contains("nope") && m.contains("no loaded profile"))
    );
    assert!(
        warnings
            .iter()
            .any(|m| m.contains("research") && m.contains("not a left operand"))
    );
}

// --- closed ------------------------------------------------------------------

#[test]
fn a_value_the_closed_source_lists_is_clean() {
    let w = with_source("participantType", dids(true));
    let r = run(&eq_did("did:web:alpha.example"), &w);
    assert!(r.findings.is_empty(), "{:?}", r.findings);
}

#[test]
fn a_value_the_closed_source_does_not_list_is_an_error_naming_the_host() {
    let w = with_source("participantType", dids(true));
    let r = run(&eq_did("did:web:gone.example"), &w);
    assert_eq!(r.findings.len(), 1, "{:?}", r.findings);
    let f = &r.findings[0];
    assert_eq!(f.rule.to_string(), "prose.value-not-allowed");
    assert_eq!(f.severity, Severity::Error);
    assert!(f.message.contains("the host allows"), "{}", f.message);
    let e = f.expected.as_ref().unwrap();
    assert!(e.closed);
    assert_eq!(e.source, Some(HintSource::Host));
    assert_eq!(e.values.len(), 3);
    assert_eq!(
        r.to_json()["findings"][0]["expected"]["source"],
        json!("host")
    );
}

#[test]
fn a_plain_string_naming_a_listed_value_is_accepted_as_it_is() {
    // Hosts list DIDs and asset ids as text; a policy may write either form.
    let w = with_source("participantType", dids(true));
    let text = policy(json!([
        {"leftOperand": "dx:participantType", "operator": "eq", "rightOperand": "did:web:beta.example"}
    ]));
    let r = run(&text, &w);
    assert!(r.findings.is_empty(), "{:?}", r.findings);
    let text = policy(json!([
        {"leftOperand": "dx:participantType", "operator": "eq", "rightOperand": "did:web:zeta.example"}
    ]));
    let r = run(&text, &w);
    assert_eq!(r.findings.len(), 1, "{:?}", r.findings);
    assert_eq!(r.findings[0].rule.to_string(), "prose.value-not-allowed");
}

#[test]
fn a_value_removed_from_the_source_is_kept_in_the_document_and_flagged() {
    let text = eq_did("did:web:gamma.example");
    // Listed first: clean.
    let before = with_source("participantType", dids(true));
    assert!(run(&text, &before).findings.is_empty());
    // The registry drops it.
    let after = with_source(
        "participantType",
        ValueSource::new(
            vec![SourceValue::new("did:web:alpha.example").with_label("Alpha Corp")],
            true,
        ),
    );
    let r = run(&text, &after);
    assert_eq!(r.findings.len(), 1, "{:?}", r.findings);
    assert_eq!(r.findings[0].rule.to_string(), "prose.value-not-allowed");
    assert_eq!(
        r.findings[0].expected.as_ref().unwrap().source,
        Some(HintSource::Host)
    );
    // The document is read and written back with the value still in it.
    let doc = read_model(&text).unwrap();
    let written = write_jsonld(&doc).to_string();
    assert!(written.contains("did:web:gamma.example"), "{written}");
    // And nothing the validator does with the new list rewrites it.
    let again = Validator::new(std::sync::Arc::new(after), Options::strict());
    let _ = again.validate(&doc);
    assert!(
        write_jsonld(&doc)
            .to_string()
            .contains("did:web:gamma.example")
    );
}

// --- suggest-only ------------------------------------------------------------

#[test]
fn a_suggest_only_source_flags_no_value_as_an_error() {
    let w = with_source("participantType", dids(false));
    let h = hints(&w, "participantType");
    assert!(!h.closed);
    assert_eq!(h.sources.values, HintSource::Host);
    assert_eq!(h.source_values.len(), 3);
    let did = "did:web:gone.example";
    let r = run(&eq_did(did), &w);
    assert!(
        r.findings.iter().all(|f| f.severity != Severity::Error),
        "{:?}",
        r.findings
    );
    let text = policy(json!([
        {"leftOperand": "dx:participantType", "operator": "eq", "rightOperand": did}
    ]));
    let r = run(&text, &w);
    assert!(r.findings.is_empty(), "{:?}", r.findings);
}

// --- layering ----------------------------------------------------------------

fn shapes_closing_purpose() -> ShapesGraph {
    let doc = json!({
        "@context": {"sh": "http://www.w3.org/ns/shacl#", "odrl": "http://www.w3.org/ns/odrl/2/",
                     "dx": DX},
        "@graph": [{
            "@id": "dx:PurposeShape", "@type": "sh:NodeShape",
            "sh:property": [
                {"sh:path": {"@id": "odrl:leftOperand"}, "sh:hasValue": {"@id": "dx:purpose"}},
                {"sh:path": {"@id": "odrl:rightOperand"},
                 "sh:in": {"@list": [{"@id": "dx:research"}, {"@id": "dx:marketing"}]}},
                {"sh:path": {"@id": "odrl:operator"},
                 "sh:in": {"@list": [{"@id": "odrl:eq"}]}}
            ]
        }]
    });
    ShapesGraph::parse(&doc.to_string()).unwrap()
}

fn overlay_closing_purpose() -> Overlay {
    let doc = json!({
        "@context": {"prose": PROSE, "odrl": "http://www.w3.org/ns/odrl/2/", "dx": DX},
        "@graph": [{"@id": "dx:purpose", "prose:closed": true,
                    "prose:rightOperand": [{"@id": "dx:research"}]}]
    });
    Overlay::parse(&doc.to_string()).unwrap()
}

#[test]
fn the_host_wins_over_the_shapes_for_the_values_it_gives() {
    let mut s = ValueSources::new();
    s.set_operand(
        &member("purpose"),
        ValueSource::new(vec![SourceValue::new(&member("clinical"))], true),
    );
    let w = plain()
        .with_shapes(shapes_closing_purpose())
        .with_value_sources(s);
    let h = hints(&w, "purpose");
    assert_eq!(h.values, [member("clinical")]);
    assert_eq!(h.sources.values, HintSource::Host);
    // A facet the host does not give keeps the shapes' source.
    assert_eq!(h.operators, ["http://www.w3.org/ns/odrl/2/eq"]);
    assert_eq!(h.sources.operators, HintSource::Shapes);
}

#[test]
fn the_host_wins_over_an_overlay() {
    let mut s = ValueSources::new();
    s.set_operand(
        &member("purpose"),
        ValueSource::new(vec![SourceValue::new(&member("marketing"))], true),
    );
    let w = plain()
        .with_overlay(overlay_closing_purpose())
        .with_value_sources(s);
    let h = hints(&w, "purpose");
    assert_eq!(h.values, [member("marketing")]);
    assert_eq!(h.sources.values, HintSource::Host);
}

#[test]
fn a_suggest_only_source_never_loosens_a_closed_operand() {
    let mut s = ValueSources::new();
    s.set_operand(
        &member("purpose"),
        ValueSource::new(vec![SourceValue::new(&member("clinical"))], false),
    );
    let w = plain()
        .with_shapes(shapes_closing_purpose())
        .with_value_sources(s);
    let h = hints(&w, "purpose");
    assert!(h.closed);
    assert_eq!(h.values, [member("research"), member("marketing")]);
    assert_eq!(h.sources.values, HintSource::Shapes);
}

/// An overlay that only suggests an operator, a unit and a datatype for
/// `dx:purpose`.
fn overlay_suggesting_purpose_facets() -> Overlay {
    let doc = json!({
        "@context": {"prose": PROSE, "odrl": "http://www.w3.org/ns/odrl/2/",
                     "xsd": "http://www.w3.org/2001/XMLSchema#", "dx": DX},
        "@graph": [{"@id": "dx:purpose",
                    "prose:operator": [{"@id": "odrl:eq"}],
                    "prose:unit": [{"@id": "dx:kg"}],
                    "prose:datatype": {"@id": "xsd:string"}}]
    });
    Overlay::parse(&doc.to_string()).unwrap()
}

fn closing_purpose_source() -> ValueSources {
    let mut s = ValueSources::new();
    s.set_operand(
        &member("purpose"),
        ValueSource::new(vec![SourceValue::new(&member("research"))], true),
    );
    s
}

#[test]
fn a_closed_source_gives_values_only_and_keeps_the_other_facets_of_an_open_base() {
    let w = plain()
        .with_overlay(overlay_suggesting_purpose_facets())
        .with_value_sources(closing_purpose_source());
    let h = hints(&w, "purpose");
    assert!(h.closed);
    assert_eq!(h.values, [member("research")]);
    assert_eq!(h.sources.values, HintSource::Host);
    // The host says nothing about operators, units or the datatype: the
    // base's stay, with their own source, and only suggest.
    assert_eq!(h.operators, ["http://www.w3.org/ns/odrl/2/eq"]);
    assert_eq!(h.units, [member("kg")]);
    assert_eq!(
        h.datatype.as_deref(),
        Some("http://www.w3.org/2001/XMLSchema#string")
    );
    assert_eq!(h.sources.operators, HintSource::Overlay);
    assert_eq!(h.sources.units, HintSource::Overlay);
    assert_eq!(h.sources.datatype, HintSource::Overlay);
    assert!(h.values_only);
    assert!(!h.binds_other_facets());
}

#[test]
fn the_values_a_closed_source_binds_but_the_suggested_operator_only_suggests() {
    let w = plain()
        .with_overlay(overlay_suggesting_purpose_facets())
        .with_value_sources(closing_purpose_source());
    let text = policy(json!([
        {"leftOperand": "dx:purpose", "operator": "lt", "rightOperand": {"@id": "dx:marketing"}}
    ]));
    let r = run(&text, &w);
    let value = r
        .findings
        .iter()
        .find(|f| f.rule.to_string() == "prose.value-not-allowed")
        .unwrap_or_else(|| panic!("{:?}", r.findings));
    assert_eq!(value.severity, Severity::Error);
    let op = r
        .findings
        .iter()
        .find(|f| f.rule.to_string() == "prose.operator-not-allowed")
        .unwrap_or_else(|| panic!("{:?}", r.findings));
    // A suggestion, not an error: the host closed the values only.
    assert_eq!(op.severity, Severity::Info, "{op:?}");
    assert!(op.message.contains("suggests"), "{}", op.message);
    assert!(!op.expected.as_ref().unwrap().closed);
    assert_eq!(
        op.expected.as_ref().unwrap().source,
        Some(HintSource::Overlay)
    );
}

#[test]
fn a_closed_source_over_a_closed_base_leaves_its_facets_binding() {
    let w = plain()
        .with_overlay(overlay_closing_purpose())
        .with_value_sources(closing_purpose_source());
    let h = hints(&w, "purpose");
    assert!(h.closed && !h.values_only && h.binds_other_facets());
}

// --- the source's name -------------------------------------------------------

#[test]
fn the_source_has_a_name() {
    assert_eq!(HintSource::Host.as_str(), "host");
    assert_eq!(HintSource::Host.noun(), "the host");
    assert!(HintSource::Shapes < HintSource::Host);
}

// --- parties and assets ------------------------------------------------------

fn party_policy(assignee: &str, target: &str) -> String {
    json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"dx": DX}],
        "@type": "Set", "uid": "urn:policy:p", "profile": PROFILE,
        "permission": [{"target": target, "assignee": assignee, "action": "dx:train"}]
    })
    .to_string()
}

fn role_wording(closed: bool) -> Wording {
    let mut s = ValueSources::new();
    s.set_role(EntityRole::Assignee, dids(closed));
    s.set_role(
        EntityRole::Target,
        ValueSource::new(
            vec![SourceValue::new("urn:asset:1").with_label("Asset one")],
            closed,
        ),
    );
    plain().with_value_sources(s)
}

#[test]
fn a_party_the_closed_source_lists_is_clean_and_another_is_flagged() {
    let w = role_wording(true);
    let r = run(&party_policy("did:web:alpha.example", "urn:asset:1"), &w);
    assert!(r.findings.is_empty(), "{:?}", r.findings);
    let r = run(&party_policy("did:web:gone.example", "urn:asset:2"), &w);
    assert_eq!(r.findings.len(), 2, "{:?}", r.findings);
    for f in &r.findings {
        assert_eq!(f.rule.to_string(), "prose.value-not-allowed");
        assert_eq!(f.severity, Severity::Error);
        assert_eq!(f.expected.as_ref().unwrap().source, Some(HintSource::Host));
        assert!(f.message.contains("the host allows"), "{}", f.message);
    }
    assert!(
        r.findings[0].message.contains("assignee"),
        "{}",
        r.findings[0].message
    );
    assert!(r.findings.iter().any(|f| f.message.contains("target")));
}

#[test]
fn a_party_value_removed_from_the_source_stays_in_the_document() {
    let w = role_wording(true);
    let text = party_policy("did:web:gone.example", "urn:asset:1");
    let r = run(&text, &w);
    assert_eq!(r.findings.len(), 1, "{:?}", r.findings);
    let written = write_jsonld(&read_model(&text).unwrap()).to_string();
    assert!(written.contains("did:web:gone.example"), "{written}");
}

#[test]
fn a_suggest_only_party_source_flags_nothing() {
    let w = role_wording(false);
    let r = run(&party_policy("did:web:gone.example", "urn:asset:2"), &w);
    assert!(r.findings.is_empty(), "{:?}", r.findings);
}

#[test]
fn the_role_sources_are_readable() {
    let w = role_wording(true);
    let src = w.role_source(EntityRole::Assignee).unwrap();
    assert!(src.is_closed());
    assert_eq!(src.values().len(), 3);
    assert!(w.role_source(EntityRole::Assigner).is_none());
}

// --- review: call order, duplicates, swaps ----------------------------------

#[test]
fn sources_set_first_still_sit_on_top_of_a_later_shapes_and_overlay() {
    let mut s = ValueSources::new();
    s.set_operand(
        &member("purpose"),
        ValueSource::new(vec![SourceValue::new(&member("clinical"))], true),
    );
    let w = Wording::new()
        .with_value_sources(s)
        .with_profile(dataspace_profile())
        .with_overlay(overlay_closing_purpose())
        .with_shapes(shapes_closing_purpose());
    let h = hints(&w, "purpose");
    assert_eq!(h.values, [member("clinical")]);
    assert_eq!(h.sources.values, HintSource::Host);
    assert!(h.closed);
}

#[test]
fn a_value_listed_twice_is_one_option_and_swapping_sources_replaces_them() {
    let src = ValueSource::new(
        vec![
            SourceValue::new("did:web:a").with_label("First"),
            SourceValue::new("did:web:a").with_label("Second"),
            SourceValue::new("did:web:b"),
        ],
        true,
    );
    assert_eq!(src.values().len(), 2);
    assert_eq!(src.values()[0].label(), Some("First"));
    let w = with_source("purpose", src);
    assert_eq!(hints(&w, "purpose").source_values.len(), 2);
    // Swap: the new source replaces the old and the old labels are gone.
    let mut s = ValueSources::new();
    s.set_operand(
        &member("purpose"),
        ValueSource::new(vec![SourceValue::new("did:web:c")], true),
    );
    let w = w.with_value_sources(s);
    let h = hints(&w, "purpose");
    assert_eq!(h.values, ["did:web:c"]);
    assert_eq!(h.source_values.len(), 1);
    // Clearing restores the profile's hints, with no host labels left.
    let w = w.with_value_sources(ValueSources::new());
    let h = hints(&w, "purpose");
    assert!(h.source_values.is_empty());
    assert_ne!(h.sources.values, HintSource::Host);
}
