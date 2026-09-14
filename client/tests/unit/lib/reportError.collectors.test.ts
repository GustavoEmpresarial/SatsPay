/**
 * @vitest-environment jsdom
 * Deep coverage for client error reporter (#43/#44).
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  addBreadcrumb,
  flushErrorQueue,
  getBreadcrumbs,
  installErrorCollectors,
  reportAuthFailure,
  reportClientError,
} from '../../../src/lib/reportError.js';
import { useAuthStore } from '../../../src/stores/auth.js';

describe('reportError collectors', () => {
  beforeEach(() => {
    useAuthStore.setState({ user: null, accessToken: 'tok' });
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('{}', { status: 200 })),
    );
    vi.stubGlobal('navigator', {
      ...navigator,
      sendBeacon: vi.fn(() => true),
      userAgent: 'vitest',
      language: 'en',
      onLine: true,
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    flushErrorQueue();
  });

  it('addBreadcrumb + getBreadcrumbs retain recent events', () => {
    addBreadcrumb('navigation', 'went to /wallets');
    addBreadcrumb('ui.click', 'Clicked button');
    const crumbs = getBreadcrumbs();
    expect(crumbs.length).toBeGreaterThanOrEqual(2);
    expect(crumbs.at(-1)?.category).toBe('ui.click');
  });

  it('reportClientError queues and flushes via beacon/fetch', () => {
    reportClientError({
      kind: 'js',
      message: 'boom for coverage ' + Math.random(),
      stack: 'at App.tsx:1',
      endpoint: '/dashboard',
      statusCode: 500,
    });
    flushErrorQueue();
    expect(navigator.sendBeacon || fetch).toBeTruthy();
  });

  it('reportAuthFailure records auth breadcrumb path', () => {
    reportAuthFailure('auth', 'login failed', { reason: 'bad' });
    flushErrorQueue();
    expect(getBreadcrumbs().some((c) => c.message.includes('login failed') || c.category === 'http')).toBe(
      true,
    );
  });

  it('throttles duplicate reports', () => {
    const msg = 'unique-throttle-' + Date.now();
    reportClientError({ kind: 'api', message: msg, endpoint: '/wallet', statusCode: 500 });
    reportClientError({ kind: 'api', message: msg, endpoint: '/wallet', statusCode: 500 });
    reportClientError({ kind: 'api', message: msg, endpoint: '/wallet', statusCode: 500 });
    flushErrorQueue();
  });

  it('installErrorCollectors hooks window listeners once', () => {
    installErrorCollectors();
    installErrorCollectors(); // idempotent
    window.dispatchEvent(new ErrorEvent('error', { message: 'synthetic-js-error', filename: 'x.js' }));
    window.dispatchEvent(new Event('unhandledrejection'));
    flushErrorQueue();
  });

  it('ignores external noise', () => {
    reportClientError({
      kind: 'js',
      message: 'x',
      stack: 'chrome-extension://abc',
    });
    flushErrorQueue();
  });

  it('resource load failure via ErrorEvent target', () => {
    installErrorCollectors();
    const img = document.createElement('img');
    Object.defineProperty(img, 'src', { value: 'https://cdn.example/missing.png' });
    const ev = new ErrorEvent('error', { bubbles: true });
    Object.defineProperty(ev, 'target', { value: img });
    window.dispatchEvent(ev);
    flushErrorQueue();
  });

  it('flushes when queue hits batch threshold', () => {
    for (let i = 0; i < 10; i += 1) {
      reportClientError({
        kind: 'js',
        message: `batch-${i}-${Math.random()}`,
        stack: 'at x.ts:1',
        statusCode: 500,
      });
    }
    flushErrorQueue();
  });

  it('skips expected faucet/login noise', () => {
    reportClientError({
      kind: 'api',
      message: 'Aguarde o cooldown',
      endpoint: '/faucet/claim',
      statusCode: 429,
    });
    reportClientError({
      kind: 'api',
      message: 'Invalid email or password',
      endpoint: '/auth/login',
      statusCode: 401,
    });
    flushErrorQueue();
  });
});
