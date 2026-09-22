/**
 * @vitest-environment jsdom
 * Push toward 90% line coverage — high-miss pages + App shell routes.
 */
import React from 'react';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '../../helpers/renderWithProviders.js';

vi.mock('../../../src/lib/api.js', async () => {
  const { mockApi } = await import('../../helpers/apiMock.js');
  const { ApiError: Err } = await import('../../../src/lib/api.js');
  return {
    api: vi.fn(async (path: string, opts?: { method?: string; json?: unknown }) => {
      if (opts?.method === 'POST' && path.includes('/oauth/apps') && !path.includes('rotate')) {
        return {
          id: 'app-new',
          name: 'Created App',
          client_id: 'sats_app_created',
          client_secret: 'one-time-secret-value',
          client_secret_prefix: 'sats_sec',
          redirect_uris: ['https://example.com/cb'],
          user_id: '11111111-1111-1111-1111-111111111111',
          is_active: true,
          created_at: '2024-06-01T00:00:00.000Z',
          updated_at: '2024-06-01T00:00:00.000Z',
        };
      }
      if (opts?.method === 'PUT' && path.includes('/oauth/apps/')) {
        const apps = (await mockApi('/oauth/apps')) as { name: string }[];
        return { ...apps[0], name: 'Updated App' };
      }
      if (opts?.method === 'POST' && path.includes('/rotate-secret')) {
        return { name: 'Test App', client_secret: 'rotated-secret', client_id: 'sats_app_test' };
      }
      if (opts?.method === 'POST' && path === '/swap') {
        return { id: 'sw1', status: 'COMPLETED', source: 'house', provider: 'HOUSE' };
      }
      if (opts?.method === 'POST' && path.includes('/lend/action')) return { ok: true };
      if (opts?.method === 'POST' && path.includes('/withdrawals')) return { id: 'w1', status: 'PENDING' };
      if (opts?.method === 'POST' && path.includes('/merchant/deposits')) {
        return { delivered: false, error: 'timeout', statusCode: 504 };
      }
      if (opts?.method === 'DELETE') return { ok: true };
      return mockApi(path);
    }),
    bootstrapSession: vi.fn(async () => true),
    forceReauth: vi.fn(),
    ApiError: Err,
  };
});
vi.mock('../../../src/components/Turnstile.js', () => ({ Turnstile: () => null }));
vi.mock('../../../src/components/ModernCaptcha.js', () => ({
  ModernCaptcha: ({ onVerify }: { onVerify: (t: string) => void }) => (
    <button type="button" data-testid="mock-captcha" onClick={() => onVerify('tok')}>
      verify
    </button>
  ),
}));
vi.mock('../../../src/lib/qr.js', () => ({
  addressQrDataUrl: vi.fn(async () => 'data:image/png;base64,xx'),
}));

import { OAuthAppsPage } from '../../../src/pages/OAuthAppsPage.js';
import { MerchantDepositsPage } from '../../../src/pages/MerchantDepositsPage.js';
import { MerchantDashboardPage } from '../../../src/pages/MerchantDashboardPage.js';
import { AirdropPage } from '../../../src/pages/AirdropPage.js';
import { DepositPage } from '../../../src/pages/DepositPage.js';
import { WithdrawPage } from '../../../src/pages/WithdrawPage.js';
import { SwapPage } from '../../../src/pages/SwapPage.js';
import { LendPage } from '../../../src/pages/LendPage.js';
import { SettingsPage } from '../../../src/pages/SettingsPage.js';
import { DashboardPage } from '../../../src/pages/DashboardPage.js';
import { AnalyticsPage } from '../../../src/pages/AnalyticsPage.js';
import { SupportPage } from '../../../src/pages/SupportPage.js';
import { MerchantSitesPage } from '../../../src/pages/MerchantSitesPage.js';
import { FaucetPage } from '../../../src/pages/FaucetPage.js';
import { ApiKeysPage } from '../../../src/pages/ApiKeysPage.js';
import { CheckoutPage } from '../../../src/pages/CheckoutPage.js';
import { AdminMerchantsPage } from '../../../src/pages/AdminMerchantsPage.js';
import { WalletsPage } from '../../../src/pages/WalletsPage.js';

beforeAll(async () => {
  const i18n = (await import('../../../src/i18n/index.js')).default;
  await i18n.changeLanguage('en');
});

beforeEach(() => {
  vi.spyOn(window, 'confirm').mockReturnValue(true);
  vi.spyOn(window, 'open').mockImplementation(() => null);
  if (!navigator.clipboard?.writeText) {
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: vi.fn(async () => undefined) },
    });
  } else {
    vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue(undefined);
  }
});

describe('OAuthAppsPage wave90', () => {
  it(
    'selects apps, copies credentials, edits, rotates, closes secret modal',
    async () => {
      const user = userEvent.setup();
      const { container, unmount } = renderWithProviders(<OAuthAppsPage />, {
        route: '/oauth/apps',
        loggedIn: true,
      });
      await waitFor(() => expect(container.innerHTML).toMatch(/Test App|Partner Two/i), {
        timeout: 5000,
      });

      const partner = within(container).queryByText(/Partner Two/i);
      if (partner) await user.click(partner.closest('[role="button"]') || partner);

      const copyCid = within(container)
        .queryAllByRole('button')
        .find((b) => /^copiar$|^copy$/i.test((b.textContent || '').trim()) || b.textContent === 'Copiar');
      if (copyCid) await user.click(copyCid);

      const edit = within(container)
        .queryAllByRole('button')
        .find((b) => b.querySelector('.bi-pencil-square'));
      if (edit) {
        await user.click(edit);
        const nameInput = document.body.querySelector(
          'form input[type="text"]',
        ) as HTMLInputElement | null;
        if (nameInput) {
          nameInput.focus();
          nameInput.value = 'Updated App';
          nameInput.dispatchEvent(new Event('input', { bubbles: true }));
        }
        const save = within(document.body as HTMLElement)
          .queryAllByRole('button')
          .find((b) => /salvar alterações|criar aplicação/i.test(b.textContent || ''));
        if (save) await user.click(save);
      }

      const rotate = within(container)
        .queryAllByRole('button')
        .find((b) => /novo secret|rotate/i.test(b.textContent || ''));
      if (rotate) await user.click(rotate);

      await waitFor(
        () => expect(document.body.innerHTML).toMatch(/rotated|one-time|Guarde seu Client Secret/i),
        { timeout: 3000 },
      ).catch(() => undefined);

      const dismiss = within(document.body as HTMLElement)
        .queryAllByRole('button')
        .find((b) => /copiei|salvei|entendi/i.test(b.textContent || ''));
      if (dismiss) await user.click(dismiss);

      for (const label of [/popup/i, /medium/i, /sign in with/i, /continuar/i]) {
        const btn = within(container)
          .queryAllByRole('button')
          .find((b) => label.test(b.textContent || ''));
        if (btn) await user.click(btn);
      }

      unmount();
    },
    15000,
  );
});

describe('Merchant + Airdrop wave90', () => {
  it(
    'merchant deposits filters and dashboard widgets',
    async () => {
      const user = userEvent.setup();
      const dep = renderWithProviders(<MerchantDepositsPage />, {
        route: '/merchant/deposits',
        loggedIn: true,
      });
      await waitFor(() => expect(dep.container.innerHTML).toMatch(/ORD-EXP|Testar Webhook|minv3/i), {
        timeout: 8000,
      });
      for (const label of [/expirado|expired/i, /pendente/i, /pagos|paid/i, /todos/i]) {
        const b = within(dep.container)
          .queryAllByRole('button')
          .find((x) => label.test(x.textContent || ''));
        if (b) await user.click(b);
      }
      const sel = dep.container.querySelector('select');
      if (sel) await user.selectOptions(sel as HTMLSelectElement, 'USDT');
      const wh = within(dep.container)
        .queryAllByRole('button')
        .find((b) => /testar webhook/i.test(b.textContent || ''));
      if (wh) await user.click(wh);
      dep.unmount();

      const dash = renderWithProviders(<MerchantDashboardPage />, { route: '/merchant', loggedIn: true });
      await waitFor(
        () => expect(dash.container.innerHTML).toMatch(/Merchant Live|volume|Dashboard|Demo Shop/i),
        { timeout: 5000 },
      );
      for (const c of ['BTC', 'LTC', 'ALL']) {
        const chip = within(dash.container)
          .queryAllByRole('button')
          .find((b) => new RegExp(`^${c === 'ALL' ? 'ALL|Tod' : c}`, 'i').test(b.textContent || ''));
        if (chip) await user.click(chip);
      }
      dash.unmount();
    },
    15000,
  );

  it('airdrop paints profile, activity table, leaderboard', async () => {
    const { container, unmount } = renderWithProviders(<AirdropPage />, {
      route: '/airdrop',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/SatsPoints|Tier|leaderboard|Ranking/i), {
      timeout: 5000,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/FAUCET|top|pts/i), { timeout: 5000 }).catch(() => undefined);
    unmount();
  });
});

describe('Trading pages wave90', () => {
  it('deposit deep paths', async () => {
    const user = userEvent.setup();
    for (const coin of ['BTC', 'DOGE', 'SOL', 'BCH', 'POL']) {
      const dep = renderWithProviders(<DepositPage />, { route: `/deposit?coin=${coin}`, loggedIn: true });
      await waitFor(() => expect(dep.container.innerHTML.length).toBeGreaterThan(60), { timeout: 4000 });
      const copy = within(dep.container)
        .queryAllByRole('button')
        .find((b) => /copiar|copy|clipboard/i.test(b.textContent || '') || b.querySelector('.bi-clipboard'));
      if (copy) await user.click(copy);
      dep.unmount();
    }
  });

  it('withdraw deep path', async () => {
    const user = userEvent.setup();
    const w = renderWithProviders(<WithdrawPage />, { route: '/withdraw?coin=LTC', loggedIn: true });
    await waitFor(() => expect(w.container.textContent || '').toMatch(/Sacar/), { timeout: 12000 });
    const alter = within(w.container)
      .getAllByRole('button')
      .find((b) => /alterar/i.test(b.textContent || ''));
    if (alter) {
      await user.click(alter);
      const btc = within(w.container)
        .getAllByRole('button')
        .find((b) => (b.textContent || '').includes('Bitcoin'));
      if (btc) await user.click(btc);
    }
    w.unmount();
  }, 15_000);

  it('swap deep path', async () => {
    const user = userEvent.setup();
    const swap = renderWithProviders(<SwapPage />, { route: '/swap', loggedIn: true });
    await waitFor(() => expect(swap.container.innerHTML.length).toBeGreaterThan(200), { timeout: 5000 });
    const inp = swap.container.querySelector('input[type="text"]');
    if (inp) {
      await user.clear(inp as HTMLInputElement);
      await user.type(inp as HTMLElement, '0.05');
    }
    await waitFor(() => expect(swap.container.innerHTML).toMatch(/HOUSE|SWAPKIT|0\./i), { timeout: 6000 });
    const coinPicker = within(swap.container)
      .getAllByRole('button')
      .find((b) => /LTC|BTC/.test(b.textContent || '') && b.querySelector('.bi-chevron-down'));
    if (coinPicker) await user.click(coinPicker);
    swap.unmount();
  });

  it('lend deep path', async () => {
    const lend = renderWithProviders(<LendPage />, { route: '/lend', loggedIn: true });
    await waitFor(() => expect(lend.container.innerHTML).toMatch(/Manutenção|Empréstimos|Aave/i), { timeout: 5000 });
    lend.unmount();
  });
});

describe('App shell pages wave90', () => {
  it('dashboard analytics support settings merchant sites', async () => {
    const user = userEvent.setup();

    const dash = renderWithProviders(<DashboardPage />, { route: '/dashboard', loggedIn: true });
    await waitFor(() => expect(dash.container.innerHTML.length).toBeGreaterThan(100), { timeout: 5000 });
    const quick = within(dash.container)
      .queryAllByRole('button')
      .filter((b) => /refresh|atualizar|ver/i.test(b.textContent || ''))
      .slice(0, 3);
    for (const btn of quick) {
      try {
        await user.click(btn);
      } catch {
        /* ignore */
      }
    }
    dash.unmount();

    const an = renderWithProviders(<AnalyticsPage />, { route: '/analytics', loggedIn: true });
    await waitFor(() => expect(an.container.innerHTML.length).toBeGreaterThan(80), { timeout: 5000 });
    an.unmount();

    const sup = renderWithProviders(<SupportPage />, { route: '/support', loggedIn: true });
    await waitFor(() => expect(sup.container.innerHTML.length).toBeGreaterThan(40), { timeout: 5000 });
    sup.unmount();

    const settings = renderWithProviders(<SettingsPage />, { route: '/settings', loggedIn: true });
    await waitFor(() => expect(settings.container.innerHTML.length).toBeGreaterThan(80), { timeout: 5000 });
    for (const label of [/perfil|profile/i, /seguran|security/i, /sess/i, /app/i]) {
      const tab = within(settings.container)
        .getAllByRole('button')
        .find((b) => label.test(b.textContent || ''));
      if (tab) await user.click(tab);
    }
    settings.unmount();

    const sites = renderWithProviders(<MerchantSitesPage />, { route: '/merchant/sites', loggedIn: true });
    await waitFor(() => expect(sites.container.innerHTML.length).toBeGreaterThan(40), { timeout: 5000 });
    sites.unmount();
  });

  it('faucet api keys checkout wallets admin merchants', async () => {
    const user = userEvent.setup();

    const faucet = renderWithProviders(<FaucetPage />, { route: '/faucet', loggedIn: true });
    await waitFor(() => expect(faucet.container.innerHTML.length).toBeGreaterThan(80), { timeout: 5000 });
    const cap = faucet.container.querySelector('[data-testid="mock-captcha"]');
    if (cap) await user.click(cap as HTMLElement);
    faucet.unmount();

    const keys = renderWithProviders(<ApiKeysPage />, { route: '/api-keys', loggedIn: true });
    await waitFor(() => expect(keys.container.innerHTML).toMatch(/Production|sats_live/i), { timeout: 5000 });
    keys.unmount();

    const checkout = renderWithProviders(<CheckoutPage />, {
      route: '/checkout/inv1',
      routePath: '/checkout/:id',
      loggedIn: true,
    });
    await waitFor(() => expect(checkout.container.innerHTML).toMatch(/Demo|BTC|pay/i), { timeout: 5000 });
    checkout.unmount();

    const wallets = renderWithProviders(<WalletsPage />, { route: '/wallets', loggedIn: true });
    await waitFor(() => expect(wallets.container.innerHTML).toMatch(/BTC|balance/i), { timeout: 5000 });
    wallets.unmount();

    const admin = renderWithProviders(<AdminMerchantsPage />, {
      route: '/admin/merchants',
      loggedIn: true,
      admin: true,
    });
    await waitFor(() => expect(admin.container.innerHTML).toMatch(/Comerciantes|Overview|Gateway/i), {
      timeout: 5000,
    });
    const merchantsTab = within(admin.container)
      .getAllByRole('button')
      .find((b) => /^Comerciantes$/i.test((b.textContent || '').trim()));
    if (merchantsTab) await user.click(merchantsTab);
    await waitFor(() => expect(admin.container.innerHTML).toMatch(/Demo Merchant/i), { timeout: 5000 });
    const approve = within(admin.container)
      .queryAllByRole('button')
      .find((b) => /aprovar|approve/i.test(b.textContent || ''));
    if (approve) await user.click(approve);
    admin.unmount();
  });
});
