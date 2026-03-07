export function addClass(base: string, ...classes: string[]): string {
  return [base, ...classes].join(' ');
}
