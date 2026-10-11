//! What the demo's strict mode shows, as pure functions the browser tests
//! and the host tests both reach: where "Go to" lands, whether the export
//! is open, and what the status badge says.
use prose_yew::prose_core::validate::{Finding, Report};

/// The selectors "Go to" tries for a finding, best first: the first one that
/// matches an element of the Edit view gets the focus.
pub fn jump_selectors(_f: &Finding) -> Vec<String> {
    todo!("jump_selectors")
}

/// The first of [`jump_selectors`].
pub fn jump_selector(f: &Finding) -> String {
    jump_selectors(f).into_iter().next().unwrap_or_default()
}

/// Whether "Copy conformant JSON" is open: only a strict report that
/// conforms certifies anything.
pub fn export_enabled(_r: &Report) -> bool {
    todo!("export_enabled")
}

/// Why the export is shut, when it is.
pub fn export_note(_r: &Report) -> Option<String> {
    todo!("export_note")
}

/// The status badge: a class and the sentence. `profile` is the name of the
/// loaded profile, when there is one.
pub fn badge(_r: &Report, _profile: Option<&str>) -> (&'static str, String) {
    todo!("badge")
}

/// The report as the `ds-prose/validation-report@1` JSON the download holds.
pub fn report_text(_r: &Report) -> String {
    todo!("report_text")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::profile_field::{PRESETS, ProfileMsg, ProfileState, reduce};
    use prose_yew::prose_core::edit::read_model;
    use prose_yew::prose_core::validate::{Options, Validator, Verdict};
    use std::sync::Arc;

    fn closed() -> ProfileState {
        let p = PRESETS
            .iter()
            .find(|p| p.id == "profile-sample-closed")
            .expect("the closed sample is a preset");
        reduce(&ProfileState::default(), ProfileMsg::Use(p.text.into()))
    }

    fn report(json: &str, options: Options, with_profile: bool) -> Report {
        let w = match (&closed().wording, with_profile) {
            (Some(w), true) => w.0.clone(),
            _ => Default::default(),
        };
        Validator::new(Arc::new((*w).clone()), options).validate(&read_model(json).unwrap())
    }

    fn preset(label: &str) -> &'static str {
        fixtures::ALL
            .iter()
            .find(|p| p.label == label)
            .unwrap_or_else(|| panic!("no preset {label}"))
            .json
    }

    const VIOLATES: &str = "Research agreement (violates the profile)";

    #[test]
    fn the_strict_presets_are_the_shared_fixtures() {
        assert_eq!(
            preset(VIOLATES),
            include_str!("../../prose-core/tests/strict/violating.json")
        );
        assert_eq!(
            preset("Strict clean"),
            include_str!("../../prose-core/tests/strict/clean.json")
        );
        // The first four are as they were.
        assert_eq!(fixtures::ALL[3].label, "Research agreement");
    }

    #[test]
    fn closed_sample_is_the_closed_profile_file() {
        let s = closed();
        assert_eq!(
            s.text,
            include_str!("../../prose-core/tests/profiles/research-closed.jsonld")
        );
        assert!(s.wording.is_some(), "{:?}", s.error);
    }

    #[test]
    fn a_slot_finding_jumps_to_its_slot_then_its_node() {
        let r = report(preset(VIOLATES), Options::strict(), true);
        let f = r
            .findings
            .iter()
            .find(|f| f.rule.as_str() == "profile.unknown-term" && f.message.contains("foo:bar"))
            .unwrap_or_else(|| panic!("{:#?}", r.findings));
        let sels = jump_selectors(f);
        assert_eq!(
            sels,
            vec![
                "[data-slot=\"policy[0].permission[2].action[0]#name\"]".to_string(),
                "[data-node=\"policy[0].permission[2].action[0]\"]".to_string(),
            ]
        );
        assert_eq!(jump_selector(f), sels[0]);
    }

    #[test]
    fn a_doc_finding_jumps_to_the_first_node() {
        let r = report(preset("Research agreement"), Options::strict(), false);
        let f = r
            .findings
            .iter()
            .find(|f| matches!(f.target, prose_yew::prose_core::edit::IssueTarget::Doc))
            .unwrap_or_else(|| panic!("{:#?}", r.findings));
        assert_eq!(jump_selectors(f), vec!["[data-node]".to_string()]);
    }

    #[test]
    fn the_export_is_open_only_for_a_strict_report_that_conforms() {
        let clean = report(preset("Strict clean"), Options::strict(), true);
        assert_eq!(clean.verdict, Verdict::Conforms, "{:#?}", clean.findings);
        assert!(export_enabled(&clean));
        assert_eq!(export_note(&clean), None);

        let bad = report(preset(VIOLATES), Options::strict(), true);
        assert!(!export_enabled(&bad));
        let note = export_note(&bad).expect("a reason");
        assert!(note.contains("error"), "{note}");

        // Nothing certifies without the profile the policy declares.
        let open = report(preset("Strict clean"), Options::strict(), false);
        assert_eq!(open.verdict, Verdict::Incomplete);
        assert!(!export_enabled(&open));
        assert_eq!(
            export_note(&open).as_deref(),
            Some("Load profile https://example.org/odrl/profile/research to certify.")
        );

        // An advisory report certifies nothing either.
        let adv = report(preset("Strict clean"), Options::advisory(), true);
        assert!(!export_enabled(&adv));
    }

    #[test]
    fn the_badge_says_the_verdict() {
        let clean = report(preset("Strict clean"), Options::strict(), true);
        assert_eq!(
            badge(&clean, Some("Research Data Profile 0.1.0")),
            (
                "prose-conforms",
                "Conforms to Research Data Profile 0.1.0 (strict)".to_string()
            )
        );
        assert_eq!(
            badge(&clean, None).1,
            "Conforms (strict)",
            "no profile named"
        );

        let bad = report(preset(VIOLATES), Options::strict(), true);
        let (class, text) = badge(&bad, Some("x"));
        assert_eq!(class, "prose-violations-found");
        let c = bad.counts();
        let part =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        assert!(c.error > 1, "{c:?}");
        assert!(
            text.starts_with(&part(c.error, "error", "errors")),
            "{text}"
        );

        let open = report(preset("Strict clean"), Options::strict(), false);
        assert_eq!(
            badge(&open, None),
            (
                "prose-incomplete",
                "Cannot certify: profile https://example.org/odrl/profile/research is not loaded"
                    .to_string()
            )
        );
    }

    #[test]
    fn the_report_text_is_the_schema() {
        let bad = report(preset(VIOLATES), Options::strict(), true);
        let v: serde_json::Value = serde_json::from_str(&report_text(&bad)).unwrap();
        assert_eq!(v["schema"], "ds-prose/validation-report@1");
        assert_eq!(v["verdict"], "violations");
        assert!(report_text(&bad).contains('\n'), "pretty-printed");
    }
}
