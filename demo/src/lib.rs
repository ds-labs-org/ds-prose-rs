//! The demo: an ODRL JSON-LD textarea and the prose it reads as, and an
//! Edit tab where the prose itself is editable. A Profile field above both
//! tabs loads the ODRL profile they are worded with.
mod edit;
mod fetch;
mod fixtures;
pub mod profile_field;
pub mod strict;

use edit::EditDemo;
use profile_field::{MAX_BYTES, PRESETS, ProfileMsg, ProfileState, declared_profile};
use prose_yew::OdrlProse;
use prose_yew::prose_core::validate::Validator;
use std::rc::Rc;
use std::sync::Arc;
use strict::StrictBar;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{HtmlInputElement, HtmlTextAreaElement};
use yew::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Read,
    Edit,
}

#[function_component(DemoApp)]
pub fn demo_app() -> Html {
    let json = use_state(|| AttrValue::from(fixtures::ALL[0].json));
    let tab = use_state(|| Tab::Read);
    let profile = use_reducer(ProfileState::default);
    // The validator strict mode asks, made again only when the profile or
    // the switches change.
    let validator = use_memo(
        (profile.wording.clone(), profile.options()),
        |(w, options)| {
            options.as_ref().map(|o| {
                let wording = w.as_ref().map(|w| (*w.0).clone()).unwrap_or_default();
                Validator::new(Arc::new(wording), o.clone())
            })
        },
    );
    let report = use_memo(((*json).clone(), (*validator).clone()), |(json, v)| {
        v.as_ref().map(|v| Rc::new(v.validate_json(json)))
    });
    let profile_name = profile.name.clone().map(AttrValue::from);
    // Bumped on every preset, so the Edit tab loads it.
    let load = use_state(|| 0u32);
    let oninput = {
        let json = json.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<HtmlTextAreaElement>() {
                json.set(AttrValue::from(el.value()));
            }
        })
    };
    // The Edit tab writes its document back, so leaving it keeps the edits.
    let onchange = {
        let json = json.clone();
        Callback::from(move |written: AttrValue| json.set(written))
    };
    let presets = fixtures::ALL.iter().map(|p| {
        let json = json.clone();
        let load = load.clone();
        let onclick = Callback::from(move |_| {
            json.set(AttrValue::from(p.json));
            load.set(*load + 1);
        });
        html! { <button type="button" onclick={onclick}>{ p.label }</button> }
    });
    let tab_button = |t: Tab, label: &str| {
        let tab = tab.clone();
        let selected = *tab == t;
        let onclick = Callback::from(move |_| tab.set(t));
        html! {
            <button type="button" role="tab" aria-selected={selected.to_string()} onclick={onclick}>
                { label.to_string() }
            </button>
        }
    };
    html! {
        <main>
            <h1>{ "ds-prose-rs demo" }</h1>
            <p>{ "PROSE: Prose Renderer for ODRL Semantic Encoding. Paste an ODRL policy as JSON-LD and read it as prose, or edit the prose itself. Nothing leaves this page unless you load a profile from a URL." }</p>
            <div class="presets">{ for presets }</div>
            { profile_section(&profile, &json) }
            <div class="tabs" role="tablist">
                { tab_button(Tab::Read, "Read") }
                { tab_button(Tab::Edit, "Edit") }
            </div>
            if *tab == Tab::Read {
                <div class="columns">
                    <section>
                        <label for="policy">{ "ODRL JSON-LD" }</label>
                        <textarea id="policy" rows="24" value={(*json).clone()} oninput={oninput} />
                    </section>
                    <section>
                        <h2>{ "Prose" }</h2>
                        if let Some(r) = &*report {
                            <StrictBar report={r.clone()} profile={profile_name.clone()} json={(*json).clone()} />
                        }
                        <OdrlProse
                            json={(*json).clone()}
                            wording={profile.wording.clone()}
                            validator={(*validator).clone()}
                            violations={true}
                        />
                    </section>
                </div>
            } else {
                <EditDemo
                    source={(*json).clone()}
                    load={*load}
                    onchange={onchange}
                    wording={profile.wording.clone()}
                    options={profile.options()}
                    strict_edit={profile.strict_edit()}
                    profile_name={profile_name.clone()}
                />
            }
        </main>
    }
}

/// The Profile field: the text, the buttons that fill or apply it, a file
/// picker, a URL, and what is loaded or went wrong. `policy` is the current
/// policy, whose declared profile can prefill the URL.
fn profile_section(state: &UseReducerHandle<ProfileState>, policy: &str) -> Html {
    let send = |m: ProfileMsg| {
        let d = state.dispatcher();
        Callback::from(move |_: MouseEvent| d.dispatch(m.clone()))
    };
    let oninput = {
        let d = state.dispatcher();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<HtmlTextAreaElement>() {
                d.dispatch(ProfileMsg::Edit(el.value()));
            }
        })
    };
    let onfile = {
        let d = state.dispatcher();
        // The generation `Begin` moves to; what a stale read brings is dropped.
        let generation = state.generation.wrapping_add(1);
        Callback::from(move |e: Event| {
            let Some(input) = e.target_dyn_into::<HtmlInputElement>() else {
                return;
            };
            let Some(file) = input.files().and_then(|f| f.get(0)) else {
                return;
            };
            // Choosing the same file again is a change too.
            input.set_value("");
            d.dispatch(ProfileMsg::Begin);
            if file.size() > MAX_BYTES as f64 {
                let msg = format!(
                    "{} is {} bytes; profiles over 4 MiB are not read.",
                    file.name(),
                    file.size()
                );
                d.dispatch(ProfileMsg::Failed { generation, msg });
                return;
            }
            let d = d.clone();
            spawn_local(async move {
                let m = match JsFuture::from(file.text()).await {
                    Ok(text) => ProfileMsg::FileRead {
                        generation,
                        text: text.as_string().unwrap_or_default(),
                    },
                    Err(_) => ProfileMsg::Failed {
                        generation,
                        msg: format!("{} could not be read.", file.name()),
                    },
                };
                d.dispatch(m);
            });
        })
    };
    let onurl = {
        let d = state.dispatcher();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                d.dispatch(ProfileMsg::Url(el.value()));
            }
        })
    };
    let onload = {
        let d = state.dispatcher();
        let url = state.url.clone();
        let generation = state.generation.wrapping_add(1);
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            d.dispatch(ProfileMsg::Begin);
            let target = match fetch::target(&url) {
                Ok(t) => t,
                Err(msg) => return d.dispatch(ProfileMsg::Failed { generation, msg }),
            };
            let d = d.clone();
            spawn_local(async move {
                d.dispatch(match fetch::fetch(&target).await {
                    Ok((text, content_type)) => ProfileMsg::FetchedFrom {
                        generation,
                        url: target.clone(),
                        text,
                        content_type,
                    },
                    Err(msg) => ProfileMsg::Failed { generation, msg },
                });
            });
        })
    };
    let declared = declared_profile(policy);
    let ondeclared = {
        let d = state.dispatcher();
        let declared = declared.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(iri) = &declared {
                d.dispatch(ProfileMsg::Url(iri.clone()));
            }
        })
    };
    let switch = |m: fn(bool) -> ProfileMsg| {
        let d = state.dispatcher();
        Callback::from(move |e: Event| {
            if let Some(el) = e.target_dyn_into::<HtmlInputElement>() {
                d.dispatch(m(el.checked()));
            }
        })
    };
    let presets = PRESETS.iter().map(|p| {
        html! {
            <button type="button" id={p.id} onclick={send(ProfileMsg::Use(p.text.to_string()))}>
                { p.label }
            </button>
        }
    });
    let status = if state.loading {
        "Reading the profile\u{2026}".to_string()
    } else {
        state
            .status
            .clone()
            .unwrap_or_else(|| "No profile: the built-in wording.".to_string())
    };
    let notes = state.warnings.len();
    html! {
        <section class="profile">
            <label for="profile-json">{ "Profile (JSON-LD or Turtle)" }</label>
            <p class="hint">
                { "The ODRL profile the policy is written to: its labels word the prose, its terms fill the edit suggestions, and a glossary follows each policy. Paste one and apply it, or pick one below." }
            </p>
            <textarea id="profile-json" rows="6" spellcheck="false" value={state.text.clone()} oninput={oninput} />
            <div class="profile-actions">
                <button type="button" id="profile-apply" onclick={send(ProfileMsg::Apply)}>{ "Apply" }</button>
                { for presets }
                <label class="file">
                    { "Open a file\u{2026} " }
                    <input
                        type="file"
                        id="profile-file"
                        accept=".json,.jsonld,.ttl,application/ld+json,application/json,text/turtle"
                        onchange={onfile}
                    />
                </label>
                <button type="button" id="profile-clear" onclick={send(ProfileMsg::Clear)}>{ "Clear" }</button>
            </div>
            // novalidate: a relative URL is valid here (it is resolved
            // against the base URI), not to the browser's type="url" check.
            <form class="profile-url" novalidate=true onsubmit={onload}>
                <label for="profile-url">{ "Profile URL" }</label>
                <input
                    type="url"
                    id="profile-url"
                    placeholder="https://example.org/profile.jsonld"
                    value={state.url.clone()}
                    oninput={onurl}
                />
                <button type="submit" id="profile-load">{ "Load from URL" }</button>
                <button
                    type="button"
                    id="profile-declared"
                    disabled={declared.is_none()}
                    title={declared.clone().unwrap_or_else(|| "The policy declares no profile.".to_string())}
                    onclick={ondeclared}
                >
                    { "Use the policy's declared profile" }
                </button>
            </form>
            <p class="hint">
                { "The URL is fetched without cookies, and only when you ask. The server must allow cross-origin requests (CORS)." }
            </p>
            <p id="profile-status" role="status" aria-live="polite">{ status }</p>
            <div class="profile-strict">
                <label>
                    <input type="checkbox" id="profile-strict" checked={state.strict} onchange={switch(ProfileMsg::Strict)} />
                    { " Strict validation" }
                </label>
                <p class="hint">
                    { "Reads ODRL and the loaded profile as every term there is: a term that resolves nowhere is an error, and a profile the policy declares but you have not loaded stops the verdict short. The two Strict presets are written to the Research (closed) profile." }
                </p>
                if state.strict {
                    <label>
                        <input type="checkbox" id="profile-structural" checked={state.structural} onchange={switch(ProfileMsg::Structural)} />
                        { " Also check the ODRL Information Model's own structure" }
                    </label>
                    <label>
                        <input type="checkbox" id="profile-refuse" checked={state.refuse} onchange={switch(ProfileMsg::Refuse)} />
                        { " Refuse a term the profile does not allow, instead of marking it" }
                    </label>
                }
            </div>
            if notes > 0 {
                <details class="profile-warnings">
                    <summary>
                        { if notes == 1 { "1 note from reading the profile".to_string() } else { format!("{notes} notes from reading the profile") } }
                    </summary>
                    <ul>{ for state.warnings.iter().map(|w| html! { <li>{ w }</li> }) }</ul>
                </details>
            }
            if let Some(e) = &state.error {
                <p id="profile-error" class="prose-error" role="alert">{ e.clone() }</p>
            }
        </section>
    }
}
