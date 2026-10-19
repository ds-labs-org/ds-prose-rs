---
title: Yew validation
label: Validation
description: Validate an ODRL policy against profiles in Yew - findings in the rendering, the strict editor, StrictEdit, use_validation and the ProseProblems panel.
nav: yew
order: 4
stylesheet: examples/prose.css
---
# Validation (Yew)

A policy can be checked against ODRL profiles. The result is a `Report` with a verdict and findings. `OdrlProse` can show it under the sentences, `OdrlProseView` can mark it at the slots it concerns, and `ProseProblems` lists it with a way to jump to each finding and apply each fix.

A verdict of `Incomplete` is never a pass. It means no error was found but something could not be checked, such as a declared profile that is not loaded.

## Profiles, options and the validator

A `Wording` holds the loaded profiles. A `Validator` pairs a wording with `Options`.

- `Wording::new().with_profile(ProfileVocab::parse(text)?)` loads a profile from JSON-LD text. Turtle is read when `prose-core` is built with its `turtle` feature.
- `Options::advisory()` is the default. Profiles widen and annotate the reading, and nothing is an error.
- `Options::strict()` reads the vocabulary as closed: ODRL plus the loaded profiles. The structural rules of the ODRL Information Model stay off until `Options::structural(true)`.
- A left operand a profile labels with a plural noun phrase says so with `prose:plural true`, in JSON-LD or Turtle. Its operators then agree in the plural ("the time windows are earlier than"), in the reader, the editor sentence and the operator select. ODRL's own terms keep their built-in number.
- `Validator::new(Arc<Wording>, Options)`, then `validate(&EditDoc) -> Report` or `validate_json(&str) -> Report`.

`Report` has `mode`, `verdict`, `findings` and `gaps`, and is `#[non_exhaustive]`. `Verdict` is `Conforms`, `Violations` or `Incomplete`, also `#[non_exhaustive]`. Always match with a wildcard arm.

## Findings under the sentences

`OdrlProse` writes a `section.prose-violations` after the policies when it has a `validator` and `violations` is `true`. With a validator and `violations` left `false`, nothing changes in the output, and with `violations` and no validator, there is nothing to show.

{{source prose-yew/tests/site_examples.rs#reader_violations}}

The panel below is the output the repository pins for a policy that breaks a closed research profile: `prose-yew/tests/snapshots/strict_read_violations.html`, produced by `ssr_strict.rs`. It is styled by the example stylesheet.

{{rendered repo:prose-yew/tests/snapshots/strict_read_violations.html}}

The status line exists only for a strict report. Its class and words come from the verdict:

| Verdict | Class | Words (`Labels` field) |
|---|---|---|
| `Verdict::Conforms` | `prose-conforms` | `status_conforms` |
| `Verdict::Incomplete` | `prose-incomplete` | `status_incomplete` |
| any other verdict | `prose-violations-found` | `status_violations` |

A verdict this version does not know is shown as a failure, not as a pass.

## The strict editor

`EditConfig::with_wording` loads the profiles into the editor, and `with_strict(Options::strict())` validates against them and makes the add buttons create admissible values. `use_prose_editor_with` takes the validator and a `StrictEdit` policy, and `use_validation` gives the report.

{{source prose-yew/tests/site_examples.rs#strict}}

Pass the hook `Rc::new(cfg.rules.clone())` and `cfg.validator.clone()` as the example does. The reducer applies the rules it is handed, not the config's.

`StrictEdit::Flag`, the default, commits every edit the rules allow and lets the validator mark a violation. `StrictEdit::RefuseTerms` makes the reducer refuse a term outside the profile written into a vocabulary slot, as `EditError::OutsideProfile`. Show the refusal with `refusal={with.refused.as_ref().and_then(|(ev, err)| err.issue(ev))}`.

Under a closed profile, a vocabulary slot is a closed combobox and takes only what the validator admits. Enter on text that names nothing admissible keeps the draft in the slot, with the reason and the nearest term. Escape reverts it. When focus leaves the slot, the draft is given up: the slot goes back to its value and a polite live region says so. So text typed into a closed combobox can be lost, with `StrictEdit::Flag` too. A closed slot has `data-closed="true"`. The operator select groups its options into those the profile allows and those it does not.

## The problems panel

`ProseProblems` groups the findings by rule, in document order. Each finding has a "Go to" button and one button for each fix. A fix is a list of events: give `onfix` the `apply_all` of the hook, which applies them as one step with one undo entry. The events carry `expect` ids, so a fix from a report the document has moved on from is refused as stale.

`onjump` receives the `IssueTarget` of the finding. The host moves focus to the `[data-slot]` or `[data-node]` element of the view that holds that path. After a fix removes the focused button, focus moves to the panel heading if it fell to the page body.

The classes of the panel are on the [CSS hooks](/yew/css-hooks/) page. The demo, `demo/src/edit.rs`, shows a complete host with `issues`, `onjump` and an undo button.
