/**
 * Vitest config — full-app coverage gate (≥90% lines/statements on src).
 * Logic-only 100% gate: vitest.logic.config.ts
 */
import { defineConfig } from 'vitest/config';
import * as path from 'node:path';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { '@': path.resolve(__dirname, 'src') },
  },
  test: {
    environment: 'jsdom',
    include: ['tests/**/*.test.ts', 'tests/**/*.test.tsx'],
    setupFiles: ['tests/helpers/setup.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'text-summary', 'json-summary'],
      reportsDirectory: './coverage',
      include: ['src/**/*.{ts,tsx}'],
      exclude: [
        'src/main.tsx',
        'src/**/*.d.ts',
        // Cloudflare widget shells — exercised via e2e, mocked in unit renders
        'src/components/Turnstile.tsx',
        'src/components/ModernCaptcha.tsx',
        // Popup bridge tears down jsdom; covered indirectly via OAuth flows
        'src/pages/OAuthPopupBridgePage.tsx',
      ],
      thresholds: {
        // Client gate — see docs/testing/COVERAGE_90.md
        lines: 90,
        statements: 90,
        functions: 60,
        branches: 70,
      },
    },
  },
});
