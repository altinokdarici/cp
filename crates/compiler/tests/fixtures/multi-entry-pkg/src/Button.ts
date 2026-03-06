import React from 'react';
import { formatName } from './utils';

interface ButtonProps {
  label: string;
}

export function Button(props: ButtonProps): React.ReactElement {
  const display: string = formatName(props.label);
  return React.createElement('button', null, display);
}
