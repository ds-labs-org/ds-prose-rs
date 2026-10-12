//! A ready-made host for simple uses: owns the document, applies events
//! with the rules, keeps a capped undo history.
use std::rc::Rc;
use std::sync::Arc;
use yew::prelude::*;

use prose_core::edit::{EditDoc, EditError, EditEvent, EditRules, StrictEdit};
use prose_core::profile::Wording;
use prose_core::validate::{Options, Report, Validator};

const HISTORY_CAP: usize = 50;

#[derive(Clone, PartialEq)]
struct State {
    doc: Rc<EditDoc>,
    history: Vec<Rc<EditDoc>>,
    last_error: Option<EditError>,
    /// The event `last_error` is about.
    refused: Option<(EditEvent, EditError)>,
}

enum Action {
    /// One event, through the gate when there is one.
    Edit(EditEvent, Rc<EditRules>, Option<Validator>),
    /// Several, all or none: one undo step.
    All(Vec<EditEvent>, Rc<EditRules>, Option<Validator>),
    Set(EditDoc),
    Undo,
    /// The gate or the validator changed: a refusal made by the old one
    /// says nothing about the new one.
    DropRefusal,
}

impl State {
    fn commit(&mut self, doc: EditDoc) {
        self.history.push(self.doc.clone());
        if self.history.len() > HISTORY_CAP {
            self.history.remove(0);
        }
        self.doc = Rc::new(doc);
        self.last_error = None;
        self.refused = None;
    }

    fn refuse(&mut self, ev: EditEvent, e: EditError) {
        self.last_error = Some(e.clone());
        self.refused = Some((ev, e));
    }
}

impl Reducible for State {
    type Action = Action;

    fn reduce(self: Rc<Self>, action: Action) -> Rc<Self> {
        let mut next = (*self).clone();
        match action {
            Action::Edit(ev, rules, gate) => {
                let mut doc = (*self.doc).clone();
                match doc.apply_gated(&ev, &rules, gate.as_ref()) {
                    Ok(()) => next.commit(doc),
                    Err(e) => next.refuse(ev, e),
                }
            }
            Action::All(evs, rules, gate) => {
                // Nothing to apply is not an edit: no history entry.
                if evs.is_empty() {
                    return self;
                }
                let mut doc = (*self.doc).clone();
                match doc.apply_all(&evs, &rules, gate.as_ref()) {
                    Ok(()) => next.commit(doc),
                    Err((at, e)) => {
                        // `at` is an index into `evs`; fall back to the first
                        // event rather than trust it.
                        let ev = evs.get(at).or(evs.first()).cloned();
                        match ev {
                            Some(ev) => next.refuse(ev, e),
                            None => next.last_error = Some(e),
                        }
                    }
                }
            }
            Action::Set(doc) => {
                next.doc = Rc::new(doc);
                next.history.clear();
                next.last_error = None;
                next.refused = None;
            }
            Action::DropRefusal => {
                next.last_error = None;
                next.refused = None;
            }
            Action::Undo => {
                if let Some(prev) = next.history.pop() {
                    next.doc = prev;
                    next.last_error = None;
                    next.refused = None;
                }
            }
        }
        Rc::new(next)
    }
}

/// What [`use_prose_editor`] returns. Pass `doc` and `onedit` to
/// `OdrlProseView`.
#[derive(Clone, PartialEq)]
pub struct ProseEditor {
    pub doc: Rc<EditDoc>,
    pub onedit: Callback<EditEvent>,
    /// Replace the document and clear the history.
    pub set_doc: Callback<EditDoc>,
    pub undo: Callback<()>,
    pub can_undo: bool,
    /// Why the last event was refused, if it was.
    pub last_error: Option<EditError>,
}

/// What [`use_prose_editor_with`] returns: the editor, and what only a
/// strict host needs.
#[derive(Clone, PartialEq)]
#[non_exhaustive]
pub struct ProseEditorWith {
    pub editor: ProseEditor,
    /// Applies the events as one: all of them or none, one undo step. This
    /// is how a fix from a [`prose_core::validate::Report`] is applied.
    pub apply_all: Callback<Vec<EditEvent>>,
    /// The last refused event and why, until the next edit succeeds or the
    /// validator or the [`StrictEdit`] policy changes. Hand
    /// `error.issue(&event)` to `OdrlProseView::refusal` to show it.
    pub refused: Option<(EditEvent, EditError)>,
}

#[hook]
pub fn use_prose_editor(initial: impl FnOnce() -> EditDoc, rules: Rc<EditRules>) -> ProseEditor {
    use_prose_editor_with(initial, rules, None, StrictEdit::Flag).editor
}

/// As [`use_prose_editor`], for a host that validates. With
/// [`StrictEdit::RefuseTerms`] the reducer refuses an event that writes a
/// term the `validator` does not allow into a vocabulary slot
/// ([`EditDoc::apply_gated`]); with [`StrictEdit::Flag`], or no validator, it
/// commits everything the rules allow and the validator's report marks it.
/// Undo is never gated.
#[hook]
pub fn use_prose_editor_with(
    initial: impl FnOnce() -> EditDoc,
    rules: Rc<EditRules>,
    validator: Option<Validator>,
    policy: StrictEdit,
) -> ProseEditorWith {
    let state = use_reducer(|| State {
        doc: Rc::new(initial()),
        history: vec![],
        last_error: None,
        refused: None,
    });
    let dispatcher = state.dispatcher();
    let gate = policy.gate(validator.as_ref()).cloned();
    {
        // A refusal is about the gate that made it: when the validator or
        // the policy changes (strict switched off, another profile), it goes.
        // Nothing is dispatched when there is none, so a mount costs no render.
        let refusal = state.refused.is_some() || state.last_error.is_some();
        let dispatcher = dispatcher.clone();
        use_effect_with((validator.clone(), policy), move |_| {
            if refusal {
                dispatcher.dispatch(Action::DropRefusal);
            }
        });
    }
    let onedit = use_callback(
        (dispatcher.clone(), rules.clone(), gate.clone()),
        |ev: EditEvent, (d, rules, gate)| d.dispatch(Action::Edit(ev, rules.clone(), gate.clone())),
    );
    let apply_all = use_callback(
        (dispatcher.clone(), rules, gate),
        |evs: Vec<EditEvent>, (d, rules, gate)| {
            d.dispatch(Action::All(evs, rules.clone(), gate.clone()))
        },
    );
    let set_doc = use_callback(dispatcher.clone(), |doc: EditDoc, d| {
        d.dispatch(Action::Set(doc))
    });
    let undo = use_callback(dispatcher, |_: (), d| d.dispatch(Action::Undo));
    ProseEditorWith {
        editor: ProseEditor {
            doc: state.doc.clone(),
            onedit,
            set_doc,
            undo,
            can_undo: !state.history.is_empty(),
            last_error: state.last_error.clone(),
        },
        apply_all,
        refused: state.refused.clone(),
    }
}

/// A document compared by identity, so the memo below does not compare
/// two whole documents.
#[derive(Clone)]
struct ByPtr(Rc<EditDoc>);

impl PartialEq for ByPtr {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// The report of `validator` on `doc`, made again only when the document
/// or the validator is another one (compared by identity). Without a
/// validator, the advisory report of no profile: no findings.
#[hook]
pub fn use_validation(doc: &Rc<EditDoc>, validator: Option<&Validator>) -> Rc<Report> {
    use_memo(
        (ByPtr(doc.clone()), validator.cloned()),
        |(ByPtr(doc), validator)| match validator {
            Some(v) => v.validate(doc),
            None => Validator::new(Arc::new(Wording::new()), Options::advisory()).validate(doc),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use prose_core::edit::{SlotPath, read_model};
    use prose_core::profile::{ProfileVocab, Wording};
    use prose_core::validate::{Options, Validator};
    use std::sync::Arc;

    const ACTION: &str = "policy[0].permission[0].action[0]#name";

    fn doc() -> EditDoc {
        read_model(
            r#"{
              "@context": ["http://www.w3.org/ns/odrl.jsonld",
                           {"ex": "https://example.org/odrl/profile/research/",
                            "foo": "https://example.net/foo/"}],
              "@type": "Set", "uid": "urn:policy:s",
              "profile": "https://example.org/odrl/profile/research",
              "permission": [{"target": "urn:asset:1", "action": "use"},
                             {"target": "urn:asset:2", "action": "read"}]
            }"#,
        )
        .unwrap()
    }

    fn gate() -> Validator {
        let text = include_str!("../../../prose-core/tests/profiles/research-closed.jsonld");
        let w = Wording::new().with_profile(ProfileVocab::parse(text).unwrap());
        Validator::new(Arc::new(w), Options::strict())
    }

    fn state() -> Rc<State> {
        Rc::new(State {
            doc: Rc::new(doc()),
            history: vec![],
            last_error: None,
            refused: None,
        })
    }

    fn set(d: &EditDoc, slot: &str, value: &str) -> EditEvent {
        let slot: SlotPath = slot.parse().unwrap();
        EditEvent::SetText {
            expect: d.node(&slot.node).unwrap().id(),
            slot,
            value: value.into(),
        }
    }

    fn rules() -> Rc<EditRules> {
        Rc::new(EditRules::default())
    }

    #[test]
    fn without_a_gate_a_foreign_action_is_committed() {
        let s = state();
        let ev = set(&s.doc, ACTION, "foo:bar");
        let s = s.reduce(Action::Edit(ev, rules(), None));
        assert_eq!(s.history.len(), 1);
        assert!(s.last_error.is_none() && s.refused.is_none());
        assert_ne!(*s.doc, doc());
    }

    #[test]
    fn a_gate_refuses_it_and_records_the_event() {
        let s = state();
        let ev = set(&s.doc, ACTION, "foo:bar");
        let after = s
            .clone()
            .reduce(Action::Edit(ev.clone(), rules(), Some(gate())));
        // No new document, no history entry (the Rc is the same).
        assert!(Rc::ptr_eq(&after.doc, &s.doc));
        assert!(after.history.is_empty());
        assert!(matches!(
            &after.last_error,
            Some(EditError::OutsideProfile(f)) if f.rule.as_str() == "profile.unknown-term"
        ));
        let (refused, error) = after.refused.clone().expect("the refused event");
        assert_eq!(refused, ev);
        assert_eq!(Some(&error), after.last_error.as_ref());
    }

    #[test]
    fn a_good_edit_or_a_new_document_clears_the_refusal() {
        let s = state();
        let bad = set(&s.doc, ACTION, "foo:bar");
        let s = s.reduce(Action::Edit(bad, rules(), Some(gate())));
        assert!(s.refused.is_some());
        let good = set(&s.doc, ACTION, "ex:train");
        let ok = s.clone().reduce(Action::Edit(good, rules(), Some(gate())));
        assert!(ok.refused.is_none() && ok.last_error.is_none());
        assert_eq!(ok.history.len(), 1);
        let replaced = s.reduce(Action::Set(doc()));
        assert!(replaced.refused.is_none() && replaced.last_error.is_none());
    }

    #[test]
    fn a_changed_gate_drops_the_refusal_and_keeps_the_document() {
        let s = state();
        let bad = set(&s.doc, ACTION, "foo:bar");
        let s = s.reduce(Action::Edit(bad, rules(), Some(gate())));
        assert!(s.refused.is_some());
        let doc = s.doc.clone();
        let s = s.reduce(Action::DropRefusal);
        assert!(s.refused.is_none() && s.last_error.is_none());
        assert!(Rc::ptr_eq(&s.doc, &doc) && s.history.is_empty());
    }

    #[test]
    fn undo_is_not_gated_and_clears_the_refusal() {
        let s = state();
        // The document already violates the profile: the edit is committed
        // by a host that flags, then undone through a gate.
        let ev = set(&s.doc, ACTION, "foo:bar");
        let s = s.reduce(Action::Edit(ev, rules(), None));
        let bad = set(&s.doc, "policy[0].permission[1].action[0]#name", "foo:baz");
        let s = s.reduce(Action::Edit(bad, rules(), Some(gate())));
        assert!(s.refused.is_some());
        let undone = s.reduce(Action::Undo);
        assert!(undone.history.is_empty());
        assert!(undone.refused.is_none() && undone.last_error.is_none());
        assert_eq!(*undone.doc, doc());
    }

    #[test]
    fn apply_all_is_one_history_entry() {
        let s = state();
        let a = set(&s.doc, ACTION, "ex:train");
        let b = set(
            &s.doc,
            "policy[0].permission[1].action[0]#name",
            "ex:fineTune",
        );
        let s = s.reduce(Action::All(vec![a, b], rules(), Some(gate())));
        assert_eq!(s.history.len(), 1, "one undo step");
        assert!(s.refused.is_none());
        let undone = s.reduce(Action::Undo);
        assert_eq!(*undone.doc, doc());
    }

    #[test]
    fn apply_all_that_fails_changes_nothing_and_names_the_event() {
        let s = state();
        let a = set(&s.doc, ACTION, "ex:train");
        let b = set(&s.doc, "policy[0].permission[1].action[0]#name", "foo:bar");
        let after = s
            .clone()
            .reduce(Action::All(vec![a, b.clone()], rules(), Some(gate())));
        assert!(Rc::ptr_eq(&after.doc, &s.doc));
        assert!(after.history.is_empty());
        let (refused, error) = after.refused.clone().expect("the failing event");
        assert_eq!(refused, b, "the event that failed, not the first");
        assert!(matches!(error, EditError::OutsideProfile(_)));
    }

    #[test]
    fn an_empty_list_of_events_changes_nothing() {
        let s = state();
        let after = s.clone().reduce(Action::All(vec![], rules(), None));
        assert!(Rc::ptr_eq(&after.doc, &s.doc));
        assert!(after.history.is_empty() && after.refused.is_none());
    }
}
