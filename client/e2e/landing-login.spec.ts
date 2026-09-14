/**
 * Playwright shell smoke — landing + login render without auth credentials.
 * Always runs against SATSPAY_BASE_URL (default prod) for public pages only.
 *
 * Full login with credentials: set SATSPAY_E2E=1 + SATSPAY_E2E_EMAIL/PASSWORD
 * (see oauth-authorize.spec.ts).
 */
import { test, expect } from '@playwright/test';

const base = process.env.SATSPAY_BASE_URL || 'https://www.satspay.pro';
const shellEnabled = process.env.SATSPAY_E2E_SHELL !== '0';

test.describe('Landing + login shell', () => {
  test.skip(!shellEnabled, 'Set SATSPAY_E2E_SHELL=0 to skip public shell checks');

  test('landing shows brand and primary CTA', async ({ page }) => {
    await page.goto(base);
    await expect(page.getByText(/satspay|bitcosats/i).first()).toBeVisible({ timeout: 20_000 });
    await expect(
      page.getByRole('link', { name: /entrar|login|começar|get started|sign in|criar/i }).first(),
    ).toBeVisible({ timeout: 15_000 });
  });

  test('login form fields are present', async ({ page }) => {
    await page.goto(`${base}/login`);
    await expect(page.locator('input[type="email"], input[name="email"]').first()).toBeVisible({
      timeout: 20_000,
    });
    await expect(page.locator('input[type="password"]').first()).toBeVisible();
    await expect(page.getByRole('button', { name: /entrar|login|sign in/i }).first()).toBeVisible();
  });
});
