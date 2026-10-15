// The code shown on the documentation site (site/content/angular/*.md),
// typechecked by `npm run typecheck`. The site embeds each region with
// `{{source prose-angular/test/docs-examples.ts#name}}`, so the docs cannot
// show code that does not compile against the package's public API.
// Templates are plain strings here: tsc does not check them, the Angular
// compiler does in an application.
import { ApplicationConfig, Component, ViewEncapsulation, inject, signal } from '@angular/core';
import {
  OdrlProseComponent,
  PROSE_WASM_MODULE,
  PROSE_WASM_URL,
  ProseValidatorService,
  type ProfileSummary,
  type ValidateOptions,
  type ValidationReport,
} from '../src/index';

declare const POLICY: string;
declare const PROFILE_TEXT: string;

// region: basic
@Component({
  selector: 'app-policy',
  imports: [OdrlProseComponent],
  template: `<ds-odrl-prose [json]="policy" />`,
})
export class PolicyComponent {
  policy = POLICY;
}
// endregion: basic

// region: wasm_url
export const appConfig: ApplicationConfig = {
  providers: [{ provide: PROSE_WASM_URL, useValue: '/assets/prose_wasm_bg.wasm' }],
};
// endregion: wasm_url

// region: turtle
export const turtleConfig: ApplicationConfig = {
  providers: [
    { provide: PROSE_WASM_MODULE, useValue: () => import('@ds-labs/prose-wasm/turtle') },
    { provide: PROSE_WASM_URL, useValue: 'prose_wasm_turtle_bg.wasm' },
  ],
};
// endregion: turtle

// region: profile
@Component({
  selector: 'app-profiled',
  imports: [OdrlProseComponent],
  template: `
    <ds-odrl-prose [json]="policy" [profile]="profileText"
                   (profileLoaded)="loaded($event)" (profileError)="failed($event)" />
  `,
})
export class ProfiledComponent {
  policy = POLICY;
  profileText = PROFILE_TEXT;
  loaded(summary: ProfileSummary): void {
    console.log(summary.title, summary.terms);
  }
  failed(error: unknown): void {
    console.error('profile not readable', error);
  }
}
// endregion: profile

// region: validation
@Component({
  selector: 'app-validated',
  imports: [OdrlProseComponent],
  template: `
    <ds-odrl-prose [json]="policy" [profile]="profileText" [strict]="true" [showViolations]="true"
                   (validationChange)="onReport($event)" (validationError)="onError($event)" />
  `,
})
export class ValidatedComponent {
  policy = POLICY;
  profileText = PROFILE_TEXT;
  verdict = signal<ValidationReport['verdict'] | null>(null);
  onReport(report: ValidationReport): void {
    this.verdict.set(report.verdict);
  }
  onError(error: unknown): void {
    console.error('validation options not readable', error);
  }
}
// endregion: validation

// region: options
@Component({
  selector: 'app-options',
  imports: [OdrlProseComponent],
  template: `<ds-odrl-prose [json]="policy" [profile]="profileText" [validation]="options" [showViolations]="true" />`,
})
export class OptionsComponent {
  policy = POLICY;
  profileText = PROFILE_TEXT;
  options: ValidateOptions = { strict: true, failClosed: true, denyDeprecated: true };
}
// endregion: options

// region: service
@Component({ selector: 'app-guard', template: '' })
export class GuardComponent {
  private readonly validator = inject(ProseValidatorService);

  async canSubmit(json: string, profileText: string): Promise<boolean> {
    const report = await this.validator.validate(json, [profileText]);
    return report.verdict === 'conforms';
  }
}
// endregion: service

// region: article_class
@Component({
  selector: 'app-classed',
  imports: [OdrlProseComponent],
  template: `<ds-odrl-prose [json]="policy" articleClass="my-prose compact" />`,
})
export class ClassedComponent {
  policy = POLICY;
}
// endregion: article_class

// region: encapsulation_none
@Component({
  selector: 'app-unencapsulated',
  imports: [OdrlProseComponent],
  encapsulation: ViewEncapsulation.None,
  template: `<ds-odrl-prose [json]="policy" articleClass="my-prose" />`,
  styles: `
    ds-odrl-prose { display: block; }
    .my-prose .prose-rule-sentence { font-weight: 600; }
  `,
})
export class UnencapsulatedComponent {
  policy = POLICY;
}
// endregion: encapsulation_none

// region: ng_deep
@Component({
  selector: 'app-deep',
  imports: [OdrlProseComponent],
  template: `<ds-odrl-prose [json]="policy" />`,
  styles: `
    ds-odrl-prose { display: block; }
    :host ::ng-deep .prose-rule-sentence { font-weight: 600; }
  `,
})
export class DeepComponent {
  policy = POLICY;
}
// endregion: ng_deep

// region: report_signal
@Component({
  selector: 'app-latest',
  imports: [OdrlProseComponent],
  template: `
    <ds-odrl-prose #prose [json]="policy" [strict]="true" />
    <p>{{ prose.report()?.verdict ?? 'validation is off' }}</p>
  `,
})
export class LatestComponent {
  policy = POLICY;
}
// endregion: report_signal
