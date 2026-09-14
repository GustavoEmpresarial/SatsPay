/**
 * @vitest-environment jsdom
 */
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { waitFor } from '@testing-library/react';
import { renderWithProviders } from '../../helpers/renderWithProviders.js';

const invoices = [
  {
    id: 'minv2',
    orderId: 'ORD-PEND',
    siteName: 'Demo Shop',
    coin: 'LTC',
    amount: '50000000',
    feeAmount: '50000',
    netAmount: '49950000',
    depositAddress: 'ltc1qmerchant000000000000000000000000',
    status: 'PENDING',
    callbackUrl: 'https://shop.example.com/webhook',
    webhookDelivered: false,
    webhookAttempts: 0,
    createdAt: '2024-06-02T12:00:00.000Z',
  },
];

vi.mock('../../../src/lib/api.js', () => ({
  api: vi.fn(async (path: string) => {
    if (path.includes('/merchant/deposits')) return { invoices };
    return { invoices: [] };
  }),
  bootstrapSession: vi.fn(async () => true),
  forceReauth: vi.fn(),
}));

import { MerchantDepositsPage } from '../../../src/pages/MerchantDepositsPage.js';

beforeAll(async () => {
  const i18n = (await import('../../../src/i18n/index.js')).default;
  await i18n.changeLanguage('en');
});

describe('MerchantDepositsPage invoice table', () => {
  it('renders table rows from API invoices', async () => {
    const { container, unmount } = renderWithProviders(<MerchantDepositsPage />, {
      route: '/merchant/deposits',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/ORD-PEND|Testar Webhook/i), {
      timeout: 10000,
    });
    unmount();
  }, 15000);
});
