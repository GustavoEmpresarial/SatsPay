/**
 * @vitest-environment jsdom
 */
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '../../helpers/renderWithProviders.js';

vi.mock('../../../src/lib/api.js', () => ({
  api: vi.fn(async (path: string, opts?: { method?: string }) => {
    if (opts?.method === 'POST' && path.includes('/oauth/apps')) {
      return {
        id: 'new1',
        name: 'First App',
        client_id: 'sats_first',
        client_secret: 'secret-once',
        client_secret_prefix: 'sats_f',
        redirect_uris: ['https://example.com/cb'],
        user_id: '1',
        is_active: true,
        created_at: '2024-06-01T00:00:00.000Z',
        updated_at: '2024-06-01T00:00:00.000Z',
      };
    }
    if (path.includes('/oauth/apps')) return [];
    return {};
  }),
  bootstrapSession: vi.fn(async () => true),
  forceReauth: vi.fn(),
}));

import { OAuthAppsPage } from '../../../src/pages/OAuthAppsPage.js';

beforeAll(async () => {
  const i18n = (await import('../../../src/i18n/index.js')).default;
  await i18n.changeLanguage('en');
});

describe('OAuthAppsPage empty state', () => {
  it('shows empty CTA and opens create modal', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<OAuthAppsPage />, {
      route: '/oauth/apps',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/Primeiro Aplicativo|Criar/i), {
      timeout: 5000,
    });
    const create = within(container)
      .getAllByRole('button')
      .find((b) => /criar|create|primeiro/i.test(b.textContent || ''));
    if (create) await user.click(create);
    await waitFor(() => expect(document.body.innerHTML).toMatch(/Criar Nova Aplicação|Nome da Aplicação/i), {
      timeout: 5000,
    });
    unmount();
  });
});
