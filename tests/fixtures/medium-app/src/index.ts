import { Dashboard } from './dashboard';
import { merge } from 'lodash/merge';
import config from './config.json';

export function init(): object {
  const settings: object = merge({}, config);
  return Dashboard(settings);
}
