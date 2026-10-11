//! Strict validation, `odrl.*` tier: what the ODRL Information Model
//! itself requires of a policy's structure, after the policy-level
//! assigner, assignee, target and action are pushed down into the rules
//! (IM 2.7.1). Off unless `Options::structural(true)`.
mod common;
mod corpus;
use corpus::*;
use prose_core::profile::Wording;
use prose_core::validate::*;
use serde_json::{Value, json};

fn strict_text(name: &str) -> String {
    std::fs::read_to_string(format!("{}/strict/{name}", tests_dir())).unwrap()
}

fn structural() -> Options {
    Options::strict().structural(true)
}

/// (rule, target) of the findings of the `odrl.` tier, sorted.
fn odrl_findings(r: &Report) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = r
        .findings
        .iter()
        .filter(|f| f.rule.tier() == Tier::Odrl)
        .map(|f| (f.rule.to_string(), target_str(&f.target)))
        .collect();
    v.sort();
    v
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = list
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    v.sort();
    v
}

fn message(r: &Report, rule: RuleId, target: &str) -> String {
    r.findings
        .iter()
        .find(|f| f.rule == rule && target_str(&f.target) == target)
        .unwrap_or_else(|| panic!("no {rule} at {target}: {:?}", r.findings))
        .message
        .clone()
}

fn policy(v: Value) -> String {
    let mut p = json!({"@context": "http://www.w3.org/ns/odrl.jsonld"});
    for (k, x) in v.as_object().unwrap() {
        p[k] = x.clone();
    }
    p.to_string()
}

#[test]
fn hostile_odrl_rules() {
    hostile_trigger_only_their_rule(&[
        "odrl.policy-uid",
        "odrl.no-rule",
        "odrl.offer-assigner",
        "odrl.agreement-parties",
        "odrl.rule-target",
        "odrl.rule-action",
        "odrl.constraint-shape",
        "odrl.set-operator-cardinality",
        "odrl.consequence-nesting",
        "odrl.inheritance-cycle",
    ]);
}

#[test]
fn structural_bad() {
    let text = strict_text("structural-bad.json");
    let r = validate_json(&text, &Wording::new(), &structural());
    let p = "policy[0]";
    assert_eq!(
        odrl_findings(&r),
        pairs(&[
            ("odrl.policy-uid", p),
            ("odrl.offer-assigner", "policy[0].permission[0]"),
            ("odrl.offer-assigner", "policy[0].prohibition[0]"),
            ("odrl.offer-assigner", "policy[0].obligation[0]"),
            ("odrl.rule-target", "policy[0].permission[0]"),
            (
                "odrl.constraint-shape",
                "policy[0].permission[0].constraint[0]"
            ),
            (
                "odrl.set-operator-cardinality",
                "policy[0].permission[0].constraint[1]"
            ),
            (
                "odrl.constraint-shape",
                "policy[0].permission[0].constraint[2]"
            ),
            (
                "odrl.consequence-nesting",
                "policy[0].prohibition[0].remedy[0]"
            ),
            (
                "odrl.consequence-nesting",
                "policy[0].obligation[0].consequence[0]"
            ),
        ])
    );
    assert_eq!(r.verdict, Verdict::Violations);
    assert_eq!(
        message(&r, RuleId::OdrlPolicyUid, p),
        "the policy has no uid"
    );
    assert_eq!(
        message(&r, RuleId::OdrlOfferAssigner, "policy[0].permission[0]"),
        "this rule of an Offer has no assigner, on the rule or on the policy"
    );
    assert_eq!(
        message(&r, RuleId::OdrlRuleTarget, "policy[0].permission[0]"),
        "this permission has no target, on the rule or on the policy"
    );
    assert_eq!(
        message(
            &r,
            RuleId::OdrlConstraintShape,
            "policy[0].permission[0].constraint[0]"
        ),
        "this constraint has no operator"
    );
    assert_eq!(
        message(
            &r,
            RuleId::OdrlConstraintShape,
            "policy[0].permission[0].constraint[2]"
        ),
        "this constraint has both a rightOperand and a rightOperandReference"
    );
    assert_eq!(
        message(
            &r,
            RuleId::OdrlSetOperatorCardinality,
            "policy[0].permission[0].constraint[1]"
        ),
        "\"eq\" compares with one value, but this constraint gives 2; use isAnyOf, isAllOf or isNoneOf for several"
    );
    assert_eq!(
        message(
            &r,
            RuleId::OdrlConsequenceNesting,
            "policy[0].prohibition[0].remedy[0]"
        ),
        "this remedy has a consequence; a remedy must not refer to a duty that has one"
    );
    assert_eq!(
        message(
            &r,
            RuleId::OdrlConsequenceNesting,
            "policy[0].obligation[0].consequence[0]"
        ),
        "this consequence has a consequence of its own"
    );
    // Without the switch, strict says nothing about structure.
    let r = validate_json(&text, &Wording::new(), &Options::strict());
    assert!(odrl_findings(&r).is_empty());
    assert!(r.findings.iter().all(|f| f.rule.tier() != Tier::Odrl));
}

#[test]
fn policy_level_properties_push_down() {
    let text = policy(json!({
        "@type": "Offer", "uid": "urn:policy:o",
        "assigner": "urn:party:a", "target": "urn:asset:1", "action": "use",
        "permission": [{"constraint": [{"leftOperand": "count", "operator": "lt", "rightOperand": 3}]}],
        "prohibition": [{"action": "sell"}]
    }));
    let r = validate_json(&text, &Wording::new(), &structural());
    assert!(r.conforms(), "{:?}", r.findings);
    let text = policy(json!({
        "@type": "Agreement", "uid": "urn:policy:g", "assigner": "urn:party:a",
        "permission": [{"target": "urn:asset:1", "action": "use", "assignee": "urn:party:b"},
                       {"target": "urn:asset:1", "action": "read"}]
    }));
    let r = validate_json(&text, &Wording::new(), &structural());
    assert_eq!(
        odrl_findings(&r),
        pairs(&[("odrl.agreement-parties", "policy[0].permission[1]")])
    );
    assert_eq!(
        message(&r, RuleId::OdrlAgreementParties, "policy[0].permission[1]"),
        "this rule of an Agreement has no assignee, on the rule or on the policy"
    );
}

#[test]
fn duties_need_no_target_and_sets_need_no_parties() {
    let text = policy(json!({
        "@type": "Set", "uid": "urn:policy:s",
        "permission": [{"target": "urn:asset:1", "action": "distribute",
                        "duty": [{"action": "attribute",
                                  "consequence": [{"action": "compensate"}]}]}],
        "obligation": [{"action": "delete",
                        "consequence": [{"action": "inform"}]}]
    }));
    let r = validate_json(&text, &Wording::new(), &structural());
    assert!(r.conforms(), "{:?}", r.findings);
}

#[test]
fn set_operators_take_several_values() {
    let text = policy(json!({
        "@type": "Set", "uid": "urn:policy:s",
        "permission": [{"target": "urn:asset:1", "action": "use",
            "constraint": [{"leftOperand": "spatial", "operator": "isAnyOf",
                            "rightOperand": [{"@id": "urn:region:eu"}, {"@id": "urn:region:ch"}]},
                           {"leftOperand": "purpose", "operator": "isNoneOf",
                            "rightOperand": ["marketing", "profiling"]}]}]
    }));
    assert!(validate_json(&text, &Wording::new(), &structural()).conforms());
}

#[test]
fn a_policy_class_from_a_profile_is_checked_as_its_odrl_class() {
    let w = wording_of(&["research.jsonld"]);
    let text = policy(json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld",
                     {"ex": "https://example.org/odrl/profile/research/"}],
        "@type": "ex:ResearchAgreement", "uid": "urn:policy:r",
        "profile": "https://example.org/odrl/profile/research",
        "assigner": "urn:party:a",
        "permission": [{"target": "urn:asset:1", "action": "ex:train"}]
    }));
    let r = validate_json(&text, &w, &structural());
    assert_eq!(
        odrl_findings(&r),
        pairs(&[("odrl.agreement-parties", "policy[0].permission[0]")])
    );
}

#[test]
fn dsp_offer_no_assigner_silent_without_structural() {
    let text = strict_text("dsp-offer-no-assigner.json");
    let r = validate_json(&text, &Wording::new(), &Options::strict());
    assert!(r.conforms(), "{:?}", r.findings);
    let r = validate_json(&text, &Wording::new(), &structural());
    assert_eq!(
        odrl_findings(&r),
        pairs(&[
            ("odrl.offer-assigner", "policy[0].permission[0]"),
            ("odrl.rule-target", "policy[0].permission[0]"),
        ])
    );
}

#[test]
fn inheritance_cycle_in_the_document() {
    let text = strict_text("cycle.json");
    let r = validate_json(&text, &Wording::new(), &structural());
    assert_eq!(
        odrl_findings(&r),
        pairs(&[
            ("odrl.inheritance-cycle", "policy[0]"),
            ("odrl.inheritance-cycle", "policy[1]"),
        ])
    );
    assert_eq!(
        message(&r, RuleId::OdrlInheritanceCycle, "policy[0]"),
        "the policy <urn:policy:a> inherits from itself through inheritFrom"
    );
    // A parent outside the document cannot be checked: a gap.
    let gaps: Vec<(String, String)> = r
        .gaps
        .iter()
        .map(|f| (f.rule.to_string(), target_str(&f.target)))
        .collect();
    assert_eq!(
        gaps,
        [("input.unchecked".to_string(), "policy[2]".to_string())]
    );
    assert_eq!(
        r.gaps[0].message,
        "the parent policy <https://example.org/policies/remote-parent> is not in this document, so the inheritance could not be checked"
    );
    assert_eq!(r.verdict, Verdict::Violations);
    // Not structural: inheritFrom is not looked at.
    assert!(validate_json(&text, &Wording::new(), &Options::strict()).conforms());
}

#[test]
fn a_child_policy_may_take_all_its_rules_from_its_parent() {
    let text = json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@graph": [
            {"@type": "Set", "uid": "urn:policy:parent",
             "permission": [{"target": "urn:asset:1", "action": "use"}]},
            {"@type": "Set", "uid": "urn:policy:child", "inheritFrom": "urn:policy:parent"}
        ]
    })
    .to_string();
    let o = structural().level(RuleId::InputReaderNote, Level::Off);
    let r = validate_json(&text, &Wording::new(), &o);
    assert!(r.conforms(), "{:?}", r.findings);
}

#[test]
fn a_long_inheritance_chain_is_cheap_and_has_no_cycle() {
    let n = 5_000;
    let graph: Vec<Value> = (0..n)
        .map(|i| {
            let mut p = json!({"@type": "Set", "uid": format!("urn:policy:{i}"),
                               "permission": [{"target": "urn:asset:1", "action": "use"}]});
            if i > 0 {
                p["inheritFrom"] = json!(format!("urn:policy:{}", i - 1));
            }
            p
        })
        .collect();
    let mut text = json!({"@context": "http://www.w3.org/ns/odrl.jsonld", "@graph": graph});
    assert!(validate_json(&text.to_string(), &Wording::new(), &structural()).conforms());
    // Close the chain into one big circle: every policy is on it.
    text["@graph"][0]["inheritFrom"] = json!(format!("urn:policy:{}", n - 1));
    let r = validate_json(&text.to_string(), &Wording::new(), &structural());
    assert_eq!(r.findings.len(), n);
    assert!(
        r.findings
            .iter()
            .all(|f| f.rule == RuleId::OdrlInheritanceCycle)
    );
}
