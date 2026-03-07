import { Layout } from '@ui/components';
import { addClass } from '../utils/dom';

export function Modal(content: string): string {
  const cls: string = addClass('modal', 'open');
  return `<div class="${cls}">${content}</div>`;
}
