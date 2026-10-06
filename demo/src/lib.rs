//! The demo: an ODRL JSON-LD textarea and the prose it reads as, and an
//! Edit tab where the prose itself is editable.
mod edit;
mod fixtures;

use edit::EditDemo;
use prose_yew::OdrlProse;
use web_sys::HtmlTextAreaElement;
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
            <p>{ "PROSE: Prose Renderer for ODRL Semantic Encoding. Paste an ODRL policy as JSON-LD and read it as prose, or edit the prose itself. Nothing leaves this page." }</p>
            <div class="presets">{ for presets }</div>
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
                        <OdrlProse json={(*json).clone()} />
                    </section>
                </div>
            } else {
                <EditDemo source={(*json).clone()} load={*load} onchange={onchange} />
            }
        </main>
    }
}
