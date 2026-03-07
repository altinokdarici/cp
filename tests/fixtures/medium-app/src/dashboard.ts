import { renderChart } from './chart';
import { Chart } from '@viz/chart';

export function Dashboard(config: object): string {
  const chart: Chart = renderChart(config);
  return `<div>${chart}</div>`;
}
