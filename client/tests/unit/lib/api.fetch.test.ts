/**
 * @vitest-environment jsdom
 * api.ts forceReauth / 401 paths touch window.location.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const authLogout = vi.fn();
vi.mock('../../../src/stores/auth.js', () => {
  const state = {
    user: { id: 'u1', email: 'a@b.c', role: 'USER' },
    accessToken: 'tok',
    logout: authLogout.mockImplementation(() => {
      state.accessToken = null;
    }),
    updateUser: vi.fn(),
  };
  return {
    useAuthStore: {
      getState: () => state,
      setState: (partial: Record<string, unknown>) => Object.assign(state, partial),
    },
  };
});

vi.mock('../../../src/stores/admin.js', () => {
  const state = { accessToken: null as string | null, logout: vi.fn() };
  return {
    useAdminStore: {
      getState: () => state,
      setState: (partial: Record<string, unknown>) => Object.assign(state, partial),
    },
  };
});

vi.mock('../../../src/lib/reportError.js', () => ({
  reportClientError: vi.fn(),
  shouldReportApiStatus: () => true,
}));

describe('api fetch branches', () => {
  beforeEach(() => {
    vi.resetModules();
    authLogout.mockClear();
    vi.stubGlobal('fetch', vi.fn());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('returns undefined on 204', async () => {
    vi.mocked(fetch).mockResolvedValueOnce(new Response(null, { status: 204 }));
    const { api } = await import('../../../src/lib/api.js');
    await expect(api('/x')).resolves.toBeUndefined();
  });

  it('retries once after 401 when refresh succeeds', async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock
      .mockResolvedValueOnce(new Response('{}', { status: 401 }))
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ access_token: 'newtok' }), { status: 200 }),
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ ok: true }), {
          status: 200,
          headers: { 'Content-Type': 'application/json' },
        }),
      );
    const { api } = await import('../../../src/lib/api.js');
    const out = await api<{ ok: boolean }>('/wallet?kind=PERSONAL');
    expect(out.ok).toBe(true);
    expect(fetchMock).toHaveBeenCalledTimes(3);
  });

  it('throws NETWORK on fetch failure', async () => {
    vi.mocked(fetch).mockRejectedValueOnce(new Error('offline'));
    const { api, ApiError } = await import('../../../src/lib/api.js');
    await expect(api('/x')).rejects.toBeInstanceOf(ApiError);
  });

  it('forceReauth when retried request still returns 401', async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock
      .mockResolvedValueOnce(new Response('{}', { status: 401 }))
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ access_token: 'newtok' }), { status: 200 }),
      )
      .mockResolvedValueOnce(new Response('{}', { status: 401 }));
    const { api } = await import('../../../src/lib/api.js');
    await expect(api('/wallet?kind=PERSONAL')).rejects.toMatchObject({ status: 401 });
    expect(authLogout).toHaveBeenCalled();
  });

  it('401 with transient refresh failure keeps session message', async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock
      .mockResolvedValueOnce(new Response('{}', { status: 401 }))
      .mockResolvedValueOnce(new Response('{}', { status: 502 }));
    const { api } = await import('../../../src/lib/api.js');
    await expect(api('/wallet?kind=PERSONAL')).rejects.toMatchObject({
      code: 'UNAUTHORIZED',
    });
  });

  it('maps 500 errors to ApiError with message', async () => {
    vi.mocked(fetch).mockResolvedValueOnce(
      new Response(JSON.stringify({ error: { code: 'X', message: 'boom' } }), { status: 500 }),
    );
    const { api, ApiError } = await import('../../../src/lib/api.js');
    await expect(api('/fail')).rejects.toMatchObject({ status: 500, code: 'X' });
    expect(ApiError).toBeTruthy();
  });
});

describe('forceReauth', () => {
  it('does not navigate on public auth paths', async () => {
    const replace = vi.fn();
    Object.defineProperty(window, 'location', {
      configurable: true,
      value: { ...window.location, pathname: '/login', replace },
    });
    const { forceReauth } = await import('../../../src/lib/api.js');
    forceReauth();
    expect(replace).not.toHaveBeenCalled();
  });
});
