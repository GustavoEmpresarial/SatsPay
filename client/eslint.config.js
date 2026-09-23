import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import reactHooks from 'eslint-plugin-react-hooks';
import globals from 'globals';

export default tseslint.config(
  {
    ignores: ['dist/**', 'node_modules/**', 'eslint.config.js', 'vite.config.ts', 'vitest.config.ts', 'postcss.config.js', 'tailwind.config.ts'],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    // Plain browser scripts served as-is (merchant SDK + pre-paint theme boot).
    // They are public URLs embedded by third parties: lint them, but as ES5-ish
    // browser code, and let them swallow storage/DOM errors on purpose.
    files: ['public/**/*.js'],
    languageOptions: { sourceType: 'script', globals: { ...globals.browser } },
    rules: {
      '@typescript-eslint/no-unused-vars': ['error', { caughtErrors: 'none', varsIgnorePattern: '^_' }],
      '@typescript-eslint/no-this-alias': 'off',
      'no-empty': ['error', { allowEmptyCatch: true }],
    },
  },
  {
    files: ['src/**/*.{ts,tsx}', 'tests/**/*.{ts,tsx}'],
    languageOptions: {
      parserOptions: {
        ecmaFeatures: { jsx: true },
      },
    },
    plugins: {
      'react-hooks': reactHooks,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_', caughtErrorsIgnorePattern: '^_' },
      ],
      '@typescript-eslint/no-unused-expressions': 'error',
      'no-console': 'error',
      'no-unreachable': 'error',
    },
  },
);
