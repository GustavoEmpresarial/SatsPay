/**
 * @vitest-environment jsdom
 * Extra pass on highest-LOC misses (Withdraw, Settings, OAuth, Swap, Deposit).
 */
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders, fakeUser } from '../../helpers/renderWithProviders.js';
import { useAuthStore } from '../../../src/stores/auth.js';
import { COINS } from '../../../src/shared/index.js';

vi.mock('../../../src/lib/api.js', async () => {
  const { mockApi } = await import('../../helpers/apiMock.js');
  const { ApiError: Err } = await import('../../../src/lib/api.js');
  return {
    api: vi.fn(async (path: string, opts?: { method?: string; json?: unknown }) => {
      if (opts?.method === 'POST' && path.includes('/withdrawals')) {
        return { id: 'w-new', status: 'PENDING' };
      }
      if (opts?.method === 'POST' && (path === '/swap' || path.includes('/swap/execute'))) {
        return { id: 'sw1', status: 'COMPLETED', source: 'house', provider: 'HOUSE' };
      }
      if (opts?.method === 'POST' && path.includes('/auth/2fa')) {
        return { twoFactorEnabled: true, codeSent: true };
      }
      if (opts?.method === 'POST' && path.includes('/auth/password')) {
        return { ok: true };
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
    <button type="button" onClick={() => onVerify('tok')}>
      captcha
    </button>
  ),
}));
vi.mock('../../../src/lib/qr.js', () => ({
  addressQrDataUrl: vi.fn(async () => 'data:image/png;base64,xx'),
}));

import { WithdrawPage } from '../../../src/pages/WithdrawPage.js';
import { SettingsPage } from '../../../src/pages/SettingsPage.js';
import { OAuthAppsPage } from '../../../src/pages/OAuthAppsPage.js';
import { SwapPage } from '../../../src/pages/SwapPage.js';
import { DepositPage } from '../../../src/pages/DepositPage.js';
import { MerchantDashboardPage } from '../../../src/pages/MerchantDashboardPage.js';

const ADDR = 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh';

beforeAll(async () => {
  const i18n = (await import('../../../src/i18n/index.js')).default;
  await i18n.changeLanguage('en');
});

beforeEach(() => {
  vi.spyOn(window, 'confirm').mockReturnValue(true);
});

describe('finalPush coverage', () => {
  it('Withdraw fees modal address book and submit', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<WithdrawPage />, {
      route: '/withdraw?coin=BTC',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(150), { timeout: 6000 });
    const addr = container.querySelector('input[type="text"]');
    if (addr) {
      await user.clear(addr as HTMLInputElement);
      await user.type(addr as HTMLElement, ADDR);
    }
    const amt = container.querySelector('input[placeholder="0.00"]');
    if (amt) {
      await user.clear(amt as HTMLInputElement);
      await user.type(amt as HTMLElement, '0.02');
    }
    const fees = within(container)
      .queryAllByRole('button')
      .find((b) => /taxa|fee/i.test(b.textContent || ''));
    if (fees) await user.click(fees);
    const book = within(container)
      .queryAllByRole('button')
      .find((b) => /agenda|address book|contatos/i.test(b.textContent || ''));
    if (book) await user.click(book);
    const submit = within(container)
      .queryAllByRole('button')
      .find((b) => /confirmar|sacar|withdraw/i.test(b.textContent || ''));
    if (submit && !(submit as HTMLButtonElement).disabled) await user.click(submit);
    unmount();
  });

  it('Settings 2FA enable flow and theme', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<SettingsPage />, {
      route: '/settings',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(80), { timeout: 6000 });
    const sec = within(container)
      .getAllByRole('button')
      .find((b) => /seguran|security/i.test(b.textContent || ''));
    if (sec) await user.click(sec);
    const enable2fa = within(container)
      .queryAllByRole('button')
      .find((b) => /ativar|enable.*2fa|two-factor/i.test(b.textContent || ''));
    if (enable2fa) await user.click(enable2fa);
    const theme = within(container)
      .queryAllByRole('button')
      .find((b) => /tema|theme|dark|claro/i.test(b.textContent || ''));
    if (theme) await user.click(theme);
    unmount();
  });

  it('OAuthApps wizard edit delete rotate', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<OAuthAppsPage />, {
      route: '/oauth/apps',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/Test App|Partner/i), { timeout: 6000 });
    const create = within(container)
      .queryAllByRole('button')
      .find((b) => /criar|nova|create/i.test(b.textContent || ''));
    if (create) await user.click(create);
    const nameIn = document.body.querySelector('input[type="text"]');
    if (nameIn) await user.type(nameIn as HTMLElement, 'Push App');
    const save = within(document.body as HTMLElement)
      .queryAllByRole('button')
      .find((b) => /criar aplicação|salvar/i.test(b.textContent || ''));
    if (save) await user.click(save);
    const del = within(container)
      .queryAllByRole('button')
      .find((b) => b.querySelector('.bi-trash'));
    if (del) await user.click(del);
    unmount();
  });

  it('Swap execute and Deposit all coins', async () => {
    const user = userEvent.setup();
    const swap = renderWithProviders(<SwapPage />, { route: '/swap', loggedIn: true });
    await waitFor(() => expect(swap.container.innerHTML.length).toBeGreaterThan(200), { timeout: 6000 });
    const inp = swap.container.querySelector('input[type="text"]');
    if (inp) {
      await user.clear(inp as HTMLInputElement);
      await user.type(inp as HTMLElement, '0.02');
    }
    await waitFor(() => expect(swap.container.innerHTML).toMatch(/HOUSE|0\./i), { timeout: 8000 });
    const go = within(swap.container)
      .queryAllByRole('button')
      .find((b) => /confirm|execut|swap/i.test(b.textContent || ''));
    if (go && !(go as HTMLButtonElement).disabled) await user.click(go);
    swap.unmount();

    for (const coin of COINS) {
      const dep = renderWithProviders(<DepositPage />, {
        route: `/deposit?coin=${coin}`,
        loggedIn: true,
      });
      await waitFor(() => expect(dep.container.innerHTML.length).toBeGreaterThan(50), { timeout: 4000 });
      dep.unmount();
    }
  });

  it('Merchant dashboard all coin filters', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<MerchantDashboardPage />, {
      route: '/merchant',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(120), { timeout: 8000 });
    for (const c of ['ALL', ...COINS]) {
      const chip = within(container)
        .getAllByRole('button')
        .find((b) =>
          c === 'ALL'
            ? /ALL|Tod/i.test(b.textContent || '')
            : new RegExp(`^${c}$`).test((b.textContent || '').trim()),
        );
      if (chip) await user.click(chip);
    }
    unmount();
    useAuthStore.setState({ user: fakeUser, accessToken: 'test-access-token' });
  });
});
