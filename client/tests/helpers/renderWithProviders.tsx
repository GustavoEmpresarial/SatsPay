import React from 'react';
import { render, type RenderOptions } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { I18nextProvider } from 'react-i18next';
import i18n from '../../src/i18n/index.js';
import { useAuthStore } from '../../src/stores/auth.js';
import { useAdminStore } from '../../src/stores/admin.js';
import type { PublicUser } from '../../src/shared/index.js';
import { ensureJsdomDocument } from './jsdomGuard.js';

export const fakeUser: PublicUser = {
  id: '11111111-1111-1111-1111-111111111111',
  email: 'cov@bitcosats.test',
  username: 'covuser',
  twoFactorEnabled: false,
  merchantStatus: 'APPROVED',
  createdAt: '2024-01-01T00:00:00.000Z',
  role: 'USER',
};

export function resetAuth(loggedIn = true): void {
  useAuthStore.setState({
    user: loggedIn ? fakeUser : null,
    accessToken: loggedIn ? 'test-access-token' : null,
  });
  try {
    const persist = useAuthStore.persist;
    // @ts-expect-error stub hydration for RequireAuth
    persist.hasHydrated = () => true;
    // @ts-expect-error stub
    persist.onFinishHydration = (cb: () => void) => {
      cb();
      return () => undefined;
    };
  } catch {
    /* ignore */
  }
}

export function createTestQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false, staleTime: Infinity, gcTime: Infinity },
      mutations: { retry: false },
    },
  });
}

type Options = RenderOptions & {
  route?: string;
  /** When set, wraps UI in <Route path={routePath}> so useParams works. */
  routePath?: string;
  loggedIn?: boolean;
  admin?: boolean;
};

export function renderWithProviders(ui: React.ReactElement, opts: Options = {}) {
  ensureJsdomDocument();
  const { route = '/', routePath, loggedIn = true, admin = false, ...rest } = opts;
  resetAuth(loggedIn);
  if (admin) {
    useAdminStore.setState({
      admin: { id: '1', email: 'admin@bitcosats.test' },
      accessToken: 'admin-token',
    });
    try {
      const persist = useAdminStore.persist;
      // @ts-expect-error stub hydration for RequireAdmin
      persist.hasHydrated = () => true;
      // @ts-expect-error stub
      persist.onFinishHydration = (cb: () => void) => {
        cb();
        return () => undefined;
      };
    } catch {
      /* ignore */
    }
  }
  const client = createTestQueryClient();

  function Wrapper({ children }: { children: React.ReactNode }) {
    const inner = routePath ? (
      <Routes>
        <Route path={routePath} element={children} />
      </Routes>
    ) : (
      children
    );
    return (
      <I18nextProvider i18n={i18n}>
        <QueryClientProvider client={client}>
          <MemoryRouter initialEntries={[route]}>{inner}</MemoryRouter>
        </QueryClientProvider>
      </I18nextProvider>
    );
  }

  return render(ui, { wrapper: Wrapper, ...rest });
}
