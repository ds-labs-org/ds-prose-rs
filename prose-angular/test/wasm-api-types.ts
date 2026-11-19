// Type-level test, run by `npm run typecheck`: the optional arguments of the
// wasm package's `validate` are optional in its generated declarations, so a
// call with two or three arguments compiles.
import { validate } from '@ds-labs/prose-wasm';

export const two: string = validate('{}', null);
export const three: string = validate('{}', null, { strict: true });
export const four: string = validate('{}', ['{}'], { strict: false }, null);
export const five: string = validate('{}', null, undefined, '', ['x']);
export const six: string = validate('{}', null, null, null, null, { 'https://x/y': ['a'] });

import type { EditLimits, RemovedSection } from '@ds-labs/prose-wasm';

// `policyTypes` rides `EditLimits` beside the list limits; it is not a `LimitName`.
export const limits: EditLimits = { policyTypes: ['Set', 'odrl:Offer'], logicalDepth: { max: 2 } };
export const cleared: EditLimits = { policyTypes: null };
export const rewritten: RemovedSection = { path: '@type', key: '@type', limit: 'policyTypes', value: 'Offer', replacedWith: 'Set' };
export const added: RemovedSection = { path: '@type', key: '@type', limit: 'policyTypes', replacedWith: 'Set' };
// @ts-expect-error a list limit is a {min, max}, not a list of names
export const wrong: EditLimits = { permission: ['Set'] };
