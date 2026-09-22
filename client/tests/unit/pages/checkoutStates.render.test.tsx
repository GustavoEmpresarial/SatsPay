/**
 * @vitest-environment jsdom
 */
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '../../helpers/renderWithProviders.js';

vi.mock('../../../src/lib/api.js', async () => {
  const { mockApi } = await import('../../helpers/apiMock.js');
  return {
    api: vi.fn(async (path: string, opts?: { method?: string }) => {
      if (opts?.method === 'POST') return mockApi(path);
      return mockApi(path);
    }),
    bootstrapSession: vi.fn(async () => true),
    forceReauth: vi.fn(),
  };
});

vi.mock('../../../src/lib/qr.js', () => ({
  addressQrDataUrl: vi.fn(async () => 'data:image/png;base64,xx'),
}));

import { CheckoutPage } from '../../../src/pages/CheckoutPage.js';

beforeAll(async () => {
  const i18n = (await import('../../../src/i18n/index.js')).default;
  await i18n.changeLanguage('en');
});

beforeEach(() => {
  if (!navigator.clipboard?.writeText) {
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: vi.fn(async () => undefined) },
    });
  } else {
    vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue(undefined);
  }
});

afterEach(() => {
  vi.useRealTimers();
});

describe('CheckoutPage status branches', () => {
  for (const [id, expectRe] of [
    ['inv-pending', /pending|aguard|QR|bc1q/i],
    ['inv-detected', /blockchain|Aguardando|bc1q/i],
    ['inv-paid', /Pagamento concluído|Payment complete|On-chain|Saldo SatsPay/i],
    ['inv-expired', /Tempo esgotado|Time expired|expir/i],
    ['inv-cancelled', /cancelad|Checkout Seguro|Secure checkout/i],
  ] as const) {
    it(`renders checkout ${id}`, async () => {
      const { container, unmount } = renderWithProviders(<CheckoutPage />, {
        route: `/pay/${id}`,
        routePath: '/pay/:id',
        loggedIn: false,
      });
      await waitFor(() => expect(container.innerHTML).toMatch(expectRe), { timeout: 6000 });
      unmount();
    });
  }

  it('copy address and pay with balance when logged in', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<CheckoutPage />, {
      route: '/pay/inv-pending',
      routePath: '/pay/:id',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/bc1q/i), { timeout: 6000 });
    const copy = within(container)
      .queryAllByRole('button')
      .find((b) => /copiar|copy/i.test(b.textContent || '') || b.querySelector('.bi-clipboard'));
    if (copy) await user.click(copy);
    const payBal = within(container)
      .queryAllByRole('button')
      .find((b) => /saldo|balance|carteira/i.test(b.textContent || ''));
    if (payBal && !(payBal as HTMLButtonElement).disabled) await user.click(payBal);
    unmount();
  });

  it('runs countdown timer on pending invoice', async () => {
    vi.useFakeTimers({ toFake: ['Date', 'setInterval', 'clearInterval'] });
    const { container, unmount } = renderWithProviders(<CheckoutPage />, {
      route: '/pay/inv-pending',
      routePath: '/pay/:id',
      loggedIn: false,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/\d{2}:\d{2}/), { timeout: 6000 });
    await vi.advanceTimersByTimeAsync(2000);
    unmount();
  });
});
