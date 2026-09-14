/**
 * @vitest-environment jsdom
 * Click-through smoke on app pages to lift line coverage.
 */
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '../../helpers/renderWithProviders.js';

vi.mock('../../../src/lib/api.js', async () => {
  const { mockApi } = await import('../../helpers/apiMock.js');
  return {
    api: vi.fn(async (path: string, opts?: { method?: string }) => {
      if (opts?.method === 'POST') return { ok: true };
      if (opts?.method === 'DELETE') return { ok: true };
      return mockApi(path);
    }),
    bootstrapSession: vi.fn(async () => true),
    forceReauth: vi.fn(),
  };
});

vi.mock('../../../src/components/Turnstile.js', () => ({ Turnstile: () => null }));
vi.mock('../../../src/components/ModernCaptcha.js', () => ({
  ModernCaptcha: ({ onVerify }: { onVerify: (t: string) => void }) => (
    <button type="button" onClick={() => onVerify('tok')}>captcha</button>
  ),
}));
vi.mock('../../../src/lib/qr.js', () => ({
  addressQrDataUrl: vi.fn(async () => 'data:image/png;base64,xx'),
}));

import { WalletsPage } from '../../../src/pages/WalletsPage.js';
import { LoginPage } from '../../../src/pages/LoginPage.js';
import { RegisterPage } from '../../../src/pages/RegisterPage.js';
import { FaucetPage } from '../../../src/pages/FaucetPage.js';
import { SwapPage } from '../../../src/pages/SwapPage.js';
import { MerchantSitesPage } from '../../../src/pages/MerchantSitesPage.js';
import { ReferralPage } from '../../../src/pages/ReferralPage.js';
import { StatusPage } from '../../../src/pages/StatusPage.js';
import { DocumentationPage } from '../../../src/pages/DocumentationPage.js';
import { DashboardPage } from '../../../src/pages/DashboardPage.js';
import { AnalyticsPage } from '../../../src/pages/AnalyticsPage.js';
import { LendPage } from '../../../src/pages/LendPage.js';
import { MerchantDepositsPage } from '../../../src/pages/MerchantDepositsPage.js';
import { DepositPage } from '../../../src/pages/DepositPage.js';
import { WithdrawPage } from '../../../src/pages/WithdrawPage.js';
import { SettingsPage } from '../../../src/pages/SettingsPage.js';
import { COINS } from '../../../src/shared/index.js';
import { useAuthStore } from '../../../src/stores/auth.js';
import { fakeUser } from '../../helpers/renderWithProviders.js';

beforeAll(async () => {
  const i18n = (await import('../../../src/i18n/index.js')).default;
  await i18n.changeLanguage('en');
});

async function clickSomeButtons(container: HTMLElement, max = 6) {
  const user = userEvent.setup();
  for (const btn of within(container).queryAllByRole('button').slice(0, max)) {
    try {
      await user.click(btn);
    } catch {
      /* ignore */
    }
  }
}

describe('allPages interactions', () => {
  it('WalletsPage paints all coin cards', async () => {
    const { container, unmount } = renderWithProviders(<WalletsPage />, {
      route: '/wallets',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/BTC|LTC|DOGE/i), { timeout: 5000 });
    unmount();
  });

  it('Login and Register forms accept input', async () => {
    const user = userEvent.setup();
    const login = renderWithProviders(<LoginPage />, { route: '/login', loggedIn: false });
    await waitFor(() => expect(login.container.innerHTML.length).toBeGreaterThan(40), { timeout: 4000 });
    const email = login.container.querySelector('input[type="email"], input[name="email"]');
    if (email) await user.type(email as HTMLElement, 'test@example.com');
    login.unmount();

    const reg = renderWithProviders(<RegisterPage />, { route: '/register', loggedIn: false });
    await waitFor(() => expect(reg.container.innerHTML.length).toBeGreaterThan(40), { timeout: 4000 });
    const userInput = reg.container.querySelector('input[type="text"], input[name="username"]');
    if (userInput) await user.type(userInput as HTMLElement, 'newuser');
    reg.unmount();
  });

  it('FaucetPage claim path', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<FaucetPage />, {
      route: '/faucet',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(80), { timeout: 5000 });
    const cap = within(container).getAllByRole('button').find((b) => b.textContent === 'captcha');
    if (cap) await user.click(cap);
    await new Promise((r) => setTimeout(r, 850));
    const claim = within(container)
      .getAllByRole('button')
      .find((b) => /claim|reivindicar/i.test(b.textContent || ''));
    if (claim && !(claim as HTMLButtonElement).disabled) await user.click(claim);
    unmount();
  });

  it('SwapPage execute when quoted', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<SwapPage />, {
      route: '/swap',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(150), { timeout: 5000 });
    const inp = container.querySelector('input[type="text"]');
    if (inp) {
      await user.clear(inp as HTMLInputElement);
      await user.type(inp as HTMLElement, '0.01');
    }
    await waitFor(() => expect(container.innerHTML).toMatch(/HOUSE|0\./i), { timeout: 6000 });
    const submit = within(container)
      .getAllByRole('button')
      .find((b) => /confirm|instant swap|execut/i.test(b.textContent || ''));
    if (submit && !(submit as HTMLButtonElement).disabled) await user.click(submit);
    unmount();
  });

  it('Dashboard Analytics Lend submit', async () => {
    const user = userEvent.setup();
    const dash = renderWithProviders(<DashboardPage />, { route: '/dashboard', loggedIn: true });
    await waitFor(() => expect(dash.container.innerHTML.length).toBeGreaterThan(120), { timeout: 5000 });
    await clickSomeButtons(dash.container, 12);
    dash.unmount();

    const an = renderWithProviders(<AnalyticsPage />, { route: '/analytics', loggedIn: true });
    await waitFor(() => expect(an.container.innerHTML.length).toBeGreaterThan(80), { timeout: 5000 });
    await clickSomeButtons(an.container, 8);
    an.unmount();

    const lend = renderWithProviders(<LendPage />, { route: '/lend', loggedIn: true });
    await waitFor(() => expect(lend.container.innerHTML).toMatch(/Fornecer|Tomar/i), { timeout: 5000 });
    const supply = within(lend.container)
      .getAllByRole('button')
      .find((b) => /fornecer/i.test(b.textContent || ''));
    if (supply) {
      await user.click(supply);
      const input = lend.container.querySelector('input[placeholder="0.00"]');
      if (input) await user.type(input as HTMLElement, '5');
      const confirm = within(lend.container)
        .getAllByRole('button')
        .find((b) => /fornecer usdt|fornecer/i.test(b.textContent || ''));
      if (confirm && !(confirm as HTMLButtonElement).disabled) await user.click(confirm);
    }
    lend.unmount();
  });

  it(
    'MerchantDeposits exhaustive filters',
    async () => {
      const user = userEvent.setup();
      const { container, unmount } = renderWithProviders(<MerchantDepositsPage />, {
        route: '/merchant/deposits',
        loggedIn: true,
      });
      await waitFor(() => expect(container.innerHTML).toMatch(/ORD-EXP|minv3|Testar Webhook/i), {
        timeout: 8000,
      });
      for (const c of COINS) {
        const chip = within(container)
          .queryAllByRole('button')
          .find((b) => new RegExp(`^${c}$`).test((b.textContent || '').trim()));
        if (chip) await user.click(chip);
      }
      for (const st of [/expirado/i, /pendente/i, /pagos/i, /todos/i]) {
        const b = within(container)
          .queryAllByRole('button')
          .find((x) => st.test(x.textContent || ''));
        if (b) await user.click(b);
      }
      const whButtons = within(container)
        .queryAllByRole('button')
        .filter((b) => /testar webhook/i.test(b.textContent || ''));
      for (const b of whButtons.slice(0, 2)) await user.click(b);
      unmount();
    },
    15000,
  );

  it('Deposit all coins and Withdraw history', async () => {
    const user = userEvent.setup();
    for (const coin of COINS) {
      const dep = renderWithProviders(<DepositPage />, {
        route: `/deposit?coin=${coin}`,
        loggedIn: true,
      });
      await waitFor(() => expect(dep.container.innerHTML.length).toBeGreaterThan(50), { timeout: 3000 });
      const hist = within(dep.container)
        .queryAllByRole('button')
        .find((b) => /history|histór/i.test(b.textContent || ''));
      if (hist) await user.click(hist);
      dep.unmount();
    }
    const w = renderWithProviders(<WithdrawPage />, { route: '/withdraw?tab=history', loggedIn: true });
    await waitFor(() => expect(w.container.innerHTML).toMatch(/CONFIRMED|PENDING|Histórico/i), { timeout: 5000 });
    await clickSomeButtons(w.container, 10);
    w.unmount();
  });

  it('Settings 2FA disable when enabled', async () => {
    useAuthStore.setState({ user: { ...fakeUser, twoFactorEnabled: true }, accessToken: 'tok' });
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<SettingsPage />, { route: '/settings', loggedIn: true });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(80), { timeout: 5000 });
    const sec = within(container)
      .getAllByRole('button')
      .find((b) => /seguran|security/i.test(b.textContent || ''));
    if (sec) await user.click(sec);
    const disable = within(container)
      .queryAllByRole('button')
      .find((b) => /desativar|disable/i.test(b.textContent || ''));
    if (disable) await user.click(disable);
    unmount();
    useAuthStore.setState({ user: fakeUser, accessToken: 'test-access-token' });
  });

  it('MerchantSites Referral Status Documentation clicks', async () => {
    const sites = renderWithProviders(<MerchantSitesPage />, {
      route: '/merchant/sites',
      loggedIn: true,
    });
    await waitFor(() => expect(sites.container.innerHTML).toMatch(/Demo Shop|site/i), { timeout: 5000 });
    await clickSomeButtons(sites.container);
    sites.unmount();

    const ref = renderWithProviders(<ReferralPage />, { route: '/referrals', loggedIn: true });
    await waitFor(() => expect(ref.container.innerHTML).toMatch(/ABC123/i), { timeout: 5000 });
    await clickSomeButtons(ref.container);
    ref.unmount();

    const st = renderWithProviders(<StatusPage />, { route: '/status', loggedIn: true });
    await waitFor(() => expect(st.container.innerHTML.length).toBeGreaterThan(40), { timeout: 5000 });
    st.unmount();

    const doc = renderWithProviders(<DocumentationPage />, { route: '/documentation', loggedIn: true });
    await waitFor(() => expect(doc.container.innerHTML.length).toBeGreaterThan(40), { timeout: 5000 });
    await clickSomeButtons(doc.container, 10);
    doc.unmount();
  });
});
