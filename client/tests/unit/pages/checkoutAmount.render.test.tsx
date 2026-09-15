/**
 * @vitest-environment jsdom
 * What the paying customer actually reads on the hosted checkout.
 */
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { waitFor } from '@testing-library/react';
import { renderWithProviders } from '../../helpers/renderWithProviders.js';

let catalogResponse: unknown = {
  priceDecimals: 8,
  coins: [{ symbol: 'USDT', priceUsd: '100000000' }],
};

const invoice = {
  id: 'inv-1',
  status: 'PENDING',
  coin: 'USDT',
  amount: '2500000000', // 25 USDT in ledger units
  depositAddress: '0x71C6705624342490cf03323decB0C392A8892A88',
  orderId: 'ORD-1',
  qrCode: 'usdt:0x71C6705624342490cf03323decB0C392A8892A88?amount=25',
  expiresAt: new Date(Date.now() + 30 * 60_000).toISOString(),
};

vi.mock('../../../src/lib/api.js', () => ({
  api: vi.fn(async (path: string) => {
    if (path.includes('/public/coins')) return catalogResponse;
    if (path.includes('/public/pay/demo')) return { ...invoice, id: 'demo', demo: true, amountDisplay: '25' };
    return invoice;
  }),
  bootstrapSession: vi.fn(async () => true),
  forceReauth: vi.fn(),
}));

vi.mock('../../../src/lib/qr.js', () => ({
  addressQrDataUrl: vi.fn(async () => 'data:image/png;base64,xx'),
}));

import { CheckoutPage } from '../../../src/pages/CheckoutPage.js';

beforeAll(async () => {
  const i18n = (await import('../../../src/i18n/index.js')).default;
  await i18n.changeLanguage('en');
});

describe('CheckoutPage — what the customer reads', () => {
  it('shows the amount in coins, never in ledger units', async () => {
    catalogResponse = { priceDecimals: 8, coins: [{ symbol: 'USDT', priceUsd: '100000000' }] };
    const { container, unmount } = renderWithProviders(<CheckoutPage />, { route: '/pay/inv-1', routePath: '/pay/:id' });

    await waitFor(() => expect(container.textContent).toContain('USDT'), { timeout: 5000 });
    // The page used to render `inv.amount` raw: "2500000000 USDT".
    expect(container.textContent).toContain('25');
    expect(container.textContent).not.toContain('2500000000');
    unmount();
  });

  it('shows an approximate fiat value from the public catalogue', async () => {
    catalogResponse = { priceDecimals: 8, coins: [{ symbol: 'USDT', priceUsd: '100000000' }] };
    const { container, unmount } = renderWithProviders(<CheckoutPage />, { route: '/pay/inv-1', routePath: '/pay/:id' });

    // 25 USDT at US$ 1.00 — the checkout showed no fiat at all before.
    await waitFor(() => expect(container.textContent).toMatch(/≈/), { timeout: 5000 });
    expect(container.textContent).toMatch(/25[.,]00/);
    unmount();
  });

  it('never blanks the checkout when the catalogue is broken', async () => {
    // A payment page must survive a bad price feed: no label beats no page.
    for (const broken of [undefined, null, {}, { coins: null }, { coins: [{}] }, { priceDecimals: 'x', coins: [{ symbol: 'USDT', priceUsd: 'nope' }] }]) {
      catalogResponse = broken;
      const { container, unmount } = renderWithProviders(<CheckoutPage />, { route: '/pay/inv-1', routePath: '/pay/:id' });
      await waitFor(() => expect(container.textContent).toContain('USDT'), { timeout: 5000 });
      expect(container.textContent).toContain('25');
      expect(container.textContent).not.toMatch(/≈/);
      unmount();
    }
  });

  it('labels the demo invoice and hides the real payment action', async () => {
    catalogResponse = { priceDecimals: 8, coins: [{ symbol: 'USDT', priceUsd: '100000000' }] };
    const { container, unmount } = renderWithProviders(<CheckoutPage />, {
      route: '/pay/demo',
      routePath: '/pay/:id',
      loggedIn: true,
    });

    await waitFor(() => expect(container.textContent).toMatch(/Demonstra/i), { timeout: 5000 });
    expect(container.textContent).toMatch(/nenhum pagamento é processado/i);
    expect(container.textContent).not.toMatch(/1-Clique/i);
    unmount();
  });
});
