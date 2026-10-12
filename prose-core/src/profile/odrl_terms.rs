//! The ODRL 2.2 vocabulary as names and kinds only: what strict validation
//! knows without a loaded profile. No labels or definitions are kept.
//!
//! Generated from the W3C's own `ODRL22.json`
//! (<https://www.w3.org/ns/odrl/2/ODRL22.json>, SHA-256
//! a897cbb82c08f1d16e4aec20a3acfe7cd91595a0642597965d5ce5f40f530637): every
//! term in one of its `skos:Collection`s whose type, super-property or
//! super-class gives an ODRL kind. A term of a collection named `...Common`
//! is in the Common Vocabulary (ODRL Vocabulary & Expression 2.2, section
//! 4, non-normative); a term of `#deprecatedTerms` is deprecated (appendix
//! A); every other one is in the Core Vocabulary (section 3).
//!
//! One correction to that source: `ODRL22.json`, like the identifier in
//! section 4.4.8 of the Vocabulary REC, misspells the Creative Commons
//! action `cc:CommercialUse` as `cc:CommericalUse`. `ODRL22.ttl`, the
//! `skos:exactMatch` of `odrl:commercialize` and Creative Commons have the
//! right spelling, and so does this table; the validator reads the
//! misspelling as the same term ([`odrl_spelling`]). The deprecated party
//! scopes (`odrl:All`, `odrl:Group`, ...) and undefined-term values
//! (`odrl:ignore`, `odrl:support`) have no kind here and are not listed.
//! `tests/validate_cc_terms.rs` compares the table with `ODRL22.ttl`.
use super::TermKind;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Which ODRL terms count as defined without a profile.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum OdrlVocab {
    /// The Core Vocabulary, the Common Vocabulary and the deprecated terms.
    /// IM 2.1 asks a policy that goes beyond the core to declare a profile;
    /// reading the Common Vocabulary as defined is a ds-prose leniency, and
    /// the default, since most policies use it.
    #[default]
    Common,
    /// The Core Vocabulary only: what IM 2.1 allows without a profile.
    Core,
}

/// One ODRL term.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OdrlTerm {
    /// The full IRI.
    pub iri: &'static str,
    pub kind: TermKind,
    /// In the Core Vocabulary.
    pub core: bool,
    /// `owl:deprecated`: still defined, not to be used.
    pub deprecated: bool,
}

/// Every term of the table, in kind order.
pub fn odrl_terms() -> &'static [OdrlTerm] {
    TERMS
}

/// The term with the full IRI `iri`, when `vocab` counts it as defined.
pub fn odrl_term(iri: &str, vocab: OdrlVocab) -> Option<&'static OdrlTerm> {
    static INDEX: OnceLock<HashMap<&'static str, usize>> = OnceLock::new();
    let index = INDEX.get_or_init(|| TERMS.iter().enumerate().map(|(i, t)| (t.iri, i)).collect());
    let t = &TERMS[*index.get(iri)?];
    match vocab {
        OdrlVocab::Core if !t.core => None,
        _ => Some(t),
    }
}

/// `iri`, or the table's spelling of it when it is the W3C texts'
/// misspelling `cc:CommericalUse`: a policy written from the REC or from
/// `ODRL22.json` uses that one.
pub(crate) fn odrl_spelling(iri: &str) -> &str {
    match iri {
        "http://creativecommons.org/ns#CommericalUse" => {
            "http://creativecommons.org/ns#CommercialUse"
        }
        _ => iri,
    }
}

/// `odrl:local` as a full IRI, at compile time.
macro_rules! o {
    ($local:literal) => {
        concat!("http://www.w3.org/ns/odrl/2/", $local)
    };
}

#[derive(Clone, Copy)]
enum Tier {
    Core,
    Common,
    Deprecated,
}

const fn t(iri: &'static str, kind: TermKind, tier: Tier) -> OdrlTerm {
    OdrlTerm {
        iri,
        kind,
        core: matches!(tier, Tier::Core),
        deprecated: matches!(tier, Tier::Deprecated),
    }
}

use TermKind::*;
use Tier::*;

static TERMS: &[OdrlTerm] = &[
    t(o!("Agreement"), PolicyClass, Core),
    t(o!("Offer"), PolicyClass, Core),
    t(o!("Policy"), PolicyClass, Core),
    t(o!("Set"), PolicyClass, Core),
    t(o!("Assertion"), PolicyClass, Common),
    t(o!("Privacy"), PolicyClass, Common),
    t(o!("Request"), PolicyClass, Common),
    t(o!("Ticket"), PolicyClass, Common),
    t(o!("Duty"), RuleClass, Core),
    t(o!("Permission"), RuleClass, Core),
    t(o!("Prohibition"), RuleClass, Core),
    t(o!("Rule"), RuleClass, Core),
    t(o!("Asset"), AssetClass, Core),
    t(o!("AssetCollection"), AssetClass, Core),
    t(o!("Party"), PartyClass, Core),
    t(o!("PartyCollection"), PartyClass, Core),
    t(o!("assignee"), PartyFunction, Core),
    t(o!("assigner"), PartyFunction, Core),
    t(o!("attributedParty"), PartyFunction, Common),
    t(o!("attributingParty"), PartyFunction, Common),
    t(o!("compensatedParty"), PartyFunction, Common),
    t(o!("compensatingParty"), PartyFunction, Common),
    t(o!("consentedParty"), PartyFunction, Common),
    t(o!("consentingParty"), PartyFunction, Common),
    t(o!("contractedParty"), PartyFunction, Common),
    t(o!("contractingParty"), PartyFunction, Common),
    t(o!("informedParty"), PartyFunction, Common),
    t(o!("informingParty"), PartyFunction, Common),
    t(o!("trackedParty"), PartyFunction, Common),
    t(o!("trackingParty"), PartyFunction, Common),
    t(o!("target"), AssetRelation, Core),
    t(o!("output"), AssetRelation, Common),
    t(o!("transfer"), Action, Core),
    t(o!("use"), Action, Core),
    t("http://creativecommons.org/ns#Attribution", Action, Common),
    // W3C's ODRL22.json and the REC's 4.4.8 spell this one
    // "cc:CommericalUse"; ODRL22.ttl and Creative Commons spell it as here.
    // See odrl_spelling.
    t(
        "http://creativecommons.org/ns#CommercialUse",
        Action,
        Common,
    ),
    t(
        "http://creativecommons.org/ns#DerivativeWorks",
        Action,
        Common,
    ),
    t("http://creativecommons.org/ns#Distribution", Action, Common),
    t("http://creativecommons.org/ns#Notice", Action, Common),
    t("http://creativecommons.org/ns#Reproduction", Action, Common),
    t("http://creativecommons.org/ns#ShareAlike", Action, Common),
    t("http://creativecommons.org/ns#Sharing", Action, Common),
    t("http://creativecommons.org/ns#SourceCode", Action, Common),
    t(o!("acceptTracking"), Action, Common),
    t(o!("aggregate"), Action, Common),
    t(o!("annotate"), Action, Common),
    t(o!("anonymize"), Action, Common),
    t(o!("archive"), Action, Common),
    t(o!("attribute"), Action, Common),
    t(o!("compensate"), Action, Common),
    t(o!("concurrentUse"), Action, Common),
    t(o!("delete"), Action, Common),
    t(o!("derive"), Action, Common),
    t(o!("digitize"), Action, Common),
    t(o!("display"), Action, Common),
    t(o!("distribute"), Action, Common),
    t(o!("ensureExclusivity"), Action, Common),
    t(o!("execute"), Action, Common),
    t(o!("extract"), Action, Common),
    t(o!("give"), Action, Common),
    t(o!("grantUse"), Action, Common),
    t(o!("include"), Action, Common),
    t(o!("index"), Action, Common),
    t(o!("inform"), Action, Common),
    t(o!("install"), Action, Common),
    t(o!("modify"), Action, Common),
    t(o!("move"), Action, Common),
    t(o!("nextPolicy"), Action, Common),
    t(o!("obtainConsent"), Action, Common),
    t(o!("play"), Action, Common),
    t(o!("present"), Action, Common),
    t(o!("print"), Action, Common),
    t(o!("read"), Action, Common),
    t(o!("reproduce"), Action, Common),
    t(o!("reviewPolicy"), Action, Common),
    t(o!("sell"), Action, Common),
    t(o!("stream"), Action, Common),
    t(o!("synchronize"), Action, Common),
    t(o!("textToSpeech"), Action, Common),
    t(o!("transform"), Action, Common),
    t(o!("translate"), Action, Common),
    t(o!("uninstall"), Action, Common),
    t(o!("watermark"), Action, Common),
    t(o!("adHocShare"), Action, Deprecated),
    t(o!("append"), Action, Deprecated),
    t(o!("appendTo"), Action, Deprecated),
    t(o!("attachPolicy"), Action, Deprecated),
    t(o!("attachSource"), Action, Deprecated),
    t(o!("commercialize"), Action, Deprecated),
    t(o!("copy"), Action, Deprecated),
    t(o!("export"), Action, Deprecated),
    t(o!("extractChar"), Action, Deprecated),
    t(o!("extractPage"), Action, Deprecated),
    t(o!("extractWord"), Action, Deprecated),
    t(o!("lease"), Action, Deprecated),
    t(o!("lend"), Action, Deprecated),
    t(o!("license"), Action, Deprecated),
    t(o!("pay"), Action, Deprecated),
    t(o!("preview"), Action, Deprecated),
    t(o!("secondaryUse"), Action, Deprecated),
    t(o!("share"), Action, Deprecated),
    t(o!("shareAlike"), Action, Deprecated),
    t(o!("write"), Action, Deprecated),
    t(o!("writeTo"), Action, Deprecated),
    t(o!("absolutePosition"), LeftOperand, Common),
    t(o!("absoluteSize"), LeftOperand, Common),
    t(o!("absoluteSpatialPosition"), LeftOperand, Common),
    t(o!("absoluteTemporalPosition"), LeftOperand, Common),
    t(o!("count"), LeftOperand, Common),
    t(o!("dateTime"), LeftOperand, Common),
    t(o!("delayPeriod"), LeftOperand, Common),
    t(o!("deliveryChannel"), LeftOperand, Common),
    t(o!("elapsedTime"), LeftOperand, Common),
    t(o!("event"), LeftOperand, Common),
    t(o!("fileFormat"), LeftOperand, Common),
    t(o!("industry"), LeftOperand, Common),
    t(o!("language"), LeftOperand, Common),
    t(o!("media"), LeftOperand, Common),
    t(o!("meteredTime"), LeftOperand, Common),
    t(o!("payAmount"), LeftOperand, Common),
    t(o!("percentage"), LeftOperand, Common),
    t(o!("product"), LeftOperand, Common),
    t(o!("purpose"), LeftOperand, Common),
    t(o!("recipient"), LeftOperand, Common),
    t(o!("relativePosition"), LeftOperand, Common),
    t(o!("relativeSize"), LeftOperand, Common),
    t(o!("relativeSpatialPosition"), LeftOperand, Common),
    t(o!("relativeTemporalPosition"), LeftOperand, Common),
    t(o!("resolution"), LeftOperand, Common),
    t(o!("spatial"), LeftOperand, Common),
    t(o!("spatialCoordinates"), LeftOperand, Common),
    t(o!("systemDevice"), LeftOperand, Common),
    t(o!("timeInterval"), LeftOperand, Common),
    t(o!("unitOfCount"), LeftOperand, Common),
    t(o!("version"), LeftOperand, Common),
    t(o!("virtualLocation"), LeftOperand, Common),
    t(o!("device"), LeftOperand, Deprecated),
    t(o!("system"), LeftOperand, Deprecated),
    t(o!("eq"), Operator, Core),
    t(o!("gt"), Operator, Core),
    t(o!("gteq"), Operator, Core),
    t(o!("hasPart"), Operator, Core),
    t(o!("isA"), Operator, Core),
    t(o!("isAllOf"), Operator, Core),
    t(o!("isAnyOf"), Operator, Core),
    t(o!("isNoneOf"), Operator, Core),
    t(o!("isPartOf"), Operator, Core),
    t(o!("lt"), Operator, Core),
    t(o!("lteq"), Operator, Core),
    t(o!("neq"), Operator, Core),
    t(o!("policyUsage"), RightOperand, Common),
    t(o!("invalid"), ConflictTerm, Core),
    t(o!("perm"), ConflictTerm, Core),
    t(o!("prohibit"), ConflictTerm, Core),
    t(o!("and"), LogicalOperand, Core),
    t(o!("andSequence"), LogicalOperand, Core),
    t(o!("or"), LogicalOperand, Core),
    t(o!("xone"), LogicalOperand, Core),
];
