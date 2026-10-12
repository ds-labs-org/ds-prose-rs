import { Component, computed, signal } from '@angular/core';
import { OdrlProseComponent, type ProfileSummary, type ValidationReport } from '@ds-labs/prose-angular';
import { PRESETS } from './fixtures';

// Profiles the demo can load. The host fetches them: the component never does.
// The files are copied into the build by the `assets` entries of angular.json.
const PROFILES: Record<string, string> = {
  research: 'profiles/research.jsonld',
};

@Component({
  selector: 'app-root',
  imports: [OdrlProseComponent],
  templateUrl: './app.html',
})
export class App {
  protected readonly presets = PRESETS;
  protected readonly json = signal(PRESETS[0].json);
  // [strict], [showViolations] and [profile] of <ds-odrl-prose>.
  protected readonly strict = signal(false);
  protected readonly findings = signal(false);
  protected readonly profileText = signal<string | null>(null);
  protected readonly profileNote = signal('No profile loaded.');
  // The latest report that (validationChange) emitted, while strict is on.
  protected readonly report = signal<ValidationReport | null>(null);
  protected readonly verdict = computed(() => {
    if (!this.strict()) return 'Validation is off.';
    const report = this.report();
    if (!report) return 'Validating.';
    const { error, warning, info } = report.counts;
    return `Verdict: ${report.verdict} (${error} errors, ${warning} warnings, ${info} notes).`;
  });

  protected setStrict(on: boolean): void {
    this.strict.set(on);
    if (!on) this.report.set(null);
  }

  protected async chooseProfile(name: string): Promise<void> {
    const url = PROFILES[name];
    if (!url) {
      this.profileText.set(null);
      this.profileNote.set('No profile loaded.');
      return;
    }
    try {
      const response = await fetch(new URL(url, document.baseURI));
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      this.profileText.set(await response.text());
    } catch (error) {
      this.profileNote.set(`The profile could not be fetched: ${error}`);
    }
  }

  protected onReport(report: ValidationReport): void {
    this.report.set(report);
  }

  protected onProfileLoaded(summary: ProfileSummary): void {
    this.profileNote.set(`Profile loaded: ${summary.title ?? summary.iri ?? 'untitled'} (${summary.terms} terms).`);
  }

  protected onProfileError(error: unknown): void {
    this.profileNote.set(`The profile could not be read: ${error}`);
  }
}
