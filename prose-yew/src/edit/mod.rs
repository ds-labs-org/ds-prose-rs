//! The editing view: [`OdrlProseView`] renders an [`EditDoc`](prose_core::edit::EditDoc) as
//! sentences, in Read mode as plain prose and in Edit mode with
//! `contenteditable` slots, selects, and + / - buttons.
//!
//! The component is controlled. It never changes the document: every
//! committed edit and every structural action leaves through `onedit` as
//! an [`EditEvent`](prose_core::edit::EditEvent), and the host applies it (see [`use_prose_editor`] for
//! a host that does so with an undo history).
//!
//! Classes (each added to, never replacing, the built-in name; see
//! [`ProseClasses`]): `ds-prose`, `prose-view`, `prose-edit`,
//! `prose-reveal-hover`, `prose-policy`, `prose-heading`, `prose-intro`,
//! `prose-notes`, `prose-rules`, `prose-rule` with `prose-permission` /
//! `prose-prohibition` / `prose-obligation`, `prose-rule-sentence`,
//! `prose-term`, `prose-slot` (`-empty`, `-invalid`, `-warning`,
//! `-whitespace`), `prose-inherited`, `prose-select`, `prose-list`,
//! `prose-item`, `prose-add`, `prose-remove`, `prose-reorder`, `prose-wrap`,
//! `prose-toolbar`, `prose-add-bar`, `prose-label`, `prose-conditions`,
//! `prose-condition`, `prose-logical`, `prose-follow-up`, `prose-locked`,
//! `prose-issue` (`-error`, `-warning`, `-info`), `prose-warnings`,
//! `prose-decoration`, `prose-suggestions`, `prose-suggestion`
//! (`-active`), `prose-live`, `prose-empty`.
//!
//! Markup: every policy, rule and condition container carries
//! `data-node="<NodePath>"`; slots `data-slot="<SlotPath>"`; add buttons
//! `data-add="<ListPath>"`. Controls the [`EditRules`](prose_core::edit::EditRules) do not allow are not
//! rendered.
//!
//! Server rendering: Edit mode's `<style>` text is HTML-escaped by Yew's
//! server renderer, so set `EditConfig::base_css` to `false` and ship
//! [`BASE_CSS`] yourself there.
mod choice;
mod config;
mod css;
mod hook;
mod slot;
mod suggest;
mod view;

use std::rc::Rc;
use yew::prelude::*;

pub use config::{
    ButtonContent, Choice, EditConfig, EmptyTexts, FieldNames, Labels, Nouns, Placeholders,
    ProseClasses, ProseStyles, Reveal, Suggestion, Vocabulary, fill,
};
pub use css::BASE_CSS;
pub use hook::{ProseEditor, use_prose_editor};
pub use suggest::filter_suggestions;
pub use view::{OdrlProseView, OdrlProseViewProps};

use prose_core::edit::{ChoiceKind, NodeId, NodePath, SlotKind, SlotPath};

/// Whether the view only reads or also edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditMode {
    /// The same sentences with display text and no controls.
    #[default]
    Read,
    /// Contenteditable slots, selects, + / - buttons, suggestions, issues
    /// and focus management.
    Edit,
}

/// What a slot or select is, reported with the focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusKind {
    Slot(SlotKind),
    Choice(ChoiceKind),
    /// The may / must not / must select; its `SlotFocus::slot` is
    /// `{ node: the rule, field: Field::Kind }`.
    RuleKind,
}

/// Which slot has the keyboard focus, for hosts that show help for it.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotFocus {
    pub slot: SlotPath,
    pub kind: FocusKind,
    pub raw: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationAt {
    Policy,
    Rule,
    Condition,
}

/// Where a host badge goes; passed to the `decorate` callback.
#[derive(Debug, Clone, PartialEq)]
pub struct Decoration {
    pub at: DecorationAt,
    pub path: NodePath,
    pub node: NodeId,
}

/// What the slot, select and view components share. Compared by identity:
/// a new one is made on every render of the view.
pub(crate) struct Env {
    pub cfg: Rc<EditConfig>,
    pub onedit: Callback<prose_core::edit::EditEvent>,
    pub on_focus_slot: Callback<Option<SlotFocus>>,
    /// (policy index, uid) of every policy that has one.
    pub policy_uids: Vec<(usize, AttrValue)>,
}

#[derive(Clone)]
pub(crate) struct EnvRef(pub Rc<Env>);

impl PartialEq for EnvRef {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
