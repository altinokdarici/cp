import { card, title, content } from './card.module.css';
import layout from './shared.module.css';

export function renderPageB(): string {
  return `
    <div class="${layout.container}">
      <div class="${card}">
        <h2 class="${title}">Card Title</h2>
        <p class="${content}">Card body text</p>
      </div>
    </div>
  `;
}
