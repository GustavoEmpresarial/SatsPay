import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../../../src/stores/auth.js', () => {
  const state: {
    user: { id: string; email: string } | null;
    accessToken: string | null;
    logout: ReturnType<typeof vi.fn>;
  } = {
    user: { id: 'u1', email: 'a@b.c' },
    accessToken: null,
    logout: vi.fn(() => {
      state.user = null;
      state.accessToken = null;
    }),
  };
  return {
    useAuthStore: {
      getState: () => state,
      setState: (partial: Record<string, unknown>) => Object.assign(state, partial),
    },
  };
});

vi.mock('../../../src/stores/admin.js', () => {
  const state = {
    admin: null as { id: string } | null,
    accessToken: null as string | null,
    logout: vi.fn(),
  };
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

describe('bootstrapSession', () => {
  beforeEach(async () => {
    vi.resetModules();
    vi.stubGlobal('fetch', vi.fn());
    const { useAuthStore } = await import('../../../src/stores/auth.js');
    const { useAdminStore } = await import('../../../src/stores/admin.js');
    useAuthStore.setState({
      user: { id: 'u1', email: 'a@b.c' },
      accessToken: null,
    });
    useAdminStore.setState({ admin: null, accessToken: null });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('returns false when no persisted identity', async () => {
    const { bootstrapSession } = await import('../../../src/lib/api.js');
    const { useAuthStore } = await import('../../../src/stores/auth.js');
    useAuthStore.setState({ user: null, accessToken: null });
    await expect(bootstrapSession()).resolves.toBe(false);
  });

  it('returns true when access token already in memory', async () => {
    const { bootstrapSession } = await import('../../../src/lib/api.js');
    const { useAuthStore } = await import('../../../src/stores/auth.js');
    useAuthStore.setState({ accessToken: 'existing' });
    await expect(bootstrapSession()).resolves.toBe(true);
  });

  it('restores session via refresh cookie', async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockResolvedValueOnce(
      new Response(JSON.stringify({ access_token: 'fresh' }), { status: 200 }),
    );
    const { bootstrapSession } = await import('../../../src/lib/api.js');
    const { useAuthStore } = await import('../../../src/stores/auth.js');
    useAuthStore.setState({ accessToken: null, user: { id: 'u1', email: 'a@b.c' } });
    await expect(bootstrapSession()).resolves.toBe(true);
    expect(useAuthStore.getState().accessToken).toBe('fresh');
  });

  it('logs out persisted user when refresh is auth-denied', async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockResolvedValueOnce(new Response('{}', { status: 401 }));
    const { bootstrapSession, wasRefreshAuthDenied } = await import('../../../src/lib/api.js');
    const { useAuthStore } = await import('../../../src/stores/auth.js');
    useAuthStore.setState({ accessToken: null, user: { id: 'u1', email: 'a@b.c' } });
    await expect(bootstrapSession()).resolves.toBe(false);
    expect(wasRefreshAuthDenied()).toBe(true);
  });
});
