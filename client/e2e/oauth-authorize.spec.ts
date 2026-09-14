/**
 * Playwright E2E smoke — critical OAuth journey.
 * Skips unless SATSPAY_E2E=1 (needs live env + credentials).
 *
 * Run:
 *   SATSPAY_E2E=1 npx playwright test
 */
import { test, expect } from '@playwright/test';

const enabled = process.env.SATSPAY_E2E === '1';
const base = process.env.SATSPAY_BASE_URL || 'https://www.satspay.pro';
const email = process.env.SATSPAY_E2E_EMAIL || '';
const password = process.env.SATSPAY_E2E_PASSWORD || '';
const clientId = process.env.SATSPAY_E2E_CLIENT_ID || '';
const redirectUri =
  process.env.SATSPAY_E2E_REDIRECT_URI || 'https://blockminer.space/api/auth/satspay/callback';

test.describe('OAuth authorize smoke', () => {
  test.skip(!enabled, 'Set SATSPAY_E2E=1 plus credentials to run');

  test('login page loads and authorize shows consent for client', async ({ page }) => {
    await page.goto(`${base}/login`);
    await expect(page.getByRole('heading', { name: /entrar|login|sign in/i })).toBeVisible({
      timeout: 20_000,
    });

    if (email && password) {
      await page.locator('input[type="email"], input[name="email"]').first().fill(email);
      await page.locator('input[type="password"]').first().fill(password);
      await page.getByRole('button', { name: /entrar|login|sign in/i }).first().click();
      await page.waitForURL(/dashboard|oauth|authorize/, { timeout: 30_000 }).catch(() => {});
    }

    if (!clientId) {
      test.info().annotations.push({ type: 'note', description: 'no client id — stopped after login page' });
      return;
    }

    const authUrl =
      `${base}/oauth/authorize?client_id=${encodeURIComponent(clientId)}` +
      `&redirect_uri=${encodeURIComponent(redirectUri)}` +
      `&scope=openid%20profile%20email&response_type=code&state=e2e`;
    await page.goto(authUrl);
    await expect(page.getByText(/quer acessar sua conta/i)).toBeVisible({ timeout: 20_000 });
  });
});
