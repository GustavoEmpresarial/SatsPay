/**
 * @vitest-environment jsdom
 * Settings → real API contract: the 2FA tab and "Desconectar outros aparelhos"
 * must hit routes that exist on the server. The session button used to fake
 * success with a 600 ms timeout and never called the API.
 */
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '../../../helpers/renderWithProviders.js';
import { useAuthStore } from '../../../../src/stores/auth.js';

const calls: Array<{ path: string; method?: string; json?: unknown }> = [];
let failRevoke = false;

vi.mock('../../../../src/lib/api.js', async () => {
  const actual = await vi.importActual<typeof import('../../../../src/lib/api.js')>('../../../../src/lib/api.js');
  const { mockApi } = await import('../../../helpers/apiMock.js');
  return {
    ...actual,
    api: vi.fn(async (path: string, opts?: { method?: string; json?: unknown }) => {
      calls.push({ path, method: opts?.method, json: opts?.json });
      if (path === '/auth/sessions/revoke-others') {
        if (failRevoke) throw new actual.ApiError(403, 'CSRF_REJECTED', 'origin not allowed');
        return { revoked: true, accessToken: 'fresh-access-token' };
      }
      if (path === '/auth/2fa/request') return { codeSent: true };
      return mockApi(path);
    }),
    bootstrapSession: vi.fn(async () => true),
    forceReauth: vi.fn(),
  };
});

import { SettingsPage } from '../../../../src/pages/SettingsPage.js';

beforeAll(async () => {
  const i18n = (await import('../../../../src/i18n/index.js')).default;
  await i18n.changeLanguage('pt');
});

beforeEach(() => {
  calls.length = 0;
  failRevoke = false;
});

async function openTab(label: RegExp) {
  const user = userEvent.setup();
  renderWithProviders(<SettingsPage />, { route: '/settings', loggedIn: true });
  await user.click(await screen.findByRole('button', { name: label }));
  return user;
}

describe('SettingsPage API contract', () => {
  it('revoke-others calls the API and stores the re-issued access token', async () => {
    const user = await openTab(/Sessões & Atividades/);
    await user.click(screen.getByRole('button', { name: /Desconectar Outros Aparelhos/ }));
    await waitFor(() => expect(calls.some((c) => c.path === '/auth/sessions/revoke-others' && c.method === 'POST')).toBe(true));
    await screen.findByText(/foram desconectados com sucesso/);
    expect(useAuthStore.getState().accessToken).toBe('fresh-access-token');
  });

  it('revoke-others surfaces the server error instead of faking success', async () => {
    failRevoke = true;
    const user = await openTab(/Sessões & Atividades/);
    await user.click(screen.getByRole('button', { name: /Desconectar Outros Aparelhos/ }));
    await screen.findByText(/origin not allowed/);
    expect(screen.queryByText(/foram desconectados com sucesso/)).toBeNull();
  });

  it('2FA request posts a toggle purpose to /auth/2fa/request', async () => {
    const user = await openTab(/Segurança & 2FA/);
    const enable = screen
      .getAllByRole('button')
      .find((b) => /ativar|enable/i.test(b.textContent || '') && !/desativ/i.test(b.textContent || ''));
    expect(enable).toBeTruthy();
    await user.click(enable!);
    await waitFor(() => {
      const c = calls.find((x) => x.path === '/auth/2fa/request');
      expect(c?.method).toBe('POST');
      expect(c?.json).toEqual({ purpose: 'ENABLE_2FA' });
    });
  });
});
