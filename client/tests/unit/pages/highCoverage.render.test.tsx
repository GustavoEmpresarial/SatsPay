/**
 * @vitest-environment jsdom
 * Targeted coverage for highest-miss client pages + interactions.
 */
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '../../helpers/renderWithProviders.js';

vi.mock('../../../src/lib/api.js', async () => {
  const { mockApi } = await import('../../helpers/apiMock.js');
  return {
    api: vi.fn(async (path: string, opts?: { method?: string; json?: unknown }) => {
      if (opts?.method === 'POST' && path.includes('/withdrawals')) {
        return { id: 'w-new', status: 'PENDING' };
      }
      if (opts?.method === 'POST' && (path.includes('/swap/execute') || path === '/swap')) {
        return {
          id: 'sw1',
          status: 'COMPLETED',
          source: 'house',
          provider: 'HOUSE',
        };
      }
      if (opts?.method === 'POST' && path.includes('/auth/2fa')) {
        return { codeSent: true, twoFactorEnabled: true };
      }
      if (opts?.method === 'PATCH' && path.includes('/auth/username')) {
        return {
          user: {
            id: '11111111-1111-1111-1111-111111111111',
            email: 'cov@bitcosats.test',
            username: 'covuser2',
            twoFactorEnabled: false,
            merchantStatus: 'APPROVED',
            createdAt: '2024-01-01T00:00:00.000Z',
            role: 'USER',
          },
        };
      }
      if (opts?.method === 'POST' && path.includes('/wallet/transfer')) {
        return { ok: true };
      }
      if (opts?.method === 'POST' && path.includes('/lend/action')) {
        return { ok: true, status: 'COMPLETED' };
      }
      if (opts?.method === 'POST' && path.includes('/merchant/deposits')) {
        return { delivered: true, statusCode: 200 };
      }
      if (opts?.method === 'POST' && path.includes('/oauth/apps')) {
        return {
          id: 'app-new',
          user_id: '11111111-1111-1111-1111-111111111111',
          name: 'New App',
          client_id: 'sats_app_new',
          client_secret: 'secret-once',
          client_secret_prefix: 'sats_sec',
          redirect_uris: ['https://example.com/cb'],
          is_active: true,
          created_at: '2024-06-01T00:00:00.000Z',
          updated_at: '2024-06-01T00:00:00.000Z',
        };
      }
      if (opts?.method === 'DELETE') return { ok: true };
      return mockApi(path);
    }),
    bootstrapSession: vi.fn(async () => true),
    forceReauth: vi.fn(),
  };
});

vi.mock('../../../src/components/Turnstile.js', () => ({ Turnstile: () => null }));
vi.mock('../../../src/components/ModernCaptcha.js', () => ({ ModernCaptcha: () => null }));
vi.mock('../../../src/lib/qr.js', () => ({
  addressQrDataUrl: vi.fn(async () => 'data:image/png;base64,xx'),
}));

import { DepositPage } from '../../../src/pages/DepositPage.js';
import { WithdrawPage } from '../../../src/pages/WithdrawPage.js';
import { SettingsPage } from '../../../src/pages/SettingsPage.js';
import { SwapPage } from '../../../src/pages/SwapPage.js';
import { ApiDocsPage } from '../../../src/pages/ApiDocsPage.js';
import { CheckoutPage } from '../../../src/pages/CheckoutPage.js';
import { OAuthAuthorizePage } from '../../../src/pages/OAuthAuthorizePage.js';
import { DeveloperBalances } from '../../../src/components/DeveloperBalances.js';
import { AppLayout } from '../../../src/components/AppLayout.js';
import { OAuthAppsPage } from '../../../src/pages/OAuthAppsPage.js';
import { LendPage } from '../../../src/pages/LendPage.js';
import { MerchantDepositsPage } from '../../../src/pages/MerchantDepositsPage.js';
import { FaucetListPage } from '../../../src/pages/FaucetListPage.js';

beforeAll(async () => {
  const i18n = (await import('../../../src/i18n/index.js')).default;
  await i18n.changeLanguage('en');
});

describe('DepositPage coverage', () => {
  it('deposit tab shows address + history tab lists deposits', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<DepositPage />, {
      route: '/deposit?coin=BTC',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/bc1q|Bitcoin|deposit|Deposit/i), {
      timeout: 4000,
    });

    const buttons = within(container).getAllByRole('button');
    const historyBtn = buttons.find((b) => /histór|history/i.test(b.textContent || ''));
    if (historyBtn) await user.click(historyBtn);
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(100));
    unmount();

    const hist = renderWithProviders(<DepositPage />, {
      route: '/deposit?tab=history&coin=LTC',
      loggedIn: true,
    });
    await waitFor(() => expect(hist.container.innerHTML.length).toBeGreaterThan(50), { timeout: 4000 });
    hist.unmount();
  });

  it('switches coins DOGE/BCH and paints network notes', async () => {
    for (const coin of ['DOGE', 'BCH', 'LTC'] as const) {
      const { container, unmount } = renderWithProviders(<DepositPage />, {
        route: `/deposit?coin=${coin}`,
        loggedIn: true,
      });
      await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(80), { timeout: 4000 });
      unmount();
    }
  });
});

describe('WithdrawPage coverage', () => {
  it('withdraw form + history with fixtures', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<WithdrawPage />, {
      route: '/withdraw?coin=BTC',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(80), { timeout: 4000 });

    const address = container.querySelector('input[type="text"], input:not([type])');
    const inputs = container.querySelectorAll('input');
    if (address) await user.type(address as HTMLElement, 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh');
    if (inputs[1]) await user.type(inputs[1] as HTMLElement, '0.01');

    const maxBtn = within(container)
      .getAllByRole('button')
      .find((b) => /^max$/i.test((b.textContent || '').trim()));
    if (maxBtn) await user.click(maxBtn);

    const submit = container.querySelector('button[type="submit"]');
    if (submit && !(submit as HTMLButtonElement).disabled) {
      await user.click(submit as HTMLElement);
    }

    const histBtn = within(container)
      .getAllByRole('button')
      .find((b) => /histór|history/i.test(b.textContent || ''));
    if (histBtn) await user.click(histBtn);
    unmount();

    const hist = renderWithProviders(<WithdrawPage />, {
      route: '/withdraw?tab=history',
      loggedIn: true,
    });
    await waitFor(() => expect(hist.container.innerHTML).toMatch(/CONFIRMED|PENDING|btc|BTC/i), {
      timeout: 4000,
    });
    // coin filter chips
    for (const label of [/all|tod/i, /btc/i, /ltc/i]) {
      const chip = within(hist.container)
        .queryAllByRole('button')
        .find((b) => label.test(b.textContent || ''));
      if (chip) await user.click(chip);
    }
    hist.unmount();
  });
});

describe('SettingsPage tabs', () => {
  it('opens profile / security / sessions / apps', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<SettingsPage />, {
      route: '/settings',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(50), { timeout: 4000 });

    for (const label of [/perfil|profile/i, /seguran|security/i, /sess/i, /app/i]) {
      const btn = within(container)
        .getAllByRole('button')
        .find((b) => label.test(b.textContent || ''));
      if (btn) await user.click(btn);
      await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(50));
    }

    // revoke connected app if button exists
    const revoke = within(container)
      .queryAllByRole('button')
      .find((b) => /revog|revoke|remover|remove|descon/i.test(b.textContent || ''));
    if (revoke) await user.click(revoke);

    unmount();
  });

  it('2FA enable flow and username save', async () => {
    const user = userEvent.setup();
    if (!navigator.clipboard?.writeText) {
      Object.defineProperty(navigator, 'clipboard', {
        configurable: true,
        value: { writeText: vi.fn(async () => undefined) },
      });
    } else {
      vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue(undefined);
    }
    const { container, unmount } = renderWithProviders(<SettingsPage />, {
      route: '/settings',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(80), { timeout: 4000 });

    const securityTab = within(container)
      .getAllByRole('button')
      .find((b) => /seguran|security|2fa/i.test(b.textContent || ''));
    if (securityTab) await user.click(securityTab);

    const enable2fa = within(container)
      .queryAllByRole('button')
      .find((b) => /ativar|enable.*2fa|habilitar/i.test(b.textContent || ''));
    if (enable2fa) await user.click(enable2fa);

    const codeInput = container.querySelector('input[inputmode="numeric"], input[maxlength="6"]');
    if (codeInput) {
      await user.type(codeInput as HTMLElement, '123456');
      const confirmBtn = within(container)
        .queryAllByRole('button')
        .find((b) => /confirm|verificar|validar/i.test(b.textContent || ''));
      if (confirmBtn) await user.click(confirmBtn);
    }

    const profileTab = within(container)
      .getAllByRole('button')
      .find((b) => /perfil|profile/i.test(b.textContent || ''));
    if (profileTab) await user.click(profileTab);

    const usernameInput = container.querySelector('input[name="username"], input#username');
    if (usernameInput) {
      await user.clear(usernameInput as HTMLInputElement);
      await user.type(usernameInput as HTMLElement, 'covuser2');
      const save = container.querySelector('button[type="submit"]');
      if (save) await user.click(save as HTMLElement);
    }

    const sessionsTab = within(container)
      .getAllByRole('button')
      .find((b) => /sess/i.test(b.textContent || ''));
    if (sessionsTab) await user.click(sessionsTab);
    const revokeSessions = within(container)
      .queryAllByRole('button')
      .find((b) => /revog|disconnect|desconectar.*sess/i.test(b.textContent || ''));
    if (revokeSessions) await user.click(revokeSessions);

    unmount();
  });
});

describe('SwapPage quote UI', () => {
  it('types amount and shows house quote route', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<SwapPage />, {
      route: '/swap',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(50), { timeout: 4000 });

    const amountInput =
      container.querySelector('input[inputmode="decimal"]') ||
      container.querySelector('input[type="text"]') ||
      container.querySelector('input');
    if (amountInput) {
      await user.clear(amountInput as HTMLInputElement);
      await user.type(amountInput as HTMLElement, '0.01');
    }

    await waitFor(
      () => {
        expect(container.innerHTML).toMatch(/0\.8|HOUSE|LTC|receber|receive|quote|rota/i);
      },
      { timeout: 5000 },
    );

    const flip = within(container)
      .queryAllByRole('button')
      .find((b) => /swap|trocar|invert|⇅|↔/i.test(b.textContent || '') || b.querySelector('.bi-arrow'));
    if (flip) await user.click(flip);

    const routeBtn = within(container)
      .queryAllByRole('button')
      .find((b) => /house|rota|route|recomend/i.test(b.textContent || ''));
    if (routeBtn) await user.click(routeBtn);

    const execute = within(container)
      .queryAllByRole('button')
      .find((b) => /execut|confirm|swap|trocar/i.test(b.textContent || ''));
    if (execute && !(execute as HTMLButtonElement).disabled) {
      await user.click(execute);
    }

    const histTab = within(container)
      .queryAllByRole('button')
      .find((b) => /histór|history/i.test(b.textContent || ''));
    if (histTab) await user.click(histTab);

    unmount();
  });
});

describe('ApiDocsPage tabs via query', () => {
  for (const tab of ['start', 'deposits', 'payouts', 'oauth', 'security', 'simulator'] as const) {
    it(`tab=${tab}`, async () => {
      const { container, unmount } = renderWithProviders(<ApiDocsPage />, {
        route: `/api-docs?tab=${tab}`,
        loggedIn: true,
      });
      await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(200), { timeout: 4000 });
      unmount();
    });
  }

  it('renders the onboarding trail, not just a shell', async () => {
    // The tab loop above only asserts that something rendered. This one fails
    // if the onboarding content itself throws or goes missing.
    const { findByText, container, unmount } = renderWithProviders(<ApiDocsPage />, {
      route: '/api-docs?tab=start',
      loggedIn: true,
    });
    await findByText(/Come\u00e7ar do zero/);
    expect(container.textContent).toContain('/v1/merchant/apply');
    expect(container.textContent).toContain('/v1/api-keys');
    unmount();
  });

  it('clicks through tab buttons', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<ApiDocsPage />, {
      route: '/api-docs',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(100), { timeout: 4000 });
    for (const label of [/deposit/i, /payout|saque/i, /oauth/i, /secur/i, /simulat/i]) {
      const btn = within(container)
        .queryAllByRole('button')
        .find((b) => label.test(b.textContent || ''));
      if (btn) await user.click(btn);
    }
    unmount();
  });
});

describe('CheckoutPage + OAuthAuthorize + DeveloperBalances + AppLayout', () => {
  it('checkout invoice pending', async () => {
    const { container, unmount } = renderWithProviders(<CheckoutPage />, {
      route: '/checkout/inv1',
      routePath: '/checkout/:id',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/Demo Shop|BTC|PENDING|invoice|pagamento|pay/i), {
      timeout: 4000,
    });
    unmount();
  });

  it('oauth authorize consent approve/deny without navigation', async () => {
    const user = userEvent.setup();
    const hrefSpy = vi.fn();
    const loc = window.location;
    Object.defineProperty(window, 'location', {
      configurable: true,
      value: {
        ...loc,
        href: loc.href,
        origin: loc.origin,
        pathname: '/oauth/authorize',
        search: '?client_id=sats_app_test',
        assign: hrefSpy,
        replace: hrefSpy,
      },
    });

    const { container, unmount } = renderWithProviders(<OAuthAuthorizePage />, {
      route:
        '/oauth/authorize?client_id=sats_app_test&redirect_uri=https%3A%2F%2Fexample.com%2Fcb&scope=openid&state=x&response_type=code',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/Partner|autoriz|consent|scop|openid/i), {
      timeout: 4000,
    });

    const deny = within(container)
      .queryAllByRole('button')
      .find((b) => /neg|deny|recus|cancel/i.test(b.textContent || ''));
    const approve = within(container)
      .queryAllByRole('button')
      .find((b) => /aprov|allow|autoriz|contin/i.test(b.textContent || ''));
    if (deny) await user.click(deny);
    if (approve) await user.click(approve);

    unmount();
    Object.defineProperty(window, 'location', { configurable: true, value: loc });
  });

  it('developer balances + app layout', async () => {
    const user = userEvent.setup();
    const bal = renderWithProviders(<DeveloperBalances />, { route: '/developer/wallets', loggedIn: true });
    await waitFor(() => expect(bal.container.innerHTML.length).toBeGreaterThan(30), { timeout: 4000 });

    const toDev = within(bal.container)
      .queryAllByRole('button')
      .find((b) => /developer|api|caixa|→/i.test(b.textContent || ''));
    if (toDev) await user.click(toDev);

    const amountInput = bal.container.querySelector('input[inputmode="decimal"], input[type="text"]');
    if (amountInput) await user.type(amountInput as HTMLElement, '0.001');

    const transferBtn = within(bal.container)
      .queryAllByRole('button')
      .find((b) => /transfer|transferir/i.test(b.textContent || ''));
    if (transferBtn && !(transferBtn as HTMLButtonElement).disabled) {
      await user.click(transferBtn);
    }

    const closeModal = within(bal.container)
      .queryAllByRole('button')
      .find((b) => /cancel|fechar|close/i.test(b.textContent || ''));
    if (closeModal) await user.click(closeModal);

    bal.unmount();

    const layout = renderWithProviders(<AppLayout />, { route: '/dashboard', loggedIn: true });
    await waitFor(() => expect(layout.container.innerHTML.length).toBeGreaterThan(40), { timeout: 4000 });
    for (const btn of within(layout.container).queryAllByRole('button').slice(0, 8)) {
      try {
        await user.click(btn);
      } catch {
        /* ignore */
      }
    }
    layout.unmount();
  });
});

describe('OAuthAppsPage UI', () => {
  it('lists apps, integration tabs, and create modal', async () => {
    const user = userEvent.setup();
    if (!navigator.clipboard?.writeText) {
      Object.defineProperty(navigator, 'clipboard', {
        configurable: true,
        value: { writeText: vi.fn(async () => undefined) },
      });
    } else {
      vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue(undefined);
    }
    const { container, unmount } = renderWithProviders(<OAuthAppsPage />, {
      route: '/oauth/apps',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/Test App|sats_app_test/i), {
      timeout: 5000,
    });

    for (const label of [/node|python|curl|html/i]) {
      const tab = within(container)
        .queryAllByRole('button')
        .find((b) => label.test(b.textContent || ''));
      if (tab) await user.click(tab);
    }

    const create = within(container)
      .queryAllByRole('button')
      .find((b) => /criar|create|novo|new app/i.test(b.textContent || ''));
    if (create) await user.click(create);

    const nameInput = container.querySelector('input[name="name"], input[placeholder*="nome"], input');
    if (nameInput && create) {
      await user.type(nameInput as HTMLElement, 'My OAuth App');
    }

    const close = within(container)
      .queryAllByRole('button')
      .find((b) => /cancel|fechar|close/i.test(b.textContent || ''));
    if (close) await user.click(close);

    unmount();
  });
});

describe('LendPage markets', () => {
  it('mounts markets and opens supply modal', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<LendPage />, {
      route: '/lend',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/USDT|Supply|Fornecer|Aave/i), {
      timeout: 5000,
    });

    const supplyBtn = within(container)
      .queryAllByRole('button')
      .find((b) => /supply|fornecer|depositar/i.test(b.textContent || ''));
    if (supplyBtn) await user.click(supplyBtn);

    const modalInput = container.querySelector('[role="dialog"] input, .modal input');
    if (modalInput) await user.type(modalInput as HTMLElement, '10');

    const close = within(container)
      .queryAllByRole('button')
      .find((b) => /cancel|fechar|close/i.test(b.textContent || ''));
    if (close) await user.click(close);

    unmount();
  });
});

describe('MerchantDepositsPage', () => {
  it(
    'filters invoices and tests webhook',
    async () => {
      const user = userEvent.setup();
      const { container, unmount } = renderWithProviders(<MerchantDepositsPage />, {
        route: '/merchant/deposits',
        loggedIn: true,
      });
      await waitFor(() => expect(container.innerHTML).toMatch(/ORD-PEND|Testar Webhook|minv2/i), {
        timeout: 8000,
      });

      for (const label of [/btc/i, /all|tod/i, /paid|pago/i, /pending|pend/i]) {
        const chip = within(container)
          .queryAllByRole('button')
          .find((b) => label.test(b.textContent || ''));
        if (chip) await user.click(chip);
      }

      const webhookBtn = within(container)
        .queryAllByRole('button')
        .find((b) => /webhook|testar|test/i.test(b.textContent || ''));
      if (webhookBtn) await user.click(webhookBtn);

      unmount();
    },
    15000,
  );
});

describe('FaucetListPage', () => {
  it('lists public faucets and filters', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<FaucetListPage />, {
      route: '/faucetlist',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/Public Faucet|faucet/i), {
      timeout: 5000,
    });

    const search = container.querySelector('input[type="search"], input[type="text"]');
    if (search) await user.type(search as HTMLElement, 'Public');

    const coinChip = within(container)
      .queryAllByRole('button')
      .find((b) => /^BTC$/i.test((b.textContent || '').trim()));
    if (coinChip) await user.click(coinChip);

    unmount();
  });
});
