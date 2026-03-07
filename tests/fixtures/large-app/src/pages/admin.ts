import { Header } from '../components/header';
import { DataTable } from '../components/table';
import { authenticate } from '../services/auth';

export function AdminPage(): string {
  authenticate();
  return `${Header('Admin')}${DataTable([])}`;
}
