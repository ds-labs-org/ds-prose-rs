import { Component, signal } from '@angular/core';
import { OdrlProseComponent } from '@ds-labs/prose-angular';
import { PRESETS } from './fixtures';

@Component({
  selector: 'app-root',
  imports: [OdrlProseComponent],
  templateUrl: './app.html',
})
export class App {
  protected readonly presets = PRESETS;
  protected readonly json = signal(PRESETS[0].json);
}
