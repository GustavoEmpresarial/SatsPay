import { describe, expect, it } from 'vitest';
import {
  HTTP_SERVER_ERROR_MIN,
  isClientErrorKind,
  isCriticalApiPath,
  isExternalNoise,
  shouldReportApiStatus,
} from '../../../src/lib/reportError.js';

describe('isExternalNoise', () => {
  it('drops browser-extension and analytics frames', () => {
    expect(isExternalNoise('chrome-extension://abc', 'boom')).toBe(true);
    expect(isExternalNoise(undefined, 'cloudflareinsights.com beacon')).toBe(true);
    expect(isExternalNoise('at App.tsx:10', 'TypeError: x is not a function')).toBe(false);
  });
});

describe('shouldReportApiStatus', () => {
  it('reports network and HTTP server errors', () => {
    expect(shouldReportApiStatus(0)).toBe(true);
    expect(shouldReportApiStatus(HTTP_SERVER_ERROR_MIN)).toBe(true);
    expect(shouldReportApiStatus(503)).toBe(true);
  });

  it('reports critical 4xx always (415/429) and auth-path 4xx', () => {
    expect(shouldReportApiStatus(415)).toBe(true);
    expect(shouldReportApiStatus(429)).toBe(true);
    // Bad credentials / validation on login are expected product noise
    expect(shouldReportApiStatus(400, '/auth/login')).toBe(false);
    expect(shouldReportApiStatus(401, '/oauth/authorize')).toBe(true);
    expect(shouldReportApiStatus(404, '/some/random')).toBe(false);
  });
});

describe('isCriticalApiPath', () => {
  it('flags auth wallet oauth faucet', () => {
    expect(isCriticalApiPath('/auth/login')).toBe(true);
    expect(isCriticalApiPath('/wallet?kind=PERSONAL')).toBe(true);
    expect(isCriticalApiPath('/docs')).toBe(false);
  });
});

describe('isClientErrorKind', () => {
  it('accepts the expanded collector kinds', () => {
    expect(isClientErrorKind('js')).toBe(true);
    expect(isClientErrorKind('api')).toBe(true);
    expect(isClientErrorKind('auth')).toBe(true);
    expect(isClientErrorKind('oauth')).toBe(true);
    expect(isClientErrorKind('query')).toBe(true);
    expect(isClientErrorKind('sql')).toBe(false);
  });
});
