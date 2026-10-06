//! The Edit tab: the same policy as sentences you edit in place, and as the
//! JSON-LD it writes, kept in step both ways.
use std::rc::Rc;

use prose_yew::prose_core::edit::{EditDoc, EditRules, read_model, write_jsonld};
use prose_yew::{EditConfig, EditMode, OdrlProseView, use_prose_editor};
use web_sys::HtmlTextAreaElement;
use yew::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub struct EditDemoProps {
    /// A preset's JSON-LD; loaded when `load` changes.
    pub source: AttrValue,
    pub load: u32,
    /// The JSON-LD the editor writes, reported after every change to the
    /// document so the Read tab (and a later visit to this tab) sees it.
    pub onchange: Callback<AttrValue>,
}

fn pretty(doc: &EditDoc) -> String {
    serde_json::to_string_pretty(&write_jsonld(doc)).unwrap_or_default()
}

#[function_component(EditDemo)]
pub fn edit_demo(props: &EditDemoProps) -> Html {
    let rules = use_memo((), |_| EditRules::default());
    let config = use_memo((), |_| EditConfig::default());
    let source = props.source.clone();
    // An unreadable source is reported, not swallowed: the editor starts empty,
    // the user's text stays in the JSON panel, and nothing is reported upward
    // until the document is really changed.
    let initial_error = use_state(|| read_model(&source).err().map(|e| e.to_string()));
    let initial_text = props.source.to_string();
    let editor = use_prose_editor(move || read_model(&source).unwrap_or_default(), rules);
    let text = use_state({
        let unreadable = initial_error.is_some();
        let doc = editor.doc.clone();
        move || {
            if unreadable {
                initial_text
            } else {
                pretty(&doc)
            }
        }
    });
    let error = use_state(|| (*initial_error).clone());
    let first_load = use_state(|| props.load);

    // A preset was chosen: load it.
    {
        let set_doc = editor.set_doc.clone();
        let error = error.clone();
        let source = props.source.clone();
        use_effect_with(props.load, move |load| {
            // The load this tab mounted with was already read at mount.
            if *load != *first_load {
                match read_model(&source) {
                    Ok(doc) => {
                        set_doc.emit(doc);
                        error.set(None);
                    }
                    Err(e) => error.set(Some(e.to_string())),
                }
            }
            || ()
        });
    }
    // The document changed: show the JSON-LD it writes.
    {
        let text = text.clone();
        let doc = editor.doc.clone();
        let onchange = props.onchange.clone();
        let first = use_mut_ref(|| true);
        use_effect_with(Rc::as_ptr(&editor.doc) as usize, move |_| {
            // Not on mount: an unchanged (or unreadable) source is left alone.
            if !std::mem::replace(&mut *first.borrow_mut(), false) {
                let json = pretty(&doc);
                text.set(json.clone());
                onchange.emit(AttrValue::from(json));
            }
            || ()
        });
    }

    let oninput = {
        let text = text.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<HtmlTextAreaElement>() {
                text.set(el.value());
            }
        })
    };
    let load_json = {
        let text = text.clone();
        let set_doc = editor.set_doc.clone();
        let error = error.clone();
        Callback::from(move |_| match read_model(&text) {
            Ok(doc) => {
                set_doc.emit(doc);
                error.set(None);
            }
            Err(e) => error.set(Some(e.to_string())),
        })
    };
    let undo = {
        let undo = editor.undo.clone();
        Callback::from(move |_| undo.emit(()))
    };

    html! {
        <div class="columns">
            <section>
                <h2>{ "Edit as prose" }</h2>
                <p class="hint">
                    { "Click a highlighted word to type; pick an option with the arrow keys and Enter. + adds, \u{2212} removes. Esc undoes the word you are typing." }
                </p>
                <button type="button" onclick={undo} disabled={!editor.can_undo}>{ "Undo" }</button>
                if let Some(e) = &editor.last_error {
                    <p class="prose-error" role="alert">{ e.to_string() }</p>
                }
                <OdrlProseView
                    doc={editor.doc.clone()}
                    mode={EditMode::Edit}
                    config={config.clone()}
                    onedit={editor.onedit.clone()}
                />
            </section>
            <section>
                <label for="written">{ "JSON-LD it writes" }</label>
                <textarea id="written" rows="24" value={(*text).clone()} oninput={oninput} />
                <button type="button" onclick={load_json}>{ "Load this JSON into the editor" }</button>
                if let Some(e) = &*error {
                    <p class="prose-error" role="alert">{ e.clone() }</p>
                }
            </section>
        </div>
    }
}
