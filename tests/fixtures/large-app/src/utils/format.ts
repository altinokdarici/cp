export function formatTitle(title: string): string {
  return title.charAt(0).toUpperCase() + title.slice(1);
}

export function formatDate(d: Date): string {
  return d.getFullYear().toString();
}

export function formatUrl(path: string): string {
  return `https://api.example.com${path}`;
}
