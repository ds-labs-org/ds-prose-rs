//! A constraint may be declared once, as its own node with an identifier,
//! and referred to by that identifier from a rule or a logical constraint
//! (ODRL Information Model, Example 19). The reading looks it up.
use prose_core::{Condition, render};

/// Example 19 of the ODRL Information Model, as the catalog reads it: the
/// policy and the two constraints it refers to, in one `@graph`.
const EG19: &str = r#"{
  "@context": "http://www.w3.org/ns/odrl.jsonld",
  "@graph": [
    {
      "@type": "Offer",
      "uid": "http://example.com/policy:88",
      "profile": "http://example.com/odrl:profile:10",
      "permission": [{
        "target": "http://example.com/book/1999",
        "assigner": "http://example.com/org/paisley-park",
        "action": [{
          "rdf:value": { "@id": "odrl:reproduce" },
          "refinement": {
            "xone": { "@list": [
              { "@id": "http://example.com/p:88/C1" },
              { "@id": "http://example.com/p:88/C2" }
            ] }
          }
        }]
      }]
    },
    {
      "@type": "Constraint",
      "uid": "http://example.com/p:88/C1",
      "leftOperand": "media",
      "operator": "eq",
      "rightOperand": { "@value": "online", "@type": "xsd:string" }
    },
    {
      "@type": "Constraint",
      "uid": "http://example.com/p:88/C2",
      "leftOperand": "media",
      "operator": "eq",
      "rightOperand": { "@value": "print", "@type": "xsd:string" }
    }
  ]
}"#;

fn texts(c: &[Condition]) -> Vec<String> {
    c.iter().map(|c| c.text.clone()).collect()
}

/// A policy whose one permission has `constraint`, beside `declared`.
fn with_declared(constraint: &str, declared: &str) -> String {
    format!(
        r#"{{"@context": "http://www.w3.org/ns/odrl.jsonld", "@graph": [
            {{"@type": "Set", "uid": "urn:p",
              "permission": [{{"action": "use", "target": "urn:a", "constraint": {constraint}}}]}},
            {declared}
        ]}}"#
    )
}

const PURPOSE: &str = r#"{"@type": "Constraint", "@id": "urn:c:purpose",
    "leftOperand": "purpose", "operator": "eq", "rightOperand": "research"}"#;

#[test]
fn example_19_reads_the_constraints_it_refers_to() {
    let doc = render(EG19).unwrap();
    assert_eq!(doc.policies.len(), 1);
    let refine = &doc.policies[0].rules[0].refinements;
    assert_eq!(refine.len(), 1);
    assert_eq!(refine[0].children.len(), 1);
    let xone = &refine[0].children[0];
    assert_eq!(xone.text, "exactly one of the following holds");
    let read = texts(&xone.children);
    assert_eq!(read.len(), 2);
    assert!(read[0].contains("online"), "{read:?}");
    assert!(read[1].contains("print"), "{read:?}");
    assert!(read.iter().all(|t| !t.contains("incomplete")), "{read:?}");
    assert_eq!(
        doc.warnings,
        Vec::<String>::new(),
        "the declared constraints are read, not skipped"
    );
}

#[test]
fn a_reference_reads_as_the_constraint_it_names() {
    let inline = render(&with_declared(
        r#"{"leftOperand": "purpose", "operator": "eq", "rightOperand": "research"}"#,
        r#"{"@type": "Constraint", "@id": "urn:c:unused", "leftOperand": "count",
            "operator": "lteq", "rightOperand": 1}"#,
    ))
    .unwrap();
    let want = texts(&inline.policies[0].rules[0].conditions);
    for reference in [
        r#"[{"@id": "urn:c:purpose"}]"#,
        r#"{"@id": "urn:c:purpose"}"#,
        r#""urn:c:purpose""#,
    ] {
        let doc = render(&with_declared(reference, PURPOSE)).unwrap();
        assert_eq!(
            texts(&doc.policies[0].rules[0].conditions),
            want,
            "{reference}"
        );
        assert_eq!(doc.warnings, Vec::<String>::new(), "{reference}");
    }
}

#[test]
fn a_reference_inside_a_logical_constraint_is_read() {
    let doc = render(&with_declared(
        r#"{"or": [{"@id": "urn:c:purpose"}, {"leftOperand": "count", "operator": "lteq", "rightOperand": 1}]}"#,
        PURPOSE,
    ))
    .unwrap();
    let or = &doc.policies[0].rules[0].conditions[0];
    assert_eq!(or.text, "at least one of the following holds");
    assert!(or.children[0].text.contains("research"), "{or:?}");
}

#[test]
fn a_declared_logical_constraint_is_read_with_its_operands() {
    let doc = render(&with_declared(
        r#""urn:c:either""#,
        &format!(
            r#"{{"@type": "LogicalConstraint", "@id": "urn:c:either",
                "or": [{{"@id": "urn:c:purpose"}}]}}, {PURPOSE}"#
        ),
    ))
    .unwrap();
    let or = &doc.policies[0].rules[0].conditions[0];
    assert_eq!(or.text, "at least one of the following holds");
    assert!(or.children[0].text.contains("research"), "{or:?}");
    assert_eq!(doc.warnings, Vec::<String>::new());
}

#[test]
fn a_reference_to_nothing_declared_reads_as_a_reference() {
    for reference in [r#""urn:c:elsewhere""#, r#"{"@id": "urn:c:elsewhere"}"#] {
        let doc = render(&with_declared(reference, PURPOSE)).unwrap();
        assert_eq!(
            texts(&doc.policies[0].rules[0].conditions),
            ["the constraint urn:c:elsewhere holds"],
            "{reference}"
        );
    }
}

#[test]
fn a_constraint_that_refers_to_itself_ends() {
    let doc = render(&with_declared(
        r#""urn:c:loop""#,
        r#"{"@type": "LogicalConstraint", "@id": "urn:c:loop", "and": [{"@id": "urn:c:loop"}]}"#,
    ))
    .unwrap();
    let and = &doc.policies[0].rules[0].conditions[0];
    assert_eq!(and.text, "all of the following hold");
    assert_eq!(texts(&and.children), ["the constraint urn:c:loop holds"]);
}

#[test]
fn a_node_that_is_neither_policy_nor_constraint_is_still_skipped() {
    let doc = render(&with_declared(
        r#""urn:c:purpose""#,
        r#"{"@id": "urn:x", "foo": 1}"#,
    ))
    .unwrap();
    assert!(
        doc.warnings
            .iter()
            .any(|w| w == "skipped a node that is not an ODRL policy"),
        "{:?}",
        doc.warnings
    );
}
