/**
 * @vitest-environment jsdom
 */
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { waitFor } from '@testing-library/react';
import { renderWithProviders } from '../../helpers/renderWithProviders.js';

vi.mock('../../../src/lib/api.js', () => ({
  api: vi.fn(async () => ({ invoices: [] })),
  bootstrapSession: vi.fn(async () => true),
  forceReauth: vi.fn(),
}));

import { MerchantDepositsPage } from '../../../src/pages/MerchantDepositsPage.js';

beforeAll(async () => {
  const i18n = (await import('../../../src/i18n/index.js')).default;
  await i18n.changeLanguage('en');
});

describe('MerchantDepositsPage empty list', () => {
  it('renders empty state CTA', async () => {
    const { container, unmount } = renderWithProviders(<MerchantDepositsPage />, {
      route: '/merchant/deposits',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/Nenhuma fatura encontrada/i), {
      timeout: 6000,
    });
    unmount();
  });
});
