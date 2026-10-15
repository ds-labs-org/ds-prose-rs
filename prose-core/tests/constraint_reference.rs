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

/// A constraint the document does not declare reads as defined at its IRI,
/// and the condition carries that IRI so a host can link it (the reader
/// itself never fetches anything).
#[test]
fn a_reference_to_nothing_declared_reads_as_a_reference() {
    for reference in [r#""urn:c:elsewhere""#, r#"{"@id": "urn:c:elsewhere"}"#] {
        let doc = render(&with_declared(reference, PURPOSE)).unwrap();
        let read = &doc.policies[0].rules[0].conditions;
        assert_eq!(
            texts(read),
            ["the constraint defined at urn:c:elsewhere holds"],
            "{reference}"
        );
        assert_eq!(read[0].reference.as_deref(), Some("urn:c:elsewhere"));
        assert!(read[0].children.is_empty());
        assert_eq!(doc.warnings, Vec::<String>::new(), "{reference}");
    }
}

#[test]
fn a_reference_inside_a_logical_constraint_keeps_its_iri() {
    let doc = render(&with_declared(
        r#"{"and": [{"@id": "https://example.com/c/1"}, {"@id": "urn:c:purpose"}]}"#,
        PURPOSE,
    ))
    .unwrap();
    let and = &doc.policies[0].rules[0].conditions[0];
    assert_eq!(and.reference, None);
    assert_eq!(
        and.children[0].text,
        "the constraint defined at https://example.com/c/1 holds"
    );
    assert_eq!(
        and.children[0].reference.as_deref(),
        Some("https://example.com/c/1")
    );
    assert_eq!(
        and.children[1].reference, None,
        "declared: read, not referred to"
    );
}

#[test]
fn a_constraint_declared_in_the_document_carries_no_reference() {
    let doc = render(&with_declared(r#""urn:c:purpose""#, PURPOSE)).unwrap();
    assert_eq!(doc.policies[0].rules[0].conditions[0].reference, None);
    let doc = render(EG19).unwrap();
    let xone = &doc.policies[0].rules[0].refinements[0].children[0];
    assert!(xone.children.iter().all(|c| c.reference.is_none()));
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
    assert_eq!(
        and.children[0].reference, None,
        "declared here, not elsewhere"
    );
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

fn count(c: &[Condition]) -> usize {
    c.iter().map(|c| 1 + count(&c.children)).sum()
}

/// Each constraint refers to the next one twice: read in full, forty of
/// them would be 2^40 clauses. Following references is bounded per
/// document, and a reference not followed reads by its identifier.
#[test]
fn references_that_multiply_are_read_up_to_a_bound() {
    let mut declared: Vec<String> = (0..40)
        .map(|i| {
            format!(
                r#"{{"@type": "LogicalConstraint", "@id": "urn:c:{i}",
                    "and": [{{"@id": "urn:c:{n}"}}, {{"@id": "urn:c:{n}"}}]}}"#,
                n = i + 1
            )
        })
        .collect();
    declared.push(
        r#"{"@type": "Constraint", "@id": "urn:c:40", "leftOperand": "count",
            "operator": "lteq", "rightOperand": 1}"#
            .into(),
    );
    let doc = render(&with_declared(r#""urn:c:0""#, &declared.join(","))).unwrap();
    let read = &doc.policies[0].rules[0].conditions;
    assert!(count(read) < 10_000, "{} clauses", count(read));
    assert!(
        doc.warnings
            .iter()
            .any(|w| w.contains("constraint references")),
        "{:?}",
        doc.warnings
    );
}

// --- #41 review: where a constraint is declared, and how it is named -------

/// A node object with `@id` declares that node wherever it appears: a
/// constraint written inline in one rule is declared for every other rule
/// that refers to it.
#[test]
fn a_constraint_declared_inline_is_found_from_another_rule() {
    let doc = render(
        r#"{"@context": "http://www.w3.org/ns/odrl.jsonld", "@type": "Set", "uid": "urn:p",
            "permission": [
              {"action": "use", "target": "urn:a", "constraint": [{"@id": "http://ex.com/c1",
                "leftOperand": "purpose", "operator": "eq", "rightOperand": "research"}]},
              {"action": "read", "target": "urn:a", "constraint": "http://ex.com/c1"}
            ]}"#,
    )
    .unwrap();
    let rules = &doc.policies[0].rules;
    assert_eq!(texts(&rules[0].conditions), texts(&rules[1].conditions));
    assert!(rules[1].conditions[0].text.contains("research"));
    assert_eq!(rules[1].conditions[0].reference, None);
    assert_eq!(doc.warnings, Vec::<String>::new());
}

/// A policy whose permission refers to `constraint`, beside `declared`,
/// under a context that binds `ex:`.
fn with_prefix(constraint: &str, declared: &str) -> String {
    format!(
        r#"{{"@context": ["http://www.w3.org/ns/odrl.jsonld", {{"ex": "http://ex.com/"}}], "@graph": [
            {{"@type": "Set", "uid": "urn:p",
              "permission": [{{"action": "use", "target": "urn:a", "constraint": {constraint}}}]}},
            {{"@type": "Constraint", "@id": "{declared}", "leftOperand": "purpose",
              "operator": "eq", "rightOperand": "research"}}
        ]}}"#
    )
}

/// `ex:c1` and `http://ex.com/c1` are one IRI under a context that binds
/// `ex`: either spelling finds the constraint declared with the other.
#[test]
fn a_compact_reference_finds_the_constraint_declared_by_its_full_iri() {
    for (refer, declared) in [
        (r#""ex:c1""#, "http://ex.com/c1"),
        (r#""http://ex.com/c1""#, "ex:c1"),
        (r#"{"@id": "ex:c1"}"#, "http://ex.com/c1"),
    ] {
        let doc = render(&with_prefix(refer, declared)).unwrap();
        let read = &doc.policies[0].rules[0].conditions;
        assert_eq!(
            texts(read),
            [r#"the purpose is equal to "research""#],
            "{refer} -> {declared}"
        );
        assert_eq!(read[0].reference, None, "{refer} -> {declared}");
        assert_eq!(doc.warnings, Vec::<String>::new(), "{refer} -> {declared}");
    }
}

/// A compact reference to a constraint the document does not declare names
/// the IRI it expands to, which a host can check and link.
#[test]
fn an_undeclared_compact_reference_names_its_full_iri() {
    let doc = render(&with_prefix(r#""ex:c9""#, "http://ex.com/c1")).unwrap();
    let read = &doc.policies[0].rules[0].conditions;
    assert_eq!(
        texts(read),
        ["the constraint defined at http://ex.com/c9 holds"]
    );
    assert_eq!(read[0].reference.as_deref(), Some("http://ex.com/c9"));
}
