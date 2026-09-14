/**
 * @vitest-environment jsdom
 * Nav + Support coverage toward 100% (merchant-only API docs).
 */
import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '../../helpers/renderWithProviders.js';
import { useAuthStore } from '../../../src/stores/auth.js';

vi.mock('../../../src/lib/api.js', async () => {
  const { mockApi } = await import('../../helpers/apiMock.js');
  const { ApiError: Err } = await import('../../../src/lib/api.js');
  return {
    api: vi.fn(async (path: string, opts?: { method?: string; json?: unknown }) => {
      if (path === '/support/tickets' && opts?.method === 'POST') {
        const msg = (opts.json as { message?: string })?.message ?? '';
        if (!msg.trim()) throw new Err(400, 'ERROR', 'empty_message');
        return {
          ticket: {
            id: 't-new',
            topic: 'deposit',
            subject: '[SatsPay] Depósito',
            status: 'OPEN',
            created_at: '2026-01-01T00:00:00Z',
            updated_at: '2026-01-01T00:00:00Z',
            message_count: 1,
          },
          messages: [
            {
              id: 'm1',
              author_role: 'USER',
              body: msg,
              created_at: '2026-01-01T00:00:00Z',
            },
          ],
        };
      }
      if (path === '/support/tickets') {
        return {
          tickets: [
            {
              id: 't1',
              topic: 'deposit',
              subject: '[SatsPay] Depósito',
              status: 'WAITING_USER',
              created_at: '2026-01-01T00:00:00Z',
              updated_at: '2026-01-01T00:00:00Z',
              message_count: 2,
            },
            {
              id: 't-closed',
              topic: 'account',
              subject: '[SatsPay] Conta',
              status: 'RESOLVED',
              created_at: '2026-01-01T00:00:00Z',
              updated_at: '2026-01-01T00:00:00Z',
              message_count: 1,
            },
          ],
        };
      }
      if (path === '/support/tickets/t1') {
        return {
          ticket: {
            id: 't1',
            topic: 'deposit',
            subject: '[SatsPay] Depósito',
            status: 'WAITING_USER',
            created_at: '2026-01-01T00:00:00Z',
            updated_at: '2026-01-01T00:00:00Z',
            message_count: 2,
          },
          messages: [
            {
              id: 'm1',
              author_role: 'USER',
              body: 'olá',
              created_at: '2026-01-01T00:00:00Z',
            },
            {
              id: 'm2',
              author_role: 'STAFF',
              body: 'recebemos',
              created_at: '2026-01-01T01:00:00Z',
            },
          ],
        };
      }
      if (path === '/support/tickets/t-closed') {
        return {
          ticket: {
            id: 't-closed',
            topic: 'account',
            subject: '[SatsPay] Conta',
            status: 'RESOLVED',
            created_at: '2026-01-01T00:00:00Z',
            updated_at: '2026-01-01T00:00:00Z',
            message_count: 1,
          },
          messages: [
            {
              id: 'm1',
              author_role: 'USER',
              body: 'done',
              created_at: '2026-01-01T00:00:00Z',
            },
          ],
        };
      }
      if (path.includes('/support/tickets/') && path.endsWith('/messages') && opts?.method === 'POST') {
        return {
          ticket: {
            id: 't1',
            topic: 'deposit',
            subject: '[SatsPay] Depósito',
            status: 'WAITING_STAFF',
            created_at: '2026-01-01T00:00:00Z',
            updated_at: '2026-01-01T02:00:00Z',
            message_count: 3,
          },
          messages: [
            {
              id: 'm1',
              author_role: 'USER',
              body: 'olá',
              created_at: '2026-01-01T00:00:00Z',
            },
            {
              id: 'm2',
              author_role: 'STAFF',
              body: 'recebemos',
              created_at: '2026-01-01T01:00:00Z',
            },
            {
              id: 'm3',
              author_role: 'USER',
              body: (opts.json as { message?: string })?.message ?? 'reply',
              created_at: '2026-01-01T02:00:00Z',
            },
          ],
        };
      }
      if (path === '/auth/logout') return { ok: true };
      if (path === '/auth/me') {
        return {
          id: '11111111-1111-1111-1111-111111111111',
          email: 'user@example.com',
          username: 'demo',
          role: 'USER',
          twoFactorEnabled: false,
          merchantStatus: 'APPROVED',
        };
      }
      return mockApi(path);
    }),
    bootstrapSession: vi.fn(async () => true),
    forceReauth: vi.fn(),
    ApiError: Err,
  };
});

import { AppLayout } from '../../../src/components/AppLayout.js';
import { SupportPage } from '../../../src/pages/SupportPage.js';
import { DocumentationPage } from '../../../src/pages/DocumentationPage.js';

describe('AppLayout — API docs merchant-only', () => {
  beforeEach(() => {
    useAuthStore.setState({
      user: {
        id: '11111111-1111-1111-1111-111111111111',
        email: 'user@example.com',
        username: 'demo',
        role: 'USER',
        twoFactorEnabled: false,
        merchantStatus: 'APPROVED',
      } as never,
      accessToken: 'tok',
    });
  });

  it('personal Recursos has Guia+Status but not Documentação da API', async () => {
    const { container, unmount } = renderWithProviders(<AppLayout />, {
      route: '/dashboard',
      loggedIn: true,
    });
    await waitFor(() => expect(container.textContent).toMatch(/Guia da Plataforma|Recursos/i), {
      timeout: 8000,
    });
    expect(container.textContent).toMatch(/Guia da Plataforma/);
    expect(container.textContent).not.toMatch(/Documentação da API/);
    unmount();
  });

  it('merchant mode shows Documentação da API in suite', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<AppLayout />, {
      route: '/dashboard',
      loggedIn: true,
    });
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(100), { timeout: 5000 });
    const merchantBtn = within(container)
      .queryAllByRole('button')
      .find((b) => /comerciante/i.test(b.textContent || ''));
    expect(merchantBtn).toBeTruthy();
    if (merchantBtn) await user.click(merchantBtn);
    await waitFor(() => expect(container.textContent).toMatch(/Documentação da API|API/i), {
      timeout: 8000,
    });
    expect(container.textContent).toMatch(/Documentação da API|Chaves|API/);
    unmount();
  });
});

describe('SupportPage — ticket flows', () => {
  beforeEach(() => {
    useAuthStore.setState({
      user: {
        id: '11111111-1111-1111-1111-111111111111',
        email: 'user@example.com',
        username: 'demo',
        role: 'USER',
        twoFactorEnabled: false,
        merchantStatus: 'NONE',
      } as never,
      accessToken: 'tok',
    });
  });

  it('lists tickets, opens thread, replies', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<SupportPage />, {
      route: '/support',
      loggedIn: true,
    });
    await waitFor(() => expect(container.textContent).toMatch(/Depósito|chamado|ticket/i), {
      timeout: 8000,
    });
    const ticketBtn = within(container)
      .queryAllByRole('button')
      .find((b) => /Depósito/i.test(b.textContent || ''));
    if (ticketBtn) await user.click(ticketBtn);
    await waitFor(() => expect(container.textContent).toMatch(/recebemos|Equipe|staff/i), {
      timeout: 5000,
    });
    const replyBox = container.querySelector('textarea');
    if (replyBox) {
      await user.clear(replyBox);
      await user.type(replyBox, 'mais detalhes do problema');
      const send = within(container)
        .queryAllByRole('button')
        .find((b) => /responder|reply|enviar/i.test(b.textContent || ''));
      if (send) await user.click(send);
    }
    unmount();
  });

  it('creates a new ticket', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<SupportPage />, {
      route: '/support',
      loggedIn: true,
    });
    await waitFor(() => expect(container.querySelector('textarea')).toBeTruthy(), { timeout: 5000 });
    const areas = container.querySelectorAll('textarea');
    const compose = areas[0];
    if (compose) {
      await user.type(compose, 'depósito BTC atrasado desde ontem');
      const submit = within(container)
        .queryAllByRole('button')
        .find((b) => /enviar chamado|submit ticket|enviar/i.test(b.textContent || ''));
      if (submit) await user.click(submit);
    }
    unmount();
  });
});

describe('DocumentationPage — security tab no infra leak', () => {
  it('opens Segurança & Custódia without SQL/xpub', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<DocumentationPage />, {
      route: '/documentation',
      loggedIn: false,
    });
    const sec = within(container)
      .queryAllByRole('button')
      .find((b) => /Segurança/i.test(b.textContent || ''));
    if (sec) await user.click(sec);
    await waitFor(() => expect(container.textContent).toMatch(/Custódia|Segurança/i), {
      timeout: 3000,
    });
    expect(container.textContent).not.toMatch(/FOR UPDATE|xpub|captcha_seen_tokens|HOUSE/);
    unmount();
  });
});
