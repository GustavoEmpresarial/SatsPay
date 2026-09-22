import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../../');
const app = readFileSync(path.join(root, 'src/App.tsx'), 'utf8');
const layout = readFileSync(path.join(root, 'src/components/AdminLayout.tsx'), 'utf8');
const pagePath = path.join(root, 'src/pages/AdminUsersPage.tsx');

describe('Admin Users — structure', () => {
  it('page module file exists', () => {
    expect(existsSync(pagePath)).toBe(true);
  });

  it('is wired in App routes', () => {
    expect(app).toContain('path="users"');
    expect(app).toContain('AdminUsersPage');
  });

  it('appears in AdminLayout nav', () => {
    expect(layout).toContain("/admin/users");
    expect(layout).toContain('Usuários');
  });

  it('exports a React page component', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toMatch(/export function AdminUsersPage/);
    expect(src).toContain("/admin/users");
  });
});
