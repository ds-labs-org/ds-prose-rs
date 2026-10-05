use prose_core::{ProseError, RuleKind, render};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[test]
fn offer_reads_as_permission_with_date_limit_and_duty() {
    let doc = render(&fixture("offer.json")).unwrap();
    let p = &doc.policies[0];
    assert_eq!(p.heading, "Offer http://example.com/policy:1010");
    assert_eq!(p.intro, "An offer made by http://example.com/org:abc.");
    let r = &p.rules[0];
    assert_eq!(r.kind, RuleKind::Permission);
    assert_eq!(
        r.sentence,
        "Anyone may display http://example.com/asset:9898.movie, as permitted by http://example.com/org:abc."
    );
    assert_eq!(
        r.conditions[0].text,
        "the date and time is before 2026-12-31"
    );
    let duty = &r.follow_ups[0].rules[0];
    assert_eq!(duty.kind, RuleKind::Obligation);
    assert!(
        duty.sentence
            .starts_with("The assignee must attribute http://example.com/asset:9898.movie")
    );
}

#[test]
fn agreement_reads_logic_refinements_and_lists() {
    let doc = render(&fixture("agreement.json")).unwrap();
    let p = &doc.policies[0];
    assert!(p.intro.contains("between http://example.com/party:alice (assigner) and http://example.com/party:bob (assignee)"));
    assert_eq!(
        p.notes,
        ["If a permission and a prohibition conflict, the prohibition wins."]
    );
    let r = &p.rules[0];
    assert!(
        r.sentence
            .starts_with("http://example.com/party:bob may distribute")
    );
    assert_eq!(
        r.refinements[0].children[0].text,
        "the number of times used is at most 5"
    );
    let or = &r.conditions[0];
    assert_eq!(or.text, "at least one of the following holds");
    assert_eq!(
        or.children[0].text,
        "the purpose is any of \"research\" or \"education\""
    );
    assert_eq!(
        or.children[1].text,
        "the location is equal to https://example.com/geo/EU"
    );
}

#[test]
fn set_inherits_target_and_reads_remedy() {
    let doc = render(&fixture("set.json")).unwrap();
    let r = &doc.policies[0].rules[0];
    assert_eq!(r.kind, RuleKind::Prohibition);
    assert_eq!(
        r.sentence,
        "No one may commercialize http://example.com/data:set1."
    );
    let remedy = &r.follow_ups[0].rules[0];
    assert_eq!(
        remedy.conditions[0].text,
        "the payment amount is equal to 100.00 euro"
    );
}

#[test]
fn graph_and_arrays_yield_several_policies() {
    let doc = render(r#"{"@graph":[{"@type":"Set","permission":{"action":"use","target":"x:a"}},{"@type":"Set","permission":{"action":"use","target":"x:b"}}]}"#).unwrap();
    assert_eq!(doc.policies.len(), 2);
}

#[test]
fn unknown_properties_are_reported_not_dropped_silently() {
    let doc =
        render(r#"{"@type":"Set","permission":{"action":"use","target":"x:a","frobnicate":1}}"#)
            .unwrap();
    assert!(doc.warnings.iter().any(|w| w.contains("frobnicate")));
}

#[test]
fn errors() {
    assert!(matches!(render("{"), Err(ProseError::Json(_))));
    assert_eq!(render(r#"{"hello":1}"#), Err(ProseError::NoPolicy));
}

#[test]
fn unknown_key_warnings_are_alphabetical_whatever_the_document_order() {
    let doc = render(
        r#"{"@type":"Set","uid":"urn:p","zz:b":1,"aa:a":2,"permission":[{"action":"use","target":"urn:t"}]}"#,
    )
    .unwrap();
    let aa = doc
        .warnings
        .iter()
        .position(|w| w.contains("\"aa:a\""))
        .unwrap();
    let zz = doc
        .warnings
        .iter()
        .position(|w| w.contains("\"zz:b\""))
        .unwrap();
    assert!(aa < zz, "{:?}", doc.warnings);
}
