/**
 * @vitest-environment jsdom
 * reportClientError no-ops when window is undefined (node env).
 */
import { describe, expect, it } from 'vitest';
import { afterEach, beforeEach, vi } from 'vitest';
import {
  addBreadcrumb,
  flushErrorQueue,
  getBreadcrumbs,
  isCriticalApiPath,
  shouldReportApiStatus,
  CLIENT_ERROR_KINDS,
  isExternalNoise,
  isExpectedApiNoise,
  isClientErrorKind,
  reportClientError,
} from '../../../src/lib/reportError.js';

describe('reportError policy helpers', () => {
  it('tracks breadcrumbs with cap', () => {
    for (let i = 0; i < 45; i++) addBreadcrumb('navigation', `step-${i}`);
    const crumbs = getBreadcrumbs();
    expect(crumbs.length).toBeLessThanOrEqual(40);
    expect(crumbs[crumbs.length - 1]?.message).toContain('step-');
  });

  it('classifies critical API paths', () => {
    expect(isCriticalApiPath('/auth/login')).toBe(true);
    expect(isCriticalApiPath('/oauth/apps')).toBe(true);
    expect(isCriticalApiPath('/public/pay/x')).toBe(true);
    expect(isCriticalApiPath('/marketing')).toBe(false);
  });

  it('shouldReportApiStatus respects status and path', () => {
    expect(shouldReportApiStatus(0, '/x')).toBe(true);
    expect(shouldReportApiStatus(500, '/x')).toBe(true);
    expect(shouldReportApiStatus(429, '/misc')).toBe(true);
    expect(shouldReportApiStatus(404, '/auth/me')).toBe(true);
    expect(shouldReportApiStatus(404, '/unknown-marketing')).toBe(false);
  });

  it('exports error kind constants', () => {
    expect(CLIENT_ERROR_KINDS).toContain('oauth');
    expect(CLIENT_ERROR_KINDS.length).toBeGreaterThan(5);
  });

  it('isExternalNoise filters extension and analytics noise', () => {
    expect(isExternalNoise('chrome-extension://abc', '')).toBe(true);
    expect(isExternalNoise('', 'Invalid email or password')).toBe(true);
    expect(isExternalNoise('at App.tsx:1', 'real bug')).toBe(false);
    expect(isExternalNoise('', 'Mutation unknown: platform inventory for Btc is insufficient')).toBe(
      true,
    );
    expect(isExternalNoise('', 'Minified React error #311; visit https://reactjs.org')).toBe(true);
  });

  it('isExpectedApiNoise for faucet and auth', () => {
    expect(isExpectedApiNoise(429, '/faucet/claim')).toBe(true);
    expect(isExpectedApiNoise(401, '/auth/login')).toBe(true);
    expect(isExpectedApiNoise(400, '/auth/login')).toBe(true);
    expect(isExpectedApiNoise(400, '/faucet/claim', 'platform inventory for Pol is insufficient')).toBe(
      true,
    );
    expect(isExpectedApiNoise(500, '/wallet')).toBe(false);
  });

  it('isClientErrorKind validates kinds', () => {
    expect(isClientErrorKind('oauth')).toBe(true);
    expect(isClientErrorKind('not-a-kind')).toBe(false);
  });
});

describe('reportClientError queue', () => {
  beforeEach(() => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response('{}', { status: 204 })));
    // Force the fetch transport path (sendBeacon short-circuits postBatch).
    vi.stubGlobal('navigator', {
      ...navigator,
      sendBeacon: undefined,
      userAgent: 'vitest',
      language: 'en',
      onLine: true,
    });
  });

  afterEach(() => {
    flushErrorQueue();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it('queues and flushes reports', async () => {
    vi.useFakeTimers();
    reportClientError({
      kind: 'api',
      message: `test flush batch ${Math.random()}`,
      endpoint: '/swap',
      statusCode: 500,
    });
    await vi.advanceTimersByTimeAsync(2600);
    expect(fetch).toHaveBeenCalled();
  });
});
