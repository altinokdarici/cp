export function formatNumber(n: number): string {
  return n.toLocaleString();
}

export function formatDate(d: Date): string {
  return d.toISOString();
}
