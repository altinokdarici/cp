import { post } from '@http/client';
import { Store } from '@store/state';

export function authenticate(): boolean {
  const store: Store = new Store();
  post('/auth', { token: store.get('token') });
  return true;
}
