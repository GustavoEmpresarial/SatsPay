/**
 * @vitest-environment jsdom
 * The coin picker a paying customer sees on a USD-priced invoice.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '../../helpers/renderWithProviders.js';

const selectCalls: unknown[] = [];

const multiCoin = {
  id: 'inv-multi',
  status: 'PENDING',
  coin: 'USDT',
  amount: '2500000000',
  amountDisplay: '25',
  amountUsd: '25.00',
  depositAddress: '0xaaa',
  orderId: 'ORD-1',
  qrCode: 'usdt:0xaaa?amount=25',
  expiresAt: new Date(Date.now() + 30 * 60_000).toISOString(),
  coinLocked: false,
  coinOptions: [
    { coin: 'USDT', name: 'Tether USD', amount: '2500000000', amountDisplay: '25', logoUrl: '/sdk/coins/usdt.svg', minConfirmations: 30 },
    { coin: 'POL', name: 'Polygon', amount: '5555555556', amountDisplay: '55.55555556', logoUrl: '/sdk/coins/pol.svg', minConfirmations: 30 },
  ],
};

let invoice: Record<string, unknown> = { ...multiCoin };

vi.mock('../../../src/lib/api.js', () => ({
  api: vi.fn(async (path: string, opts?: { method?: string; json?: unknown }) => {
    if (path.includes('/select-coin')) {
      selectCalls.push(opts?.json);
      return invoice;
    }
    if (path.includes('/public/coins')) return { priceDecimals: 8, coins: [] };
    return invoice;
  }),
  bootstrapSession: vi.fn(async () => true),
  forceReauth: vi.fn(),
}));

vi.mock('../../../src/lib/qr.js', () => ({ addressQrDataUrl: vi.fn(async () => 'data:image/png;base64,xx') }));

import { CheckoutPage } from '../../../src/pages/CheckoutPage.js';

beforeEach(() => {
  selectCalls.length = 0;
  invoice = { ...multiCoin };
});

describe('CheckoutPage — coin picker', () => {
  it('lists every offered coin with its own amount', async () => {
    const { container, unmount } = renderWithProviders(<CheckoutPage />, {
      route: '/pay/inv-multi',
      routePath: '/pay/:id',
    });
    await waitFor(() => expect(container.textContent).toMatch(/Escolha a moeda|Choose a coin/i), { timeout: 5000 });

    // Each coin shows what it would actually cost, not one shared number.
    expect(container.textContent).toContain('55.55555556 POL');
    expect(container.textContent).toContain('25 USDT');
    expect(container.textContent).toContain('US$ 25.00');
    unmount();
  });

  it('sends the chosen coin to the server rather than deciding locally', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<CheckoutPage />, {
      route: '/pay/inv-multi',
      routePath: '/pay/:id',
    });
    await waitFor(() => expect(container.textContent).toMatch(/Escolha a moeda|Choose a coin/i), { timeout: 5000 });

    const pol = within(container)
      .getAllByRole('button')
      .find((b) => /POL/.test(b.textContent || '') && !(b as HTMLButtonElement).disabled)!;
    await user.click(pol);

    await waitFor(() => expect(selectCalls).toHaveLength(1));
    expect(selectCalls[0]).toEqual({ coin: 'POL' });
    unmount();
  });

  it('tells the customer the quote is locked on choosing', async () => {
    const { container, unmount } = renderWithProviders(<CheckoutPage />, {
      route: '/pay/inv-multi',
      routePath: '/pay/:id',
    });
    await waitFor(() => expect(container.textContent).toMatch(/Escolha a moeda|Choose a coin/i), { timeout: 5000 });
    expect(container.textContent).toMatch(/cotação é travada|quote locks/i);
    unmount();
  });

  it('hides the picker once the coin is locked', async () => {
    // A payment is in flight: switching would strand it.
    invoice = { ...multiCoin, coinLocked: true };
    const { container, unmount } = renderWithProviders(<CheckoutPage />, {
      route: '/pay/inv-multi',
      routePath: '/pay/:id',
    });
    await waitFor(() => expect(container.textContent).toContain('USDT'), { timeout: 5000 });
    expect(container.textContent).not.toMatch(/Escolha a moeda|Choose a coin/i);
    expect(container.textContent).not.toMatch(/Pagar com outra moeda|Pay with another coin/i);
    unmount();
  });

  it('shows the picker on the demo invoice too', async () => {
    // The demo was hardcoded to one coin, so a merchant who accepted several
    // opened it, saw one, and concluded the picker did not work.
    invoice = { ...multiCoin, demo: true, amountUsd: '25.00' };
    const { container, unmount } = renderWithProviders(<CheckoutPage />, {
      route: '/pay/demo',
      routePath: '/pay/:id',
    });
    await waitFor(() => expect(container.textContent).toMatch(/Escolha a moeda|Choose a coin/i), { timeout: 5000 });
    expect(container.textContent).toMatch(/Demonstra/i);
    expect(container.textContent).not.toMatch(/Pagar agora|Pay now/i);
    unmount();
  });

  it('shows no picker on a single-coin invoice', async () => {
    invoice = { ...multiCoin, coinOptions: [], coinLocked: true, amountUsd: null };
    const { container, unmount } = renderWithProviders(<CheckoutPage />, {
      route: '/pay/inv-multi',
      routePath: '/pay/:id',
    });
    await waitFor(() => expect(container.textContent).toContain('USDT'), { timeout: 5000 });
    expect(container.textContent).not.toMatch(/Escolha a moeda|Choose a coin/i);
    expect(container.textContent).toContain('25 USDT');
    unmount();
  });
});
