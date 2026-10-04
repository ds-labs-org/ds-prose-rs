//! The demo: an ODRL JSON-LD textarea and the prose it reads as.
mod fixtures;

use prose_yew::OdrlProse;
use web_sys::HtmlTextAreaElement;
use yew::prelude::*;

#[function_component(DemoApp)]
pub fn demo_app() -> Html {
    let json = use_state(|| AttrValue::from(fixtures::ALL[0].json));
    let oninput = {
        let json = json.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(el) = e.target_dyn_into::<HtmlTextAreaElement>() {
                json.set(AttrValue::from(el.value()));
            }
        })
    };
    let presets = fixtures::ALL.iter().map(|p| {
        let json = json.clone();
        let onclick = Callback::from(move |_| json.set(AttrValue::from(p.json)));
        html! { <button type="button" onclick={onclick}>{ p.label }</button> }
    });
    html! {
        <main>
            <h1>{ "ds-prose-rs demo" }</h1>
            <p>{ "PROSE: Prose Renderer for ODRL Semantic Encoding. Paste an ODRL policy as JSON-LD and read it as prose. Nothing leaves this page." }</p>
            <div class="presets">{ for presets }</div>
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
        </main>
    }
}
