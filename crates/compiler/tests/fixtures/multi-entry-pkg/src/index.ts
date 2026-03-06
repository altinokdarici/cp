import React from 'react';
import { Button } from './Button';
import { formatName } from './utils';

export function App(): React.ReactElement {
  const name: string = formatName('world');
  return React.createElement(Button, { label: name });
}
