//! The ds-prose extension (`prose:`, namespace
//! `https://ds-labs-org.github.io/ds-prose-rs/profile#`): suggestions a
//! profile makes for one left operand. A ds-prose convention, not ODRL; read
//! only on left operands.
use prose_core::profile::{OperandHints, ProfileLimits, ProfileVocab, Term};
use serde_json::{Value, json};

const ODRL: &str = "http://www.w3.org/ns/odrl/2/";
const EX: &str = "https://example.org/odrl/profile/research/";
const XSD: &str = "http://www.w3.org/2001/XMLSchema#";
const PROSE: &str = "https://ds-labs-org.github.io/ds-prose-rs/profile#";

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

fn from(v: Value) -> ProfileVocab {
    ProfileVocab::from_value(&v, &ProfileLimits::default()).unwrap()
}

fn term<'a>(v: &'a ProfileVocab, iri: &str) -> &'a Term {
    v.term(iri).unwrap_or_else(|| panic!("no term {iri}"))
}

fn ex(local: &str) -> String {
    format!("{EX}{local}")
}

fn odrl(local: &str) -> String {
    format!("{ODRL}{local}")
}

fn ctx() -> Value {
    json!({"odrl": ODRL, "ex": EX, "prose": PROSE, "xsd": XSD,
           "skos": "http://www.w3.org/2004/02/skos/core#"})
}

/// (value, depth) pairs of a left operand's value tree.
fn tree(t: &Term) -> Vec<(String, usize)> {
    t.hints.value_tree.clone()
}

#[test]
fn hints_operators_values_datatype_units() {
    let v = research();
    let count = &term(&v, &ex("dataSubjectCount")).hints;
    assert_eq!(count.operators, [odrl("gteq"), odrl("gt")]);
    assert_eq!(count.datatype.as_deref(), Some(&*format!("{XSD}integer")));
    assert!(count.values.is_empty());
    assert!(count.units.is_empty());

    let purpose = &term(&v, &ex("researchPurpose")).hints;
    assert_eq!(
        purpose.operators,
        [odrl("eq"), odrl("isAnyOf"), odrl("isNoneOf")]
    );
    assert_eq!(purpose.values, [ex("academic"), ex("commercialResearch")]);
    assert_eq!(purpose.datatype, None);

    let period = &term(&v, &ex("retentionPeriod")).hints;
    assert_eq!(period.operators, [odrl("lteq"), odrl("lt")]);
    assert_eq!(period.datatype.as_deref(), Some(&*format!("{XSD}duration")));

    // Units, and string values read as IRIs under prose: keys.
    let v = from(json!({
        "@context": ctx(),
        "@graph": [{
            "@id": "ex:distance", "@type": "odrl:LeftOperand",
            "prose:operator": "odrl:lt",
            "prose:unit": ["ex:km", {"@id": "ex:mile"}],
            "prose:datatype": ["xsd:decimal", "xsd:integer"]
        }]
    }));
    let d = &term(&v, &ex("distance")).hints;
    assert_eq!(d.operators, [odrl("lt")]);
    assert_eq!(d.units, [ex("km"), ex("mile")]);
    // One datatype: the first one given.
    assert_eq!(d.datatype.as_deref(), Some(&*format!("{XSD}decimal")));
}

#[test]
fn no_extension_no_hints() {
    let v = from(json!({
        "@context": ctx(),
        "@graph": [{"@id": "ex:plain", "@type": "odrl:LeftOperand",
                    "skos:note": "Use only gteq with an xsd:integer."}]
    }));
    assert_eq!(term(&v, &ex("plain")).hints, OperandHints::default());
    // The ODRL vocabulary carries no extension: no hints anywhere.
    let v = ProfileVocab::parse(&profile_text("odrl22-excerpt.json")).unwrap();
    assert!(v.terms().all(|t| t.hints == OperandHints::default()));
}

#[test]
fn value_descendants_with_depth() {
    let v = research();
    assert_eq!(
        tree(term(&v, &ex("researchPurpose"))),
        [
            (ex("academic"), 0),
            (ex("clinical"), 1),
            (ex("commercialResearch"), 0)
        ]
    );
    // Without suggested values there is no tree.
    assert!(tree(term(&v, &ex("dataSubjectCount"))).is_empty());
}

#[test]
fn value_tree_walks_deep_once_and_skips_non_values() {
    let v = from(json!({
        "@context": ctx(),
        "@graph": [
            {"@id": "ex:region", "@type": "odrl:LeftOperand",
             "prose:rightOperand": ["ex:europe", "ex:france", "ex:undeclared"]},
            {"@id": "ex:europe", "@type": "odrl:RightOperand"},
            {"@id": "ex:france", "@type": "odrl:RightOperand", "skos:broader": "ex:europe"},
            {"@id": "ex:paris", "@type": "odrl:RightOperand", "skos:broader": "ex:france"},
            // A broader link from something that is not a value is not followed.
            {"@id": "ex:euro", "@type": "odrl:LeftOperand", "skos:broader": "ex:europe"},
            {"@id": "ex:lyon", "@type": "odrl:RightOperand",
             "rdfs:subClassOf": {"@id": "ex:france"}}
        ]
    }));
    // Each value once, at its first place; an undeclared value is kept.
    assert_eq!(
        tree(term(&v, &ex("region"))),
        [
            (ex("europe"), 0),
            (ex("france"), 1),
            (ex("paris"), 2),
            (ex("lyon"), 2),
            (ex("undeclared"), 0)
        ]
    );
}

#[test]
fn value_tree_cycle_terminates() {
    let v = from(json!({
        "@context": ctx(),
        "@graph": [
            {"@id": "ex:op", "@type": "odrl:LeftOperand", "prose:rightOperand": "ex:a"},
            {"@id": "ex:a", "@type": "odrl:RightOperand", "skos:broader": "ex:b"},
            {"@id": "ex:b", "@type": "odrl:RightOperand", "skos:broader": "ex:a"}
        ]
    }));
    assert_eq!(tree(term(&v, &ex("op"))), [(ex("a"), 0), (ex("b"), 1)]);
}

#[test]
fn value_tree_depth_bounded_by_max_depth() {
    let mut graph = vec![json!({"@id": "ex:op", "@type": "odrl:LeftOperand",
                                "prose:rightOperand": "ex:v0"})];
    for i in 0..50 {
        let mut node = json!({"@id": format!("ex:v{i}"), "@type": "odrl:RightOperand"});
        if i > 0 {
            node["skos:broader"] = json!(format!("ex:v{}", i - 1));
        }
        graph.push(node);
    }
    let mut limits = ProfileLimits::default();
    limits.max_depth = 5;
    let v =
        ProfileVocab::from_value(&json!({"@context": ctx(), "@graph": graph}), &limits).unwrap();
    let t = tree(term(&v, &ex("op")));
    assert!(t.iter().all(|(_, d)| *d < 5), "{t:?}");
    assert_eq!(t.len(), 5);
}

#[test]
fn hints_ignored_on_non_left_operand_nodes() {
    let v = from(json!({
        "@context": ctx(),
        "@graph": [
            {"@id": "ex:act", "@type": "odrl:Action",
             "prose:operator": "odrl:eq", "prose:rightOperand": "ex:val",
             "prose:datatype": "xsd:integer", "prose:unit": "ex:km"},
            {"@id": "ex:val", "@type": "odrl:RightOperand", "prose:operator": "odrl:eq"},
            {"@id": "ex:op", "@type": "odrl:Operator", "prose:datatype": "xsd:integer"}
        ]
    }));
    for t in ["act", "val", "op"] {
        assert_eq!(term(&v, &ex(t)).hints, OperandHints::default(), "{t}");
    }
}

#[test]
fn other_namespaces_are_not_the_extension() {
    // The namespace the design draft used before the extension shipped.
    let v = from(json!({
        "@context": {"odrl": ODRL, "ex": EX, "p": "https://w3id.org/ds-prose/profile#"},
        "@graph": [{"@id": "ex:lo", "@type": "odrl:LeftOperand",
                    "p:operator": {"@id": "odrl:eq"}}]
    }));
    assert_eq!(term(&v, &ex("lo")).hints, OperandHints::default());
}

#[test]
fn blank_node_hint_values_are_dropped() {
    let v = from(json!({
        "@context": ctx(),
        "@graph": [{"@id": "ex:lo", "@type": "odrl:LeftOperand",
                    "prose:operator": [{"@id": "_:b0"}, {"@id": "odrl:eq"}],
                    "prose:datatype": {"@id": "_:b1"}}]
    }));
    let h = &term(&v, &ex("lo")).hints;
    assert_eq!(h.operators, [odrl("eq")]);
    assert_eq!(h.datatype, None);
}

#[test]
fn value_tree_capped_with_a_warning() {
    let mut graph = vec![json!({"@id": "ex:op", "@type": "odrl:LeftOperand",
                                "prose:rightOperand": "ex:root"})];
    graph.push(json!({"@id": "ex:root", "@type": "odrl:RightOperand"}));
    for i in 0..20 {
        graph.push(
            json!({"@id": format!("ex:v{i}"), "@type": "odrl:RightOperand",
                          "skos:broader": "ex:root"}),
        );
    }
    let mut limits = ProfileLimits::default();
    limits.max_values_per_key = 8;
    let v =
        ProfileVocab::from_value(&json!({"@context": ctx(), "@graph": graph}), &limits).unwrap();
    let t = tree(term(&v, &ex("op")));
    assert_eq!(t.len(), 8);
    assert_eq!(t[0], (ex("root"), 0));
    assert_eq!(t[1], (ex("v0"), 1));
    assert!(
        v.warnings()
            .iter()
            .any(|w| w.contains("suggested values") && w.contains(&ex("op"))),
        "{:?}",
        v.warnings()
    );
    // Within the cap, no warning.
    assert!(
        !research()
            .warnings()
            .iter()
            .any(|w| w.contains("suggested values"))
    );
}

#[test]
fn value_trees_share_one_budget_per_profile() {
    // Three left operands suggest the same 4-value hierarchy; with room for
    // 7 descendants in the whole profile (max_terms), the third tree is cut
    // after one.
    let mut graph = vec![json!({"@id": "ex:root", "@type": "odrl:RightOperand"})];
    for i in 0..3 {
        graph.push(
            json!({"@id": format!("ex:v{i}"), "@type": "odrl:RightOperand",
                          "skos:broader": "ex:root"}),
        );
    }
    for i in 0..3 {
        graph.push(
            json!({"@id": format!("ex:lo{i}"), "@type": "odrl:LeftOperand",
                          "prose:rightOperand": "ex:root"}),
        );
    }
    let mut limits = ProfileLimits::default();
    limits.max_terms = 7;
    let v =
        ProfileVocab::from_value(&json!({"@context": ctx(), "@graph": graph}), &limits).unwrap();
    assert_eq!(tree(term(&v, &ex("lo0"))).len(), 4);
    assert_eq!(tree(term(&v, &ex("lo1"))).len(), 4);
    assert_eq!(tree(term(&v, &ex("lo2"))), [(ex("root"), 0), (ex("v0"), 1)]);
    assert!(
        v.warnings()
            .iter()
            .any(|w| w.contains("suggested values") && w.contains(&ex("lo2")))
    );
}
