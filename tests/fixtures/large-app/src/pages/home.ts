import { Header } from '../components/header';
import { Footer } from '../components/footer';
import { fetchData } from '../services/api';

export function Home(store: any, name: string): string {
  const data = fetchData('/home');
  return `${Header(name)}${data}${Footer()}`;
}
