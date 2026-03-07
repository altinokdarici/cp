import { createElement } from 'react';
import { Store } from '@store/state';
import { Home } from './pages/home';
import { APP_NAME } from './utils/constants';

export function App() {
  const store: Store = new Store();
  const name: string = APP_NAME;
  return createElement('div', null, Home(store, name));
}
