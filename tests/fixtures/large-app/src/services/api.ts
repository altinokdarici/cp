import { get } from '@http/client';
import { formatUrl } from '../utils/format';

export function fetchData(path: string): string {
  const url: string = formatUrl(path);
  return get(url);
}
