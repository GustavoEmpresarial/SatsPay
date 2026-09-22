import { useEffect, useState, type JSX } from 'react';
import { Navigate, Route, Routes, useLocation, useParams } from 'react-router-dom';
import { useAuthStore } from './stores/auth.js';
import type { PublicUser } from '@/shared';
import { useAdminStore } from './stores/admin.js';
import { api, bootstrapSession, forceReauth } from './lib/api.js';
import { AppLayout } from './components/AppLayout.js';
import { AdminLayout } from './components/AdminLayout.js';
import { MarketingLayout } from './components/MarketingLayout.js';
import { ScrollToTop } from './components/ScrollToTop.js';
import { LandingPage } from './pages/LandingPage.js';
import { LoginPage } from './pages/LoginPage.js';
import { RegisterPage } from './pages/RegisterPage.js';
import { DashboardPage } from './pages/DashboardPage.js';
import { WalletsPage } from './pages/WalletsPage.js';
import { FaucetPage } from './pages/FaucetPage.js';
import { DepositPage } from './pages/DepositPage.js';
import { WithdrawPage } from './pages/WithdrawPage.js';
import { SettingsPage } from './pages/SettingsPage.js';
import { SupportPage } from './pages/SupportPage.js';
import { ApiDocsPage } from './pages/ApiDocsPage.js';
import { ApiKeysPage } from './pages/ApiKeysPage.js';
import { DeveloperWalletsPage } from './pages/DeveloperWalletsPage.js';
import { StakePage } from './pages/StakePage.js';
import { SwapPage } from './pages/SwapPage.js';
import { LendPage } from './pages/LendPage.js';
import { AnalyticsPage } from './pages/AnalyticsPage.js';
import { FaucetListPage } from './pages/FaucetListPage.js';
import { MerchantSitesPage } from './pages/MerchantSitesPage.js';
import { AdminLoginPage } from './pages/AdminLoginPage.js';
import { AdminOverviewPage } from './pages/AdminOverviewPage.js';
import { AdminUsersPage } from './pages/AdminUsersPage.js';
import { AdminWithdrawalsPage } from './pages/AdminWithdrawalsPage.js';
import { AdminMerchantsPage } from './pages/AdminMerchantsPage.js';
import { AdminFaucetSitesPage } from './pages/AdminFaucetSitesPage.js';
import { AdminSupportPage } from './pages/AdminSupportPage.js';
import { AdminStakePage } from './pages/AdminStakePage.js';
import { AdminTelemetryPage } from './pages/AdminTelemetryPage.js';
import { PrivacyPage } from './pages/PrivacyPage.js';
import { TermsPage } from './pages/TermsPage.js';
import { CookiesPage } from './pages/CookiesPage.js';
import { DocumentationPage } from './pages/DocumentationPage.js';
import { FeaturesPage } from './pages/FeaturesPage.js';
import { CoinsPage } from './pages/CoinsPage.js';
import { FaqPage } from './pages/FaqPage.js';
import { SecurityPage } from './pages/SecurityPage.js';
import { GuidesPage } from './pages/GuidesPage.js';
import { SitemapPage } from './pages/SitemapPage.js';
import { StatusPage } from './pages/StatusPage.js';
import { LlmPage } from './pages/LlmPage.js';
import { CheckoutPage } from './pages/CheckoutPage.js';
import { MerchantDepositsPage } from './pages/MerchantDepositsPage.js';
import { MerchantDashboardPage } from './pages/MerchantDashboardPage.js';
import { ReferralPage } from './pages/ReferralPage.js';
import { AirdropPage } from './pages/AirdropPage.js';
import { OAuthAuthorizePage } from './pages/OAuthAuthorizePage.js';
import { OAuthPopupBridgePage } from './pages/OAuthPopupBridgePage.js';
import { OAuthAppsPage } from './pages/OAuthAppsPage.js';
import { addBreadcrumb } from './lib/reportError.js';

function ReferralRedirect() {
  const { code } = useParams();
  return <Navigate to={`/register?r=${encodeURIComponent(code || '')}`} replace />;
}

function RootRoute() {
  const user = useAuthStore((s) => s.user);
  if (user) return <Navigate to="/dashboard" replace />;
  return <LandingPage />;
}

function RequireAuth({ children }: { children: JSX.Element }) {
  const user = useAuthStore((s) => s.user);
  const accessToken = useAuthStore((s) => s.accessToken);
  const updateUser = useAuthStore((s) => s.updateUser);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      // Persist may still be hydrating user; wait a tick for zustand persist.
      await new Promise<void>((resolve) => {
        if (useAuthStore.persist.hasHydrated()) {
          resolve();
          return;
        }
        const unsub = useAuthStore.persist.onFinishHydration(() => {
          unsub();
          resolve();
        });
      });
      if (cancelled) return;

      const hasUser = Boolean(useAuthStore.getState().user);
      if (!hasUser) {
        if (!cancelled) setReady(true);
        return;
      }

      // Access JWT is memory-only — restore from HttpOnly refresh cookie after reload.
      if (!useAuthStore.getState().accessToken) {
        const ok = await bootstrapSession();
        if (cancelled) return;
        if (!ok) {
          setReady(true);
          return;
        }
      }

      if (!cancelled) setReady(true);

      api<{ user: PublicUser }>('/auth/me')
        .then((res) => {
          if (!cancelled && res.user) updateUser(res.user);
        })
        .catch((err) => {
          const status = typeof err === 'object' && err && 'status' in err ? Number((err as { status: number }).status) : 0;
          if (status === 401 || status === 403) {
            forceReauth();
          }
        });
    })();
    return () => {
      cancelled = true;
    };
  }, [user, accessToken, updateUser]);

  if (!ready) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-canvas">
        <div className="h-6 w-6 animate-spin rounded-full border-2 border-primary border-t-transparent" />
      </div>
    );
  }

  if (!user) return <Navigate to="/login" replace />;
  return children;
}

function RequireAdmin({ children }: { children: JSX.Element }) {
  const user = useAuthStore((s) => s.user);
  const admin = useAdminStore((s) => s.admin);
  // Hooks must always run — never short-circuit useStore calls (React #311).
  const adminToken = useAdminStore((s) => s.accessToken);
  const userToken = useAuthStore((s) => s.accessToken);
  const accessToken = adminToken || userToken;
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      await new Promise<void>((resolve) => {
        const done = () => resolve();
        if (useAdminStore.persist.hasHydrated() && useAuthStore.persist.hasHydrated()) {
          done();
          return;
        }
        let pending = 2;
        const tick = () => {
          pending -= 1;
          if (pending <= 0) done();
        };
        if (useAdminStore.persist.hasHydrated()) tick();
        else useAdminStore.persist.onFinishHydration(tick);
        if (useAuthStore.persist.hasHydrated()) tick();
        else useAuthStore.persist.onFinishHydration(tick);
      });
      if (cancelled) return;

      const isAdmin = Boolean(useAdminStore.getState().admin || useAuthStore.getState().user?.role === 'ADMIN');
      if (!isAdmin) {
        if (!cancelled) setReady(true);
        return;
      }
      if (!useAuthStore.getState().accessToken && !useAdminStore.getState().accessToken) {
        await bootstrapSession();
      }
      if (!cancelled) setReady(true);
    })();
    return () => {
      cancelled = true;
    };
  }, [admin, user, accessToken]);

  if (!ready) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-canvas">
        <div className="h-6 w-6 animate-spin rounded-full border-2 border-primary border-t-transparent" />
      </div>
    );
  }

  const isAdmin = Boolean(admin || user?.role === 'ADMIN');
  return isAdmin ? children : <Navigate to="/admin/login" replace />;
}

export function App() {
  const location = useLocation();

  useEffect(() => {
    addBreadcrumb('navigation', `Navigated to ${location.pathname}${location.search}`);
  }, [location.pathname, location.search]);

  return (
    <>
      <ScrollToTop />
      <Routes>
        <Route path="/" element={<RootRoute />} />
        <Route path="/welcome" element={<LandingPage />} />
        <Route path="/login" element={<LoginPage />} />
        <Route path="/register" element={<RegisterPage />} />
        <Route path="/r/:code" element={<ReferralRedirect />} />
        <Route path="/pay/:id" element={<CheckoutPage />} />
        <Route path="/oauth/authorize" element={<OAuthAuthorizePage />} />
        <Route path="/oauth/bridge" element={<OAuthPopupBridgePage />} />
        <Route path="/oauth/popup-done" element={<OAuthPopupBridgePage />} />

        <Route element={<MarketingLayout />}>
          <Route path="/features" element={<FeaturesPage />} />
          <Route path="/coins" element={<CoinsPage />} />
          <Route path="/faq" element={<FaqPage />} />
          <Route path="/privacy" element={<PrivacyPage />} />
          <Route path="/terms" element={<TermsPage />} />
          <Route path="/cookies" element={<CookiesPage />} />
          <Route path="/security" element={<SecurityPage />} />
          <Route path="/documentation" element={<DocumentationPage />} />
          <Route path="/guides" element={<GuidesPage />} />
          <Route path="/sitemap" element={<SitemapPage />} />
          <Route path="/status" element={<StatusPage />} />
          <Route path="/llm" element={<LlmPage />} />
          <Route path="/demo" element={<Navigate to="/pay/demo" replace />} />
          <Route
            path="/api"
            element={
              <div className="mx-auto max-w-6xl px-4 py-8 md:px-6 md:py-10">
                <ApiDocsPage />
              </div>
            }
          />
        </Route>

        <Route path="/admin/login" element={<AdminLoginPage />} />
        <Route
          path="/admin"
          element={
            <RequireAdmin>
              <AdminLayout />
            </RequireAdmin>
          }
        >
          <Route index element={<AdminOverviewPage />} />
          <Route path="users" element={<AdminUsersPage />} />
          <Route path="telemetry" element={<AdminTelemetryPage />} />
          <Route path="withdrawals" element={<AdminWithdrawalsPage />} />
          <Route path="merchants" element={<AdminMerchantsPage />} />
          <Route path="faucet-sites" element={<AdminFaucetSitesPage />} />
          <Route path="support" element={<AdminSupportPage />} />
          <Route path="stake" element={<AdminStakePage />} />
        </Route>

        <Route
          element={
            <RequireAuth>
              <AppLayout />
            </RequireAuth>
          }
        >
          <Route path="/dashboard" element={<DashboardPage />} />
          <Route path="/merchant/dashboard" element={<MerchantDashboardPage />} />
          <Route path="/merchant" element={<MerchantDashboardPage />} />
          <Route path="/faucetlist" element={<FaucetListPage />} />
          <Route path="/merchant/sites" element={<MerchantSitesPage />} />
          <Route path="/merchant/deposits" element={<MerchantDepositsPage />} />
          <Route path="/wallets" element={<WalletsPage />} />
          <Route path="/faucet" element={<FaucetPage />} />
          <Route path="/deposit" element={<DepositPage />} />
          <Route path="/withdraw" element={<WithdrawPage />} />
          <Route path="/stake" element={<StakePage />} />
          <Route path="/lend" element={<LendPage />} />
          <Route path="/swap" element={<SwapPage />} />
          <Route path="/analytics" element={<AnalyticsPage />} />
          <Route path="/settings" element={<SettingsPage />} />
          <Route path="/support" element={<SupportPage />} />
          <Route path="/documentation" element={<DocumentationPage />} />
          <Route path="/status" element={<StatusPage />} />
          <Route path="/docs" element={<ApiDocsPage />} />
          <Route path="/api-docs" element={<Navigate to="/docs" replace />} />
          <Route path="/api-keys" element={<ApiKeysPage />} />
          <Route path="/developer/apps" element={<OAuthAppsPage />} />
          <Route path="/developer/oauth" element={<OAuthAppsPage />} />
          <Route path="/oauth/apps" element={<OAuthAppsPage />} />
          <Route path="/developer-wallets" element={<DeveloperWalletsPage />} />
          <Route path="/referrals" element={<ReferralPage />} />
          <Route path="/airdrop" element={<AirdropPage />} />
          <Route path="/merchant/demo" element={<Navigate to="/pay/demo" replace />} />
        </Route>

        <Route path="/r/:code" element={<ReferralRedirect />} />

        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </>
  );
}
