import { Store } from '@store/state';

export function cached(key: string): any {
  const store: Store = new Store();
  return store.get(key);
}
