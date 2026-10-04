# prose-angular

Angular bridge for the Yew `<OdrlProse>` component.

```
Angular <ds-odrl-prose [json]>  ──►  ProseHandle (prose-wasm, wasm-bindgen)  ──►  <OdrlProse> (prose-yew)
```

`prose-wasm` mounts the Yew component into the element Angular gives it and
forwards input changes through `AppHandle::update`. The Angular component only
loads the wasm once, forwards inputs, and destroys the handle.

## Build

```bash
npm run build:wasm   # wasm-pack -> ./wasm (prose_wasm.js, prose_wasm_bg.wasm, .d.ts)
npm install && npm run typecheck
```

## Use

```ts
import { OdrlProseComponent } from '@ds-labs/prose-angular';

@Component({ imports: [OdrlProseComponent], template: `<ds-odrl-prose [json]="policy" articleClass="my-prose" />` })
```

The build does not emit the `.wasm`. Copy it into your assets in `angular.json` (`{ "glob": "prose_wasm_bg.wasm", "input": "../prose-angular/wasm", "output": "/" }`), or serve it elsewhere and provide the `PROSE_WASM_URL` token. The wasm is not loaded until the first component renders. Style with the `prose-*` classes listed in `prose-yew/src/lib.rs`.

Notes: the component owns its host element's children, so use it as a leaf. It renders client-side only (no SSR); guard with `isPlatformBrowser` or `@defer` if the app uses SSR.
