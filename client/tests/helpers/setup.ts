import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { afterEach, beforeEach, vi } from 'vitest';
import { ensureJsdomDocument } from './jsdomGuard.js';

beforeEach(() => {
  ensureJsdomDocument();
});

afterEach(() => {
  cleanup();
  vi.clearAllTimers();
  ensureJsdomDocument();
});

// jsdom stubs used by layouts / theme / captcha widgets
if (typeof window !== 'undefined') {
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });

  // Prevent OAuth popup pages from tearing down the jsdom window mid-suite
  window.close = vi.fn() as unknown as typeof window.close;
  // @ts-expect-error jsdom incomplete
  window.scrollTo = vi.fn();
}

vi.mock('framer-motion', async () => {
  const React = await import('react');
  const Pass = ({ children, ...rest }: { children?: React.ReactNode }) =>
    React.createElement('div', rest, children);
  return {
    motion: new Proxy(
      {},
      {
        get: () => Pass,
      },
    ),
    AnimatePresence: ({ children }: { children?: React.ReactNode }) => children,
    useReducedMotion: () => true,
  };
});
