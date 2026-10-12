//! Strict validation, `profile.*` tier: ODRL plus the loaded profiles is
//! the whole vocabulary, so a term that resolves nowhere, or resolves to
//! the wrong kind of term, or comes from a profile the policy does not
//! declare, is an error.
mod common;
mod corpus;
use corpus::*;
use prose_core::edit::*;
use prose_core::profile::{OdrlVocab, ProfileLimits, ProfileVocab, TermKind, Wording};
use prose_core::validate::*;
use serde_json::{Value, json};

const RESEARCH: &str = "https://example.org/odrl/profile/research";

fn strict_text(name: &str) -> String {
    std::fs::read_to_string(format!("{}/strict/{name}", tests_dir())).unwrap()
}

/// (rule, target, severity) of every finding, in order.
fn summary(r: &Report) -> Vec<(String, String, Severity)> {
    r.findings
        .iter()
        .map(|f| (f.rule.to_string(), target_str(&f.target), f.severity))
        .collect()
}

fn row(rule: &str, target: &str, severity: Severity) -> (String, String, Severity) {
    (rule.to_string(), target.to_string(), severity)
}

fn strict(text: &str, w: &Wording) -> Report {
    validate_json(text, w, &Options::strict())
}

fn advisory(text: &str, w: &Wording) -> Report {
    validate_json(text, w, &Options::advisory())
}

fn research() -> Wording {
    wording_of(&["research.jsonld"])
}

/// A Set with the research and foo prefixes bound around `extra`.
fn set_with(extra: Value) -> String {
    let mut p = json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld",
                     {"ex": "https://example.org/odrl/profile/research/",
                      "foo": "https://example.net/foo/"}],
        "@type": "Set", "uid": "urn:policy:t"
    });
    for (k, v) in extra.as_object().unwrap() {
        p[k] = v.clone();
    }
    p.to_string()
}

/// Applies a fix's events to `doc`, in order.
fn apply_fix(doc: &mut EditDoc, fix: &Fix) {
    for ev in &fix.events {
        doc.apply(ev, &EditRules::default())
            .unwrap_or_else(|e| panic!("{ev:?}: {e}"));
    }
}

#[test]
fn every_rule_has_a_hostile_case() {
    let cases = hostile_cases();
    for r in RULES {
        assert!(
            cases.iter().any(|h| h.rule == r.id.as_str()),
            "no hostile case for {}",
            r.id
        );
    }
    assert_eq!(cases.len(), RULES.len());
}

#[test]
fn hostile_profile_rules() {
    hostile_trigger_only_their_rule(&[
        "profile.undeclared-in-namespace",
        "profile.unknown-term",
        "profile.wrong-kind",
        "profile.not-declared",
        "profile.conflict-term",
        "profile.unbound-prefix",
        "profile.deprecated-term",
    ]);
}

#[test]
fn foreign_term_silent_advisory_error_strict() {
    let text = strict_text("foreign-term.json");
    assert!(advisory(&text, &research()).findings.is_empty());
    for w in [Wording::new(), research()] {
        let r = strict(&text, &w);
        assert_eq!(
            summary(&r),
            [
                row(
                    "profile.unknown-term",
                    "policy[0].permission[0].action[0]#name",
                    Severity::Error
                ),
                row(
                    "profile.unknown-term",
                    "policy[0].permission[0].constraint[0]#leftOperand",
                    Severity::Error
                ),
                row(
                    "profile.unknown-term",
                    "policy[0].permission[0].constraint[0]#operator",
                    Severity::Error
                ),
                row(
                    "profile.unknown-term",
                    "policy[0].permission[1].action[0]#name",
                    Severity::Error
                ),
            ]
        );
        assert_eq!(r.verdict, Verdict::Violations);
        let f = &r.findings[0];
        assert_eq!(
            f.message,
            "the action \"foo:bar\" is not defined by ODRL or by a loaded profile"
        );
        let term = f.term.as_ref().unwrap();
        assert_eq!(
            (term.written.as_str(), term.iri.as_str()),
            ("foo:bar", "https://example.net/foo/bar")
        );
        // A bare word is an ODRL name; "distil" is not one.
        assert_eq!(
            r.findings[3].term.as_ref().unwrap().iri,
            "http://www.w3.org/ns/odrl/2/distil"
        );
        assert_eq!(
            r.findings[1].message,
            "the left operand \"foo:size\" is not defined by ODRL or by a loaded profile"
        );
    }
}

#[test]
fn wrong_kind_left_operand_as_action() {
    let r = strict(&strict_text("wrong-kind.json"), &research());
    assert_eq!(
        summary(&r),
        [
            row("profile.wrong-kind", "policy[0]#kind", Severity::Error),
            row(
                "profile.wrong-kind",
                "policy[0].permission[0].action[0]#name",
                Severity::Error
            ),
            row(
                "profile.wrong-kind",
                "policy[0].permission[0].constraint[0]#leftOperand",
                Severity::Error
            ),
            row(
                "profile.wrong-kind",
                "policy[0].permission[0].constraint[0]#operator",
                Severity::Error
            ),
        ]
    );
    assert_eq!(
        r.findings[1].message,
        "\"ex:researchPurpose\" is a left operand, not an action"
    );
    assert_eq!(
        r.findings[0].message,
        "\"ex:train\" is an action, not a policy type"
    );
    assert_eq!(
        r.findings[3].message,
        "\"use\" is an action, not an operator"
    );
    let kinds: Vec<Vec<TermKind>> = r
        .findings
        .iter()
        .map(|f| f.expected.as_ref().unwrap().kinds.clone())
        .collect();
    assert_eq!(
        kinds,
        [
            vec![TermKind::PolicyClass],
            vec![TermKind::Action],
            vec![TermKind::LeftOperand],
            vec![TermKind::Operator]
        ]
    );
    assert!(
        advisory(&strict_text("wrong-kind.json"), &research())
            .findings
            .is_empty()
    );
}

#[test]
fn not_declared_with_add_profile_fix() {
    let text = strict_text("not-declared.json");
    let r = strict(&text, &research());
    assert_eq!(
        summary(&r),
        [row(
            "profile.not-declared",
            "policy[0].permission[0].action[0]#name",
            Severity::Error
        )]
    );
    let f = &r.findings[0];
    assert_eq!(f.profile.as_deref(), Some(RESEARCH));
    assert_eq!(
        f.message,
        format!(
            "\"ex:train\" comes from the profile <{RESEARCH}>, which the policy does not declare"
        )
    );
    assert_eq!(f.fixes.len(), 1);
    let fix = &f.fixes[0];
    assert!(fix.safe);
    assert_eq!(fix.replacement, RESEARCH);
    assert_eq!(fix.label, format!("Declare the profile <{RESEARCH}>"));
    let mut doc = read_model(&text).unwrap();
    apply_fix(&mut doc, fix);
    assert_eq!(doc.policies[0].profile, [RESEARCH]);
    let after = validate(&doc, &research(), &Options::strict());
    assert!(after.conforms(), "{:?}", after.findings);
    // Advisory says nothing about it.
    assert!(advisory(&text, &research()).findings.is_empty());
}

#[test]
fn not_declared_fix_appends_to_an_existing_profile_list() {
    let w = wording_of(&["research.jsonld", "other.jsonld"]);
    let text = set_with(json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld",
                     {"ex": "https://example.org/odrl/profile/research/",
                      "other": "https://example.com/odrl/profile/other/"}],
        "profile": [RESEARCH],
        "permission": [{"target": "urn:asset:1", "action": "other:archiveCopy"},
                       {"target": "urn:asset:1", "action": "ex:train",
                        "constraint": [{"leftOperand": "purpose", "operator": "eq",
                                        "rightOperand": {"@id": "ex:academic"}}]}]
    }));
    let r = strict(&text, &w);
    assert_eq!(
        summary(&r),
        [row(
            "profile.not-declared",
            "policy[0].permission[0].action[0]#name",
            Severity::Error
        )]
    );
    let mut doc = read_model(&text).unwrap();
    apply_fix(&mut doc, &r.findings[0].fixes[0]);
    assert_eq!(
        doc.policies[0].profile,
        [RESEARCH, "https://example.com/odrl/profile/other"]
    );
    assert!(validate(&doc, &w, &Options::strict()).conforms());
}

#[test]
fn a_value_from_a_profile_needs_the_profile_declared() {
    let text = set_with(json!({
        "permission": [{"target": "urn:asset:1", "action": "use",
            "constraint": [{"leftOperand": "purpose", "operator": "eq",
                            "rightOperand": {"@id": "ex:academic"}}]}]
    }));
    assert_eq!(
        summary(&strict(&text, &research())),
        [row(
            "profile.not-declared",
            "policy[0].permission[0].constraint[0]#rightOperand[0]",
            Severity::Error
        )]
    );
}

#[test]
fn conflict_term() {
    let r = strict(&strict_text("conflict.json"), &Wording::new());
    assert_eq!(
        summary(&r),
        [
            row(
                "profile.conflict-term",
                "policy[0]#conflict",
                Severity::Error
            ),
            row(
                "profile.conflict-term",
                "policy[2]#conflict",
                Severity::Error
            ),
        ]
    );
    assert_eq!(
        r.findings[0].message,
        "\"first\" is not a conflict strategy: use perm, prohibit or invalid, or a conflict term a loaded profile declares"
    );
    // A profile's conflict term is a conflict strategy.
    let text = set_with(json!({
        "profile": RESEARCH, "conflict": "ex:ignore",
        "permission": [{"target": "urn:asset:1", "action": "use"}]
    }));
    assert!(strict(&text, &research()).conforms());
    assert!(!strict(&text, &Wording::new()).conforms());
}

#[test]
fn unbound_prefix_fix_full_iri() {
    let text = strict_text("unbound-prefix.json");
    let train = "https://example.org/odrl/profile/research/train";
    // Advisory reads ex:train through the profile's own prefix and is
    // silent, as it always was.
    assert!(advisory(&text, &research()).findings.is_empty());
    let r = strict(&text, &research());
    assert_eq!(
        summary(&r),
        [row(
            "profile.unbound-prefix",
            "policy[0].permission[0].action[0]#name",
            Severity::Error
        )]
    );
    let f = &r.findings[0];
    assert_eq!(
        f.message,
        format!(
            "the policy's @context does not bind the prefix \"ex\" of \"ex:train\"; only a loaded profile's own prefixes read it as <{train}>"
        )
    );
    assert_eq!(f.term.as_ref().unwrap().iri, train);
    let fix = &f.fixes[0];
    assert!(fix.safe);
    assert_eq!(fix.replacement, train);
    let mut doc = read_model(&text).unwrap();
    apply_fix(&mut doc, fix);
    assert_eq!(doc.policies[0].permission[0].action[0].name, train);
    assert!(validate(&doc, &research(), &Options::strict()).conforms());
}

#[test]
fn deprecated_warning_unless_deny() {
    let text = set_with(json!({
        "permission": [{"target": "urn:asset:1", "action": "commercialize"}]
    }));
    let r = strict(&text, &Wording::new());
    assert_eq!(
        summary(&r),
        [row(
            "profile.deprecated-term",
            "policy[0].permission[0].action[0]#name",
            Severity::Warning
        )]
    );
    assert_eq!(
        r.findings[0].message,
        "\"commercialize\" is deprecated in ODRL 2.2"
    );
    assert_eq!(r.verdict, Verdict::Conforms);
    let r = validate_json(
        &text,
        &Wording::new(),
        &Options::strict().deny_deprecated(true),
    );
    assert_eq!(r.findings[0].severity, Severity::Error);
    assert_eq!(r.verdict, Verdict::Violations);
    // Advisory knows no ODRL table, with or without a profile.
    assert!(advisory(&text, &research()).findings.is_empty());

    // A profile's deprecated term keeps today's message, strict or not.
    let w = Wording::new().with_profile(
        ProfileVocab::from_value(
            &json!({
                "@context": {"odrl": "http://www.w3.org/ns/odrl/2/",
                             "owl": "http://www.w3.org/2002/07/owl#",
                             "ex": "https://example.org/odrl/profile/research/"},
                "@graph": [
                    {"@id": RESEARCH, "@type": "owl:Ontology"},
                    {"@id": "ex:old", "@type": "odrl:Action", "owl:deprecated": true}
                ]
            }),
            &ProfileLimits::default(),
        )
        .unwrap(),
    );
    let text = set_with(json!({
        "profile": RESEARCH,
        "permission": [{"target": "urn:asset:1", "action": "ex:old"}]
    }));
    for o in [Options::advisory(), Options::strict()] {
        let r = validate_json(&text, &w, &o);
        assert_eq!(
            summary(&r),
            [row(
                "profile.deprecated-term",
                "policy[0].permission[0].action[0]#name",
                Severity::Warning
            )]
        );
        assert_eq!(
            r.findings[0].message,
            format!("\"ex:old\" is deprecated in the profile <{RESEARCH}>")
        );
    }
}

#[test]
fn mixed_two_profiles_conform_when_both_loaded() {
    let text = strict_text("mixed-two-profiles.json");
    let both = strict(&text, &wording_of(&["research.jsonld", "other.jsonld"]));
    assert!(both.conforms(), "{:?}", both.findings);
    // With one of them missing the check is incomplete, not wrong: the
    // other profile's terms are not called unknown.
    let one = strict(&text, &research());
    assert_eq!(one.verdict, Verdict::Incomplete);
    assert_eq!(
        summary(&one),
        [row("profile.not-loaded", "policy[0]", Severity::Error)]
    );
}

#[test]
fn strict_with_an_empty_wording() {
    // ODRL alone is the vocabulary: an ODRL-only policy conforms.
    let text = set_with(json!({
        "permission": [{"target": "urn:asset:1", "action": "distribute",
            "constraint": [{"leftOperand": "dateTime", "operator": "lt",
                            "rightOperand": {"@value": "2027-01-01T00:00:00Z", "@type": "xsd:dateTime"}},
                           {"leftOperand": "payAmount", "operator": "eq", "rightOperand": 5,
                            "unit": "http://dbpedia.org/resource/Euro"},
                           {"leftOperand": "purpose", "operator": "isAnyOf",
                            "rightOperand": [{"@id": "https://example.net/purposes/research"}, "teaching"]}]}],
        "prohibition": [{"target": "urn:asset:1", "action": "odrl:sell"}],
        "obligation": [{"action": "http://www.w3.org/ns/odrl/2/compensate"}]
    }));
    let r = strict(&text, &Wording::new());
    assert!(r.conforms(), "{:?}", r.findings);
    // An unknown term is an error.
    let text = set_with(json!({"permission": [{"target": "urn:asset:1", "action": "ex:x"}]}));
    assert_eq!(
        summary(&strict(&text, &Wording::new())),
        [row(
            "profile.unknown-term",
            "policy[0].permission[0].action[0]#name",
            Severity::Error
        )]
    );
    // A declared profile that is not loaded: incomplete, never a pass.
    let text = set_with(json!({
        "profile": RESEARCH,
        "permission": [{"target": "urn:asset:1", "action": "ex:x"}]
    }));
    let r = strict(&text, &Wording::new());
    assert_eq!(r.verdict, Verdict::Incomplete);
    assert_eq!(
        summary(&r),
        [row("profile.not-loaded", "policy[0]", Severity::Error)]
    );
}

#[test]
fn core_vocabulary_only() {
    let core = Options::strict().odrl_vocabulary(OdrlVocab::Core);
    let text = set_with(json!({"permission": [{"target": "urn:asset:1", "action": "distribute"}]}));
    assert!(strict(&text, &Wording::new()).conforms());
    let r = validate_json(&text, &Wording::new(), &core);
    assert_eq!(
        summary(&r),
        [row(
            "profile.unknown-term",
            "policy[0].permission[0].action[0]#name",
            Severity::Error
        )]
    );
    assert_eq!(
        r.findings[0].message,
        "the action \"distribute\" is not defined by the ODRL core vocabulary or by a loaded profile"
    );
    let text = set_with(json!({"permission": [{"target": "urn:asset:1", "action": "use"}]}));
    assert!(validate_json(&text, &Wording::new(), &core).conforms());
}

#[test]
fn clean_policy_conforms() {
    let text = strict_text("clean.json");
    for w in [research(), wording_of(&["research-closed.jsonld"])] {
        let r = strict(&text, &w);
        assert!(r.findings.is_empty(), "{:?}", r.findings);
        assert!(r.conforms());
    }
}

#[test]
fn research_policy_strict() {
    // The demo's research agreement: the undeclared ex:distill is an error,
    // and ODRL itself deprecates commercialize.
    let r = strict(&profile_text("research-policy.json"), &research());
    assert_eq!(
        summary(&r),
        [
            row(
                "profile.undeclared-in-namespace",
                "policy[0].permission[1].action[0]#name",
                Severity::Error
            ),
            row(
                "profile.deprecated-term",
                "policy[0].prohibition[0].action[0]#name",
                Severity::Warning
            ),
        ]
    );
    assert_eq!(r.verdict, Verdict::Violations);
}

#[test]
fn odrl_terms_a_profile_redeclares_need_no_declaration() {
    // The W3C vocabulary loaded as a profile: its terms are ODRL's own.
    let w = wording_of(&["odrl22-excerpt.json"]);
    let text = set_with(json!({
        "permission": [{"target": "urn:asset:1", "action": "use",
            "constraint": [{"leftOperand": "elapsedTime", "operator": "lteq",
                            "rightOperand": {"@value": "PT1H", "@type": "xsd:duration"}}]}]
    }));
    let r = strict(&text, &w);
    assert!(r.conforms(), "{:?}", r.findings);
}

#[test]
fn findings_carry_their_node() {
    let text = strict_text("foreign-term.json");
    let doc = read_model(&text).unwrap();
    let r = validate(&doc, &Wording::new(), &Options::strict());
    for f in &r.findings {
        let IssueTarget::Slot(s) = &f.target else {
            panic!("{f:?}")
        };
        assert_eq!(f.node, Some(doc.node(&s.node).unwrap().id()));
    }
}
