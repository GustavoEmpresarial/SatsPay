/**
 * @vitest-environment jsdom
 * Cover App.tsx public route table (RequireAuth full mount is flaky under jsdom persist).
 */
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { waitFor } from '@testing-library/react';
import { renderWithProviders } from '../helpers/renderWithProviders.js';

vi.mock('../../src/lib/api.js', async () => {
  const { mockApi } = await import('../helpers/apiMock.js');
  return {
    api: vi.fn(async (path: string) => mockApi(path)),
    bootstrapSession: vi.fn(async () => true),
    forceReauth: vi.fn(),
  };
});

vi.mock('../../src/components/Turnstile.js', () => ({ Turnstile: () => null }));
vi.mock('../../src/components/ModernCaptcha.js', () => ({ ModernCaptcha: () => null }));
vi.mock('../../src/lib/qr.js', () => ({
  addressQrDataUrl: vi.fn(async () => 'data:image/png;base64,xx'),
}));

import { App } from '../../src/App.js';

beforeAll(async () => {
  const i18n = (await import('../../src/i18n/index.js')).default;
  await i18n.changeLanguage('en');
});

describe('App public route coverage', () => {
  for (const route of [
    '/',
    '/welcome',
    '/login',
    '/register',
    '/faq',
    '/features',
    '/r/REFCODE',
    '/api',
    '/pay/inv1',
    '/pay/inv-paid',
    '/pay/inv-expired',
    '/admin/login',
    '/no-such-page',
    '/oauth/authorize?client_id=x',
    '/oauth/bridge',
    '/documentation',
    '/coins',
    '/status',
    '/privacy',
    '/terms',
    '/cookies',
    '/security',
    '/guides',
    '/sitemap',
    '/llm',
    '/docs',
    '/demo',
  ] as const) {
    it(`renders ${route}`, async () => {
      const { container, unmount } = renderWithProviders(<App />, {
        route,
        loggedIn: false,
      });
      await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(20), {
        timeout: 4000,
      });
      unmount();
    });
  }
});
