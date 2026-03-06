import { helper } from './helpers';

export function greet(name: string): string {
  return `Hello, ${helper()} ${name}!`;
}
