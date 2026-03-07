import { Layout } from '@ui/components';
import { merge } from 'lodash/merge';

export function DataTable(rows: any[]): string {
  const config = merge({}, { sortable: true });
  return `<table>${rows.length} rows</table>`;
}
