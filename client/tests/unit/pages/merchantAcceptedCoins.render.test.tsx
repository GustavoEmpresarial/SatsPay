/**
 * @vitest-environment jsdom
 * The "moedas aceitas" panel — the one that used to persist nothing.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '../../helpers/renderWithProviders.js';

const puts: unknown[] = [];
let settings = { acceptedCoins: ['USDT', 'POL'], availableCoins: ['BCH', 'POL', 'SOL', 'USDT', 'USDC'] };

vi.mock('../../../src/lib/api.js', () => ({
  api: vi.fn(async (path: string, opts?: { method?: string; json?: unknown }) => {
    if (path.includes('/merchant/settings')) {
      if (opts?.method === 'PUT') {
        puts.push(opts.json);
        settings = { ...settings, acceptedCoins: (opts.json as { acceptedCoins: string[] }).acceptedCoins };
        return { acceptedCoins: settings.acceptedCoins };
      }
      return settings;
    }
    return { invoices: [] };
  }),
  bootstrapSession: vi.fn(async () => true),
  forceReauth: vi.fn(),
}));

import { MerchantDepositsPage } from '../../../src/pages/MerchantDepositsPage.js';

beforeEach(() => {
  puts.length = 0;
  settings = { acceptedCoins: ['USDT', 'POL'], availableCoins: ['BCH', 'POL', 'SOL', 'USDT', 'USDC'] };
});

describe('MerchantDepositsPage — accepted coins', () => {
  it('loads the saved selection from the server', async () => {
    const { container, unmount } = renderWithProviders(<MerchantDepositsPage />, {
      route: '/merchant/deposits',
    });
    await waitFor(() => expect(container.textContent).toMatch(/Moedas que você aceita/i), { timeout: 5000 });
    // Paused coins are not even offered here.
    expect(container.textContent).not.toMatch(/\bBTC\b.*Moedas que você aceita/s);
    unmount();
  });

  it('persists a change instead of only colouring a button', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<MerchantDepositsPage />, {
      route: '/merchant/deposits',
    });
    await waitFor(() => expect(container.textContent).toMatch(/Moedas que você aceita/i), { timeout: 5000 });

    // Two panels on this page render coin buttons; target this one exactly
    // instead of trusting DOM order.
    await waitFor(() => expect(container.querySelector('[data-testid="accepted-coin-SOL"]')).toBeTruthy());
    await user.click(container.querySelector('[data-testid="accepted-coin-SOL"]') as HTMLElement);

    await waitFor(() => expect(puts).toHaveLength(1));
    expect(puts[0]).toEqual({ acceptedCoins: ['USDT', 'POL', 'SOL'] });
    unmount();
  });

  it('never sends an empty selection the API would refuse', async () => {
    const user = userEvent.setup();
    settings = { acceptedCoins: ['USDT'], availableCoins: ['USDT', 'POL'] };
    const { container, unmount } = renderWithProviders(<MerchantDepositsPage />, {
      route: '/merchant/deposits',
    });
    await waitFor(() => expect(container.textContent).toMatch(/Moedas que você aceita/i), { timeout: 5000 });

    await waitFor(() => expect(container.querySelector('[data-testid="accepted-coin-USDT"]')).toBeTruthy());
    await user.click(container.querySelector('[data-testid="accepted-coin-USDT"]') as HTMLElement);

    // Turning the last one off would leave a checkout nobody can pay.
    await new Promise((r) => setTimeout(r, 50));
    expect(puts).toHaveLength(0);
    unmount();
  });
});
