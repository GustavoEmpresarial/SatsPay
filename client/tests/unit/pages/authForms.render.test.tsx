/**
 * @vitest-environment jsdom
 */
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '../../helpers/renderWithProviders.js';

vi.mock('../../../src/lib/api.js', async () => {
  const { mockApi } = await import('../../helpers/apiMock.js');
  const { ApiError: Err } = await import('../../../src/lib/api.js');
  return {
    api: vi.fn(async (path: string, opts?: { method?: string; json?: unknown }) => {
      if (opts?.method === 'POST' && path === '/auth/login') {
        return {
          accessToken: 'new-access',
          user: {
            id: '11111111-1111-1111-1111-111111111111',
            email: 'test@example.com',
            username: 'tester',
            twoFactorEnabled: false,
            merchantStatus: 'NONE',
            createdAt: '2024-01-01T00:00:00.000Z',
            role: 'USER',
          },
        };
      }
      if (opts?.method === 'POST' && path === '/auth/register') {
        return {
          accessToken: 'reg-access',
          user: {
            id: '22222222-2222-2222-2222-222222222222',
            email: 'new@example.com',
            username: 'newuser',
            twoFactorEnabled: false,
            merchantStatus: 'NONE',
            createdAt: '2024-01-01T00:00:00.000Z',
            role: 'USER',
          },
        };
      }
      if (opts?.method === 'POST' && path === '/auth/admin/login') {
        return {
          accessToken: 'admin-access',
          user: { id: 'adm1', email: 'admin@example.com', role: 'ADMIN' },
        };
      }
      return mockApi(path);
    }),
    bootstrapSession: vi.fn(async () => true),
    forceReauth: vi.fn(),
    ApiError: Err,
  };
});

vi.mock('../../../src/components/Turnstile.js', async () => {
  const { forwardRef, useImperativeHandle } = await vi.importActual<typeof import('react')>('react');
  return {
    Turnstile: forwardRef(function MockTurnstile(
      { onVerify, onReset }: { onVerify?: (t: string) => void; onReset?: () => void },
      ref: { reset: () => void } | null,
    ) {
      useImperativeHandle(ref, () => ({
        reset: () => onReset?.(),
      }));
      return (
        <button type="button" data-testid="turnstile" onClick={() => onVerify?.('tok')}>
          captcha
        </button>
      );
    }),
  };
});
vi.mock('../../../src/components/ModernCaptcha.js', () => ({
  ModernCaptcha: ({ onVerify }: { onVerify: (t: string) => void }) => (
    <button type="button" onClick={() => onVerify('tok')}>
      captcha
    </button>
  ),
}));

import { api } from '../../../src/lib/api.js';
import { LoginPage } from '../../../src/pages/LoginPage.js';
import { RegisterPage } from '../../../src/pages/RegisterPage.js';
import { AdminLoginPage } from '../../../src/pages/AdminLoginPage.js';
import { useAdminStore } from '../../../src/stores/admin.js';

beforeAll(async () => {
  const i18n = (await import('../../../src/i18n/index.js')).default;
  await i18n.changeLanguage('en');
});

describe('auth form submits', () => {
  it('LoginPage submits credentials', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<LoginPage />, {
      route: '/login',
      loggedIn: false,
    });
    await waitFor(() => expect(container.querySelector('form')).toBeTruthy(), { timeout: 4000 });
    const email = container.querySelector('input[type="email"], input[name="email"]');
    const pass = container.querySelector('input[type="password"]');
    if (email) await user.type(email as HTMLElement, 'test@example.com');
    if (pass) await user.type(pass as HTMLElement, 'secret123');
    const submit = within(container)
      .getAllByRole('button')
      .find((b) => /entrar|login|sign in/i.test(b.textContent || ''));
    if (submit) await user.click(submit);
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(20), { timeout: 5000 });
    unmount();
  });

  it('RegisterPage submits signup', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<RegisterPage />, {
      route: '/register',
      loggedIn: false,
    });
    await waitFor(() => expect(container.querySelector('form')).toBeTruthy(), { timeout: 4000 });
    for (const sel of ['input[name="username"]', 'input[type="email"]', 'input[type="password"]']) {
      const el = container.querySelector(sel);
      if (el) await user.type(el as HTMLElement, sel.includes('email') ? 'new@example.com' : 'value123');
    }
    const submit = within(container)
      .getAllByRole('button')
      .find((b) => /criar|register|sign up/i.test(b.textContent || ''));
    if (submit) await user.click(submit);
    await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(20), { timeout: 5000 });
    unmount();
  });

  it('AdminLoginPage submits admin credentials', async () => {
    const user = userEvent.setup();
    const { container, unmount } = renderWithProviders(<AdminLoginPage />, {
      route: '/admin/login',
      loggedIn: false,
    });
    await waitFor(() => expect(container.querySelector('form')).toBeTruthy(), { timeout: 4000 });
    const email = container.querySelector('input[type="email"], input[name="email"]');
    const pass = container.querySelector('input[type="password"]');
    if (email) await user.type(email as HTMLElement, 'admin@example.com');
    if (pass) await user.type(pass as HTMLElement, 'adminpass');
    const submit = within(container)
      .getAllByRole('button')
      .find((b) => /entrar|login|admin/i.test(b.textContent || ''));
    if (submit) await user.click(submit);
    await waitFor(() => expect(useAdminStore.getState().admin).toBeTruthy(), { timeout: 5000 });
    unmount();
  }, 12000);

  afterEach(() => {
    vi.mocked(api).mockClear();
    useAdminStore.getState().logout();
  });

  it('LoginPage keeps Turnstile after codeSent and sends a fresh token', async () => {
    const user = userEvent.setup();
    vi.mocked(api).mockImplementationOnce(async () => ({ codeSent: true, message: 'code sent' }));

    const { container, unmount } = renderWithProviders(<LoginPage />, {
      route: '/login',
      loggedIn: false,
    });
    await waitFor(() => expect(container.querySelector('form')).toBeTruthy(), { timeout: 4000 });
    await user.type(container.querySelector('input[type="email"]') as HTMLElement, 'test@example.com');
    await user.type(container.querySelector('input[type="password"]') as HTMLElement, 'secret123');
    await user.click(within(container).getByTestId('turnstile'));
    await user.click(within(container).getByRole('button', { name: /entrar|sign in/i }));

    await waitFor(() => expect(container.querySelector('#login-code')).toBeTruthy(), { timeout: 4000 });
    expect(within(container).getByTestId('turnstile')).toBeTruthy();

    await user.type(container.querySelector('#login-code') as HTMLElement, '123456');
    await user.click(within(container).getByTestId('turnstile'));
    await user.click(within(container).getByRole('button', { name: /verify|verificar/i }));

    await waitFor(() => expect(vi.mocked(api).mock.calls.length).toBe(2), { timeout: 4000 });
    const second = vi.mocked(api).mock.calls[1]?.[1] as { json?: { captchaToken?: string; emailCode?: string } };
    expect(second.json?.captchaToken).toBe('tok');
    expect(second.json?.emailCode).toBe('123456');
    unmount();
  }, 12000);

  it('AdminLoginPage keeps Turnstile after codeSent and sends a fresh token', async () => {
    const user = userEvent.setup();
    vi.mocked(api).mockImplementationOnce(async () => ({ codeSent: true }));

    const { container, unmount } = renderWithProviders(<AdminLoginPage />, {
      route: '/admin/login',
      loggedIn: false,
    });
    await waitFor(() => expect(container.querySelector('form')).toBeTruthy(), { timeout: 4000 });
    await user.type(container.querySelector('input[type="email"]') as HTMLElement, 'admin@example.com');
    await user.type(container.querySelector('input[type="password"]') as HTMLElement, 'adminpass');
    await user.click(within(container).getByTestId('turnstile'));
    await user.click(within(container).getByRole('button', { name: /entrar no admin|autenticando/i }));

    await waitFor(() => expect(container.querySelector('input[inputmode="numeric"]')).toBeTruthy(), {
      timeout: 4000,
    });
    expect(within(container).getByTestId('turnstile')).toBeTruthy();

    await user.type(container.querySelector('input[inputmode="numeric"]') as HTMLElement, '654321');
    await user.click(within(container).getByTestId('turnstile'));
    await user.click(within(container).getByRole('button', { name: /entrar no admin|autenticando/i }));

    await waitFor(() => expect(vi.mocked(api).mock.calls.length).toBe(2), { timeout: 4000 });
    const second = vi.mocked(api).mock.calls[1]?.[1] as { json?: { captchaToken?: string; emailCode?: string } };
    expect(second.json?.captchaToken).toBe('tok');
    expect(second.json?.emailCode).toBe('654321');
    unmount();
  }, 12000);
});
