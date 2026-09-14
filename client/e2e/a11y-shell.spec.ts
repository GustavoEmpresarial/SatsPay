/**
 * Accessibility shell checks (#18) — landmark/label smoke without axe dependency.
 * Full axe suite can be added later with @axe-core/playwright.
 */
import { test, expect } from '@playwright/test';

const base = process.env.SATSPAY_BASE_URL || 'https://www.satspay.pro';
const enabled = process.env.SATSPAY_E2E_SHELL !== '0';

test.describe('a11y shell', () => {
  test.skip(!enabled, 'SATSPAY_E2E_SHELL=0 skips');

  test('login form controls are labeled / associated', async ({ page }) => {
    await page.goto(`${base}/login`);
    const email = page.locator('input[type="email"], input[name="email"]').first();
    await expect(email).toBeVisible({ timeout: 20_000 });
    const emailId = await email.getAttribute('id');
    const emailName = await email.getAttribute('name');
    const emailAria = await email.getAttribute('aria-label');
    const hasLabel =
      !!emailAria ||
      (!!emailId && (await page.locator(`label[for="${emailId}"]`).count()) > 0) ||
      !!emailName;
    expect(hasLabel).toBe(true);

    const password = page.locator('input[type="password"]').first();
    await expect(password).toBeVisible();
  });

  test('landing has a main landmark or heading hierarchy', async ({ page }) => {
    await page.goto(base);
    const main = page.locator('main, [role="main"], h1').first();
    await expect(main).toBeVisible({ timeout: 20_000 });
  });
});
