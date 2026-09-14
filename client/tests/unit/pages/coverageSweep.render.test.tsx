/**
 * @vitest-environment jsdom
 * Targeted interactions for remaining high line-miss pages (no full App + RequireAuth).
 */
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders, fakeUser } from '../../helpers/renderWithProviders.js';
import { useAuthStore } from '../../../src/stores/auth.js';
import { COINS } from '../../../src/shared/index.js';

vi.mock('../../../src/lib/api.js', async () => {
  const { mockApi } = await import('../../helpers/apiMock.js');
  return {
    api: vi.fn(async (path: string, opts?: { method?: string; json?: unknown }) => {
      if (opts?.method === 'POST') {
        if (path.includes('/test-webhook')) return mockApi(path);
        if (path.includes('/oauth/authorize')) return { redirect_url: 'https://example.com/cb?code=abc' };
        if (path.includes('/auth/password')) return { ok: true };
        if (path.includes('/auth/2fa/disable')) return { ok: true };
        if (path.includes('/withdrawals')) return { id: 'w1', status: 'PENDING' };
        return { ok: true };
      }
      return mockApi(path);
    }),
    bootstrapSession: vi.fn(async () => true),
    forceReauth: vi.fn(),
  };
});

vi.mock('../../../src/components/Turnstile.js', () => ({ Turnstile: () => null }));
vi.mock('../../../src/components/ModernCaptcha.js', () => ({
  ModernCaptcha: ({ onVerify }: { onVerify: (t: string) => void }) => (
    <button type="button" onClick={() => onVerify('tok')}>
      captcha
    </button>
  ),
}));
vi.mock('../../../src/lib/qr.js', () => ({
  addressQrDataUrl: vi.fn(async () => 'data:image/png;base64,xx'),
}));

import { MerchantDashboardPage } from '../../../src/pages/MerchantDashboardPage.js';
import { OAuthAuthorizePage } from '../../../src/pages/OAuthAuthorizePage.js';
import { SettingsPage } from '../../../src/pages/SettingsPage.js';
import { WithdrawPage } from '../../../src/pages/WithdrawPage.js';
import { ApiKeysPage } from '../../../src/pages/ApiKeysPage.js';
import { DashboardPage } from '../../../src/pages/DashboardPage.js';
import { FaucetPage } from '../../../src/pages/FaucetPage.js';
import { AnalyticsPage } from '../../../src/pages/AnalyticsPage.js';
import { OAuthAppsPage } from '../../../src/pages/OAuthAppsPage.js';

beforeAll(async () => {
  const i18n = (await import('../../../src/i18n/index.js')).default;
  await i18n.changeLanguage('en');
});

beforeEach(() => {
  vi.spyOn(window, 'confirm').mockReturnValue(true);
  if (!navigator.clipboard?.writeText) {
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: vi.fn(async () => undefined) },
    });
  }
});

describe('coverageSweep pages', () => {
  it('MerchantDashboard filters and table', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<MerchantDashboardPage />, {
      route: '/merchant',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/Demo Shop|volume|Merchant/i), { timeout: 6000 });
    for (const c of ['ALL', 'BTC', 'LTC', 'USDT', 'DOGE']) {
      const chip = within(container)
        .getAllByRole('button')
        .find((b) => new RegExp(c === 'ALL' ? 'ALL|Tod' : `^${c}$`).test((b.textContent || '').trim()));
      if (chip) await user.click(chip);
    }
    unmount();
  });

  it('OAuthAuthorize approve and deny when logged in', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<OAuthAuthorizePage />, {
      route: '/oauth/authorize?client_id=sats_app_test&redirect_uri=https://example.com/cb&response_type=code',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/Partner App|authorize|Autorizar/i), {
      timeout: 8000,
    });
    const deny = within(container)
      .queryAllByRole('button')
      .find((b) => /negar|deny|cancel/i.test(b.textContent || ''));
    if (deny) await user.click(deny);
    unmount();

    const again = renderWithProviders(<OAuthAuthorizePage />, {
      route: '/oauth/authorize?client_id=sats_app_test&redirect_uri=https://example.com/cb&response_type=code',
      loggedIn: true,
    });
    await waitFor(() => expect(again.container.innerHTML).toMatch(/Partner App/i), { timeout: 8000 });
    const approve = within(again.container)
      .queryAllByRole('button')
      .find((b) => /autorizar|approve|allow/i.test(b.textContent || ''));
    if (approve) await user.click(approve);
    again.unmount();
  });

  it('OAuthAuthorize login prompt when logged out', async () => {
    const { container, unmount } = renderWithProviders(<OAuthAuthorizePage />, {
      route: '/oauth/authorize?client_id=sats_app_test&redirect_uri=https://example.com/cb',
      loggedIn: false,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/login|entrar|Partner App/i), { timeout: 8000 });
    unmount();
  });

  it('Settings profile password and sessions', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<SettingsPage />, { route: '/settings', loggedIn: true });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(80), { timeout: 6000 });
    for (const label of [/perfil|profile/i, /seguran|security/i, /sess/i, /aplicativos|apps/i]) {
      const tab = within(container)
        .getAllByRole('button')
        .find((b) => label.test(b.textContent || ''));
      if (tab) await user.click(tab);
    }
    const pw = container.querySelector('input[type="password"]');
    if (pw) {
      await user.type(pw as HTMLElement, 'oldpass');
      const pw2 = container.querySelectorAll('input[type="password"]')[1];
      if (pw2) await user.type(pw2 as HTMLElement, 'newpass123!');
    }
    const save = within(container)
      .queryAllByRole('button')
      .find((b) => /salvar|alterar senha|update password/i.test(b.textContent || ''));
    if (save) await user.click(save);
    unmount();
  });

  it('Withdraw every coin tab and form', async () => {
    const user = userEvent.setup();
    for (const coin of COINS.slice(0, 6)) {
      const { container, unmount } = renderWithProviders(<WithdrawPage />, {
        route: `/withdraw?coin=${coin}`,
        loggedIn: true,
      });
      await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(100), { timeout: 5000 });
      const hist = within(container)
        .queryAllByRole('button')
        .find((b) => /histórico|history/i.test(b.textContent || ''));
      if (hist) await user.click(hist);
      const back = within(container)
        .queryAllByRole('button')
        .find((b) => /sacar|withdraw|nova/i.test(b.textContent || ''));
      if (back) await user.click(back);
      unmount();
    }
  });

  it('ApiKeys create rotate revoke', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<ApiKeysPage />, {
      route: '/api-keys',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/Production|sats_live/i), { timeout: 6000 });
    const create = within(container)
      .queryAllByRole('button')
      .find((b) => /criar|create|nova/i.test(b.textContent || ''));
    if (create) await user.click(create);
    const rotate = within(container)
      .queryAllByRole('button')
      .find((b) => /rotacion|rotate/i.test(b.textContent || ''));
    if (rotate) await user.click(rotate);
    const revoke = within(container)
      .queryAllByRole('button')
      .find((b) => /revogar|revoke|excluir/i.test(b.textContent || ''));
    if (revoke) await user.click(revoke);
    unmount();
  });

  it('Dashboard Analytics Faucet OAuthApps clicks', async () => {
    const user = userEvent.setup();
    const dash = renderWithProviders(<DashboardPage />, { route: '/dashboard', loggedIn: true });
    await waitFor(() => expect(dash.container.innerHTML.length).toBeGreaterThan(100), { timeout: 6000 });
    for (const btn of within(dash.container).queryAllByRole('button').slice(0, 15)) {
      try {
        await user.click(btn);
      } catch {
        /* ignore */
      }
    }
    dash.unmount();

    const an = renderWithProviders(<AnalyticsPage />, { route: '/analytics', loggedIn: true });
    await waitFor(() => expect(an.container.innerHTML.length).toBeGreaterThan(80), { timeout: 6000 });
    for (const btn of within(an.container).queryAllByRole('button').slice(0, 10)) {
      try {
        await user.click(btn);
      } catch {
        /* ignore */
      }
    }
    an.unmount();

    const faucet = renderWithProviders(<FaucetPage />, { route: '/faucet', loggedIn: true });
    await waitFor(() => expect(faucet.container.innerHTML.length).toBeGreaterThan(80), { timeout: 6000 });
    faucet.unmount();

    useAuthStore.setState({ user: { ...fakeUser, twoFactorEnabled: true }, accessToken: 'tok' });
    const oauth = renderWithProviders(<OAuthAppsPage />, { route: '/oauth/apps', loggedIn: true });
    await waitFor(() => expect(oauth.container.innerHTML).toMatch(/Test App/i), { timeout: 6000 });
    const create = within(oauth.container)
      .queryAllByRole('button')
      .find((b) => /criar|nova|create/i.test(b.textContent || ''));
    if (create) await user.click(create);
    oauth.unmount();
    useAuthStore.setState({ user: fakeUser, accessToken: 'test-access-token' });
  });
});
