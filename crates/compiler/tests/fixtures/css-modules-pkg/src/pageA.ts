import styles from './button.module.css';
import layout from './shared.module.css';

export function renderPageA(): string {
  return `
    <div class="${layout.container}">
      <button class="${styles.button} ${styles.primary}">Click me</button>
    </div>
  `;
}
