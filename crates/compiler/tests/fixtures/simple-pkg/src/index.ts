import { greet } from './greet';
import { helper } from './helpers';

export function main(): string {
  return greet(helper());
}
