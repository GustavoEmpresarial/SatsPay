import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

vi.mock('../../../src/stores/auth.js', () => {
  const state = {
    user: { id: 'u1', email: 'a@b.c' },
    accessToken: 'tok',
    refreshToken: 'ref',
    logout: vi.fn(),
  };
  return {
    useAuthStore: {
      getState: () => state,
      setState: (partial: Record<string, unknown>) => Object.assign(state, partial),
    },
  };
});

vi.mock('../../../src/stores/admin.js', () => {
  const state = { accessToken: null as string | null, refreshToken: null as string | null, logout: vi.fn() };
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

describe('api Content-Type (415 regression)', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      }),
    ));
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it('sets application/json when using json option', async () => {
    const { api } = await import('../../../src/lib/api.js');
    await api('/oauth/apps', { method: 'POST', json: { name: 'App' } });
    const [, init] = vi.mocked(fetch).mock.calls[0]!;
    const headers = init?.headers as Record<string, string>;
    expect(headers['Content-Type']).toBe('application/json');
    expect(init?.body).toBe(JSON.stringify({ name: 'App' }));
  });

  it('still sets application/json when caller uses body: JSON.stringify (legacy footgun)', async () => {
    const { api } = await import('../../../src/lib/api.js');
    await api('/oauth/apps', {
      method: 'POST',
      body: JSON.stringify({ name: 'App', redirect_uris: ['https://x'] }),
    });
    const [, init] = vi.mocked(fetch).mock.calls[0]!;
    const headers = init?.headers as Record<string, string>;
    expect(headers['Content-Type']).toBe('application/json');
  });

  it('does not force JSON Content-Type on empty POST (rotate-secret)', async () => {
    const { api } = await import('../../../src/lib/api.js');
    await api('/oauth/apps/1/rotate-secret', { method: 'POST' });
    const [, init] = vi.mocked(fetch).mock.calls[0]!;
    const headers = (init?.headers ?? {}) as Record<string, string>;
    expect(headers['Content-Type']).toBeUndefined();
  });
});

describe('no api(... body: JSON.stringify) in pages (static)', () => {
  function walk(dir: string, out: string[] = []): string[] {
    for (const name of readdirSync(dir)) {
      const p = path.join(dir, name);
      if (statSync(p).isDirectory()) walk(p, out);
      else if (/\.(tsx?)$/.test(name)) out.push(p);
    }
    return out;
  }

  it('forbids raw body: JSON.stringify on api() calls (use json: instead)', () => {
    const srcRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../src');
    const files = walk(srcRoot);
    const offenders: string[] = [];
    // Detect api( ... body: JSON.stringify inside the same call roughly
    const pattern = /api(?:<[^>]*>)?\s*\([^;]{0,400}?body\s*:\s*JSON\.stringify/s;
    for (const file of files) {
      if (file.endsWith(`${path.sep}lib${path.sep}api.ts`)) continue;
      const text = readFileSync(file, 'utf8');
      if (pattern.test(text)) {
        offenders.push(path.relative(srcRoot, file));
      }
    }
    expect(offenders, `Use api({ json: {...} }) not body: JSON.stringify — causes HTTP 415`).toEqual([]);
  });
});
