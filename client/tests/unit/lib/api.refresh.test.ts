import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const authLogout = vi.fn();
vi.mock('../../../src/stores/auth.js', () => {
  const state = {
    user: { id: 'u1', email: 'a@b.c', role: 'USER' },
    accessToken: null as string | null,
    logout: authLogout,
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
  shouldReportApiStatus: () => false,
}));

describe('refreshAccessToken', () => {
  beforeEach(() => {
    vi.resetModules();
    authLogout.mockClear();
    vi.stubGlobal('fetch', vi.fn());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('forceReauth on 403 refresh response', async () => {
    vi.mocked(fetch).mockResolvedValueOnce(new Response('{}', { status: 403 }));
    const { refreshAccessToken } = await import('../../../src/lib/api.js');
    await expect(refreshAccessToken()).resolves.toBeNull();
    expect(authLogout).toHaveBeenCalled();
  });

  it('returns null on 502 without logging out', async () => {
    vi.mocked(fetch).mockResolvedValueOnce(new Response('{}', { status: 502 }));
    const { refreshAccessToken } = await import('../../../src/lib/api.js');
    await expect(refreshAccessToken()).resolves.toBeNull();
    expect(authLogout).not.toHaveBeenCalled();
  });

  it('returns null on network failure', async () => {
    vi.mocked(fetch).mockRejectedValueOnce(new Error('down'));
    const { refreshAccessToken } = await import('../../../src/lib/api.js');
    await expect(refreshAccessToken()).resolves.toBeNull();
    expect(authLogout).not.toHaveBeenCalled();
  });
});

describe('looksLikeJsonBody', () => {
  it('detects JSON object and array prefixes', async () => {
    const { looksLikeJsonBody } = await import('../../../src/lib/api.js');
    expect(looksLikeJsonBody('  {"a":1}')).toBe(true);
    expect(looksLikeJsonBody('[1]')).toBe(true);
    expect(looksLikeJsonBody('plain')).toBe(false);
  });
});
