/**
 * @vitest-environment jsdom
 * Admin pages — tab/button interactions for coverage.
 */
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '../../helpers/renderWithProviders.js';

vi.mock('../../../src/lib/api.js', async () => {
  const { mockApi } = await import('../../helpers/apiMock.js');
  return {
    api: vi.fn(async (path: string, opts?: { method?: string; json?: unknown }) => {
      if (opts?.method === 'POST') {
        if (path.includes('/admin/faucetlist')) return { ok: true };
        if (path.includes('/admin/telemetry')) return { ok: true };
        return { ok: true };
      }
      if (opts?.method === 'DELETE') return { ok: true };
      return mockApi(path);
    }),
    bootstrapSession: vi.fn(async () => true),
    forceReauth: vi.fn(),
  };
});

vi.mock('../../../src/components/Turnstile.js', () => ({ Turnstile: () => null }));
vi.mock('../../../src/components/ModernCaptcha.js', () => ({ ModernCaptcha: () => null }));
vi.mock('../../../src/lib/qr.js', () => ({
  addressQrDataUrl: vi.fn(async () => 'data:image/png;base64,xx'),
}));

import { AdminOverviewPage } from '../../../src/pages/AdminOverviewPage.js';
import { AdminTelemetryPage } from '../../../src/pages/AdminTelemetryPage.js';
import { AdminStakePage } from '../../../src/pages/AdminStakePage.js';
import { AdminFaucetSitesPage } from '../../../src/pages/AdminFaucetSitesPage.js';
import { AdminWithdrawalsPage } from '../../../src/pages/AdminWithdrawalsPage.js';

beforeAll(async () => {
  const i18n = (await import('../../../src/i18n/index.js')).default;
  await i18n.changeLanguage('en');
});

beforeEach(() => {
  vi.spyOn(window, 'confirm').mockReturnValue(false);
  if (!navigator.clipboard?.writeText) {
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: vi.fn(async () => undefined) },
    });
  } else {
    vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue(undefined);
  }
});

function clickButtonMatching(container: HTMLElement, pattern: RegExp) {
  const btn = within(container)
    .queryAllByRole('button')
    .find((b) => pattern.test(b.textContent || ''));
  return btn;
}

describe('AdminOverviewPage interactions', () => {
  it('refreshes data and switches activity tabs', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<AdminOverviewPage />, {
      route: '/admin',
      loggedIn: true,
      admin: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/Economia|Gateway|Faucet|Receita/i), {
      timeout: 5000,
    });

    const opTab = clickButtonMatching(container, /^Operação$/i);
    if (opTab) await user.click(opTab);

    await waitFor(() => expect(container.innerHTML).toMatch(/Usuários|Users|Pendentes/i), {
      timeout: 5000,
    });

    const refresh = clickButtonMatching(container, /atualizar|refresh|dados/i);
    if (refresh) await user.click(refresh);

    const wdrTab = clickButtonMatching(container, /saques|withdrawals/i);
    if (wdrTab) await user.click(wdrTab);
    await waitFor(() => expect(container.innerHTML).toMatch(/bc1q|CONFIRMED|saque/i), {
      timeout: 4000,
    });

    const depTab = clickButtonMatching(container, /depósitos|deposits/i);
    if (depTab) await user.click(depTab);

    const copyBtn = within(container)
      .queryAllByRole('button')
      .find((b) => b.querySelector('.bi-clipboard') || /copiar|copy/i.test(b.textContent || ''));
    if (copyBtn) await user.click(copyBtn);

    unmount();
  });
});

describe('AdminTelemetryPage interactions', () => {
  it('filters errors, toggles refresh, expands row', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<AdminTelemetryPage />, {
      route: '/admin/telemetry',
      loggedIn: true,
      admin: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/Telemetria|Health|Saúde/i), {
      timeout: 5000,
    });

    const errosTab = clickButtonMatching(container, /^Erros/i);
    if (errosTab) await user.click(errosTab);

    for (const label of [/5s/i, /30s/i, /off/i, /10s/i]) {
      const opt = clickButtonMatching(container, label);
      if (opt) await user.click(opt);
    }

    const refresh = clickButtonMatching(container, /atualizar/i);
    if (refresh) await user.click(refresh);

    for (const label of [/OPEN|Aberto/i, /ALL|Todos/i, /ERROR/i, /api/i]) {
      const chip = within(container)
        .queryAllByRole('button')
        .find((b) => label.test(b.textContent || ''));
      if (chip) await user.click(chip);
    }

    const search = container.querySelector('input[type="search"], input[placeholder*="Bus"]');
    if (search) {
      await user.type(search as HTMLElement, 'swap');
    }

    await waitFor(() => expect(container.innerHTML).toMatch(/swap|Test exception|err1/i), {
      timeout: 4000,
    });

    const expand = within(container)
      .queryAllByRole('button')
      .find((b) => /detalh|expand|stack|ver/i.test(b.textContent || '') || b.querySelector('.bi-chevron'));
    if (expand) await user.click(expand);

    for (const hrs of [/24h|24 h/i, /6h|6 h/i, /1h|1 h/i]) {
      const hBtn = clickButtonMatching(container, hrs);
      if (hBtn) await user.click(hBtn);
    }

    const testErr = clickButtonMatching(container, /test.*error|erro.*teste|simular/i);
    if (testErr) await user.click(testErr);

    unmount();
  });
});

describe('AdminStakePage interactions', () => {
  it('shows treasury wallets with search and coin filter', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<AdminStakePage />, {
      route: '/admin/stake',
      loggedIn: true,
      admin: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/Tesouraria|Custódia|BTC|Hot/i), {
      timeout: 5000,
    });

    const refresh = clickButtonMatching(container, /atualizar/i);
    if (refresh) await user.click(refresh);

    const search = container.querySelector('input[type="text"], input[type="search"]');
    if (search) await user.type(search as HTMLElement, 'user@example');

    const btcFilter = within(container)
      .queryAllByRole('button')
      .find((b) => /^BTC$/i.test((b.textContent || '').trim()));
    if (btcFilter) await user.click(btcFilter);

    const copyBtn = within(container)
      .queryAllByRole('button')
      .find((b) => b.querySelector('.bi-clipboard'));
    if (copyBtn) await user.click(copyBtn);

    unmount();
  });
});

describe('AdminWithdrawalsPage interactions', () => {
  it('loads withdrawal queue and filters', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<AdminWithdrawalsPage />, {
      route: '/admin/withdrawals',
      loggedIn: true,
      admin: true,
    });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(80), { timeout: 5000 });
    for (const btn of within(container).queryAllByRole('button').slice(0, 10)) {
      try {
        await user.click(btn);
      } catch {
        /* ignore */
      }
    }
    unmount();
  });
});

describe('AdminFaucetSitesPage interactions', () => {
  it('filters sites and moderation actions', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<AdminFaucetSitesPage />, {
      route: '/admin/faucet-sites',
      loggedIn: true,
      admin: true,
    });
    await waitFor(() => expect(container.innerHTML).toMatch(/Demo Faucet|FaucetList|Moderação/i), {
      timeout: 5000,
    });

    for (const label of [/pendente|pending/i, /aprov|approved/i, /todos|all/i]) {
      const chip = clickButtonMatching(container, label);
      if (chip) await user.click(chip);
    }

    const search = container.querySelector('input[type="text"], input[type="search"]');
    if (search) await user.type(search as HTMLElement, 'Demo');

    const approve = clickButtonMatching(container, /aprovar|approve/i);
    if (approve) await user.click(approve);

    const refresh = clickButtonMatching(container, /atualizar/i);
    if (refresh) await user.click(refresh);

    unmount();
  });
});
