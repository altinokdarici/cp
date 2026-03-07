import { createElement } from 'react';
import { formatName } from './utils';

export function App() {
  const name: string = formatName('world');
  return createElement('div', null, name);
}
