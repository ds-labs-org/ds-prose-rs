//! Shapes correctness (audit): the 256-value edge of `sh:in`, an empty
//! JSON-LD list, `sh:class` members cut by a limit, exact closed lists under
//! a profile that is not loaded, and how suggestion-only layers, shapes and
//! host sources combine.
mod common;
mod corpus;
use corpus::*;
use prose_core::edit::Severity;
use prose_core::profile::{
    HintSource, OperandHints, Overlay, ProfileLimits, ProfileVocab, SourceValue, ValueSource,
    ValueSources, Wording,
};
use prose_core::shapes::ShapesGraph;
use prose_core::validate::*;
use serde_json::{Value, json};

const DX: &str = "https://example.org/dataspace/";
const PROFILE: &str = "https://example.org/dataspace/profile";
const PROSE: &str = "https://ds-labs-org.github.io/ds-prose-rs/profile#";
const ODRL: &str = "http://www.w3.org/ns/odrl/2/";

fn dataspace_profile() -> ProfileVocab {
    let text = std::fs::read_to_string(format!("{}/shapes/dataspace-profile.jsonld", tests_dir()))
        .unwrap();
    ProfileVocab::parse(&text).unwrap()
}

fn plain() -> Wording {
    Wording::new().with_profile(dataspace_profile())
}

fn shapes_doc(props: Vec<Value>) -> Value {
    let mut all =
        vec![json!({"sh:path": {"@id": "odrl:leftOperand"}, "sh:hasValue": {"@id": "dx:purpose"}})];
    all.extend(props);
    json!({
        "@context": {"sh": "http://www.w3.org/ns/shacl#", "odrl": ODRL, "dx": DX},
        "@graph": [{"@id": "dx:S", "@type": "sh:NodeShape", "sh:property": all}]
    })
}

fn right_in(items: Value) -> Value {
    json!({"sh:path": {"@id": "odrl:rightOperand"}, "sh:in": items})
}

fn ids(n: usize) -> Value {
    json!({"@list": (0..n).map(|i| json!({"@id": format!("dx:v{i}")})).collect::<Vec<_>>()})
}

fn hints<'a>(w: &'a Wording, left: &str) -> &'a OperandHints {
    let (t, _) = w.lookup(&format!("{DX}{left}")).unwrap();
    w.operand_hints(t)
}

// --- #7 the 256 edge ---------------------------------------------------------

#[test]
fn a_jsonld_sh_in_of_exactly_the_limit_is_read_and_one_more_warns() {
    let limit = ProfileLimits::default().max_values_per_key;
    let s = ShapesGraph::parse(&shapes_doc(vec![right_in(ids(limit))]).to_string()).unwrap();
    assert_eq!(
        s.constraint(&format!("{DX}purpose")).unwrap().values.len(),
        limit,
        "{:?}",
        s.warnings()
    );
    assert!(s.warnings().is_empty(), "{:?}", s.warnings());
    let w = plain().with_shapes(s);
    assert!(hints(&w, "purpose").closed);

    let s = ShapesGraph::parse(&shapes_doc(vec![right_in(ids(limit + 1))]).to_string()).unwrap();
    assert!(
        s.warnings().iter().any(|m| m.contains("more values")),
        "{:?}",
        s.warnings()
    );
    let w = plain().with_shapes(s);
    assert!(!hints(&w, "purpose").closed);
}

#[cfg(feature = "turtle")]
#[test]
fn the_turtle_twin_of_a_limit_sized_list_reads_the_same() {
    let limit = ProfileLimits::default().max_values_per_key;
    let json = ShapesGraph::parse(&shapes_doc(vec![right_in(ids(limit))]).to_string()).unwrap();
    let mut t = String::from(
        "@prefix sh: <http://www.w3.org/ns/shacl#> .\n@prefix odrl: <http://www.w3.org/ns/odrl/2/> .\n@prefix dx: <https://example.org/dataspace/> .\n",
    );
    t.push_str("dx:S a sh:NodeShape ; sh:property [ sh:path odrl:leftOperand ; sh:hasValue dx:purpose ] , [ sh:path odrl:rightOperand ; sh:in (");
    for i in 0..limit {
        t.push_str(&format!(" dx:v{i}"));
    }
    t.push_str(" ) ] .\n");
    let ttl = ShapesGraph::parse(&t).unwrap();
    assert!(json == ttl, "the twins differ");
    assert!(ttl.warnings().is_empty(), "{:?}", ttl.warnings());
}

// --- #16 an empty JSON-LD list -----------------------------------------------

#[test]
fn an_empty_jsonld_list_warns_and_is_not_counted() {
    let s =
        ShapesGraph::parse(&shapes_doc(vec![right_in(json!({"@list": []}))]).to_string()).unwrap();
    assert!(
        s.warnings().iter().any(|m| m.contains("empty sh:in")),
        "{:?}",
        s.warnings()
    );
    assert_eq!(s.constraints().count(), 0);
    assert!(s.is_empty());
}

#[cfg(feature = "turtle")]
#[test]
fn an_empty_turtle_list_reads_like_its_jsonld_twin() {
    let t = "@prefix sh: <http://www.w3.org/ns/shacl#> .\n@prefix odrl: <http://www.w3.org/ns/odrl/2/> .\n@prefix dx: <https://example.org/dataspace/> .\ndx:S a sh:NodeShape ; sh:property [ sh:path odrl:leftOperand ; sh:hasValue dx:purpose ] , [ sh:path odrl:rightOperand ; sh:in () ] .\n";
    let ttl = ShapesGraph::parse(t).unwrap();
    let json =
        ShapesGraph::parse(&shapes_doc(vec![right_in(json!({"@list": []}))]).to_string()).unwrap();
    assert!(ttl == json, "the twins differ");
    assert_eq!(ttl.constraints().count(), 0);
    assert_eq!(ttl.warnings(), json.warnings());
}

// --- #8 sh:class members cut -------------------------------------------------

fn big_profile(n: usize) -> ProfileVocab {
    let mut graph = vec![
        json!({"@id": format!("{DX}profile"), "@type": "owl:Ontology"}),
        json!({"@id": "dx:purpose", "@type": ["odrl:LeftOperand", "skos:Concept"]}),
        json!({"@id": "dx:Big", "@type": "rdfs:Class"}),
    ];
    for i in 0..n {
        graph.push(json!({"@id": format!("dx:m{i}"), "@type": ["odrl:RightOperand", "dx:Big", "skos:Concept"]}));
    }
    let doc = json!({
        "@context": {"odrl": ODRL, "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
                     "skos": "http://www.w3.org/2004/02/skos/core#",
                     "owl": "http://www.w3.org/2002/07/owl#", "dx": DX},
        "@graph": graph
    });
    ProfileVocab::parse(&doc.to_string()).unwrap()
}

fn class_shape(limits: &ProfileLimits) -> ShapesGraph {
    let doc = shapes_doc(vec![
        json!({"sh:path": {"@id": "odrl:rightOperand"}, "sh:class": {"@id": "dx:Big"}}),
    ]);
    ShapesGraph::parse_with(&doc.to_string(), limits).unwrap()
}

#[test]
fn class_members_past_the_per_key_limit_are_all_read_under_the_parse_limits() {
    let w = Wording::new()
        .with_profile(big_profile(300))
        .with_shapes(class_shape(&ProfileLimits::default()));
    let h = hints(&w, "purpose");
    assert!(h.closed);
    assert_eq!(h.values.len(), 300);
    assert!(w.shapes_warnings().is_empty(), "{:?}", w.shapes_warnings());
}

#[test]
fn class_members_cut_by_the_limit_warn_and_close_nothing() {
    let mut limits = ProfileLimits::default();
    limits.max_terms = 10;
    let w = Wording::new()
        .with_profile(big_profile(30))
        .with_shapes(class_shape(&limits));
    assert!(!hints(&w, "purpose").closed);
    let warnings = w.shapes_warnings();
    assert!(
        warnings
            .iter()
            .any(|m| m.contains("sh:class") && m.contains("more members")),
        "{warnings:?}"
    );
}

// --- #14 exact lists are not skipped under a profile that is not loaded ------

const OTHER: &str = "https://example.org/not-loaded";

fn gap_policy(value: &str) -> String {
    json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"dx": DX}],
        "@type": "Set", "uid": "urn:policy:p", "profile": [PROFILE, OTHER],
        "permission": [{"target": "urn:asset:1", "action": "dx:train", "constraint": [
            {"leftOperand": "dx:purpose", "operator": "eq", "rightOperand": {"@id": value}}]}]
    })
    .to_string()
}

fn errors(r: &Report) -> Vec<String> {
    r.findings
        .iter()
        .filter(|f| f.severity == Severity::Error && f.rule.to_string().starts_with("prose."))
        .map(|f| f.rule.to_string())
        .collect()
}

#[test]
fn an_exact_shapes_list_is_checked_even_when_a_profile_is_not_loaded() {
    let s = ShapesGraph::parse(
        &shapes_doc(vec![right_in(json!({"@list": [{"@id": "dx:research"}]}))]).to_string(),
    )
    .unwrap();
    let w = plain().with_shapes(s);
    let r = validate_json(&gap_policy("dx:clinical"), &w, &Options::strict());
    assert_eq!(errors(&r), ["prose.value-not-allowed"], "{:?}", r.findings);
}

#[test]
fn an_exact_overlay_list_is_checked_even_when_a_profile_is_not_loaded() {
    let o = Overlay::parse(
        &json!({
            "@context": {"prose": PROSE, "dx": DX},
            "@graph": [{"@id": "dx:purpose", "prose:closed": true,
                        "prose:rightOperand": [{"@id": "dx:research"}]}]
        })
        .to_string(),
    )
    .unwrap();
    let w = plain().with_overlay(o);
    let r = validate_json(&gap_policy("dx:clinical"), &w, &Options::strict());
    assert_eq!(errors(&r), ["prose.value-not-allowed"], "{:?}", r.findings);
}

#[test]
fn a_class_list_is_still_skipped_when_a_profile_is_not_loaded() {
    // Members come from the loaded profiles, and the missing one may add more.
    let s = ShapesGraph::parse(
        &shapes_doc(vec![
            json!({"sh:path": {"@id": "odrl:rightOperand"}, "sh:class": {"@id": "dx:Purpose"}}),
        ])
        .to_string(),
    )
    .unwrap();
    let w = plain().with_shapes(s);
    let r = validate_json(&gap_policy("dx:us"), &w, &Options::strict());
    assert!(errors(&r).is_empty(), "{:?}", r.findings);
}

// --- #10 suggestion-only layers ----------------------------------------------

fn suggesting_overlay() -> Overlay {
    Overlay::parse(
        &json!({
            "@context": {"prose": PROSE, "odrl": ODRL, "dx": DX, "xsd": "http://www.w3.org/2001/XMLSchema#"},
            "@graph": [{"@id": "dx:purpose",
                        "prose:operator": [{"@id": "odrl:eq"}],
                        "prose:unit": [{"@id": "dx:unitA"}],
                        "prose:datatype": {"@id": "xsd:string"}}]
        })
        .to_string(),
    )
    .unwrap()
}

#[test]
fn shapes_that_close_only_values_leave_an_overlays_suggestions_suggestions() {
    let s = ShapesGraph::parse(
        &shapes_doc(vec![right_in(json!({"@list": [{"@id": "dx:research"}]}))]).to_string(),
    )
    .unwrap();
    let w = plain().with_overlay(suggesting_overlay()).with_shapes(s);
    let h = hints(&w, "purpose");
    assert!(h.closed && h.values_only && !h.binds_other_facets());
    assert_eq!(h.values, [format!("{DX}research")]);
    assert_eq!(h.sources.values, HintSource::Shapes);
    assert_eq!(h.operators, [format!("{ODRL}eq")]);
    assert_eq!(h.sources.operators, HintSource::Overlay);
    assert_eq!(h.units, [format!("{DX}unitA")]);
    assert_eq!(
        h.datatype.as_deref(),
        Some("http://www.w3.org/2001/XMLSchema#string")
    );
}

#[test]
fn shapes_that_give_a_binding_facet_still_drop_the_suggestions_they_do_not_give() {
    let s = ShapesGraph::parse(
        &shapes_doc(vec![
            right_in(json!({"@list": [{"@id": "dx:research"}]})),
            json!({"sh:path": {"@id": "odrl:operator"}, "sh:in": {"@list": [{"@id": "odrl:neq"}]}}),
        ])
        .to_string(),
    )
    .unwrap();
    let w = plain().with_overlay(suggesting_overlay()).with_shapes(s);
    let h = hints(&w, "purpose");
    assert!(h.binds_other_facets());
    assert_eq!(h.operators, [format!("{ODRL}neq")]);
    assert!(h.units.is_empty() && h.datatype.is_none(), "{h:?}");
}

// --- #11 a closed host source can widen what the shapes closed ---------------

fn closing_shapes() -> ShapesGraph {
    ShapesGraph::parse(
        &shapes_doc(vec![right_in(json!({"@list": [{"@id": "dx:research"}]}))]).to_string(),
    )
    .unwrap()
}

fn host(values: &[&str], closed: bool) -> ValueSources {
    let mut s = ValueSources::new();
    s.set_operand(
        &format!("{DX}purpose"),
        ValueSource::new(values.iter().map(|v| SourceValue::new(v)).collect(), closed),
    );
    s
}

#[test]
fn a_closed_host_source_listing_what_the_shapes_do_not_is_warned_about() {
    let w = plain()
        .with_shapes(closing_shapes())
        .with_value_sources(host(
            &[&format!("{DX}research"), &format!("{DX}clinical")],
            true,
        ));
    let warnings = w.value_source_warnings();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0].contains("purpose")
            && warnings[0].contains("clinical")
            && warnings[0].contains("shapes"),
        "{warnings:?}"
    );
    // The host's list is still the one in force.
    assert_eq!(hints(&w, "purpose").values.len(), 2);
}

#[test]
fn a_closed_host_source_inside_the_shapes_list_or_an_open_one_warns_nothing() {
    let inside = plain()
        .with_shapes(closing_shapes())
        .with_value_sources(host(&[&format!("{DX}research")], true));
    assert!(inside.value_source_warnings().is_empty());
    let open = plain()
        .with_shapes(closing_shapes())
        .with_value_sources(host(&[&format!("{DX}clinical")], false));
    assert!(open.value_source_warnings().is_empty());
    let no_base = plain().with_value_sources(host(&[&format!("{DX}clinical")], true));
    assert!(no_base.value_source_warnings().is_empty());
}

// --- #12 an open host source over a closed base keeps its labels -------------

#[test]
fn an_open_host_source_over_a_closed_base_keeps_the_labels_of_the_listed_values() {
    let mut s = ValueSources::new();
    s.set_operand(
        &format!("{DX}purpose"),
        ValueSource::new(
            vec![
                SourceValue::new(&format!("{DX}research"))
                    .with_label("Research!")
                    .with_group("Core"),
                SourceValue::new(&format!("{DX}elsewhere")).with_label("Not listed"),
            ],
            false,
        ),
    );
    let w = plain().with_shapes(closing_shapes()).with_value_sources(s);
    let h = hints(&w, "purpose");
    // Still closed to the shapes' list, the shapes' source.
    assert!(h.closed);
    assert_eq!(h.values, [format!("{DX}research")]);
    assert_eq!(h.sources.values, HintSource::Shapes);
    // The listed value keeps what the host called it; the unlisted one is not offered.
    assert_eq!(h.source_values.len(), 1);
    assert_eq!(h.source_values[0].label(), Some("Research!"));
    assert_eq!(h.source_values[0].group(), Some("Core"));
}

// --- #19 the wording of a datatype conflict names the source in the right number --

#[test]
fn a_datatype_conflict_says_the_shapes_give_not_gives() {
    let s = ShapesGraph::parse(
        &shapes_doc(vec![
            json!({"sh:path": {"@id": "odrl:rightOperand"}, "sh:datatype": {"@id": "xsd:integer"}}),
        ])
        .to_string(),
    )
    .unwrap();
    let w = plain().with_shapes(s);
    let text = json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"dx": DX}],
        "@type": "Set", "uid": "urn:policy:p", "profile": [PROFILE],
        "permission": [{"target": "urn:asset:1", "action": "dx:train", "constraint": [
            {"leftOperand": "dx:purpose", "operator": "eq",
             "rightOperand": {"@value": "5", "@type": "xsd:decimal"},
             "dataType": "xsd:decimal"}]}]
    })
    .to_string();
    let r = validate_json(&text, &w, &Options::strict());
    let msgs: Vec<&str> = r
        .findings
        .iter()
        .filter(|f| f.rule.to_string() == "prose.datatype-conflict")
        .map(|f| f.message.as_str())
        .collect();
    assert_eq!(msgs.len(), 2, "{:?}", r.findings);
    for m in msgs {
        assert!(m.contains("the shapes give \""), "{m}");
        assert!(!m.contains("the shapes gives"), "{m}");
    }
}
