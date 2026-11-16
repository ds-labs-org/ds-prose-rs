//! `ProseEditorHandle`: mounts `<OdrlProseView>` in Edit mode into a DOM
//! element and reports every committed edit as JSON-LD text.
use prose_yew::prose_core::edit::{EditDoc, EditRules, Limit, write_jsonld};
use prose_yew::prose_core::profile::Wording;
use prose_yew::prose_core::validate::Options;
use prose_yew::EditConfig;
use serde_json::Value;
use std::rc::Rc;

/// Sets the list limits of `rules` from a host's `EditLimits` value.
pub(crate) fn apply_limits(_rules: &mut EditRules, _limits: Option<&Value>) -> Result<(), String> {
    todo!()
}

/// The JSON-LD the editor writes for `doc`, compact.
pub(crate) fn emitted_text(_doc: &EditDoc) -> String {
    todo!()
}

/// `EditConfig::default()`, then the wording, then the validation options,
/// then the limits.
pub(crate) fn build_config(
    _wording: Option<&Rc<Wording>>,
    _options: Option<&Options>,
    _limits: Option<&Value>,
) -> Result<EditConfig, String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use prose_yew::prose_core::edit::read_model;
    use prose_yew::prose_core::profile::ProfileVocab;
    use serde_json::json;

    const RESEARCH: &str = include_str!("../../prose-core/tests/profiles/research.jsonld");
    const POLICY: &str = include_str!("../../prose-core/tests/profiles/research-policy.json");

    /// Every name with the limit it sets.
    fn fields(r: &EditRules) -> [(&'static str, Limit); 22] {
        [
            ("policies", r.policies),
            ("permission", r.permission),
            ("prohibition", r.prohibition),
            ("obligation", r.obligation),
            ("duty", r.duty),
            ("remedy", r.remedy),
            ("consequence", r.consequence),
            ("policyAssigner", r.policy_assigner),
            ("policyAssignee", r.policy_assignee),
            ("policyTarget", r.policy_target),
            ("policyAction", r.policy_action),
            ("ruleAssigner", r.rule_assigner),
            ("ruleAssignee", r.rule_assignee),
            ("ruleTarget", r.rule_target),
            ("ruleAction", r.rule_action),
            ("actionRefinement", r.action_refinement),
            ("entityRefinement", r.entity_refinement),
            ("constraints", r.constraints),
            ("logicalChildren", r.logical_children),
            ("rightOperandValues", r.right_operand_values),
            ("profile", r.profile),
            ("inheritFrom", r.inherit_from),
        ]
    }

    fn limits(v: Value) -> Result<EditRules, String> {
        let mut r = EditRules::default();
        apply_limits(&mut r, Some(&v)).map(|_| r)
    }

    #[test]
    fn every_name_sets_its_own_field() {
        let defaults = EditRules::default();
        let names: Vec<&str> = fields(&defaults).iter().map(|(n, _)| *n).collect();
        assert_eq!(names.len(), 22);
        for (i, name) in names.iter().enumerate() {
            let r = limits(json!({ *name: { "min": 2, "max": 5 } })).unwrap();
            let want = Limit {
                min: 2,
                max: Some(5),
            };
            for (j, (n, l)) in fields(&r).iter().enumerate() {
                if i == j {
                    assert_eq!(*l, want, "{name} sets {n}");
                } else {
                    assert_eq!(*l, fields(&defaults)[j].1, "{name} must not set {n}");
                }
            }
        }
    }

    #[test]
    fn a_given_entry_replaces_the_default() {
        // policies defaults to at least 1; {max: 1} replaces it: min 0.
        let r = limits(json!({ "policies": { "max": 1 } })).unwrap();
        assert_eq!(
            r.policies,
            Limit {
                min: 0,
                max: Some(1)
            }
        );
        // ruleAction defaults to at least 1; {} replaces it with any.
        assert_eq!(limits(json!({ "ruleAction": {} })).unwrap().rule_action, Limit::ANY);
        assert_eq!(
            limits(json!({ "permission": { "min": 1, "max": null } }))
                .unwrap()
                .permission,
            Limit::at_least(1)
        );
        assert_eq!(
            limits(json!({ "policyAssigner": { "max": 0 } }))
                .unwrap()
                .policy_assigner,
            Limit::NONE
        );
    }

    #[test]
    fn null_and_none_reset_to_the_default_limits() {
        let mut r = limits(json!({ "policyAssigner": { "max": 0 }, "policies": {} })).unwrap();
        r.allow_reorder = !r.allow_reorder;
        let reorder = r.allow_reorder;
        apply_limits(&mut r, Some(&Value::Null)).unwrap();
        let d = EditRules::default();
        assert_eq!(fields(&r), fields(&d));
        assert_eq!(r.allow_reorder, reorder, "only the limits are reset");
        let mut r = limits(json!({ "profile": { "max": 0 } })).unwrap();
        apply_limits(&mut r, None).unwrap();
        assert_eq!(fields(&r), fields(&d));
    }

    #[test]
    fn bad_limits_are_refused_and_change_nothing() {
        for bad in [
            json!({ "bogus": {} }),
            json!({ "permission": { "min": -1 } }),
            json!({ "permission": { "max": -1 } }),
            json!({ "permission": { "min": 1.5 } }),
            json!({ "permission": { "max": 0.5 } }),
            json!({ "permission": { "min": "1" } }),
            json!({ "permission": { "min": 3, "max": 2 } }),
            json!({ "permission": { "foo": 1 } }),
            json!({ "permission": 3 }),
            json!([]),
            json!("x"),
        ] {
            let mut r = EditRules::default();
            // A valid entry before the bad one must not stick.
            let mut v = bad.clone();
            if let Some(o) = v.as_object_mut() {
                o.insert("duty".into(), json!({ "max": 0 }));
            }
            for value in [bad, v] {
                let before = fields(&r);
                assert!(apply_limits(&mut r, Some(&value)).is_err(), "{value}");
                assert_eq!(fields(&r), before, "{value}");
            }
        }
    }

    #[test]
    fn emitted_text_is_compact_write_jsonld() {
        let doc = read_model(POLICY).unwrap();
        let text = emitted_text(&doc);
        assert_eq!(text, serde_json::to_string(&write_jsonld(&doc)).unwrap());
        assert!(!text.contains('\n') && !text.contains(": "), "{text}");
        assert_eq!(read_model(&text).unwrap(), doc, "it reads back");
    }

    #[test]
    fn limits_survive_with_strict() {
        let l = json!({ "policyAssigner": { "max": 0 }, "policies": { "min": 1, "max": 1 } });
        let w = Rc::new(Wording::new().with_profile(ProfileVocab::parse(RESEARCH).unwrap()));
        let c = build_config(Some(&w), Some(&Options::strict()), Some(&l)).unwrap();
        assert!(c.validator.is_some());
        assert_eq!(c.rules.policy_assigner, Limit::NONE);
        assert_eq!(c.rules.policies, Limit::exactly(1));
        assert!(Rc::ptr_eq(&c.wording, &w));
        let c = build_config(None, None, None).unwrap();
        assert!(c.validator.is_none());
        assert_eq!(fields(&c.rules), fields(&EditRules::default()));
        assert!(build_config(None, None, Some(&json!({ "x": {} }))).is_err());
    }
}
