/**
 * Logic-only coverage gate (100% on pure money/auth helpers).
 * Run: npx vitest run --config vitest.logic.config.ts --coverage
 */
import { defineConfig } from 'vitest/config';
import * as path from 'node:path';

const LOGIC_GLOBS = [
  'src/lib/amountInput.ts',
  'src/lib/authValidation.ts',
  'src/lib/chainExplorers.ts',
  'src/lib/qr.ts',
  'src/lib/returnTo.ts',
  'src/lib/formatError.ts',
  'src/lib/captchaActions.ts',
  'src/lib/auditActions.ts',
  'src/lib/admin.ts',
  'src/shared/coins.ts',
];

export default defineConfig({
  resolve: {
    alias: { '@': path.resolve(__dirname, 'src') },
  },
  test: {
    environment: 'node',
    include: [
      'tests/unit/lib/**/*.test.ts',
      'tests/unit/shared/**/*.test.ts',
      'tests/unit/pages/login/returnTo.test.ts',
      'tests/unit/pages/register/authValidation.test.ts',
      'tests/unit/property/**/*.test.ts',
      'tests/unit/acceptance/**/*.test.ts',
    ],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'text-summary', 'json-summary'],
      reportsDirectory: './coverage-logic',
      include: LOGIC_GLOBS,
      exclude: ['**/*.d.ts'],
      thresholds: {
        lines: 100,
        functions: 100,
        branches: 95,
        statements: 100,
      },
    },
  },
});
