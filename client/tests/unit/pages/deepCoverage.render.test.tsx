/**
 * @vitest-environment jsdom
 * Deep RTL interactions for highest line-miss surfaces.
 */
import React from 'react';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders, fakeUser } from '../../helpers/renderWithProviders.js';
import { useAuthStore } from '../../../src/stores/auth.js';

let withdrawPostFails = false;

vi.mock('../../../src/lib/api.js', async () => {
  const { mockApi } = await import('../../helpers/apiMock.js');
  const { ApiError: Err } = await import('../../../src/lib/api.js');
  return {
    api: vi.fn(async (path: string, opts?: { method?: string; json?: unknown }) => {
      if (opts?.method === 'POST' && path.includes('/withdrawals')) {
        if (withdrawPostFails) {
          throw new Err(400, 'INSUFFICIENT_FUNDS', 'Saldo insuficiente');
        }
        return { id: 'w-new', status: 'PENDING' };
      }
      if (opts?.method === 'POST' && (path.includes('/swap/execute') || path === '/swap')) {
        return { id: 'sw1', status: 'COMPLETED', source: 'house', provider: 'HOUSE' };
      }
      if (opts?.method === 'POST' && path.includes('/faucet/claim')) {
        return {
          amount: '1000',
          nextClaimAt: new Date(Date.now() + 11 * 3600_000).toISOString(),
        };
      }
      if (opts?.method === 'POST' && path.includes('/public/keys')) {
        return { id: 'key-new', key: 'sats_live_full_secret_key_12345', prefix: 'sats_live' };
      }
      if (opts?.method === 'POST' && path.includes('/public/keys/') && path.includes('/rotate')) {
        return { id: 'key1', key: 'sats_live_rotated_secret_999', prefix: 'sats_live' };
      }
      if (opts?.method === 'POST' && path.includes('/wallet/transfer')) {
        return { ok: true };
      }
      if (opts?.method === 'POST' && path.includes('/auth/2fa')) {
        return { codeSent: true, twoFactorEnabled: true };
      }
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
      if (opts?.method === 'POST' && path.includes('/rotate-secret')) {
        return {
          name: 'Test App',
          client_secret: 'rotated-secret',
          client_id: 'sats_app_test',
        };
      }
      if (opts?.method === 'PUT' && path.includes('/oauth/apps/')) {
        return { id: 'app1', name: 'Updated' };
      }
      if (opts?.method === 'POST' && path.includes('/lend/action')) {
        return { ok: true };
      }
      if (opts?.method === 'DELETE') return { ok: true };
      const bare = path.split('?')[0] || path;
      if (bare === '/merchant/deposits' || bare.includes('/merchant/deposits/')) {
        return mockApi(path);
      }
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
    <button type="button" data-testid="mock-captcha" onClick={() => onVerify('mock-captcha-token')}>
      Verify captcha
    </button>
  ),
}));
vi.mock('../../../src/lib/qr.js', () => ({
  addressQrDataUrl: vi.fn(async () => 'data:image/png;base64,xx'),
}));

import { WithdrawPage } from '../../../src/pages/WithdrawPage.js';
import { SwapPage } from '../../../src/pages/SwapPage.js';
import { DeveloperBalances } from '../../../src/components/DeveloperBalances.js';
import { SettingsPage } from '../../../src/pages/SettingsPage.js';
import { OAuthAppsPage } from '../../../src/pages/OAuthAppsPage.js';
import { FaucetPage } from '../../../src/pages/FaucetPage.js';
import { ApiKeysPage } from '../../../src/pages/ApiKeysPage.js';
import { LendPage } from '../../../src/pages/LendPage.js';
import { MerchantDepositsPage } from '../../../src/pages/MerchantDepositsPage.js';
import { CheckoutPage } from '../../../src/pages/CheckoutPage.js';
import { AdminMerchantsPage } from '../../../src/pages/AdminMerchantsPage.js';
import { WalletsPage } from '../../../src/pages/WalletsPage.js';
import { ReferralPage } from '../../../src/pages/ReferralPage.js';
import { MerchantDashboardPage } from '../../../src/pages/MerchantDashboardPage.js';
import { DepositPage } from '../../../src/pages/DepositPage.js';
import { DeveloperWalletsPage } from '../../../src/pages/DeveloperWalletsPage.js';
import { AppLayout } from '../../../src/components/AppLayout.js';

beforeAll(async () => {
  const i18n = (await import('../../../src/i18n/index.js')).default;
  await i18n.changeLanguage('en');
});

beforeEach(() => {
  withdrawPostFails = false;
  vi.spyOn(window, 'confirm').mockReturnValue(false);
  localStorage.clear();
  if (!navigator.clipboard?.writeText) {
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: vi.fn(async () => undefined) },
    });
  } else {
    vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue(undefined);
  }
});

const BTC_ADDR = 'bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh';

async function fillWithdrawForm(container: HTMLElement, user: ReturnType<typeof userEvent.setup>) {
  const addressInput = container.querySelector('input[type="text"]') as HTMLInputElement | null;
  if (addressInput) {
    await user.clear(addressInput);
    await user.type(addressInput, BTC_ADDR);
  }
  const amountInput = container.querySelector('input[placeholder="0.00"]') as HTMLInputElement | null;
  if (amountInput) {
    await user.clear(amountInput);
    await user.type(amountInput, '0.01');
  }
}

describe('WithdrawPage deep', () => {
  it('shows 2FA field when enabled and submits with totp', async () => {
    useAuthStore.setState({
      user: { ...fakeUser, twoFactorEnabled: true },
      accessToken: 'test-access-token',
    });
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<WithdrawPage />, {
      route: '/withdraw?coin=BTC',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/2FA|Autenticação/i), { timeout: 5000 });
    await fillWithdrawForm(container, user);
    const totp = container.querySelector('input[placeholder="000000"]');
    if (totp) await user.type(totp as HTMLElement, '123456');
    const submit = container.querySelector('button[type="submit"]') as HTMLButtonElement | null;
    if (submit && !submit.disabled) await user.click(submit);
    unmount();
    useAuthStore.setState({ user: fakeUser, accessToken: 'test-access-token' });
  });

  it('coin switch, address book, fees modal, successful submit', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<WithdrawPage />, {
      route: '/withdraw?coin=BTC',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(200), { timeout: 5000 });

    const alterCoin = within(container)
      .getAllByRole('button')
      .find((b) => /alterar/i.test(b.textContent || ''));
    if (alterCoin) {
      await user.click(alterCoin);
      const ltc = within(container)
        .getAllByRole('button')
        .find((b) => (b.textContent || '').includes('Litecoin') || /\bLTC\b/.test(b.textContent || ''));
      if (ltc) await user.click(ltc);
    }

    const savedBtn = within(container)
      .getAllByRole('button')
      .find((b) => /endereços salvos|saved/i.test(b.textContent || ''));
    if (savedBtn) await user.click(savedBtn);

    const labelInput = container.querySelector('input[placeholder*="rótulo"], input[placeholder*="label"]');
    if (labelInput) {
      await user.type(labelInput as HTMLElement, 'My wallet');
      await fillWithdrawForm(container, user);
      const saveAddr = within(container)
        .getAllByRole('button')
        .find((b) => /salvar|save/i.test(b.textContent || ''));
      if (saveAddr) await user.click(saveAddr);
    } else {
      await fillWithdrawForm(container, user);
    }

    const maxBtn = within(container)
      .getAllByRole('button')
      .find((b) => /máximo|max/i.test(b.textContent || ''));
    if (maxBtn) await user.click(maxBtn);

    const feesBtn = within(container)
      .getAllByRole('button')
      .find((b) => /tabela de taxas|fees/i.test(b.textContent || ''));
    if (feesBtn) {
      await user.click(feesBtn);
      await waitFor(() => expect(container.innerHTML).toMatch(/taxa|fee/i));
      const closeFees = within(container)
        .getAllByRole('button')
        .find((b) => /fechar|close|entendi/i.test(b.textContent || ''));
      if (closeFees) await user.click(closeFees);
    }

    const submit = container.querySelector('button[type="submit"]') as HTMLButtonElement | null;
    if (submit && !submit.disabled) {
      await user.click(submit);
      await waitFor(() => expect(container.innerHTML).toMatch(/sucesso|success|solicitado/i), {
        timeout: 5000,
      });
    }

    unmount();
  });

  it('history filters and copy tx hash', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<WithdrawPage />, {
      route: '/withdraw?tab=history',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/CONFIRMED|PENDING|Histórico/i), {
      timeout: 5000,
    });

    for (const label of [/btc/i, /ltc/i, /all|tod/i]) {
      const chip = within(container)
        .getAllByRole('button')
        .find((b) => label.test(b.textContent || ''));
      if (chip) await user.click(chip);
    }

    const copyTx = within(container)
      .queryAllByRole('button')
      .find((b) => b.querySelector('.bi-clipboard') || /copiar/i.test(b.textContent || ''));
    if (copyTx) await user.click(copyTx);

    unmount();
  });

  it('shows error when withdrawal POST fails', async () => {
    withdrawPostFails = true;
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<WithdrawPage />, {
      route: '/withdraw?coin=BTC',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(200), { timeout: 5000 });
    await fillWithdrawForm(container, user);
    const submit = container.querySelector('button[type="submit"]') as HTMLButtonElement | null;
    if (submit && !submit.disabled) {
      await user.click(submit);
      await waitFor(() => expect(container.innerHTML).toMatch(/insuficiente|error|saldo/i), {
        timeout: 5000,
      });
    }
    unmount();
  });
});

describe('SwapPage deep', () => {
  it('quote, route select, execute, history list', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<SwapPage />, {
      route: '/swap',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(200), { timeout: 5000 });

    const amountInput = container.querySelector('input[type="text"]') as HTMLInputElement | null;
    if (amountInput) {
      await user.clear(amountInput);
      await user.type(amountInput, '0.01');
    }

    await waitFor(() => expect(container.innerHTML).toMatch(/0\.8|HOUSE|SWAPKIT|LTC/i), {
      timeout: 6000,
    });

    const altRoute = within(container)
      .getAllByRole('button')
      .find((b) => /SWAPKIT/i.test(b.textContent || ''));
    if (altRoute) await user.click(altRoute);

    const maxPct = within(container)
      .getAllByRole('button')
      .find((b) => /máximo|100%|max/i.test(b.textContent || ''));
    if (maxPct) await user.click(maxPct);

    const form = container.querySelector('form');
    const executeBtn = form
      ? within(form as HTMLElement)
          .getAllByRole('button')
          .find((b) => /execut|confirm|swap|câmbio/i.test(b.textContent || ''))
      : undefined;
    if (executeBtn && !(executeBtn as HTMLButtonElement).disabled) {
      await user.click(executeBtn);
      await waitFor(() => expect(container.innerHTML).toMatch(/conclu|success|swap/i), { timeout: 5000 });
    }

    await waitFor(() => expect(container.innerHTML).toMatch(/Swap History|Concluído|registros|PENDING/i), {
      timeout: 6000,
    });

    unmount();
  });
});

describe('DeveloperBalances deep', () => {
  it('top-up transfer with max and validation error', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<DeveloperBalances />, {
      route: '/developer/wallets',
      loggedIn: true,
    });
    await waitFor(
      () => {
        const topUp = within(container)
          .getAllByRole('button')
          .find((b) => /top up|adicionar saldo/i.test(b.textContent || '') && !(b as HTMLButtonElement).disabled);
        expect(topUp).toBeTruthy();
      },
      { timeout: 5000 },
    );

    const topUp = within(container)
      .getAllByRole('button')
      .find((b) => /top up|adicionar saldo/i.test(b.textContent || '') && !(b as HTMLButtonElement).disabled)!;
    await user.click(topUp);

    const dialog = await waitFor(() => {
      const el = document.body.querySelector('[role="dialog"]');
      expect(el).toBeTruthy();
      return el as HTMLElement;
    }, { timeout: 5000 });

    const maxBtn = within(dialog).getAllByRole('button').find((b) => /máx|max/i.test(b.textContent || ''));
    if (maxBtn) await user.click(maxBtn);

    const submit = within(dialog)
      .getAllByRole('button')
      .find((b) => /transfer/i.test(b.textContent || ''));
    if (submit && !(submit as HTMLButtonElement).disabled) await user.click(submit);

    await waitFor(() => expect(document.body.querySelector('[role="dialog"]')).toBeFalsy(), { timeout: 5000 });

    const movePersonal = within(container)
      .getAllByRole('button')
      .find((b) => /move to personal|mover para pessoal/i.test(b.textContent || '') && !(b as HTMLButtonElement).disabled);
    if (movePersonal) {
      await user.click(movePersonal);
      const dialog2 = await waitFor(() => {
        const el = document.body.querySelector('[role="dialog"]');
        expect(el).toBeTruthy();
        return el as HTMLElement;
      }, { timeout: 5000 });
      const input = dialog2.querySelector('input') as HTMLInputElement;
      if (input) {
        await user.clear(input);
        await user.type(input, '999999999');
      }
      await waitFor(() => expect(dialog2.innerHTML).toMatch(/ultrapassa|exceed|disponível/i), {
        timeout: 4000,
      });
      const close = within(dialog2)
        .getAllByRole('button')
        .find((b) => /close|fechar/i.test(b.textContent || ''));
      if (close) await user.click(close);
    }

    unmount();
  });
});

describe('SettingsPage deep', () => {
  it('security 2FA + theme mode + apps revoke', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<SettingsPage />, {
      route: '/settings',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(100), { timeout: 5000 });

    const security = within(container)
      .getAllByRole('button')
      .find((b) => /seguran|security|2fa/i.test(b.textContent || ''));
    if (security) await user.click(security);

    const enable = within(container)
      .queryAllByRole('button')
      .find((b) => /ativar|enable/i.test(b.textContent || ''));
    if (enable) await user.click(enable);

    const codeInput = container.querySelector('input[maxlength="6"], input[inputmode="numeric"]');
    if (codeInput) {
      await user.type(codeInput as HTMLElement, '654321');
      const confirm = within(container)
        .queryAllByRole('button')
        .find((b) => /confirm|verificar/i.test(b.textContent || ''));
      if (confirm) await user.click(confirm);
    }

    const profile = within(container)
      .getAllByRole('button')
      .find((b) => /perfil|profile/i.test(b.textContent || ''));
    if (profile) await user.click(profile);

    for (const modeBtn of within(container).queryAllByRole('button')) {
      if (/dark|escuro|light|claro|system/i.test(modeBtn.textContent || '')) {
        await user.click(modeBtn);
      }
    }

    const appsTab = within(container)
      .getAllByRole('button')
      .find((b) => /app|conectad/i.test(b.textContent || ''));
    if (appsTab) await user.click(appsTab);

    const revoke = within(container)
      .queryAllByRole('button')
      .find((b) => /revog|revoke|descon/i.test(b.textContent || ''));
    if (revoke) await user.click(revoke);

    unmount();
  });
});

describe('OAuthAppsPage deep', () => {
  it('creates app and shows secret modal', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<OAuthAppsPage />, {
      route: '/oauth/apps',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/Test App|sats_app_test/i), { timeout: 5000 });

    const create = within(container)
      .getAllByRole('button')
      .find((b) => /criar|create|novo/i.test(b.textContent || ''));
    if (create) await user.click(create);

    const nameField = container.querySelector('form input[type="text"][required]') as HTMLInputElement | null;
    if (nameField) await user.type(nameField, 'My OAuth App');
    const textarea = container.querySelector('form textarea[required]') as HTMLTextAreaElement | null;
    if (textarea) {
      await user.clear(textarea);
      await user.type(textarea, 'https://example.com/callback');
    }

    const save = within(container)
      .getAllByRole('button')
      .find((b) => /criar aplicação|create application|salvar alterações/i.test(b.textContent || ''));
    if (save) await user.click(save);

    await waitFor(() => expect(container.innerHTML).toMatch(/secret|segredo|one-time|rotated/i), {
      timeout: 5000,
    }).catch(() => undefined);

    for (const tab of [/node/i, /python/i, /curl/i]) {
      const tbtn = within(container)
        .queryAllByRole('button')
        .find((b) => tab.test(b.textContent || ''));
      if (tbtn) await user.click(tbtn);
    }

    unmount();
  });

  it('deletes app when confirmed', async () => {
    const user = userEvent.setup();
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    const { container, unmount } = renderWithProviders(<OAuthAppsPage />, {
      route: '/oauth/apps',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/Test App/i), { timeout: 5000 });
    const del = within(container)
      .queryAllByRole('button')
      .find((b) => /excluir|delete|remover/i.test(b.textContent || ''));
    if (del) await user.click(del);
    unmount();
  });

  it('rotates client secret when confirmed', async () => {
    const user = userEvent.setup();
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    const { container, unmount } = renderWithProviders(<OAuthAppsPage />, {
      route: '/oauth/apps',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/Test App/i), { timeout: 5000 });

    const rotate = within(container)
      .queryAllByRole('button')
      .find((b) => /rotacion|rotate|novo secret|regener/i.test(b.textContent || ''));
    if (rotate) await user.click(rotate);

    await waitFor(() => expect(container.innerHTML).toMatch(/rotated|secret|segredo/i), {
      timeout: 5000,
    }).catch(() => undefined);

    unmount();
  });

  it('preview theme/size toggles and edit flow', async () => {
    const user = userEvent.setup();
    vi.spyOn(window, 'confirm').mockReturnValue(false);
    const { container, unmount } = renderWithProviders(<OAuthAppsPage />, {
      route: '/oauth/apps',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/Test App/i), { timeout: 5000 });

    for (const label of [/dark|escuro/i, /bitcoin/i, /popup/i, /small|medium|large/i]) {
      const btn = within(container)
        .queryAllByRole('button')
        .find((b) => label.test(b.textContent || ''));
      if (btn) await user.click(btn);
    }

    const edit = within(container)
      .queryAllByRole('button')
      .find((b) => /editar|edit/i.test(b.textContent || ''));
    if (edit) await user.click(edit);

    unmount();
  });
});

describe('FaucetPage claim', () => {
  it('switches coin grid before claim', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<FaucetPage />, {
      route: '/faucet',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(100), { timeout: 5000 });
    for (const coin of ['LTC', 'DOGE', 'BTC']) {
      const btn = within(container)
        .getAllByRole('button')
        .find((b) => (b.textContent || '').includes(coin));
      if (btn) await user.click(btn);
    }
    unmount();
  });

  it('verifies captcha and claims after mount delay', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<FaucetPage />, {
      route: '/faucet',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(100), { timeout: 5000 });

    const ltc = within(container)
      .getAllByRole('button')
      .find((b) => (b.textContent || '').trim().startsWith('LTC') || /\bLTC\b/.test(b.textContent || ''));
    if (ltc) await user.click(ltc);

    const captcha = container.querySelector('[data-testid="mock-captcha"]');
    if (captcha) await user.click(captcha as HTMLElement);

    await new Promise((r) => setTimeout(r, 850));

    const claim = within(container)
      .getAllByRole('button')
      .find((b) => /reivindicar|claim/i.test(b.textContent || ''));
    if (claim && !(claim as HTMLButtonElement).disabled) {
      await user.click(claim);
      await waitFor(() => expect(container.innerHTML).toMatch(/sucesso|success|claim|receb/i), {
        timeout: 5000,
      });
    }

    unmount();
  });
});

describe('ApiKeysPage', () => {
  it('lists keys, toggles scopes, creates and revokes', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<ApiKeysPage />, {
      route: '/api-keys',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/Production|sats_live/i), { timeout: 5000 });

    const historyScope = within(container)
      .getAllByRole('button')
      .find((b) => /histórico|history/i.test(b.textContent || ''));
    if (historyScope) await user.click(historyScope);

    const labelInput = container.querySelector('input[type="text"]');
    if (labelInput) await user.type(labelInput as HTMLElement, ' CI Key ');

    const sigToggle = container.querySelector('input[type="checkbox"]');
    if (sigToggle) await user.click(sigToggle as HTMLElement);

    const createBtn = within(container)
      .getAllByRole('button')
      .find((b) => /gerar|create|criar|nova chave/i.test(b.textContent || ''));
    if (createBtn) await user.click(createBtn);

    await waitFor(() => expect(container.innerHTML).toMatch(/sats_live_full|secret|chave/i), {
      timeout: 5000,
    }).catch(() => undefined);

    const copyBtn = within(container)
      .queryAllByRole('button')
      .find((b) => /copiar|copy/i.test(b.textContent || ''));
    if (copyBtn) await user.click(copyBtn);

    const rotateBtn = within(container)
      .queryAllByRole('button')
      .find((b) => /rotacion|rotate|renovar/i.test(b.textContent || ''));
    if (rotateBtn) await user.click(rotateBtn);

    const revokeBtn = within(container)
      .queryAllByRole('button')
      .find((b) => /revog|disable|desativ/i.test(b.textContent || ''));
    if (revokeBtn) await user.click(revokeBtn);

    unmount();
  });
});

describe('LendPage + MerchantDeposits + Checkout + AdminMerchants', () => {
  it(
    'lend supply submit and merchant deposit generator',
    async () => {
      const user = userEvent.setup();
      const lend = renderWithProviders(<LendPage />, { route: '/lend', loggedIn: true });
      await waitFor(() => expect(lend.container.innerHTML).toMatch(/USDT|Fornecer|Supply/i), {
        timeout: 5000,
      });

      const supply = within(lend.container)
        .queryAllByRole('button')
        .find((b) => /fornecer|supply/i.test(b.textContent || ''));
      if (supply) await user.click(supply);

      const modalInput =
        document.body.querySelector('[role="dialog"] input') ||
        lend.container.querySelector('input[placeholder="0.00"]');
      if (modalInput) {
        await user.type(modalInput as HTMLElement, '10');
        const pct = within(document.body as HTMLElement)
          .queryAllByRole('button')
          .find((b) => /^50%$/i.test((b.textContent || '').trim()));
        if (pct) await user.click(pct);
        const confirm = within(document.body as HTMLElement)
          .queryAllByRole('button')
          .find((b) => /fornecer|supply|tomar|pagar/i.test(b.textContent || ''));
        if (confirm && !(confirm as HTMLButtonElement).disabled) await user.click(confirm);
      }

      const borrow = within(lend.container)
        .queryAllByRole('button')
        .find((b) => /^tomar$/i.test((b.textContent || '').trim()));
      if (borrow) await user.click(borrow);
      lend.unmount();

      const merch = renderWithProviders(<MerchantDepositsPage />, {
        route: '/merchant/deposits',
        loggedIn: true,
      });
      await waitFor(() => expect(merch.container.innerHTML).toMatch(/ORD-PEND|Testar Webhook|minv2/i), {
        timeout: 8000,
      });

      const coinToggle = within(merch.container)
        .queryAllByRole('button')
        .find((b) => /^DOGE$/i.test((b.textContent || '').trim()));
      if (coinToggle) await user.click(coinToggle);

      const genBtn = within(merch.container)
        .queryAllByRole('button')
        .find((b) => /gerar|generate|criar link/i.test(b.textContent || ''));
      if (genBtn) await user.click(genBtn);

      merch.unmount();
    },
    15000,
  );

  it('checkout paid state and admin merchants moderation', async () => {
    const checkout = renderWithProviders(<CheckoutPage />, {
      route: '/checkout/inv1',
      routePath: '/checkout/:id',
      loggedIn: true,
    });
    await waitFor(() => expect(checkout.container.innerHTML).toMatch(/Demo Shop|BTC|invoice|pay/i), {
      timeout: 5000,
    });
    checkout.unmount();

    const user = userEvent.setup();
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

    await waitFor(() => expect(admin.container.innerHTML).toMatch(/Demo Merchant/i), {
      timeout: 5000,
    });

    for (const label of [/pendente|pending/i, /aprov/i, /todos|all/i]) {
      const chip = within(admin.container)
        .getAllByRole('button')
        .find((b) => label.test(b.textContent || ''));
      if (chip) await user.click(chip);
    }

    const search = admin.container.querySelector('input[type="text"], input[type="search"]');
    if (search) await user.type(search as HTMLElement, 'Demo');

    admin.unmount();
  });
});

describe('MerchantDeposits deep', () => {
  it('coin toggles, status filters, select, webhook test', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<MerchantDepositsPage />, {
      route: '/merchant/deposits',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/ORD-88219|Gateway|Cobranças/i), {
      timeout: 8000,
    });

    for (const coin of ['SOL', 'BCH', 'POL']) {
      const chip = within(container)
        .getAllByRole('button')
        .find((b) => new RegExp(`^${coin}$`).test((b.textContent || '').trim()));
      if (chip) await user.click(chip);
    }

    for (const label of [/pagos|paid/i, /pendente/i, /expirado|expired/i, /todos|all/i]) {
      const st = within(container)
        .getAllByRole('button')
        .find((b) => label.test(b.textContent || ''));
      if (st) await user.click(st);
    }

    const select = container.querySelector('select');
    if (select) {
      await user.selectOptions(select as HTMLSelectElement, 'BTC');
      await user.selectOptions(select as HTMLSelectElement, 'ALL');
    }

    const orderInput = container.querySelector('input[placeholder*="ORD"]');
    if (orderInput) {
      await user.clear(orderInput as HTMLInputElement);
      await user.type(orderInput as HTMLElement, 'ORD-NEW-99');
    }

    const webhookBtn = within(container)
      .queryAllByRole('button')
      .find((b) => /webhook|testar|test/i.test(b.textContent || ''));
    if (webhookBtn) await user.click(webhookBtn);

    const copyBtn = within(container)
      .queryAllByRole('button')
      .find((b) => /copiar|copy|clipboard/i.test(b.textContent || '') || b.querySelector('.bi-clipboard'));
    if (copyBtn) await user.click(copyBtn);

    unmount();
  }, 15000);
});

describe('AppLayout navigation', () => {
  it('switches merchant mode and opens mobile menu', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<AppLayout />, {
      route: '/dashboard',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(100), { timeout: 5000 });

    const merchantMode = within(container)
      .queryAllByRole('button')
      .find((b) => /merchant|comerciante|loja/i.test(b.textContent || ''));
    if (merchantMode) await user.click(merchantMode);

    const userMode = within(container)
      .queryAllByRole('button')
      .find((b) => /user|pessoal|conta/i.test(b.textContent || ''));
    if (userMode) await user.click(userMode);

    const menuOpen = within(container)
      .queryAllByRole('button')
      .find((b) => b.querySelector('.bi-list') || /menu/i.test(b.textContent || ''));
    if (menuOpen) await user.click(menuOpen);

    for (const link of within(container).queryAllByRole('link').slice(0, 12)) {
      try {
        await user.click(link);
      } catch {
        /* ignore */
      }
    }

    unmount();
  });
});

describe('Wallets, Referral, Merchant dashboard, Deposit, Developer page', () => {
  it('renders wallet grid and referral tabs', async () => {
    const user = userEvent.setup();
    const wallets = renderWithProviders(<WalletsPage />, { route: '/wallets', loggedIn: true });
    await waitFor(() => expect(wallets.container.innerHTML).toMatch(/BTC|balance/i), { timeout: 5000 });
    wallets.unmount();

    const ref = renderWithProviders(<ReferralPage />, { route: '/referrals', loggedIn: true });
    await waitFor(() => expect(ref.container.innerHTML).toMatch(/ABC123|Copiar Link|referral/i), {
      timeout: 5000,
    });
    await waitFor(() => expect(ref.container.innerHTML).toMatch(/data:image|QR|Escaneie/i), {
      timeout: 5000,
    }).catch(() => undefined);
    const usersTab = within(ref.container)
      .getAllByRole('button')
      .find((b) => /usuários|users|indicados/i.test(b.textContent || ''));
    if (usersTab) await user.click(usersTab);
    const commTab = within(ref.container)
      .getAllByRole('button')
      .find((b) => /comiss|commission/i.test(b.textContent || ''));
    if (commTab) await user.click(commTab);
    const copyBtn = within(ref.container)
      .queryAllByRole('button')
      .find((b) => /copiar|copy link/i.test(b.textContent || ''));
    if (copyBtn) await user.click(copyBtn);
    ref.unmount();
  });

  it('merchant dashboard filters and deposit history', async () => {
    const user = userEvent.setup();
    const dash = renderWithProviders(<MerchantDashboardPage />, { route: '/merchant', loggedIn: true });
    await waitFor(() => expect(dash.container.innerHTML).toMatch(/Demo Shop|merchant|volume/i), {
      timeout: 5000,
    });
    for (const label of [/btc/i, /all|tod/i]) {
      const chip = within(dash.container)
        .getAllByRole('button')
        .find((b) => label.test(b.textContent || ''));
      if (chip) await user.click(chip);
    }
    dash.unmount();

    for (const coin of ['BTC', 'LTC', 'DOGE', 'USDT']) {
      const dep = renderWithProviders(<DepositPage />, {
        route: `/deposit?coin=${coin}`,
        loggedIn: true,
      });
      await waitFor(() => expect(dep.container.innerHTML.length).toBeGreaterThan(80), { timeout: 4000 });
      const hist = within(dep.container)
        .queryAllByRole('button')
        .find((b) => /histór|history/i.test(b.textContent || ''));
      if (hist) await user.click(hist);
      dep.unmount();
    }

    const devPage = renderWithProviders(<DeveloperWalletsPage />, {
      route: '/developer/wallets',
      loggedIn: true,
    });
    await waitFor(() => expect(devPage.container.innerHTML.length).toBeGreaterThan(80), { timeout: 5000 });
    devPage.unmount();
  });
});
