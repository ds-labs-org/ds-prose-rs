//! Companion SHACL shapes (issue #76, section A): a dataspace's own shapes
//! add hints to the left operands the loaded profiles declare. Only a subset
//! of SHACL is read; the rest is ignored with a warning. Every finding that a
//! shape closed names `shapes` as the source.
mod common;
mod corpus;
use corpus::*;
use prose_core::edit::*;
use prose_core::profile::{HintSource, ProfileVocab, TermKind, Wording};
use prose_core::shapes::ShapesGraph;
use prose_core::validate::*;
use serde_json::{Value, json};

const DX: &str = "https://example.org/dataspace/";
const PROFILE: &str = "https://example.org/dataspace/profile";

fn shapes_text(name: &str) -> String {
    std::fs::read_to_string(format!("{}/shapes/{name}", tests_dir())).unwrap()
}

fn dataspace_profile() -> ProfileVocab {
    ProfileVocab::parse(&shapes_text("dataspace-profile.jsonld")).unwrap()
}

fn fixture_shapes() -> ShapesGraph {
    ShapesGraph::parse(&shapes_text("dataspace-shapes.jsonld")).unwrap()
}

/// The dataspace profile with the fixture shapes.
fn shaped() -> Wording {
    Wording::new()
        .with_profile(dataspace_profile())
        .with_shapes(fixture_shapes())
}

fn unshaped() -> Wording {
    Wording::new().with_profile(dataspace_profile())
}

/// Shapes written as a JSON-LD graph, the SHACL prefixes bound.
fn graph(nodes: Value) -> ShapesGraph {
    let doc = json!({
        "@context": {
            "sh": "http://www.w3.org/ns/shacl#",
            "odrl": "http://www.w3.org/ns/odrl/2/",
            "xsd": "http://www.w3.org/2001/XMLSchema#",
            "dx": DX
        },
        "@graph": nodes
    });
    ShapesGraph::parse(&doc.to_string()).unwrap()
}

/// A constraint shape for `left` with these property shapes after the
/// selector.
fn constraint_shape(left: &str, props: Vec<Value>) -> Value {
    let mut all =
        vec![json!({"sh:path": {"@id": "odrl:leftOperand"}, "sh:hasValue": {"@id": left}})];
    all.extend(props);
    json!({"@id": "dx:S", "@type": "sh:NodeShape", "sh:property": all})
}

fn right(extra: Value) -> Value {
    let mut p = json!({"sh:path": {"@id": "odrl:rightOperand"}});
    for (k, v) in extra.as_object().unwrap() {
        p[k] = v.clone();
    }
    p
}

fn list(items: &[&str]) -> Value {
    json!({"@list": items.iter().map(|i| json!({"@id": i})).collect::<Vec<_>>()})
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

fn rules(r: &Report) -> Vec<(String, String)> {
    r.findings
        .iter()
        .map(|f| (f.rule.to_string(), target_str(&f.target)))
        .collect()
}

fn row(rule: &str, target: &str) -> (String, String) {
    (rule.to_string(), target.to_string())
}

const C0: &str = "policy[0].permission[0].constraint[0]";

fn hints<'a>(w: &'a Wording, left: &str) -> &'a prose_core::profile::OperandHints {
    let (t, _) = w.lookup(&format!("{DX}{left}")).unwrap();
    w.operand_hints(t)
}

// --- reading -----------------------------------------------------------------

#[test]
fn the_fixture_shapes_are_read_with_the_warnings_they_earn() {
    let s = fixture_shapes();
    // The fixture sticks to the subset: nothing to warn about.
    assert_eq!(s.warnings(), &[] as &[String]);
}

#[test]
fn sh_in_on_the_right_operand_closes_the_values() {
    let w = shaped();
    let h = hints(&w, "purpose");
    assert!(h.closed);
    assert_eq!(
        h.values,
        [format!("{DX}research"), format!("{DX}marketing")]
    );
    // sh:in is exactly its list: no descendants (dx:clinical is narrower
    // than dx:research, and a shape would reject it).
    let tree: Vec<&str> = h.value_tree.iter().map(|(v, _)| v.as_str()).collect();
    assert_eq!(tree, [format!("{DX}research"), format!("{DX}marketing")]);
    assert_eq!(h.sources.values, HintSource::Shapes);
    assert_eq!(h.sources.operators, HintSource::Shapes);
}

#[test]
fn a_shape_adds_to_declared_terms_only_and_never_relabels() {
    let with = shaped();
    let without = unshaped();
    // No shape on dx:price: its hints are the profile's, untouched.
    assert_eq!(hints(&with, "price"), hints(&without, "price"));
    assert!(!hints(&with, "price").closed);
    // The label stays the profile's.
    let (t, _) = with.lookup(&format!("{DX}purpose")).unwrap();
    assert_eq!(t.display_label(&[]), "Purpose");
    // A shape on a left operand no profile declares adds nothing.
    let ghost = graph(json!([constraint_shape(
        "dx:ghost",
        vec![right(json!({"sh:in": list(&["dx:research"])}))]
    )]));
    let w = unshaped().with_shapes(ghost);
    assert!(w.lookup(&format!("{DX}ghost")).is_none());
}

#[test]
fn the_order_of_profile_and_shapes_does_not_matter() {
    let a = Wording::new()
        .with_shapes(fixture_shapes())
        .with_profile(dataspace_profile());
    assert_eq!(hints(&a, "purpose"), hints(&shaped(), "purpose"));
    assert!(hints(&a, "purpose").closed);
}

#[test]
fn sh_class_means_the_profile_terms_of_that_class_with_their_descendants() {
    let w = shaped();
    let h = hints(&w, "participantType");
    assert!(h.closed);
    assert_eq!(h.sources.values, HintSource::Shapes);
    let tree: Vec<&str> = h.value_tree.iter().map(|(v, _)| v.as_str()).collect();
    assert_eq!(tree, [format!("{DX}member"), format!("{DX}partner")]);
    // A class with narrower values: skos:broader descendants are included,
    // each with its depth.
    let s = graph(json!([constraint_shape(
        "dx:purpose",
        vec![right(json!({"sh:class": {"@id": "dx:Purpose"}}))]
    )]));
    let w = unshaped().with_shapes(s);
    let tree: Vec<(&str, usize)> = hints(&w, "purpose")
        .value_tree
        .iter()
        .map(|(v, d)| (v.strip_prefix(DX).unwrap(), *d))
        .collect();
    assert_eq!(
        tree,
        [("research", 0), ("clinical", 1), ("marketing", 0)],
        "a typed term's descendants are listed under it"
    );
}

#[test]
fn sh_node_reads_like_sh_class() {
    let s = graph(json!([constraint_shape(
        "dx:participantType",
        vec![right(json!({"sh:node": {"@id": "dx:Membership"}}))]
    )]));
    let w = unshaped().with_shapes(s);
    assert_eq!(hints(&w, "participantType").value_tree.len(), 2);
}

#[test]
fn a_class_no_term_belongs_to_closes_nothing() {
    let s = graph(json!([constraint_shape(
        "dx:purpose",
        vec![right(json!({"sh:class": {"@id": "dx:Nothing"}}))]
    )]));
    let w = unshaped().with_shapes(s);
    assert!(!hints(&w, "purpose").closed);
}

#[test]
fn datatype_and_the_lexical_bounds_are_read() {
    let w = shaped();
    let h = hints(&w, "count");
    assert_eq!(
        h.datatype.as_deref(),
        Some("http://www.w3.org/2001/XMLSchema#integer")
    );
    assert_eq!(h.min_inclusive.as_deref(), Some("1"));
    assert_eq!(h.max_inclusive.as_deref(), Some("100"));
    assert_eq!(h.units, [format!("{DX}people")]);
    assert_eq!(h.sources.datatype, HintSource::Shapes);
    assert_eq!(h.sources.lexical, HintSource::Shapes);
    assert_eq!(h.sources.units, HintSource::Shapes);
    assert!(h.closed);
    let w = shaped();
    let c = hints(&w, "code");
    assert_eq!(c.pattern.as_deref(), Some("^[A-Z]{3}-[0-9]+$"));
}

#[test]
fn constraint_shapes_are_found_through_sh_or_and_qualified_value_shapes() {
    // Region sits under sh:or, count under sh:qualifiedValueShape.
    assert!(hints(&shaped(), "region").closed);
    assert!(hints(&shaped(), "count").closed);
}

#[test]
fn shapes_close_term_lists_for_the_whole_policy() {
    let w = shaped();
    let actions: Vec<&str> = w
        .closed_terms(TermKind::Action)
        .unwrap()
        .iter()
        .map(|a| a.strip_prefix(DX).unwrap())
        .collect();
    assert_eq!(actions, ["train", "share"]);
    assert_eq!(w.closed_terms(TermKind::LeftOperand).unwrap().len(), 5);
    assert!(w.closed_terms(TermKind::Operator).is_none());
    assert!(unshaped().closed_terms(TermKind::Action).is_none());
}

#[test]
fn what_is_not_in_the_subset_is_ignored_with_a_warning() {
    let s = graph(json!([
        {
            "@id": "dx:S", "@type": "sh:NodeShape",
            "sh:property": [
                {"sh:path": {"@id": "odrl:leftOperand"}, "sh:hasValue": {"@id": "dx:purpose"}},
                {"sh:path": {"@id": "odrl:rightOperand"}, "sh:in": list(&["dx:research"]), "sh:minCount": 1},
                {"sh:path": {"@id": "odrl:target"}, "sh:minCount": 1}
            ],
            "sh:closed": true
        },
        {
            "@id": "dx:T",
            "sh:property": [
                {"sh:path": {"@id": "odrl:rightOperand"}, "sh:in": list(&["dx:member"])}
            ]
        }
    ]));
    let joined = s.warnings().join("\n");
    assert!(joined.contains("sh:minCount"), "{joined}");
    assert!(joined.contains("sh:closed"), "{joined}");
    assert!(joined.contains("odrl:target"), "{joined}");
    // A right operand shape no left operand selects cannot say whose values.
    assert!(joined.contains("no left operand"), "{joined}");
    // What could be read still is.
    let w = unshaped().with_shapes(s);
    assert_eq!(hints(&w, "purpose").values, [format!("{DX}research")]);
}

#[test]
fn a_literal_in_sh_in_ignores_the_whole_list_with_a_warning() {
    let s = graph(json!([constraint_shape(
        "dx:purpose",
        vec![right(json!({"sh:in": {"@list": ["a", "b"]}}))]
    )]));
    assert!(
        s.warnings().iter().any(|w| w.contains("literal")),
        "{:?}",
        s.warnings()
    );
    let w = unshaped().with_shapes(s);
    assert!(!hints(&w, "purpose").closed);
}

#[test]
fn a_pattern_that_cannot_be_read_is_ignored_with_a_warning() {
    let s = graph(json!([constraint_shape(
        "dx:code",
        vec![right(json!({"sh:pattern": "^(?=a)b"}))]
    )]));
    assert!(
        s.warnings().iter().any(|w| w.contains("pattern")),
        "{:?}",
        s.warnings()
    );
    let w = unshaped().with_shapes(s);
    assert_eq!(hints(&w, "code").pattern, None);
    assert!(!hints(&w, "code").closed);
}

#[test]
fn the_text_must_be_shapes() {
    for bad in ["", "{", "<!doctype html><p>hi</p>", "[1,2]"] {
        assert!(ShapesGraph::parse(bad).is_err(), "{bad:?}");
    }
    // A profile is not shapes: nothing in it is SHACL.
    let e = ShapesGraph::parse(&shapes_text("dataspace-profile.jsonld")).unwrap_err();
    assert!(e.to_string().contains("SHACL"), "{e}");
}

// --- validation --------------------------------------------------------------

#[test]
fn a_value_outside_sh_in_is_an_error_naming_the_shapes() {
    let text = policy(json!([
        {"leftOperand": "dx:purpose", "operator": "eq", "rightOperand": {"@id": "dx:clinical"}}
    ]));
    let r = run(&text, &shaped());
    assert_eq!(
        rules(&r),
        [row(
            "prose.value-not-allowed",
            &format!("{C0}#rightOperand[0]")
        )]
    );
    let f = &r.findings[0];
    assert_eq!(f.severity, Severity::Error);
    assert!(f.message.contains("the shapes allow"), "{}", f.message);
    let e = f.expected.as_ref().unwrap();
    assert!(e.closed);
    assert_eq!(e.source, Some(HintSource::Shapes));
    assert_eq!(
        e.values,
        [format!("{DX}research"), format!("{DX}marketing")]
    );
    // The same policy without the shapes is clean.
    assert!(run(&text, &unshaped()).findings.is_empty());
}

#[test]
fn a_value_in_sh_in_conforms() {
    let text = policy(json!([
        {"leftOperand": "dx:purpose", "operator": "eq", "rightOperand": {"@id": "dx:research"}}
    ]));
    assert_eq!(run(&text, &shaped()).verdict, Verdict::Conforms);
}

#[test]
fn a_string_for_an_allowed_iri_is_a_fixable_iri_expected() {
    let text = policy(json!([
        {"leftOperand": "dx:purpose", "operator": "eq", "rightOperand": "dx:research"}
    ]));
    let r = run(&text, &shaped());
    assert_eq!(
        rules(&r),
        [row("prose.iri-expected", &format!("{C0}#rightOperand[0]"))]
    );
    assert_eq!(
        r.findings[0].expected.as_ref().unwrap().source,
        Some(HintSource::Shapes)
    );
}

#[test]
fn an_operator_outside_sh_in_is_an_error_naming_the_shapes() {
    let text = policy(json!([
        {"leftOperand": "dx:purpose", "operator": "neq", "rightOperand": {"@id": "dx:research"}}
    ]));
    let r = run(&text, &shaped());
    assert_eq!(
        rules(&r),
        [row("prose.operator-not-allowed", &format!("{C0}#operator"))]
    );
    let f = &r.findings[0];
    assert_eq!(f.severity, Severity::Error);
    assert!(f.message.contains("the shapes allow only"), "{}", f.message);
    assert_eq!(
        f.expected.as_ref().unwrap().source,
        Some(HintSource::Shapes)
    );
}

#[test]
fn a_unit_outside_sh_in_is_an_error_naming_the_shapes() {
    let text = policy(json!([
        {"leftOperand": "dx:count", "operator": "lteq", "rightOperand": {"@value": "5", "@type": "xsd:integer"},
         "unit": "dx:kilos"}
    ]));
    let r = run(&text, &shaped());
    let f = r
        .findings
        .iter()
        .find(|f| f.rule == RuleId::ProseUnitNotAllowed)
        .unwrap();
    assert_eq!(f.severity, Severity::Error);
    assert!(f.message.contains("the shapes allow only"), "{}", f.message);
    assert_eq!(
        f.expected.as_ref().unwrap().source,
        Some(HintSource::Shapes)
    );
}

#[test]
fn the_class_closes_the_values_to_its_members() {
    let ok = policy(json!([
        {"leftOperand": "dx:participantType", "operator": "eq", "rightOperand": {"@id": "dx:partner"}}
    ]));
    assert_eq!(run(&ok, &shaped()).verdict, Verdict::Conforms);
    let bad = policy(json!([
        {"leftOperand": "dx:participantType", "operator": "eq", "rightOperand": {"@id": "dx:eu"}}
    ]));
    let r = run(&bad, &shaped());
    assert_eq!(
        rules(&r),
        [row(
            "prose.value-not-allowed",
            &format!("{C0}#rightOperand[0]")
        )]
    );
}

#[test]
fn the_shape_datatype_is_checked_lexically() {
    let bad = policy(json!([
        {"leftOperand": "dx:count", "operator": "lteq", "rightOperand": "many"}
    ]));
    let r = run(&bad, &shaped());
    let f = r
        .findings
        .iter()
        .find(|f| f.rule == RuleId::ProseDatatypeLexical)
        .unwrap();
    assert_eq!(f.severity, Severity::Error);
    assert!(f.message.contains("the shapes expect"), "{}", f.message);
    assert_eq!(
        f.expected.as_ref().unwrap().source,
        Some(HintSource::Shapes)
    );
}

#[test]
fn the_bounds_are_lexical_checks_on_a_literal() {
    for (value, ok) in [
        ("1", true),
        ("50", true),
        ("100", true),
        ("0", false),
        ("101", false),
    ] {
        let text = policy(json!([
            {"leftOperand": "dx:count", "operator": "lteq", "rightOperand": value}
        ]));
        let r = run(&text, &shaped());
        let bound = r
            .findings
            .iter()
            .find(|f| f.rule == RuleId::ProseValueNotAllowed);
        assert_eq!(bound.is_none(), ok, "{value}: {:?}", rules(&r));
        if let Some(f) = bound {
            assert_eq!(f.severity, Severity::Error);
            assert!(f.message.contains("the shapes"), "{}", f.message);
            assert_eq!(
                f.target,
                IssueTarget::Slot(format!("{C0}#rightOperand[0]").parse().unwrap())
            );
            let e = f.expected.as_ref().unwrap();
            assert_eq!(e.source, Some(HintSource::Shapes));
            assert_eq!(e.min_inclusive.as_deref(), Some("1"));
            assert_eq!(e.max_inclusive.as_deref(), Some("100"));
        }
    }
}

#[test]
fn the_pattern_is_a_lexical_check_on_a_literal() {
    for (value, ok) in [
        ("ABC-1", true),
        ("XYZ-20", true),
        ("abc-1", false),
        ("ABCD-1", false),
        ("ABC-", false),
    ] {
        let text = policy(json!([
            {"leftOperand": "dx:code", "operator": "eq", "rightOperand": value}
        ]));
        let r = run(&text, &shaped());
        let bad = r
            .findings
            .iter()
            .find(|f| f.rule == RuleId::ProseValueNotAllowed);
        assert_eq!(bad.is_none(), ok, "{value}: {:?}", rules(&r));
        if let Some(f) = bad {
            assert!(f.message.contains("pattern"), "{}", f.message);
            assert_eq!(
                f.expected.as_ref().unwrap().pattern.as_deref(),
                Some("^[A-Z]{3}-[0-9]+$")
            );
        }
    }
}

#[test]
fn an_action_outside_the_closed_list_is_unknown_to_the_shapes() {
    let text = json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"dx": DX}],
        "@type": "Set", "uid": "urn:policy:p", "profile": PROFILE,
        "permission": [{"target": "urn:asset:1", "action": ["dx:train", "dx:publish"]}]
    })
    .to_string();
    let r = run(&text, &shaped());
    let found: Vec<_> = r
        .findings
        .iter()
        .filter(|f| f.rule == RuleId::ProfileUnknownTerm)
        .collect();
    assert_eq!(found.len(), 1, "{:?}", rules(&r));
    let f = found[0];
    assert_eq!(f.severity, Severity::Error);
    assert!(f.message.contains("dx:publish"), "{}", f.message);
    assert!(f.message.contains("the shapes"), "{}", f.message);
    assert_eq!(f.term.as_ref().unwrap().iri, format!("{DX}publish"));
    assert_eq!(
        f.expected.as_ref().unwrap().source,
        Some(HintSource::Shapes)
    );
    // Unshaped, dx:publish is a declared action.
    assert!(
        run(&text, &unshaped())
            .findings
            .iter()
            .all(|f| f.rule != RuleId::ProfileUnknownTerm)
    );
}

#[test]
fn a_left_operand_outside_the_closed_list_is_unknown_to_the_shapes() {
    let text = policy(json!([
        {"leftOperand": "dx:price", "operator": "eq", "rightOperand": "1"}
    ]));
    let r = run(&text, &shaped());
    assert_eq!(
        rules(&r),
        [row("profile.unknown-term", &format!("{C0}#leftOperand"))]
    );
    assert!(
        r.findings[0].message.contains("the shapes"),
        "{}",
        r.findings[0].message
    );
}

#[test]
fn a_closed_list_is_checked_in_strict_mode_only() {
    let text = policy(json!([
        {"leftOperand": "dx:price", "operator": "eq", "rightOperand": "1"}
    ]));
    let r = validate_json(&text, &shaped(), &Options::advisory());
    assert!(
        r.findings
            .iter()
            .all(|f| f.rule != RuleId::ProfileUnknownTerm)
    );
}

#[test]
fn the_validator_refuses_a_term_the_shapes_close_out() {
    let v = Validator::new(std::sync::Arc::new(shaped()), Options::strict());
    let doc = read_model(&policy(json!([
        {"leftOperand": "dx:purpose", "operator": "eq", "rightOperand": {"@id": "dx:research"}}
    ])))
    .unwrap();
    let slot: SlotPath = format!("{C0}#leftOperand").parse().unwrap();
    let r = v.admissible_values(&doc, &slot, &["dx:count", "dx:price"]);
    assert!(r[0].is_ok());
    let e = r[1].as_ref().unwrap_err();
    assert_eq!(e.rule, RuleId::ProfileUnknownTerm);
}

#[test]
fn json_report_names_the_source_only_when_it_is_not_the_profile() {
    let text = policy(json!([
        {"leftOperand": "dx:purpose", "operator": "eq", "rightOperand": {"@id": "dx:clinical"}}
    ]));
    let j = run(&text, &shaped()).to_json();
    assert_eq!(j["findings"][0]["expected"]["source"], json!("shapes"));
    // A profile-closed operand's report is as it always was.
    let closed = Wording::new()
        .with_profile(ProfileVocab::parse(&profile_text("research-closed.jsonld")).unwrap());
    let text = json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"ex": "https://example.org/odrl/profile/research/"}],
        "@type": "Set", "uid": "urn:policy:p", "profile": "https://example.org/odrl/profile/research",
        "permission": [{"target": "urn:asset:1", "action": "use", "constraint": [
            {"leftOperand": "ex:researchPurpose", "operator": "eq", "rightOperand": {"@id": "https://example.net/x"}}
        ]}]
    })
    .to_string();
    let j = run(&text, &closed).to_json();
    assert!(j["findings"][0]["expected"].get("source").is_none(), "{j}");
}

#[test]
fn a_shape_replaces_the_profile_facet_it_gives_and_keeps_a_closed_profiles_others() {
    // research-closed closes ex:dataSubjectCount (operators gteq/gt, integer).
    let profile = ProfileVocab::parse(&profile_text("research-closed.jsonld")).unwrap();
    let ex = "https://example.org/odrl/profile/research/";
    let s = ShapesGraph::parse(
        &json!({
            "@context": {"sh": "http://www.w3.org/ns/shacl#", "odrl": "http://www.w3.org/ns/odrl/2/",
                         "ex": ex},
            "@graph": [{
                "@id": "ex:S",
                "sh:property": [
                    {"sh:path": {"@id": "odrl:leftOperand"}, "sh:hasValue": {"@id": "ex:dataSubjectCount"}},
                    {"sh:path": {"@id": "odrl:operator"}, "sh:in": {"@list": [{"@id": "odrl:gteq"}]}}
                ]
            }]
        })
        .to_string(),
    )
    .unwrap();
    let w = Wording::new().with_profile(profile).with_shapes(s);
    let (t, _) = w.lookup(&format!("{ex}dataSubjectCount")).unwrap();
    let h = w.operand_hints(t);
    assert_eq!(h.operators, ["http://www.w3.org/ns/odrl/2/gteq"]);
    assert_eq!(h.sources.operators, HintSource::Shapes);
    // The profile's own, still binding, still the profile's.
    assert_eq!(
        h.datatype.as_deref(),
        Some("http://www.w3.org/2001/XMLSchema#integer")
    );
    assert_eq!(h.sources.datatype, HintSource::Profile);
}

#[test]
fn a_shape_that_closes_makes_only_its_own_facets_binding() {
    // dx:purpose has an open profile hint for operators (none here), so use
    // research.jsonld: ex:researchPurpose suggests operators and values (not closed).
    let profile = ProfileVocab::parse(&profile_text("research.jsonld")).unwrap();
    let ex = "https://example.org/odrl/profile/research/";
    let s = ShapesGraph::parse(
        &json!({
            "@context": {"sh": "http://www.w3.org/ns/shacl#", "odrl": "http://www.w3.org/ns/odrl/2/",
                         "ex": ex},
            "@graph": [{
                "@id": "ex:S",
                "sh:property": [
                    {"sh:path": {"@id": "odrl:leftOperand"}, "sh:hasValue": {"@id": "ex:researchPurpose"}},
                    {"sh:path": {"@id": "odrl:rightOperand"}, "sh:in": {"@list": [{"@id": "ex:academic"}]}}
                ]
            }]
        })
        .to_string(),
    )
    .unwrap();
    let w = Wording::new().with_profile(profile).with_shapes(s);
    let (t, _) = w.lookup(&format!("{ex}researchPurpose")).unwrap();
    assert!(
        !t.hints.operators.is_empty(),
        "the profile suggests operators"
    );
    let h = w.operand_hints(t);
    assert!(h.closed);
    assert!(
        h.operators.is_empty(),
        "a suggestion does not become binding: {h:?}"
    );
    assert_eq!(h.values, [format!("{ex}academic")]);
}

#[test]
fn shapes_are_part_of_the_wording() {
    assert!(shaped().shapes().is_some());
    assert!(unshaped().shapes().is_none());
    assert_ne!(shaped(), unshaped());
    assert_eq!(shaped(), shaped());
}

#[cfg(feature = "turtle")]
#[test]
fn turtle_shapes_equal_their_jsonld_twin() {
    let ttl = ShapesGraph::parse(&shapes_text("dataspace-shapes.ttl")).unwrap();
    assert_eq!(ttl.warnings(), &[] as &[String]);
    assert_eq!(ttl, fixture_shapes());
    let w = unshaped().with_shapes(ttl);
    assert_eq!(hints(&w, "purpose"), hints(&shaped(), "purpose"));
    assert_eq!(hints(&w, "count"), hints(&shaped(), "count"));
}

#[test]
fn a_deactivated_shape_says_nothing() {
    // sh:deactivated true on a node shape or on a property shape switches it
    // off: reading its list as allowed would close values it no longer closes.
    let mut node = constraint_shape("dx:code", vec![right(json!({"sh:in": list(&["dx:a"])}))]);
    node["sh:deactivated"] = json!(true);
    let s = graph(json!([node]));
    assert!(s.is_empty(), "{:?}", s.constraints().collect::<Vec<_>>());

    let s = graph(json!([constraint_shape(
        "dx:code",
        vec![right(
            json!({"sh:in": list(&["dx:a"]), "sh:deactivated": true})
        )]
    )]));
    assert!(
        s.constraint("https://example.org/dataspace/code")
            .is_none_or(|c| c.values.is_empty()),
        "{:?}",
        s.constraints().collect::<Vec<_>>()
    );
}

#[cfg(feature = "turtle")]
#[test]
fn hostile_turtle_is_an_error_or_a_bounded_read_never_a_panic() {
    let head = "@prefix sh: <http://www.w3.org/ns/shacl#> . @prefix odrl: <http://www.w3.org/ns/odrl/2/> . @prefix ex: <https://e.org/> .\n";
    let rdf = "<http://www.w3.org/1999/02/22-rdf-syntax-ns#";
    // A list that loops back on itself, and shapes that name themselves.
    let cyclic = format!(
        "{head}ex:S sh:property [ sh:path odrl:action ; sh:in _:a ] .\n_:a {rdf}first> ex:x ; {rdf}rest> _:a ."
    );
    let own = format!(
        "{head}ex:S sh:property ex:S ; sh:or ( ex:S ) ; sh:node ex:S ; sh:path odrl:rightOperand ."
    );
    for text in [
        cyclic,
        own,
        format!("{head}ex:S sh:in ( ex:a "),
        format!("{head}ex:S sh:property [ sh:path "),
    ] {
        let _ = ShapesGraph::parse(&text);
    }
    let many: String = (0..200_000).map(|i| format!("ex:v{i} ")).collect();
    let huge = format!("{head}ex:S sh:property [ sh:path odrl:action ; sh:in ( {many} ) ] .");
    assert!(ShapesGraph::parse(&huge).is_err());
}
