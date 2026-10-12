//! `prose_core::validate`: the advisory mode is `check_profile`, exactly;
//! the rule catalogue and how options resolve a rule's level.
mod common;
mod corpus;
use corpus::*;
use prose_core::edit::*;
use prose_core::profile::Wording;
use prose_core::validate::*;
use std::sync::Arc;

// --- T1: advisory equals check_profile --------------------------------------

#[test]
fn advisory_equals_check_profile_on_every_fixture() {
    let mut compared = 0;
    for (name, text) in policy_corpus() {
        let Ok(doc) = read_model(&text) else {
            continue;
        };
        for (label, w) in wordings() {
            let want = check_profile(&doc, &w);
            let report = validate(&doc, &w, &Options::advisory());
            assert_eq!(report.issues(), want, "{name} under {label}");
            assert_eq!(report.mode, Mode::Advisory);
            let v = Validator::new(Arc::new(w.clone()), Options::advisory());
            assert_eq!(v.validate(&doc).issues(), want, "{name} under {label}");
            compared += 1;
        }
    }
    assert!(compared >= 300, "{compared}");
}

#[test]
fn advisory_report_matches_the_goldens() {
    // The same goldens check_profile is pinned by, built from the report.
    for (name, text) in policy_corpus() {
        let Ok(doc) = read_model(&text) else {
            continue;
        };
        let mut got = String::new();
        for (label, w) in wordings() {
            got.push_str(&format!("## {label}\n"));
            for i in validate(&doc, &w, &Options::advisory()).issues() {
                got.push_str(&issue_line(&i));
                got.push('\n');
            }
        }
        compare_golden(&format!("advisory/{name}.txt"), &got);
    }
}

#[test]
fn empty_wording_empty_report() {
    let w = Wording::default();
    for (name, text) in policy_corpus() {
        let Ok(doc) = read_model(&text) else {
            continue;
        };
        let r = validate(&doc, &w, &Options::advisory());
        assert!(r.findings.is_empty(), "{name}: {:?}", r.findings);
        assert!(r.gaps.is_empty(), "{name}");
        assert_eq!(r.verdict, Verdict::Conforms, "{name}");
        assert!(r.conforms(), "{name}");
    }
}

#[test]
fn options_default_is_advisory() {
    assert_eq!(Options::default(), Options::advisory());
    assert_eq!(Options::default().mode(), Mode::Advisory);
    assert_eq!(Options::strict().mode(), Mode::Strict);
    assert_ne!(Options::strict(), Options::advisory());
}

#[test]
fn validator_equality_is_by_wording_pointer_and_options() {
    let w = Arc::new(wording_of(&["research.jsonld"]));
    let a = Validator::new(w.clone(), Options::strict());
    let b = Validator::new(w.clone(), Options::strict());
    assert!(a == b);
    assert!(a.clone() == a);
    // Equal content, another allocation: a different validator.
    let c = Validator::new(Arc::new((*w).clone()), Options::strict());
    assert!(a != c);
    assert!(a != Validator::new(w, Options::advisory()));
    assert_eq!(a.options(), &Options::strict());
}

#[test]
fn validate_json_advisory_equals_validate_on_the_model() {
    let w = wording_of(&["research.jsonld"]);
    let text = profile_text("research-policy.json");
    let doc = read_model(&text).unwrap();
    assert_eq!(
        validate_json(&text, &w, &Options::advisory()),
        validate(&doc, &w, &Options::advisory())
    );
}

// --- T2: catalogue and options ------------------------------------------------

/// (id, tier, basis, advisory, strict, strict on a closed operand, gap,
/// refusable) for every rule, in catalogue order.
type Row = (
    &'static str,
    Tier,
    Basis,
    Level,
    Level,
    Option<Level>,
    bool,
    bool,
);

fn catalogue() -> Vec<Row> {
    use Basis::*;
    use Level::*;
    vec![
        (
            "profile.not-loaded",
            Tier::Profile,
            StandardDerived,
            Warning,
            Error,
            None,
            true,
            false,
        ),
        (
            "profile.import-not-loaded",
            Tier::Profile,
            StandardDerived,
            Off,
            Warning,
            None,
            true,
            false,
        ),
        (
            "profile.undeclared-in-namespace",
            Tier::Profile,
            StandardDerived,
            Warning,
            Error,
            None,
            false,
            true,
        ),
        (
            "profile.unknown-term",
            Tier::Profile,
            StandardDerived,
            Off,
            Error,
            None,
            false,
            true,
        ),
        (
            "profile.wrong-kind",
            Tier::Profile,
            StandardDerived,
            Off,
            Error,
            None,
            false,
            true,
        ),
        (
            "profile.not-declared",
            Tier::Profile,
            StandardDerived,
            Off,
            Error,
            None,
            false,
            false,
        ),
        (
            "profile.conflict-term",
            Tier::Profile,
            StandardDerived,
            Off,
            Error,
            None,
            false,
            true,
        ),
        (
            "profile.unbound-prefix",
            Tier::Profile,
            StandardDerived,
            Off,
            Error,
            None,
            false,
            true,
        ),
        (
            "profile.deprecated-term",
            Tier::Profile,
            Convention,
            Warning,
            Warning,
            None,
            false,
            false,
        ),
        (
            "odrl.policy-uid",
            Tier::Odrl,
            Standard,
            Off,
            Error,
            None,
            false,
            false,
        ),
        (
            "odrl.no-rule",
            Tier::Odrl,
            Standard,
            Off,
            Error,
            None,
            false,
            false,
        ),
        (
            "odrl.offer-assigner",
            Tier::Odrl,
            Standard,
            Off,
            Error,
            None,
            false,
            false,
        ),
        (
            "odrl.agreement-parties",
            Tier::Odrl,
            Standard,
            Off,
            Error,
            None,
            false,
            false,
        ),
        (
            "odrl.rule-target",
            Tier::Odrl,
            Standard,
            Off,
            Error,
            None,
            false,
            false,
        ),
        (
            "odrl.rule-action",
            Tier::Odrl,
            Standard,
            Off,
            Error,
            None,
            false,
            false,
        ),
        (
            "odrl.constraint-shape",
            Tier::Odrl,
            Standard,
            Off,
            Error,
            None,
            false,
            false,
        ),
        (
            "odrl.set-operator-cardinality",
            Tier::Odrl,
            Standard,
            Off,
            Error,
            None,
            false,
            false,
        ),
        (
            "odrl.consequence-nesting",
            Tier::Odrl,
            Standard,
            Off,
            Error,
            None,
            false,
            false,
        ),
        (
            "odrl.inheritance-cycle",
            Tier::Odrl,
            Standard,
            Off,
            Error,
            None,
            false,
            false,
        ),
        (
            "prose.operator-not-allowed",
            Tier::Prose,
            Convention,
            Info,
            Info,
            Some(Error),
            false,
            true,
        ),
        (
            "prose.value-not-allowed",
            Tier::Prose,
            Convention,
            Off,
            Info,
            Some(Error),
            false,
            false,
        ),
        (
            "prose.iri-expected",
            Tier::Prose,
            Convention,
            Off,
            Off,
            Some(Error),
            false,
            false,
        ),
        (
            "prose.unit-not-allowed",
            Tier::Prose,
            Convention,
            Off,
            Info,
            Some(Error),
            false,
            true,
        ),
        (
            "prose.datatype-lexical",
            Tier::Prose,
            Convention,
            Info,
            Info,
            Some(Error),
            false,
            false,
        ),
        (
            "prose.datatype-conflict",
            Tier::Prose,
            Convention,
            Off,
            Warning,
            Some(Error),
            false,
            false,
        ),
        (
            "prose.ill-typed-literal",
            Tier::Prose,
            Convention,
            Off,
            Error,
            None,
            false,
            false,
        ),
        (
            "input.unreadable",
            Tier::Input,
            Basis::Input,
            Error,
            Error,
            None,
            false,
            false,
        ),
        (
            "input.unchecked",
            Tier::Input,
            Basis::Input,
            Off,
            Warning,
            None,
            true,
            false,
        ),
        (
            "input.reader-note",
            Tier::Input,
            Basis::Input,
            Off,
            Info,
            None,
            false,
            false,
        ),
        (
            "heuristic.ordered-on-unordered",
            Tier::Heuristic,
            Basis::Heuristic,
            Off,
            Off,
            None,
            false,
            false,
        ),
    ]
}

#[test]
fn rules_snapshot() {
    let got: Vec<Row> = RULES
        .iter()
        .map(|r| {
            (
                r.id.as_str(),
                r.tier,
                r.basis,
                r.advisory,
                r.strict,
                r.strict_closed,
                r.gap,
                r.refusable,
            )
        })
        .collect();
    assert_eq!(got, catalogue());
    for r in RULES {
        assert_eq!(r.since_ruleset, 1, "{}", r.id);
        assert!(!r.title.is_empty() && !r.source.is_empty(), "{}", r.id);
        assert!(r.since_ruleset <= RULESET);
    }
    assert_eq!(RULESET, 1);
}

#[test]
fn every_odrl_rule_cites_a_section_of_the_information_model() {
    for r in RULES.iter().filter(|r| r.tier == Tier::Odrl) {
        assert!(r.source.starts_with("ODRL IM 2."), "{}: {}", r.id, r.source);
        assert_eq!(r.basis, Basis::Standard, "{}", r.id);
    }
}

#[test]
fn rule_id_roundtrip() {
    let mut seen = std::collections::HashSet::new();
    for r in RULES {
        let id = r.id.as_str();
        assert!(seen.insert(id), "duplicate {id}");
        assert_eq!(id.parse::<RuleId>(), Ok(r.id));
        assert_eq!(r.id.to_string(), id);
        assert_eq!(r.id.tier(), r.tier);
        assert_eq!(r.id.info(), r);
        let prefix = match r.tier {
            Tier::Profile => "profile.",
            Tier::Odrl => "odrl.",
            Tier::Prose => "prose.",
            Tier::Input => "input.",
            Tier::Heuristic => "heuristic.",
            _ => unreachable!(),
        };
        assert!(id.starts_with(prefix), "{id}");
    }
    for bad in [
        "",
        "profile",
        "profile.unknown",
        "PROFILE.UNKNOWN-TERM",
        " odrl.no-rule",
    ] {
        assert!(bad.parse::<RuleId>().is_err(), "{bad:?}");
    }
}

#[test]
fn level_of_table_both_modes_open_and_closed() {
    let (advisory, strict) = (Options::advisory(), Options::strict());
    for r in RULES {
        let row = catalogue()
            .into_iter()
            .find(|row| row.0 == r.id.as_str())
            .unwrap();
        // The structural tier is off until switched on; see below.
        let on = r.tier != Tier::Odrl;
        let off = |l: Level| if on { l } else { Level::Off };
        assert_eq!(advisory.level_of(r.id, false), row.3, "{}", r.id);
        assert_eq!(advisory.level_of(r.id, true), row.3, "{} closed", r.id);
        assert_eq!(strict.level_of(r.id, false), off(row.4), "{}", r.id);
        assert_eq!(
            strict.level_of(r.id, true),
            off(row.5.unwrap_or(row.4)),
            "{} closed",
            r.id
        );
    }
}

#[test]
fn structural_off_by_default_in_strict() {
    let odrl: Vec<RuleId> = RULES
        .iter()
        .filter(|r| r.tier == Tier::Odrl)
        .map(|r| r.id)
        .collect();
    assert_eq!(odrl.len(), 10);
    for &r in &odrl {
        assert_eq!(Options::strict().level_of(r, false), Level::Off, "{r}");
        assert_eq!(
            Options::strict().structural(true).level_of(r, false),
            Level::Error,
            "{r}"
        );
        assert_eq!(
            Options::strict()
                .structural(true)
                .structural(false)
                .level_of(r, false),
            Level::Off
        );
        // Advisory never makes an error out of structure.
        assert_eq!(
            Options::advisory().structural(true).level_of(r, false),
            Level::Off,
            "{r}"
        );
    }
    assert_ne!(Options::strict().structural(true), Options::strict());
}

#[test]
fn fail_closed_and_deny_deprecated() {
    let s = Options::strict();
    assert_eq!(s.level_of(RuleId::InputUnchecked, false), Level::Warning);
    assert_eq!(
        s.clone()
            .fail_closed(true)
            .level_of(RuleId::InputUnchecked, false),
        Level::Error
    );
    assert_eq!(
        s.level_of(RuleId::ProfileDeprecatedTerm, false),
        Level::Warning
    );
    assert_eq!(
        s.clone()
            .deny_deprecated(true)
            .level_of(RuleId::ProfileDeprecatedTerm, false),
        Level::Error
    );
    // Neither makes the advisory mode loud.
    let a = Options::advisory().fail_closed(true).deny_deprecated(true);
    assert_eq!(a.level_of(RuleId::InputUnchecked, false), Level::Off);
    assert_eq!(
        a.level_of(RuleId::ProfileDeprecatedTerm, false),
        Level::Warning
    );
}

#[test]
fn level_overrides_win() {
    let o = Options::strict()
        .level(RuleId::ProfileUnknownTerm, Level::Warning)
        .level(RuleId::HeuristicOrderedOnUnordered, Level::Info)
        .level(RuleId::OdrlPolicyUid, Level::Warning);
    assert_eq!(
        o.level_of(RuleId::ProfileUnknownTerm, false),
        Level::Warning
    );
    assert_eq!(
        o.level_of(RuleId::HeuristicOrderedOnUnordered, false),
        Level::Info
    );
    // An explicit level switches a structural rule on by itself.
    assert_eq!(o.level_of(RuleId::OdrlPolicyUid, false), Level::Warning);
    // The last word on a rule wins.
    let o = o.level(RuleId::ProfileUnknownTerm, Level::Off);
    assert_eq!(o.level_of(RuleId::ProfileUnknownTerm, false), Level::Off);
    // The override applies on a closed operand too.
    let o = Options::strict().level(RuleId::ProseOperatorNotAllowed, Level::Warning);
    assert_eq!(
        o.level_of(RuleId::ProseOperatorNotAllowed, true),
        Level::Warning
    );
}

#[test]
fn ruleset_pin_hides_newer_rules() {
    assert_eq!(Options::strict().ruleset(RULESET), Options::strict());
    let pinned = Options::strict().structural(true).ruleset(0);
    for r in RULES {
        let want = if r.id == RuleId::InputUnreadable {
            Level::Error
        } else {
            Level::Off
        };
        assert_eq!(pinned.level_of(r.id, false), want, "{}", r.id);
    }
    let w = wording_of(&["research.jsonld"]);
    let r = validate_json(&profile_text("research-policy.json"), &w, &pinned);
    assert!(r.findings.is_empty(), "{:?}", r.findings);
}

#[test]
fn unreadable_cannot_be_switched_off() {
    let o = Options::advisory().level(RuleId::InputUnreadable, Level::Off);
    assert_eq!(o.level_of(RuleId::InputUnreadable, false), Level::Error);
    for opts in [o, Options::strict(), Options::strict().ruleset(0)] {
        for text in ["", "not json", "[]", "{}", "42", r#"{"@context": {}}"#] {
            let r = validate_json(text, &Wording::new(), &opts);
            assert_eq!(r.findings.len(), 1, "{text:?}");
            assert_eq!(r.findings[0].rule, RuleId::InputUnreadable);
            assert_eq!(r.findings[0].severity, Severity::Error);
            assert_eq!(r.findings[0].target, IssueTarget::Doc);
            assert_eq!(r.verdict, Verdict::Violations);
        }
    }
}

#[test]
fn options_hash_as_memo_keys() {
    use std::collections::HashSet;
    let set: HashSet<Options> = [
        Options::advisory(),
        Options::strict(),
        Options::strict().structural(true),
        Options::strict().level(RuleId::ProfileUnknownTerm, Level::Warning),
        Options::strict(),
    ]
    .into_iter()
    .collect();
    assert_eq!(set.len(), 4);
}

#[test]
fn odrl_vocabulary_is_an_option() {
    use prose_core::profile::OdrlVocab;
    assert_eq!(
        Options::strict().odrl_vocabulary(OdrlVocab::Common),
        Options::strict()
    );
    let core = Options::strict().odrl_vocabulary(OdrlVocab::Core);
    assert_ne!(core, Options::strict());
    assert_eq!(core.odrl(), OdrlVocab::Core);
    assert_eq!(Options::strict().odrl(), OdrlVocab::Common);
}

/// The consequence-on-a-consequence MUST NOT is in IM 2.6.3 (Duty class);
/// 2.6.6 only introduces the property. The catalogue cites where the
/// sentence is.
#[test]
fn consequence_nesting_cites_the_section_that_states_it() {
    let info = prose_core::validate::RuleId::OdrlConsequenceNesting.info();
    assert!(info.source.contains("2.6.3"), "{}", info.source);
    assert!(!info.source.contains("2.6.6"), "{}", info.source);
}
