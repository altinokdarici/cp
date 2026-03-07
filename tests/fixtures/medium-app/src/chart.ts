import { Chart, createAxis } from '@viz/chart';

export function renderChart(config: object): string {
  const axis = createAxis('x');
  return Chart(axis, config);
}
