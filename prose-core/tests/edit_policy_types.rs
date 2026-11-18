//! `EditRules::policy_types` (issue #79): the policy types a host offers and
//! accepts. The reducer refuses another one, a new policy starts with an
//! allowed one, an incoming document is scoped (`out_of_scope`), and a
//! closed host source flags what it holds (`prose.value-not-allowed`).
mod common;
mod corpus;
use corpus::target_str;
use prose_core::edit::Severity;
use prose_core::edit::{
    EditError, EditEvent, EditRules, NewItem, NodeId, SlotPath, out_of_scope, read_model,
    strip_out_of_scope, write_jsonld,
};
use prose_core::profile::{HintSource, Prefixes, SourceValue, ValueSource, ValueSources, Wording};
use prose_core::validate::{Options, RuleId, validate_json};
use serde_json::{Value, json};

const ODRL: &str = "http://www.w3.org/ns/odrl/2/";

fn only(types: &[&str]) -> EditRules {
    EditRules {
        policy_types: types.iter().map(|t| t.to_string()).collect(),
        ..EditRules::default()
    }
}

fn odrl(name: &str) -> String {
    format!("{ODRL}{name}")
}

fn rules_of(names: &[&str]) -> EditRules {
    EditRules {
        policy_types: names.iter().map(|n| odrl(n)).collect(),
        ..EditRules::default()
    }
}

fn doc_of(v: Value) -> prose_core::edit::EditDoc {
    read_model(&v.to_string()).unwrap()
}

fn set_kind(
    doc: &mut prose_core::edit::EditDoc,
    rules: &EditRules,
    value: &str,
) -> Result<(), EditError> {
    let id = doc.policies[0].id;
    doc.apply(
        &EditEvent::SetText {
            slot: "policy[0]#kind".parse::<SlotPath>().unwrap(),
            expect: id,
            value: value.into(),
        },
        rules,
    )
}

fn offer() -> Value {
    json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@type": "Offer", "uid": "urn:p:1",
        "permission": [{"target": "urn:t", "action": "use"}]
    })
}

// --- the rules ---------------------------------------------------------------

#[test]
fn an_empty_list_allows_every_type() {
    let r = EditRules::default();
    assert!(r.policy_types.is_empty());
    for t in ["Set", "Offer", "ex:Anything", "", "Policy", "urn:x:y"] {
        assert!(r.policy_type_allowed(t, &Prefixes::default()), "{t}");
    }
    assert_eq!(r.policy_type_default(), None);
}

#[test]
fn the_bare_prefixed_and_full_spellings_of_an_odrl_class_are_one_type() {
    let r = rules_of(&["Set"]);
    let p = Prefixes::default();
    for t in ["Set", "odrl:Set", &odrl("Set")] {
        assert!(r.policy_type_allowed(t, &p), "{t}");
    }
    for t in ["Offer", "odrl:Offer", &odrl("Offer"), "ex:Set", "set"] {
        assert!(!r.policy_type_allowed(t, &p), "{t}");
    }
}

#[test]
fn an_empty_type_or_policy_alone_counts_as_set() {
    let sets = rules_of(&["Set"]);
    let offers = rules_of(&["Offer"]);
    let p = Prefixes::default();
    for t in ["", "Policy", "odrl:Policy", &odrl("Policy")] {
        assert!(sets.policy_type_allowed(t, &p), "{t}");
        assert!(!offers.policy_type_allowed(t, &p), "{t}");
    }
}

#[test]
fn a_compact_iri_is_compared_by_its_expansion() {
    let r = EditRules {
        policy_types: vec!["https://example.org/p#Research".into()],
        ..EditRules::default()
    };
    let doc = json!({"@context": {"ex": "https://example.org/p#"}});
    let p = Prefixes::of_document(&doc);
    assert!(r.policy_type_allowed("ex:Research", &p));
    assert!(r.policy_type_allowed("https://example.org/p#Research", &p));
    assert!(!r.policy_type_allowed("ex:Other", &p));
    // Without the prefix bound it is another IRI.
    assert!(!r.policy_type_allowed("ex:Research", &Prefixes::default()));
}

#[test]
fn the_first_allowed_type_is_the_default() {
    assert_eq!(
        rules_of(&["Offer", "Set"]).policy_type_default(),
        Some(odrl("Offer").as_str())
    );
}

#[test]
fn a_new_policy_kind_is_the_default_when_allowed_else_the_first_allowed() {
    assert_eq!(EditRules::default().new_policy_kind(), "Set");
    assert_eq!(rules_of(&["Offer", "Set"]).new_policy_kind(), "Set");
    assert_eq!(rules_of(&["Offer", "Agreement"]).new_policy_kind(), "Offer");
    let ex = only(&["https://example.org/p#Research"]);
    assert_eq!(ex.new_policy_kind(), "https://example.org/p#Research");
}

// --- the reducer -------------------------------------------------------------

#[test]
fn a_type_the_host_does_not_allow_is_refused() {
    let r = rules_of(&["Set"]);
    let mut doc = doc_of(json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld", "@type": "Set", "uid": "urn:p",
        "permission": [{"target": "urn:t", "action": "use"}]
    }));
    let before = doc.clone();
    let err = set_kind(&mut doc, &r, "Offer").unwrap_err();
    assert!(matches!(err, EditError::NotAllowed(_)), "{err}");
    assert!(err.to_string().contains("not one of the types"), "{err}");
    assert_eq!(doc, before);
    // Every spelling of the other classes is refused alike.
    for t in ["odrl:Agreement", &odrl("Agreement"), "ex:Foo"] {
        assert!(set_kind(&mut doc, &r, t).is_err(), "{t}");
    }
}

#[test]
fn a_listed_type_is_accepted_in_every_spelling() {
    let r = rules_of(&["Set", "Offer"]);
    let mut doc = doc_of(offer());
    for t in ["Set", "odrl:Offer", &odrl("Set"), "Offer"] {
        set_kind(&mut doc, &r, t).unwrap_or_else(|e| panic!("{t}: {e}"));
        assert_eq!(doc.policies[0].kind, t);
    }
}

#[test]
fn setting_the_type_the_document_already_holds_is_a_no_op_that_is_allowed() {
    // A foreign document stays editable: its outside type is kept.
    let r = rules_of(&["Set"]);
    let mut doc = doc_of(offer());
    set_kind(&mut doc, &r, "Offer").unwrap();
    assert_eq!(doc.policies[0].kind, "Offer");
    // Moving to another unlisted one is still refused.
    assert!(set_kind(&mut doc, &r, "Agreement").is_err());
    // And back to a listed one is allowed.
    set_kind(&mut doc, &r, "Set").unwrap();
}

#[test]
fn an_empty_type_is_still_refused_as_invalid() {
    let r = rules_of(&["Set"]);
    let mut doc = doc_of(offer());
    let err = set_kind(&mut doc, &r, "").unwrap_err();
    assert!(matches!(err, EditError::Invalid(_)), "{err}");
}

#[test]
fn a_compact_type_is_read_with_the_prefixes_of_the_document() {
    let r = only(&["https://example.org/p#Research"]);
    let mut doc = doc_of(json!({
        "@context": ["http://www.w3.org/ns/odrl.jsonld", {"ex": "https://example.org/p#"}],
        "@type": "Set", "uid": "urn:p", "permission": [{"target": "urn:t", "action": "use"}]
    }));
    set_kind(&mut doc, &r, "ex:Research").unwrap();
    assert!(set_kind(&mut doc, &r, "ex:Other").is_err());
}

#[test]
fn the_default_rules_leave_the_reducer_alone() {
    let r = EditRules::default();
    let mut doc = doc_of(offer());
    for t in ["Set", "ex:Whatever", "https://x.y/z"] {
        set_kind(&mut doc, &r, t).unwrap();
    }
}

#[test]
fn a_new_policy_gets_an_allowed_type() {
    let add = |r: &EditRules| {
        let mut doc = doc_of(offer());
        let id: NodeId = doc.policies[0].id;
        let _ = id;
        doc.apply(
            &EditEvent::Add {
                list: prose_core::edit::ListPath::policies(),
                index: 1,
                expect: None,
                item: NewItem::Default,
            },
            r,
        )
        .unwrap();
        doc.policies[1].kind.clone()
    };
    assert_eq!(add(&EditRules::default()), "Set");
    assert_eq!(add(&rules_of(&["Set", "Offer"])), "Set");
    assert_eq!(add(&rules_of(&["Offer", "Agreement"])), "Offer");
}

// --- the scope of an incoming document ---------------------------------------

fn stripped(mut v: Value, r: &EditRules) -> (Value, Vec<prose_core::edit::OutOfScope>) {
    let hits = strip_out_of_scope(&mut v, r);
    (v, hits)
}

#[test]
fn nothing_is_out_of_scope_without_a_list_of_types() {
    assert!(out_of_scope(&offer(), &EditRules::default()).is_empty());
    let (v, hits) = stripped(offer(), &EditRules::default());
    assert_eq!(v, offer());
    assert!(hits.is_empty());
}

#[test]
fn an_allowed_type_is_in_scope() {
    assert!(out_of_scope(&offer(), &rules_of(&["Offer"])).is_empty());
    assert!(out_of_scope(&offer(), &rules_of(&["Set", "Offer"])).is_empty());
}

#[test]
fn a_type_the_host_does_not_allow_is_reported_and_rewritten_to_the_first_allowed() {
    let r = rules_of(&["Set"]);
    let found = out_of_scope(&offer(), &r);
    assert_eq!(found.len(), 1);
    let h = &found[0];
    assert_eq!(h.path, "@type");
    assert_eq!(h.key, "@type");
    assert_eq!(h.limit, "policyTypes");
    assert_eq!(h.value.as_deref(), Some("Offer"));
    assert_eq!(h.replaced_with.as_deref(), Some("Set"));
    let (v, hits) = stripped(offer(), &r);
    assert_eq!(v["@type"], "Set");
    assert_eq!(hits, found);
    // Everything else is kept.
    assert_eq!(v["permission"], offer()["permission"]);
}

#[test]
fn other_limits_report_no_type() {
    let r = EditRules {
        policy_uid: prose_core::edit::Limit::NONE,
        ..EditRules::default()
    };
    let hits = out_of_scope(&offer(), &r);
    assert_eq!(hits[0].value, None);
    assert_eq!(hits[0].replaced_with, None);
}

#[test]
fn the_rewrite_is_spelled_as_the_document_spells_the_type() {
    let r = rules_of(&["Set"]);
    let with = |t: &str| {
        let mut v = offer();
        v["@type"] = json!(t);
        stripped(v, &r)
    };
    assert_eq!(with("odrl:Offer").0["@type"], "odrl:Set");
    assert_eq!(with(&odrl("Offer")).0["@type"], odrl("Set"));
    assert_eq!(with("Offer").0["@type"], "Set");
    // The reported value is the one as written.
    assert_eq!(with("odrl:Offer").1[0].value.as_deref(), Some("odrl:Offer"));
}

#[test]
fn a_type_that_is_not_odrl_is_written_compact_when_the_document_binds_a_prefix() {
    let r = only(&["https://example.org/p#Research"]);
    let mut v = offer();
    v["@context"] = json!(["http://www.w3.org/ns/odrl.jsonld", {"ex": "https://example.org/p#"}]);
    let (v, hits) = stripped(v, &r);
    assert_eq!(v["@type"], "ex:Research");
    assert_eq!(hits[0].replaced_with.as_deref(), Some("ex:Research"));
    // Without a binding, in full.
    let (v, _) = stripped(offer(), &r);
    assert_eq!(v["@type"], "https://example.org/p#Research");
}

#[test]
fn the_first_allowed_type_is_chosen_when_several_are_allowed() {
    let r = rules_of(&["Agreement", "Set"]);
    let (v, hits) = stripped(offer(), &r);
    assert_eq!(v["@type"], "Agreement");
    assert_eq!(hits[0].replaced_with.as_deref(), Some("Agreement"));
}

#[test]
fn an_array_of_types_keeps_policy_and_the_other_entries() {
    let r = rules_of(&["Set"]);
    let mut v = offer();
    v["@type"] = json!(["Policy", "Offer", "ex:Extra"]);
    let (v, hits) = stripped(v, &r);
    assert_eq!(v["@type"], json!(["Policy", "Set", "ex:Extra"]));
    assert_eq!(hits[0].value.as_deref(), Some("Offer"));
    assert_eq!(hits.len(), 1);
}

#[test]
fn the_type_the_reader_would_pick_is_the_one_judged() {
    // `Policy` is skipped: the reader takes the first other entry.
    let r = rules_of(&["Offer"]);
    let mut v = offer();
    v["@type"] = json!(["Policy", "Offer"]);
    assert!(out_of_scope(&v, &r).is_empty());
}

#[test]
fn the_type_alias_is_found() {
    let r = rules_of(&["Set"]);
    let mut v = offer();
    let o = v.as_object_mut().unwrap();
    o.remove("@type");
    o.insert("type".into(), json!("Offer"));
    let (v, hits) = stripped(v, &r);
    assert_eq!(v["type"], "Set");
    assert!(v.get("@type").is_none());
    assert_eq!(hits[0].key, "type");
    assert_eq!(hits[0].path, "type");
}

#[test]
fn every_policy_of_a_graph_is_judged() {
    let r = rules_of(&["Set"]);
    let v = json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@graph": [
            {"@type": "Set", "uid": "urn:a", "permission": [{"target": "urn:t", "action": "use"}]},
            {"@type": "Offer", "uid": "urn:b", "permission": [{"target": "urn:t", "action": "use"}]},
            {"@type": "Agreement", "uid": "urn:c", "permission": [{"target": "urn:t", "action": "use"}]}
        ]
    });
    let (out, hits) = stripped(v, &r);
    let paths: Vec<&str> = hits.iter().map(|h| h.path.as_str()).collect();
    assert_eq!(paths, ["@graph[1].@type", "@graph[2].@type"]);
    for i in 0..3 {
        assert_eq!(out["@graph"][i]["@type"], "Set");
    }
}

#[test]
fn a_node_that_is_not_a_policy_keeps_its_type() {
    // The reader skips such a node and keeps it as written; so does the scope.
    let r = rules_of(&["Set"]);
    let v = json!({
        "@context": "http://www.w3.org/ns/odrl.jsonld",
        "@graph": [
            {"@id": "urn:x", "@type": "Asset"},
            {"@type": "Offer", "uid": "urn:b", "permission": [{"target": "urn:t", "action": "use"}]}
        ]
    });
    let (out, hits) = stripped(v, &r);
    let paths: Vec<&str> = hits.iter().map(|h| h.path.as_str()).collect();
    assert_eq!(paths, ["@graph[1].@type"]);
    assert_eq!(out["@graph"][0], json!({"@id": "urn:x", "@type": "Asset"}));
    // A node without a type or rules is no policy either.
    let (out, hits) = stripped(
        json!({"@graph": [{"@id": "urn:y"}, {"permission": [{"action": "use"}]}]}),
        &rules_of(&["Offer"]),
    );
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(out["@graph"][0].get("@type").is_none(), "{out}");
}

#[test]
fn a_top_level_array_of_policies_is_judged() {
    let r = rules_of(&["Set"]);
    let v = json!([offer(), offer()]);
    let paths: Vec<String> = out_of_scope(&v, &r).into_iter().map(|h| h.path).collect();
    assert_eq!(paths, ["[0].@type", "[1].@type"]);
}

#[test]
fn a_missing_type_counts_as_set() {
    let mut v = offer();
    v.as_object_mut().unwrap().remove("@type");
    // Set is allowed: nothing to do.
    assert!(out_of_scope(&v, &rules_of(&["Set", "Offer"])).is_empty());
    // Set is not: the type is written, and nothing was there to report.
    let (out, hits) = stripped(v, &rules_of(&["Offer"]));
    assert_eq!(out["@type"], "Offer");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].value, None);
    assert_eq!(hits[0].replaced_with.as_deref(), Some("Offer"));
    assert_eq!(hits[0].path, "@type");
}

#[test]
fn policy_alone_counts_as_set_and_stays_beside_the_new_type() {
    let mut v = offer();
    v["@type"] = json!("Policy");
    assert!(out_of_scope(&v, &rules_of(&["Set"])).is_empty());
    let (out, hits) = stripped(v.clone(), &rules_of(&["Offer"]));
    assert_eq!(out["@type"], json!(["Policy", "Offer"]));
    assert_eq!(hits[0].value, None);
    v["@type"] = json!(["Policy"]);
    let (out, _) = stripped(v, &rules_of(&["Offer"]));
    assert_eq!(out["@type"], json!(["Policy", "Offer"]));
}

#[test]
fn the_key_order_of_the_document_is_kept() {
    let r = rules_of(&["Set"]);
    let (out, _) = stripped(offer(), &r);
    let keys: Vec<&String> = out.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["@context", "@type", "uid", "permission"]);
}

#[test]
fn the_rewritten_document_reads_back_with_the_allowed_type() {
    let r = rules_of(&["Set"]);
    let (out, _) = stripped(offer(), &r);
    let doc = read_model(&out.to_string()).unwrap();
    assert_eq!(doc.policies[0].kind, "Set");
    assert_eq!(write_jsonld(&doc)["@type"], "Set");
    // A second pass has nothing left to do.
    assert!(out_of_scope(&out, &r).is_empty());
}

// --- the host source ---------------------------------------------------------

fn source(types: &[&str], closed: bool) -> ValueSource {
    ValueSource::new(types.iter().map(|t| SourceValue::new(t)).collect(), closed)
}

fn wording_for(types: &[&str], closed: bool) -> Wording {
    let mut s = ValueSources::new();
    s.set_policy_type(source(types, closed));
    Wording::new().with_value_sources(s)
}

fn run(v: &Value, w: &Wording) -> prose_core::validate::Report {
    validate_json(&v.to_string(), w, &Options::strict())
}

#[test]
fn the_policy_type_source_round_trips() {
    let mut s = ValueSources::new();
    assert!(s.is_empty());
    assert!(s.policy_type().is_none());
    s.set_policy_type(source(&[&odrl("Set")], true));
    assert!(!s.is_empty());
    assert!(s.policy_type().unwrap().is_closed());
    assert!(s.remove_policy_type());
    assert!(!s.remove_policy_type());
    assert!(s.is_empty());
    let w = Wording::new().with_value_sources({
        let mut s = ValueSources::new();
        s.set_policy_type(source(&[&odrl("Set")], true));
        s
    });
    assert!(w.policy_type_source().is_some());
    assert!(Wording::new().policy_type_source().is_none());
}

#[test]
fn a_type_the_closed_source_does_not_list_is_flagged_and_kept() {
    let w = wording_for(&[&odrl("Set")], true);
    let r = run(&offer(), &w);
    let f = r
        .findings
        .iter()
        .find(|f| f.rule == RuleId::ProseValueNotAllowed)
        .unwrap_or_else(|| panic!("{:?}", r.findings));
    assert_eq!(f.severity, Severity::Error);
    let e = f.expected.as_ref().unwrap();
    assert_eq!(e.source, Some(HintSource::Host));
    assert!(e.closed);
    assert_eq!(e.values, [odrl("Set")]);
    assert!(f.message.contains("\"Offer\""), "{}", f.message);
    assert!(f.message.contains("the host allows"), "{}", f.message);
    assert!(target_str(&f.target).ends_with("#kind"), "{:?}", f.target);
    assert_eq!(f.term.as_ref().unwrap().iri, odrl("Offer"));
}

#[test]
fn a_type_the_source_lists_is_clean_in_every_spelling() {
    let w = wording_for(&[&odrl("Set"), &odrl("Offer")], true);
    for t in ["Set", "odrl:Set", &odrl("Set"), "Offer"] {
        let mut v = offer();
        v["@type"] = json!(t);
        let r = run(&v, &w);
        assert!(
            r.findings
                .iter()
                .all(|f| f.rule != RuleId::ProseValueNotAllowed),
            "{t}: {:?}",
            r.findings
        );
    }
}

#[test]
fn a_compact_type_equals_its_expansion_in_the_source() {
    let w = wording_for(&["https://example.org/p#Research"], true);
    let mut v = offer();
    v["@context"] = json!(["http://www.w3.org/ns/odrl.jsonld", {"ex": "https://example.org/p#"}]);
    v["@type"] = json!("ex:Research");
    let r = run(&v, &w);
    assert!(
        r.findings
            .iter()
            .all(|f| f.rule != RuleId::ProseValueNotAllowed),
        "{:?}",
        r.findings
    );
}

#[test]
fn a_compact_type_the_source_does_not_list_is_flagged_and_a_compact_listing_matches() {
    let ctx = json!(["http://www.w3.org/ns/odrl.jsonld", {"ex": "https://example.org/p#"}]);
    let flagged = |source: &[&str], kind: &str| {
        let mut v = offer();
        v["@context"] = ctx.clone();
        v["@type"] = json!(kind);
        run(&v, &wording_for(source, true))
            .findings
            .iter()
            .any(|f| f.rule == RuleId::ProseValueNotAllowed)
    };
    assert!(flagged(&["https://example.org/p#Research"], "ex:Other"));
    assert!(!flagged(&["https://example.org/p#Research"], "ex:Research"));
    // The host may list it compact, the document write it in full.
    assert!(!flagged(&["ex:Research"], "https://example.org/p#Research"));
}

#[test]
fn an_empty_source_or_a_suggesting_one_flags_nothing() {
    for w in [wording_for(&[], true), wording_for(&[&odrl("Set")], false)] {
        let r = run(&offer(), &w);
        assert!(
            r.findings
                .iter()
                .all(|f| f.rule != RuleId::ProseValueNotAllowed),
            "{:?}",
            r.findings
        );
    }
}

#[test]
fn a_missing_type_is_set_for_the_source() {
    let w = wording_for(&[&odrl("Offer")], true);
    let mut v = offer();
    v.as_object_mut().unwrap().remove("@type");
    let r = run(&v, &w);
    let f = r
        .findings
        .iter()
        .find(|f| f.rule == RuleId::ProseValueNotAllowed)
        .unwrap();
    assert!(f.message.contains("\"Set\""), "{}", f.message);
}

#[test]
fn the_default_wording_has_no_policy_type_finding() {
    let r = run(&offer(), &Wording::new());
    assert!(
        r.findings
            .iter()
            .all(|f| f.rule != RuleId::ProseValueNotAllowed)
    );
}
