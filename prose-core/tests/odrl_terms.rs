//! The ODRL 2.2 names strict validation knows without a loaded profile:
//! IRIs and kinds only, from the W3C vocabulary's own collections (ODRL
//! Vocabulary & Expression 2.2, sections 3 and 4, and appendix A).
use prose_core::profile::{OdrlTerm, OdrlVocab, TermKind, odrl_term, odrl_terms};
use std::collections::HashSet;

const ODRL: &str = "http://www.w3.org/ns/odrl/2/";
const CC: &str = "http://creativecommons.org/ns#";

fn odrl(local: &str) -> String {
    format!("{ODRL}{local}")
}

fn get(local: &str) -> &'static OdrlTerm {
    odrl_term(&odrl(local), OdrlVocab::Common).unwrap_or_else(|| panic!("no {local}"))
}

#[test]
fn odrl_terms_kinds() {
    for (local, kind, core) in [
        ("use", TermKind::Action, true),
        ("transfer", TermKind::Action, true),
        ("distribute", TermKind::Action, false),
        ("purpose", TermKind::LeftOperand, false),
        ("dateTime", TermKind::LeftOperand, false),
        ("isAnyOf", TermKind::Operator, true),
        ("eq", TermKind::Operator, true),
        ("policyUsage", TermKind::RightOperand, false),
        ("perm", TermKind::ConflictTerm, true),
        ("prohibit", TermKind::ConflictTerm, true),
        ("invalid", TermKind::ConflictTerm, true),
        ("Policy", TermKind::PolicyClass, true),
        ("Set", TermKind::PolicyClass, true),
        ("Offer", TermKind::PolicyClass, true),
        ("Agreement", TermKind::PolicyClass, true),
        ("Privacy", TermKind::PolicyClass, false),
        ("Ticket", TermKind::PolicyClass, false),
        ("Permission", TermKind::RuleClass, true),
        ("assigner", TermKind::PartyFunction, true),
        ("informedParty", TermKind::PartyFunction, false),
        ("target", TermKind::AssetRelation, true),
        ("output", TermKind::AssetRelation, false),
        ("andSequence", TermKind::LogicalOperand, true),
    ] {
        let t = get(local);
        assert_eq!(t.kind, kind, "{local}");
        assert_eq!(t.core, core, "{local}");
        assert!(!t.deprecated, "{local}");
        assert_eq!(t.iri, odrl(local));
    }
    // The Creative Commons actions the Common Vocabulary lists, as written.
    let t = odrl_term(&format!("{CC}Distribution"), OdrlVocab::Common).unwrap();
    assert_eq!((t.kind, t.core), (TermKind::Action, false));
    assert!(odrl_term(&format!("{CC}CommercialUse"), OdrlVocab::Common).is_some());
}

#[test]
fn deprecated_terms_are_known_and_marked() {
    for local in ["commercialize", "copy", "pay", "write", "device", "system"] {
        let t = get(local);
        assert!(t.deprecated, "{local}");
        assert!(!t.core, "{local}");
    }
    assert_eq!(get("device").kind, TermKind::LeftOperand);
    // "ignore" was a conflict value in ODRL 2.1; 2.2 deprecates it as an
    // undefined-term handling, so it is no conflict strategy.
    assert!(odrl_term(&odrl("ignore"), OdrlVocab::Common).is_none());
}

#[test]
fn core_only_rejects_common() {
    for local in [
        "use",
        "transfer",
        "eq",
        "isNoneOf",
        "perm",
        "Set",
        "Agreement",
    ] {
        assert!(
            odrl_term(&odrl(local), OdrlVocab::Core).is_some(),
            "{local}"
        );
    }
    for local in [
        "distribute",
        "purpose",
        "Privacy",
        "policyUsage",
        "commercialize",
    ] {
        assert!(
            odrl_term(&odrl(local), OdrlVocab::Core).is_none(),
            "{local}"
        );
        assert!(
            odrl_term(&odrl(local), OdrlVocab::Common).is_some(),
            "{local}"
        );
    }
    assert_eq!(OdrlVocab::default(), OdrlVocab::Common);
}

#[test]
fn table_is_well_formed() {
    let all = odrl_terms();
    assert!(all.len() > 150, "{}", all.len());
    let mut seen = HashSet::new();
    for t in all {
        assert!(seen.insert(&t.iri), "duplicate {}", t.iri);
        assert!(
            t.iri.starts_with(ODRL) || t.iri.starts_with(CC),
            "{}",
            t.iri
        );
        assert!(!(t.core && t.deprecated), "{}", t.iri);
    }
    // Nothing but full IRIs: no compact or bare names.
    for bad in ["use", "odrl:use", "", "http://www.w3.org/ns/odrl/2/"] {
        assert!(odrl_term(bad, OdrlVocab::Common).is_none(), "{bad:?}");
    }
}

#[test]
fn the_built_in_left_operands_are_odrl_terms() {
    for (term, _) in prose_core::wording::known_left_operands() {
        let t = get(term);
        assert_eq!(t.kind, TermKind::LeftOperand, "{term}");
    }
}
