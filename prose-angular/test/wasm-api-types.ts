// Type-level test, run by `npm run typecheck`: the optional arguments of the
// wasm package's `validate` are optional in its generated declarations, so a
// call with two or three arguments compiles.
import { validate } from '@ds-labs/prose-wasm';

export const two: string = validate('{}', null);
export const three: string = validate('{}', null, { strict: true });
export const four: string = validate('{}', ['{}'], { strict: false }, null);
export const five: string = validate('{}', null, undefined, '', ['x']);
export const six: string = validate('{}', null, null, null, null, { 'https://x/y': ['a'] });
