/**
 * @vitest-environment jsdom
 * The "Como integrar" section — documentation, not a form.
 *
 * It replaced a panel of inputs that looked like it created an invoice and
 * only assembled text to copy. The assertions below exist to keep that kind
 * of decorative UI from coming back.
 */
import { describe, expect, it, vi } from 'vitest';
import { waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
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
  await waitFor(() => expect(rendered.container.textContent).toMatch(/Como integrar/i), { timeout: 5000 });
  return rendered;
}

describe('MerchantDepositsPage — Como integrar', () => {
  it('walks through the three steps of an integration', async () => {
    const { container, unmount } = await renderPage();
    expect(container.textContent).toMatch(/1\. Crie sua chave/i);
    expect(container.textContent).toMatch(/2\. Crie a cobrança/i);
    expect(container.textContent).toMatch(/3\. Leve o cliente/i);
    unmount();
  });

  it('leads with the dollar-priced call, where the customer picks the coin', async () => {
    const { container, unmount } = await renderPage();
    const code = container.querySelector('pre')!;
    expect(code.textContent).toContain('POST');
    expect(code.textContent).toContain('/v1/merchant/deposits');
    expect(code.textContent).toContain('"amountUsd": "25.00"');
    unmount();
  });

  it('still teaches the ledger-unit rule for the crypto-priced call', async () => {
    const { container, unmount } = await renderPage();
    // The exact trap that broke a real integration.
    expect(container.textContent).toContain('2500000000');
    expect(container.textContent).toMatch(/AMOUNT_NOT_INTEGER/);
    unmount();
  });

  it('switches the example between languages', async () => {
    const user = userEvent.setup();
    const { container, unmount } = await renderPage();

    const node = within(container)
      .getAllByRole('button')
      .find((b) => /Node\.js/i.test(b.textContent || ''))!;
    await user.click(node);

    await waitFor(() => {
      const code = container.querySelector('pre')!;
      expect(code.textContent).toContain('await fetch');
      expect(code.textContent).toContain('invoice.checkoutUrl');
    });
    unmount();
  });

  it('shows the branded button without ever putting a key in the browser', async () => {
    const { container, unmount } = await renderPage();
    const blocks = [...container.querySelectorAll('pre')].map((p) => p.textContent ?? '');
    const button = blocks.find((b) => b.includes('satspay-pay.js'));
    expect(button, 'the pay button snippet must be shown').toBeTruthy();
    expect(button).toContain('data-checkout_url');
    expect(button).not.toMatch(/x-api-key/i);
    unmount();
  });

  it('is documentation, not a form', async () => {
    const { container, unmount } = await renderPage();
    // The removed panel had inputs for coin, amount and orderId that created
    // nothing. The only inputs left on this page belong to the invoice filter.
    const inputs = [...container.querySelectorAll('input')];
    expect(inputs, 'the integration section must not take typed input').toHaveLength(0);
    unmount();
  });

  it('links to the demo checkout from where the merchant works', async () => {
    const { container, unmount } = await renderPage();
    const demo = within(container)
      .queryAllByRole('link')
      .filter((a) => a.getAttribute('href') === '/pay/demo');
    expect(demo.length).toBeGreaterThan(0);
    unmount();
  });

  it('gives the four toolbar actions one shared size', async () => {
    const { container, unmount } = await renderPage();
    const labels = ['Endereços de Depósito', 'Chaves de API', 'Ver Checkout (Demo)', 'Documentação'];
    // Exact text: the section footer also has a "Documentação completa" link.
    const buttons = within(container)
      .queryAllByRole('link')
      .filter((a) => labels.includes((a.textContent || '').trim()));
    expect(buttons).toHaveLength(4);
    // Same class string means same size, shape and weight — no odd one out.
    const classes = new Set(buttons.map((b) => b.className));
    expect(classes.size, 'toolbar buttons must share one style').toBe(1);
    unmount();
  });
});
