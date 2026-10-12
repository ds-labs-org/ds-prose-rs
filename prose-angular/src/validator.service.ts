import { Injectable, inject } from '@angular/core';
import { validate, type ValidateOptions, type ValidationReport } from '@ds-labs/prose-wasm';
import { PROSE_WASM_URL, loadWasm } from './wasm-runtime';

/**
 * Validates ODRL policies against ODRL profiles without rendering anything
 * (a form's submit guard, a list of policies). It runs the same engine as
 * `<ds-odrl-prose [strict]>` and the `ds-prose-validate` command, so the
 * report is the same.
 */
@Injectable({ providedIn: 'root' })
export class ProseValidatorService {
  private readonly wasmUrl = inject(PROSE_WASM_URL);

  /**
   * The report for the policy `json` against `profiles` (the text of each
   * profile, JSON-LD or Turtle; the service does no network access). Strict
   * unless `options.strict` is `false`. Waits for the wasm module to load;
   * rejects when it cannot, or when a profile or the options cannot be read.
   * A problem in the policy is never a rejection: it is a finding.
   */
  async validate(
    json: string,
    profiles: string | readonly string[] | null = null,
    options: ValidateOptions | null = null,
  ): Promise<ValidationReport> {
    await loadWasm(this.wasmUrl);
    const list = typeof profiles === 'string' || profiles == null ? profiles : [...profiles];
    return JSON.parse(validate(json, list, options)) as ValidationReport;
  }
}
