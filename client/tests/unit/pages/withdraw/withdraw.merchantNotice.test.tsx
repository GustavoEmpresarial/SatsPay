/**
 * @vitest-environment jsdom
 * The personal withdrawal page is personal-only: merchant caixa must not be
 * read, shown, or reachable from here.
 */
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { waitFor } from '@testing-library/react';
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

// The shared mock funds the merchant wallet too, so if the page asked for it the
// balance would surface. USDT is funded and is not a paused network.
describe('Withdraw — merchant infra stays out', () => {
  beforeAll(async () => {
    await i18n.changeLanguage('pt');
  });

  it('never queries the merchant wallet', async () => {
    apiSpy.mockClear();
    const w = renderWithProviders(<WithdrawPage />, { route: '/withdraw?coin=USDT', loggedIn: true });

    await waitFor(() => expect(w.container.textContent).toMatch(/Saldo pessoal/i), { timeout: 5000 });

    const paths = apiSpy.mock.calls.map(([p]) => String(p));
    expect(paths.some((p) => p.includes('MERCHANT'))).toBe(false);
    expect(paths.some((p) => p.includes('/wallet/transfer'))).toBe(false);
    expect(paths.some((p) => p.includes('kind=PERSONAL'))).toBe(true);

    w.unmount();
  });

  it('shows no merchant balance, notice, or transfer control', async () => {
    const w = renderWithProviders(<WithdrawPage />, { route: '/withdraw?coin=USDT', loggedIn: true });

    await waitFor(() => expect(w.container.textContent).toMatch(/Saldo pessoal/i), { timeout: 5000 });

    const text = w.container.textContent ?? '';
    expect(text).not.toMatch(/caixa/i);
    expect(text).not.toMatch(/comerciante/i);
    expect(text).not.toMatch(/Transferir para pessoal/i);
    // The merchant USDT balance from the mock must not appear anywhere.
    expect(text).not.toContain('100.00000000');

    w.unmount();
  });
});
