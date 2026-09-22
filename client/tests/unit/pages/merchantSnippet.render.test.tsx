/**
 * @vitest-environment jsdom
 * The merchant panel shows the payment button and nothing else about the API.
 *
 * It previously carried a form that looked like it created an invoice and
 * only assembled text to copy, and then a full integration guide. Both are
 * gone: API documentation belongs on /docs. These assertions keep either
 * from creeping back.
 */
import { describe, expect, it, vi } from 'vitest';
import { waitFor, within } from '@testing-library/react';
import { renderWithProviders } from '../../helpers/renderWithProviders.js';

vi.mock('../../../src/lib/api.js', () => ({
  api: vi.fn(async (path: string) => {
    if (path.includes('/merchant/settings')) {
      return { acceptedCoins: ['USDT', 'POL'], availableCoins: ['BCH', 'POL', 'SOL', 'USDT', 'USDC'] };
    }
    return { invoices: [] };
  }),
  bootstrapSession: vi.fn(async () => true),
  forceReauth: vi.fn(),
}));

import { MerchantDepositsPage } from '../../../src/pages/MerchantDepositsPage.js';

async function renderPage() {
  const rendered = renderWithProviders(<MerchantDepositsPage />, { route: '/merchant/deposits' });
  await waitFor(() => expect(rendered.container.textContent).toMatch(/Botão de pagamento/i), { timeout: 5000 });
  return rendered;
}

describe('MerchantDepositsPage — botão de pagamento', () => {
  it('previews the button with the real SDK markup', async () => {
    const { container, unmount } = await renderPage();
    // Rendered by /sdk/satspay-pay.js, so the preview cannot drift from the
    // button the merchant's customer actually sees.
    const previews = container.querySelectorAll('.satspay-pay');
    expect(previews.length).toBeGreaterThan(1);
    previews.forEach((p) => expect(p.getAttribute('data-checkout_url')).toBe('/pay/demo'));
    // More than one theme, so the options are visible rather than described.
    const themes = new Set([...previews].map((p) => p.getAttribute('data-theme')));
    expect(themes.size).toBeGreaterThan(1);
    unmount();
  });

  it('gives the HTML to copy, with no key in it', async () => {
    const { container, unmount } = await renderPage();
    const html = [...container.querySelectorAll('pre')].map((p) => p.textContent ?? '');
    const snippet = html.find((h) => h.includes('satspay-pay.js'));
    expect(snippet, 'the embed snippet must be shown').toBeTruthy();
    expect(snippet).toContain('data-checkout_url');
    expect(snippet).not.toMatch(/x-api-key/i);
    unmount();
  });

  it('carries no API documentation — that lives on /docs', async () => {
    const { container, unmount } = await renderPage();
    const text = container.textContent ?? '';
    for (const gone of ['Como integrar', '1. Crie sua chave', 'amountUsd', 'AMOUNT_NOT_INTEGER']) {
      expect(text, `${gone} belongs on /docs, not on the panel`).not.toContain(gone);
    }
    // Only one code block remains: the embed.
    expect(container.querySelectorAll('pre')).toHaveLength(1);
    unmount();
  });

  it('is documentation-free and form-free', async () => {
    const { container, unmount } = await renderPage();
    // The old panel had inputs for coin, amount and orderId that created
    // nothing at all.
    expect(container.querySelectorAll('input')).toHaveLength(0);
    unmount();
  });

  it('gives the four toolbar actions one shared size', async () => {
    const { container, unmount } = await renderPage();
    const labels = ['Endereços de Depósito', 'Chaves de API', 'Ver Checkout (Demo)', 'Documentação'];
    // Exact text: other links elsewhere mention documentation too.
    const buttons = within(container)
      .queryAllByRole('link')
      .filter((a) => labels.includes((a.textContent || '').trim()));
    expect(buttons).toHaveLength(4);
    const classes = new Set(buttons.map((b) => b.className));
    expect(classes.size, 'toolbar buttons must share one style').toBe(1);
    unmount();
  });
});
