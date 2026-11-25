//! A rule sentence names its assigner by role: "as permitted by the assigner
//! X", "the assigners X and Y". Policy intro sentences are not touched.
use prose_core::edit::{Segment, read_model, sentences};
use prose_core::render;
use serde_json::{Value, json};

fn policy(kind: &str, rule: Value, top: Value) -> String {
    let mut p = json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Offer",
        "uid": "urn:p",
        kind: [rule],
    });
    for (k, v) in top.as_object().unwrap() {
        p[k] = v.clone();
    }
    p.to_string()
}

fn first_rule_sentence(json: &str) -> String {
    render(json).unwrap().policies[0].rules[0].sentence.clone()
}

#[test]
fn one_assigner_is_named_by_role() {
    let perm = policy(
        "permission",
        json!({"action": "use", "target": "urn:t", "assigner": "urn:a"}),
        json!({}),
    );
    assert_eq!(
        first_rule_sentence(&perm),
        "Anyone may use urn:t, as permitted by the assigner urn:a."
    );
    let proh = policy(
        "prohibition",
        json!({"action": "use", "target": "urn:t", "assigner": "urn:a"}),
        json!({}),
    );
    assert_eq!(
        first_rule_sentence(&proh),
        "No one may use urn:t, as prohibited by the assigner urn:a."
    );
    let proh_to = policy(
        "prohibition",
        json!({"action": "use", "target": "urn:t", "assigner": "urn:a", "assignee": "urn:b"}),
        json!({}),
    );
    assert_eq!(
        first_rule_sentence(&proh_to),
        "urn:b must not use urn:t, as prohibited by the assigner urn:a."
    );
    let obl = policy(
        "obligation",
        json!({"action": "delete", "target": "urn:t", "assigner": "urn:a"}),
        json!({}),
    );
    assert_eq!(
        first_rule_sentence(&obl),
        "The assignee must delete urn:t, as required by the assigner urn:a."
    );
}

#[test]
fn several_assigners_are_the_assigners() {
    let perm = policy(
        "permission",
        json!({"action": "use", "target": "urn:t", "assigner": ["urn:a", "urn:b"]}),
        json!({}),
    );
    assert_eq!(
        first_rule_sentence(&perm),
        "Anyone may use urn:t, as permitted by the assigners urn:a and urn:b."
    );
}

#[test]
fn an_inherited_assigner_is_named_by_role() {
    let one = policy(
        "permission",
        json!({"action": "use", "target": "urn:t"}),
        json!({"assigner": "urn:a"}),
    );
    assert_eq!(
        first_rule_sentence(&one),
        "Anyone may use urn:t, as permitted by the assigner urn:a."
    );
    let two = policy(
        "permission",
        json!({"action": "use", "target": "urn:t"}),
        json!({"assigner": ["urn:a", "urn:b"]}),
    );
    assert_eq!(
        first_rule_sentence(&two),
        "Anyone may use urn:t, as permitted by the assigners urn:a and urn:b."
    );
}

#[test]
fn no_assigner_no_clause() {
    let perm = policy(
        "permission",
        json!({"action": "use", "target": "urn:t"}),
        json!({}),
    );
    assert_eq!(first_rule_sentence(&perm), "Anyone may use urn:t.");
}

#[test]
fn the_intro_is_unchanged() {
    let perm = policy(
        "permission",
        json!({"action": "use", "target": "urn:t"}),
        json!({"assigner": "urn:a"}),
    );
    assert_eq!(
        render(&perm).unwrap().policies[0].intro,
        "An offer made by urn:a."
    );
}

fn assigner_lead(json: &str) -> (String, Option<String>) {
    let model = read_model(json).unwrap();
    let rule = &sentences(&model)[0].rules[0];
    let lead = rule.sentence.segments.iter().find_map(|s| match s {
        Segment::List(l) if l.lead.as_deref().is_some_and(|x| x.starts_with(", as")) => {
            l.lead.clone()
        }
        _ => None,
    });
    (rule.sentence.plain(), lead)
}

#[test]
fn editor_lead_names_the_role() {
    let cases = [
        (json!({"assigner": "urn:a"}), json!({}), "assigner"),
        (
            json!({"assigner": ["urn:a", "urn:b"]}),
            json!({}),
            "assigners",
        ),
        (json!({}), json!({"assigner": "urn:a"}), "assigner"),
        (
            json!({}),
            json!({"assigner": ["urn:a", "urn:b"]}),
            "assigners",
        ),
    ];
    for (own, top, word) in cases {
        let mut rule = json!({"action": "use", "target": "urn:t"});
        for (k, v) in own.as_object().unwrap() {
            rule[k] = v.clone();
        }
        let src = policy("permission", rule, top);
        let (plain, lead) = assigner_lead(&src);
        assert_eq!(plain, first_rule_sentence(&src));
        assert_eq!(lead, Some(format!(", as permitted by the {word} ")));
    }
}
