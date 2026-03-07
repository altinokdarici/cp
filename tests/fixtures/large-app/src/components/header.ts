import { Button } from '@ui/components';
import { formatTitle } from '../utils/format';

export function Header(title: string): string {
  const formatted: string = formatTitle(title);
  return `<header>${formatted}${Button('menu')}</header>`;
}
