import { processA } from 'lib-a';
import { processB } from 'lib-b';

export function run(): string {
  return processA() + processB();
}
