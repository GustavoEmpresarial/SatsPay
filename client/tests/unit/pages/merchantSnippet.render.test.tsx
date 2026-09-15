/**
 * @vitest-environment jsdom
 * The merchant dashboard panel that used to be decorative.
 */
import { describe, expect, it, vi } from 'vitest';
import { waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '../../helpers/renderWithProviders.js';

vi.mock('../../../src/lib/api.js', () => ({
  api: vi.fn(async () => ({ invoices: [] })),
  bootstrapSession: vi.fn(async () => true),
  forceReauth: vi.fn(),
}));

import { MerchantDepositsPage, toLedgerUnits } from '../../../src/pages/MerchantDepositsPage.js';

describe('toLedgerUnits', () => {
  it('converts coin quantities to the integer the API takes', () => {
    // The panel used to default to "25.00" and send it nowhere; the API now
    // rejects that spelling outright.
    expect(toLedgerUnits('25')).toBe('2500000000');
    expect(toLedgerUnits('25.00')).toBe('2500000000');
    expect(toLedgerUnits('0.005')).toBe('500000');
    expect(toLedgerUnits('1')).toBe('100000000');
    expect(toLedgerUnits('0.00000001')).toBe('1');
    expect(toLedgerUnits('25,5')).toBe('2550000000');
  });

  it('refuses what the ledger cannot hold', () => {
    expect(toLedgerUnits('0.000000001')).toBeNull(); // 9 decimals
    expect(toLedgerUnits('abc')).toBeNull();
    expect(toLedgerUnits('')).toBeNull();
    expect(toLedgerUnits('-1')).toBeNull();
    expect(toLedgerUnits('0')).toBeNull();
  });
});

describe('MerchantDepositsPage — integration helper', () => {
  it('links to the demo checkout from where the merchant works', async () => {
    const { container, unmount } = renderWithProviders(<MerchantDepositsPage />, {
      route: '/merchant/deposits',
    });
    await waitFor(() => expect(container.textContent).toMatch(/Gerar sua integração/i), { timeout: 5000 });

    const demoLinks = within(container)
      .queryAllByRole('link')
      .filter((a) => a.getAttribute('href') === '/pay/demo');
    expect(demoLinks.length, 'the demo must be reachable from the dashboard').toBeGreaterThan(0);
    unmount();
  });

  it('builds a snippet with the converted amount', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<MerchantDepositsPage />, {
      route: '/merchant/deposits',
    });
    await waitFor(() => expect(container.textContent).toMatch(/Gerar sua integração/i), { timeout: 5000 });

    const pre = container.querySelector('pre')!;
    expect(pre.textContent).toContain('POST');
    expect(pre.textContent).toContain('/v1/merchant/deposits');
    expect(pre.textContent).toContain('"amount": "2500000000"');
    expect(pre.textContent).not.toContain('"amount": "25.00"');

    const amountInput = container.querySelector('input') as HTMLInputElement;
    await user.clear(amountInput);
    await user.type(amountInput, '0.5');
    await waitFor(() => expect(container.querySelector('pre')!.textContent).toContain('"amount": "50000000"'));
    unmount();
  });

  it('switches to the payment button snippet', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<MerchantDepositsPage />, {
      route: '/merchant/deposits',
    });
    await waitFor(() => expect(container.textContent).toMatch(/Gerar sua integração/i), { timeout: 5000 });

    const btn = within(container)
      .getAllByRole('button')
      .find((b) => /2\. Botão/i.test(b.textContent || ''))!;
    await user.click(btn);

    await waitFor(() => {
      const pre = container.querySelector('pre')!;
      expect(pre.textContent).toContain('satspay-pay.js');
      expect(pre.textContent).toContain('data-checkout_url');
      // The button must never be shown carrying a key.
      expect(pre.textContent).not.toMatch(/x-api-key/i);
    });
    unmount();
  });

  it('marks paused coins instead of offering them', async () => {
    const { container, unmount } = renderWithProviders(<MerchantDepositsPage />, {
      route: '/merchant/deposits',
    });
    await waitFor(() => expect(container.textContent).toMatch(/Gerar sua integração/i), { timeout: 5000 });

    const btc = within(container)
      .getAllByRole('button')
      .find((b) => (b.textContent || '').trim() === 'BTC') as HTMLButtonElement;
    expect(btc.disabled, 'BTC deposits are paused').toBe(true);

    const usdt = within(container)
      .getAllByRole('button')
      .find((b) => (b.textContent || '').trim().startsWith('USDT')) as HTMLButtonElement;
    expect(usdt.disabled).toBe(false);
    unmount();
  });
});
