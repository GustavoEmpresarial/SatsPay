/**
 * @vitest-environment jsdom
 * The merchant caixa must never become the on-chain withdrawal source on its own.
 */
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '../../../helpers/renderWithProviders.js';
import i18n from '../../../../src/i18n/index.js';

const apiSpy = vi.fn();

vi.mock('../../../../src/lib/api.js', async () => {
  const { mockApi } = await import('../../../helpers/apiMock.js');
  return {
    api: vi.fn(async (path: string, opts?: { method?: string; json?: unknown }) => {
      apiSpy(path, opts);
      return mockApi(path, opts);
    }),
    bootstrapSession: vi.fn(async () => null),
    forceReauth: vi.fn(),
  };
});

vi.mock('../../../../src/components/Turnstile.js', () => ({ Turnstile: () => null }));
vi.mock('../../../../src/components/ModernCaptcha.js', () => ({ ModernCaptcha: () => null }));

import { WithdrawPage } from '../../../../src/pages/WithdrawPage.js';

// USDT is funded by the shared mock for both kinds (personal 20000000000 /
// merchant 10000000000) and is not a deposit/withdraw-paused network, so the
// caixa notice renders here. BTC and LTC are paused and would fall back to POL.
describe('Withdraw — merchant caixa', () => {
  beforeAll(async () => {
    await i18n.changeLanguage('pt');
  });

  it('shows the caixa as a notice and keeps the withdrawal on the personal balance', async () => {
    const w = renderWithProviders(<WithdrawPage />, { route: '/withdraw?coin=USDT', loggedIn: true });

    await waitFor(() => expect(w.container.textContent).toMatch(/caixa de comerciante/i), {
      timeout: 5000,
    });

    // Personal balance drives the form, not the caixa.
    expect(w.container.textContent).toMatch(/Saldo pessoal/i);
    expect(w.container.textContent).not.toMatch(/Saldo \(caixa\)/i);

    w.unmount();
  });

  it('moves the caixa to personal through /wallet/transfer', async () => {
    apiSpy.mockClear();
    const w = renderWithProviders(<WithdrawPage />, { route: '/withdraw?coin=USDT', loggedIn: true });

    await waitFor(() => expect(w.container.textContent).toMatch(/caixa de comerciante/i), {
      timeout: 5000,
    });

    const transferBtn = Array.from(w.container.querySelectorAll('button')).find(
      (b) => b.textContent?.trim() === 'Transferir para pessoal'
    );
    expect(transferBtn).toBeTruthy();
    // Nothing typed yet, so there is nothing to move.
    expect(transferBtn).toBeDisabled();

    const allBtn = Array.from(w.container.querySelectorAll('button')).find(
      (b) => b.textContent?.trim() === 'Tudo'
    );
    await userEvent.click(allBtn!);
    await waitFor(() => expect(transferBtn).toBeEnabled());
    await userEvent.click(transferBtn!);

    await waitFor(() => {
      const call = apiSpy.mock.calls.find(([p]) => String(p).includes('/wallet/transfer'));
      expect(call).toBeTruthy();
      expect(call![1]).toMatchObject({
        method: 'POST',
        json: { coin: 'USDT', amount: '10000000000', toDeveloper: false },
      });
    });

    w.unmount();
  });
});
