/**
 * @vitest-environment jsdom
 * Layouts + tab interactions to cover remaining JSX branches (no full App auth gate).
 */
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '../../helpers/renderWithProviders.js';

vi.mock('../../../src/lib/api.js', async () => {
  const { mockApi } = await import('../../helpers/apiMock.js');
  return {
    api: vi.fn(async (path: string) => mockApi(path)),
    bootstrapSession: vi.fn(async () => null),
    forceReauth: vi.fn(),
  };
});

vi.mock('../../../src/components/Turnstile.js', () => ({ Turnstile: () => null }));
vi.mock('../../../src/components/ModernCaptcha.js', () => ({ ModernCaptcha: () => null }));
vi.mock('../../../src/lib/qr.js', () => ({
  addressQrDataUrl: vi.fn(async () => 'data:image/png;base64,xx'),
}));

import { DocumentationPage } from '../../../src/pages/DocumentationPage.js';
import { ApiDocsPage } from '../../../src/pages/ApiDocsPage.js';
import { DepositPage } from '../../../src/pages/DepositPage.js';
import { WithdrawPage } from '../../../src/pages/WithdrawPage.js';
import { SwapPage } from '../../../src/pages/SwapPage.js';
import { Modal } from '../../../src/components/Modal.js';
import { PasswordInput } from '../../../src/components/PasswordInput.js';
import { CookieConsent } from '../../../src/components/CookieConsent.js';
import { ThemeToggle } from '../../../src/components/ThemeToggle.js';
import { LanguageSwitch } from '../../../src/components/LanguageSwitch.js';
import { AddressQr } from '../../../src/components/AddressQr.js';
import { LegalDocument } from '../../../src/components/LegalDocument.js';
import { AppErrorBoundary } from '../../../src/components/AppErrorBoundary.js';
import { ScrollToTop } from '../../../src/components/ScrollToTop.js';
import { AppLayout } from '../../../src/components/AppLayout.js';
import { AdminLayout } from '../../../src/components/AdminLayout.js';
import { MarketingLayout } from '../../../src/components/MarketingLayout.js';
import { AuthLayout } from '../../../src/components/AuthLayout.js';

beforeAll(async () => {
  const i18n = (await import('../../../src/i18n/index.js')).default;
  await i18n.changeLanguage('en');
});

describe('layouts coverage', () => {
  it('AppLayout / AdminLayout / MarketingLayout / AuthLayout', async () => {
    const app = renderWithProviders(<AppLayout />, { route: '/dashboard', loggedIn: true });
    await waitFor(() => expect(app.container.innerHTML.length).toBeGreaterThan(20), { timeout: 3000 });

    const admin = renderWithProviders(<AdminLayout />, { route: '/admin', loggedIn: true });
    expect(admin.container.innerHTML.length).toBeGreaterThan(10);

    const mkt = renderWithProviders(<MarketingLayout />, { route: '/faq', loggedIn: false });
    expect(mkt.container.innerHTML.length).toBeGreaterThan(10);

    const auth = renderWithProviders(
      <AuthLayout>
        <div>auth-child</div>
      </AuthLayout>,
      { route: '/login', loggedIn: false },
    );
    expect(auth.container.textContent).toMatch(/auth-child/);
  });
});

describe('Documentation tabs cover all sections', () => {
  it('clicks every doc tab', async () => {
    const user = userEvent.setup();
    const { container } = renderWithProviders(<DocumentationPage />, {
      route: '/documentation',
      loggedIn: false,
    });
    const buttons = within(container).getAllByRole('button');
    for (const btn of buttons) {
      await user.click(btn);
    }
    expect(container.innerHTML.length).toBeGreaterThan(100);
  });
});

describe('ApiDocs / Deposit / Withdraw / Swap mount', () => {
  it('renders heavy pages without crashing', async () => {
    for (const [Page, route] of [
      [ApiDocsPage, '/api-docs'],
      [DepositPage, '/deposit'],
      [WithdrawPage, '/withdraw'],
      [SwapPage, '/swap'],
    ] as const) {
      const { container, unmount } = renderWithProviders(<Page />, { route, loggedIn: true });
      await waitFor(() => expect(container.innerHTML.length).toBeGreaterThan(30), { timeout: 3000 });
      unmount();
    }
  });
});

describe('shared components', () => {
  it('Modal / PasswordInput / CookieConsent / Theme / Language / QR / Legal / Boundary / Scroll', async () => {
    const user = userEvent.setup();
    const { rerender, container } = renderWithProviders(
      <Modal open onClose={() => undefined} aria-label="dialog">
        body
      </Modal>,
    );
    expect(container.ownerDocument.body.textContent).toMatch(/body/);

    rerender(
      <div>
        <PasswordInput value="x" onChange={() => undefined} label="Password" />
        <CookieConsent />
        <ThemeToggle />
        <LanguageSwitch />
        <AddressQr address="bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh" />
        <LegalDocument
          titleKey="legal.terms.title"
          updatedKey="legal.terms.updated"
          sectionsKey="legal.terms.sections"
        />
        <ScrollToTop />
        <AppErrorBoundary>
          <div>ok</div>
        </AppErrorBoundary>
      </div>,
    );

    for (const btn of within(container).queryAllByRole('button').slice(0, 15)) {
      await user.click(btn);
    }
  });
});
