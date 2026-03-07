import { formatDate } from '../utils/format';

export function Footer(): string {
  const year: string = formatDate(new Date());
  return `<footer>${year}</footer>`;
}
