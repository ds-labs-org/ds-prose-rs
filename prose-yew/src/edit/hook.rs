//! A ready-made host for simple uses: owns the document, applies events
//! with the rules, keeps a capped undo history.
use std::rc::Rc;
use yew::prelude::*;

use prose_core::edit::{EditDoc, EditError, EditEvent, EditRules};

const HISTORY_CAP: usize = 50;

#[derive(Clone, PartialEq)]
struct State {
    doc: Rc<EditDoc>,
    history: Vec<Rc<EditDoc>>,
    last_error: Option<EditError>,
}

enum Action {
    Edit(EditEvent, Rc<EditRules>),
    Set(EditDoc),
    Undo,
}

impl Reducible for State {
    type Action = Action;

    fn reduce(self: Rc<Self>, action: Action) -> Rc<Self> {
        let mut next = (*self).clone();
        match action {
            Action::Edit(ev, rules) => {
                let mut doc = (*self.doc).clone();
                match doc.apply(&ev, &rules) {
                    Ok(()) => {
                        next.history.push(self.doc.clone());
                        if next.history.len() > HISTORY_CAP {
                            next.history.remove(0);
                        }
                        next.doc = Rc::new(doc);
                        next.last_error = None;
                    }
                    Err(e) => next.last_error = Some(e),
                }
            }
            Action::Set(doc) => {
                next.doc = Rc::new(doc);
                next.history.clear();
                next.last_error = None;
            }
            Action::Undo => {
                if let Some(prev) = next.history.pop() {
                    next.doc = prev;
                    next.last_error = None;
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

#[hook]
pub fn use_prose_editor(initial: impl FnOnce() -> EditDoc, rules: Rc<EditRules>) -> ProseEditor {
    let state = use_reducer(|| State {
        doc: Rc::new(initial()),
        history: vec![],
        last_error: None,
    });
    let dispatcher = state.dispatcher();
    let onedit = use_callback((dispatcher.clone(), rules), |ev: EditEvent, (d, rules)| {
        d.dispatch(Action::Edit(ev, rules.clone()))
    });
    let set_doc = use_callback(dispatcher.clone(), |doc: EditDoc, d| {
        d.dispatch(Action::Set(doc))
    });
    let undo = use_callback(dispatcher, |_: (), d| d.dispatch(Action::Undo));
    ProseEditor {
        doc: state.doc.clone(),
        onedit,
        set_doc,
        undo,
        can_undo: !state.history.is_empty(),
        last_error: state.last_error.clone(),
    }
}
