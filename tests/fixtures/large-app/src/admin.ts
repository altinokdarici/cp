import { createElement } from 'react';
import { Layout } from '@ui/components';
import { AdminPage } from './pages/admin';

export function AdminApp() {
  return createElement(Layout, null, AdminPage());
}
