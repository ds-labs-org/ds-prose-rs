//! The rules a dataspace asks of a policy beyond its vocabulary: a constraint
//! that is finished, no group of one, no action listed twice. See the
//! `odrl.constraint-incomplete`, `heuristic.single-child-group` and
//! `heuristic.duplicate-action` rules.
mod common;
mod corpus;
use corpus::*;
use prose_core::edit::*;
use prose_core::profile::Wording;
use prose_core::validate::*;
use serde_json::{Value, json};

fn policy(rule: Value) -> String {
    json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld", "@type": "Set", "uid": "urn:p",
        "permission": [rule]
    })
    .to_string()
}

fn constraint(c: Value) -> String {
    policy(json!({"target": "urn:a", "action": "use", "constraint": [c]}))
}

fn run(text: &str, o: &Options) -> Report {
    validate_json(text, &Wording::default(), o)
}

fn rules_of(r: &Report) -> Vec<(String, String)> {
    // What the reader noted about a missing part is `input.reader-note`'s.
    r.findings
        .iter()
        .filter(|f| f.rule != RuleId::InputReaderNote)
        .map(|f| (f.rule.to_string(), target_str(&f.target)))
        .collect()
}

fn finding(r: &Report, rule: RuleId) -> &Finding {
    r.findings.iter().find(|f| f.rule == rule).unwrap()
}

const C: &str = "policy[0].permission[0].constraint[0]";

fn warn(rule: RuleId) -> Options {
    Options::strict().level(rule, Level::Warning)
}

// --- catalogue ---------------------------------------------------------------

#[test]
fn the_new_rules_are_in_the_catalogue_from_ruleset_2() {
    assert_eq!(RULESET, 2);
    for (id, tier, basis, advisory, strict) in [
        (
            "odrl.constraint-incomplete",
            Tier::Odrl,
            Basis::Standard,
            Level::Off,
            Level::Error,
        ),
        (
            "heuristic.single-child-group",
            Tier::Heuristic,
            Basis::Heuristic,
            Level::Off,
            Level::Off,
        ),
        (
            "heuristic.duplicate-action",
            Tier::Heuristic,
            Basis::Heuristic,
            Level::Off,
            Level::Off,
        ),
    ] {
        let rule: RuleId = id.parse().unwrap();
        let info = rule.info();
        assert_eq!((info.tier, info.basis), (tier, basis), "{id}");
        assert_eq!((info.advisory, info.strict), (advisory, strict), "{id}");
        assert_eq!(info.since_ruleset, 2, "{id}");
        assert!(!info.gap && !info.title.is_empty() && !info.source.is_empty());
    }
}

#[test]
fn a_pinned_ruleset_1_does_not_see_the_new_rules() {
    let text = constraint(json!({"leftOperand": "", "operator": "eq", "rightOperand": ""}));
    assert!(!run(&text, &Options::strict()).conforms());
    assert!(run(&text, &Options::strict().ruleset(1)).conforms());
}

// --- odrl.constraint-incomplete -------------------------------------------------

#[test]
fn an_unfinished_constraint_is_an_error_in_strict_mode_alone() {
    for c in [
        json!({"leftOperand": "", "operator": "eq", "rightOperand": "x"}),
        json!({"operator": "eq", "rightOperand": "x"}),
        json!({"leftOperand": "count", "operator": "", "rightOperand": "x"}),
        json!({"leftOperand": "count", "rightOperand": "x"}),
        json!({"leftOperand": "count", "operator": "eq", "rightOperand": ""}),
        json!({"leftOperand": "count", "operator": "eq", "rightOperand": "  "}),
        json!({"leftOperand": "count", "operator": "eq"}),
        json!({"leftOperand": "count", "operator": "isAnyOf", "rightOperand": ["a", ""]}),
        json!({"leftOperand": "", "operator": "eq", "rightOperand": ""}),
    ] {
        let text = constraint(c.clone());
        let r = run(&text, &Options::strict());
        assert_eq!(
            rules_of(&r),
            [("odrl.constraint-incomplete".to_string(), C.to_string())],
            "{c}"
        );
        let f = finding(&r, RuleId::OdrlConstraintIncomplete);
        assert_eq!(f.severity, Severity::Error, "{c}");
        assert_eq!(r.verdict, Verdict::Violations, "{c}");
        // Not in the advisory reading.
        assert!(run(&text, &Options::advisory()).findings.is_empty(), "{c}");
        // Switched off by level.
        let off = Options::strict().level(RuleId::OdrlConstraintIncomplete, Level::Off);
        assert!(rules_of(&run(&text, &off)).is_empty(), "{c}");
    }
}

#[test]
fn the_message_names_what_is_missing() {
    let text = constraint(json!({"leftOperand": "", "operator": "eq", "rightOperand": ""}));
    let r = run(&text, &Options::strict());
    assert_eq!(
        finding(&r, RuleId::OdrlConstraintIncomplete).message,
        "this constraint is not finished: it has no left operand and an empty right operand"
    );
    let text = constraint(json!({"leftOperand": "count", "operator": ""}));
    let r = run(&text, &Options::strict());
    assert_eq!(
        finding(&r, RuleId::OdrlConstraintIncomplete).message,
        "this constraint is not finished: it has no operator and no right operand"
    );
}

#[test]
fn a_finished_constraint_is_clean() {
    for c in [
        json!({"leftOperand": "count", "operator": "lteq", "rightOperand": 3}),
        json!({"leftOperand": "count", "operator": "eq", "rightOperand": false}),
        json!({"leftOperand": "count", "operator": "eq", "rightOperand": 0}),
        json!({"leftOperand": "count", "operator": "eq",
               "rightOperandReference": "https://example.net/v"}),
        json!({"and": [{"leftOperand": "count", "operator": "eq", "rightOperand": 1}]}),
    ] {
        let r = run(&constraint(c.clone()), &Options::strict());
        assert!(
            r.findings
                .iter()
                .all(|f| f.rule != RuleId::OdrlConstraintIncomplete),
            "{c}"
        );
    }
}

#[test]
fn it_is_found_inside_groups_and_refinements() {
    let text = constraint(json!({"or": [
        {"leftOperand": "count", "operator": "eq", "rightOperand": 1},
        {"leftOperand": "", "operator": "eq", "rightOperand": 1}
    ]}));
    let r = run(&text, &Options::strict());
    assert_eq!(
        rules_of(&r),
        [(
            "odrl.constraint-incomplete".to_string(),
            format!("{C}.child[1]")
        )]
    );
    let text = policy(json!({"target": "urn:a",
        "action": {"rdf:value": {"@id": "use"},
                   "refinement": [{"leftOperand": "count", "operator": "eq", "rightOperand": ""}]}}));
    let r = run(&text, &Options::strict());
    assert_eq!(
        rules_of(&r),
        [(
            "odrl.constraint-incomplete".to_string(),
            "policy[0].permission[0].action[0].refinement[0]".to_string()
        )]
    );
}

#[test]
fn with_the_structural_tier_on_the_shape_rule_speaks_first() {
    // A constraint that lacks a part is reported once: by constraint-shape
    // when the structural tier is on, else by constraint-incomplete. An empty
    // value is not a shape fault, so it is still constraint-incomplete.
    let o = Options::strict().structural(true);
    let r = run(
        &constraint(json!({"operator": "eq", "rightOperand": "x"})),
        &o,
    );
    assert_eq!(
        rules_of(&r),
        [("odrl.constraint-shape".to_string(), C.to_string())]
    );
    let r = run(
        &constraint(json!({"leftOperand": "count", "operator": "eq", "rightOperand": ""})),
        &o,
    );
    assert_eq!(
        rules_of(&r),
        [("odrl.constraint-incomplete".to_string(), C.to_string())]
    );
}

#[test]
fn it_is_not_refused_by_the_gate_and_has_no_fix() {
    let text = constraint(json!({"leftOperand": "", "operator": "eq", "rightOperand": ""}));
    let r = run(&text, &Options::strict());
    assert!(!RuleId::OdrlConstraintIncomplete.info().refusable);
    let f = finding(&r, RuleId::OdrlConstraintIncomplete);
    assert!(f.fixes.is_empty());
    assert_eq!(
        f.node,
        Some(
            read_model(&text)
                .unwrap()
                .node(&C.parse().unwrap())
                .unwrap()
                .id()
        )
    );
}

// --- heuristic.single-child-group -----------------------------------------------

fn group(op: &str, kids: Vec<Value>) -> Value {
    json!({ op: kids })
}

fn atom(n: u32) -> Value {
    json!({"leftOperand": "count", "operator": "eq", "rightOperand": n})
}

#[test]
fn a_group_of_one_is_opt_in() {
    let text = constraint(group("and", vec![atom(1)]));
    assert!(run(&text, &Options::strict()).conforms());
    let r = run(&text, &warn(RuleId::HeuristicSingleChildGroup));
    assert_eq!(
        rules_of(&r),
        [("heuristic.single-child-group".to_string(), C.to_string())]
    );
    assert_eq!(r.findings[0].severity, Severity::Warning);
    assert_eq!(
        r.findings[0].message,
        "this \"and\" group holds one condition; the group adds nothing"
    );
}

#[test]
fn groups_of_two_are_not_reported_and_nesting_is() {
    let o = warn(RuleId::HeuristicSingleChildGroup);
    let two = constraint(group("or", vec![atom(1), atom(2)]));
    assert!(run(&two, &o).conforms());
    let nested = constraint(group("and", vec![group("xone", vec![atom(1)])]));
    let r = run(&nested, &o);
    assert_eq!(
        rules_of(&r),
        [
            ("heuristic.single-child-group".to_string(), C.to_string()),
            (
                "heuristic.single-child-group".to_string(),
                format!("{C}.child[0]")
            ),
        ]
    );
}

#[test]
fn the_ungroup_fix_replaces_the_group_by_its_condition() {
    let text = constraint(group("and", vec![atom(7)]));
    let r = run(&text, &warn(RuleId::HeuristicSingleChildGroup));
    let fix = &r.findings[0].fixes[0];
    assert_eq!(fix.label, "Ungroup");
    assert!(fix.safe);
    let mut doc = read_model(&text).unwrap();
    for ev in &fix.events {
        doc.apply(ev, &EditRules::default()).unwrap();
    }
    let out: Value = write_jsonld(&doc);
    assert_eq!(out["permission"][0]["constraint"][0]["rightOperand"], 7);
    assert!(out["permission"][0]["constraint"][0].get("and").is_none());
    // The fixed policy is clean.
    let after = write_jsonld(&doc).to_string();
    assert!(run(&after, &warn(RuleId::HeuristicSingleChildGroup)).conforms());
}

// --- heuristic.duplicate-action --------------------------------------------------

fn rule_with_actions(actions: Value) -> String {
    policy(json!({"target": "urn:a", "action": actions}))
}

#[test]
fn an_action_listed_twice_in_a_rule_is_opt_in() {
    let text = rule_with_actions(json!(["use", "read", "use"]));
    assert!(run(&text, &Options::strict()).conforms());
    let r = run(&text, &warn(RuleId::HeuristicDuplicateAction));
    assert_eq!(
        rules_of(&r),
        [(
            "heuristic.duplicate-action".to_string(),
            "policy[0].permission[0].action[2]#name".to_string()
        )]
    );
    assert_eq!(
        r.findings[0].message,
        "the action \"use\" is listed twice in this rule"
    );
}

#[test]
fn the_same_action_in_another_spelling_is_the_same_action() {
    let text = rule_with_actions(json!([
        "use",
        "odrl:use",
        "http://www.w3.org/ns/odrl/2/use"
    ]));
    let r = run(&text, &warn(RuleId::HeuristicDuplicateAction));
    assert_eq!(r.findings.len(), 2, "{:?}", r.findings);
    // Different actions, or the same one in two rules, are fine.
    let text = json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld", "@type": "Set", "uid": "urn:p",
        "permission": [{"target": "urn:a", "action": ["use", "read"]},
                       {"target": "urn:a", "action": "use"}]
    })
    .to_string();
    assert!(run(&text, &warn(RuleId::HeuristicDuplicateAction)).conforms());
}

#[test]
fn the_duplicate_action_fix_removes_the_second() {
    let text = rule_with_actions(json!(["use", "read", "use"]));
    let r = run(&text, &warn(RuleId::HeuristicDuplicateAction));
    let fix = &r.findings[0].fixes[0];
    assert_eq!(fix.label, "Remove the duplicate");
    assert!(fix.safe);
    let mut doc = read_model(&text).unwrap();
    for ev in &fix.events {
        doc.apply(ev, &EditRules::default()).unwrap();
    }
    let out: Value = write_jsonld(&doc);
    assert_eq!(out["permission"][0]["action"], json!(["use", "read"]));
}

// --- report ----------------------------------------------------------------------

#[test]
fn the_new_rules_appear_in_the_report_json() {
    let text =
        constraint(json!({"and": [{"leftOperand": "", "operator": "eq", "rightOperand": ""}]}));
    let o = warn(RuleId::HeuristicSingleChildGroup);
    let j = run(&text, &o).to_json();
    let rules: Vec<(&str, &str, &str)> = j["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["rule"].as_str().unwrap(),
                f["tier"].as_str().unwrap(),
                f["basis"].as_str().unwrap(),
            )
        })
        .collect();
    assert!(
        rules.contains(&("odrl.constraint-incomplete", "odrl", "standard")),
        "{rules:?}"
    );
    assert!(
        rules.contains(&("heuristic.single-child-group", "heuristic", "heuristic")),
        "{rules:?}"
    );
    assert_eq!(j["ruleset"], 2);
    assert_eq!(j["verdict"], "violations");
}

#[test]
fn hostile_cases_of_the_new_rules() {
    hostile_trigger_only_their_rule(&[
        "odrl.constraint-incomplete",
        "heuristic.single-child-group",
        "heuristic.duplicate-action",
    ]);
}

#[test]
fn three_missing_parts_read_as_a_list() {
    let text = constraint(json!({}));
    let r = run(&text, &Options::strict());
    assert_eq!(
        finding(&r, RuleId::OdrlConstraintIncomplete).message,
        "this constraint is not finished: it has no left operand, no operator and no right operand"
    );
}

#[test]
fn a_constraint_under_keys_the_reader_skipped_is_not_called_unfinished() {
    // `odrl:leftOperand` is not read, so the constraint looks bare: that is a
    // gap (input.unchecked), not an unfinished constraint.
    let text = json!({
        "@context": {"odrl": "http://www.w3.org/ns/odrl/2/"}, "@type": "Set", "uid": "urn:p",
        "permission": [{"target": "urn:a", "action": "use", "constraint": [
            {"odrl:leftOperand": "count", "odrl:operator": "eq", "odrl:rightOperand": 1}]}]
    })
    .to_string();
    let r = run(&text, &Options::strict());
    assert!(
        r.findings
            .iter()
            .all(|f| f.rule != RuleId::OdrlConstraintIncomplete)
    );
    assert_eq!(r.verdict, Verdict::Incomplete);
}

// --- audit: per-constraint unread gate, duplicate actions with refinements --------

fn strict_incomplete() -> Options {
    Options::strict()
}

#[test]
fn an_unread_key_elsewhere_does_not_hide_an_incomplete_constraint() {
    let text = json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld", "@type": "Set", "uid": "urn:p",
        "odrl:profile": "urn:x",
        "permission": [{"target": "urn:a", "action": "use", "constraint": [{}]}]
    })
    .to_string();
    let r = run(&text, &strict_incomplete());
    assert!(
        r.findings
            .iter()
            .any(|f| f.rule == RuleId::OdrlConstraintIncomplete),
        "{:?}",
        r.findings
    );
}

#[test]
fn an_unread_key_inside_the_constraint_still_hides_it() {
    let text = constraint(json!({"odrl:leftOperand": "count", "operator": "lt"}));
    let r = run(&text, &strict_incomplete());
    assert!(
        !r.findings
            .iter()
            .any(|f| f.rule == RuleId::OdrlConstraintIncomplete),
        "{:?}",
        r.findings
    );
}

#[test]
fn a_duplicate_action_with_different_refinements_has_no_fix() {
    let text = rule_with_actions(json!([
        "use",
        {"rdf:value": {"@id": "odrl:use"}, "refinement": [
            {"leftOperand": "count", "operator": "lt", "rightOperand": 5}]}
    ]));
    let r = run(&text, &warn(RuleId::HeuristicDuplicateAction));
    let dup: Vec<_> = r
        .findings
        .iter()
        .filter(|f| f.rule == RuleId::HeuristicDuplicateAction)
        .collect();
    assert_eq!(dup.len(), 1, "{:?}", r.findings);
    assert!(dup[0].fixes.iter().all(|f| !f.safe), "{:?}", dup[0].fixes);
}

#[test]
fn a_duplicate_action_with_equal_refinements_keeps_the_safe_fix() {
    let a = json!({"rdf:value": {"@id": "odrl:use"}, "refinement": [
        {"leftOperand": "count", "operator": "lt", "rightOperand": 5}]});
    let text = rule_with_actions(json!([a.clone(), a]));
    let r = run(&text, &warn(RuleId::HeuristicDuplicateAction));
    let f = finding(&r, RuleId::HeuristicDuplicateAction);
    assert!(f.fixes.iter().any(|f| f.safe), "{:?}", f.fixes);
}
