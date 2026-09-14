#!/usr/bin/env node
/**
 * Generates docs/pages/<slug>/ + client/tests/unit/pages/<slug>/*.structure.test.ts
 * for every SatsPay page module. Idempotent: skips files that already exist
 * unless --force is passed.
 *
 * Usage: node scripts/generate_page_coverage.mjs [--force]
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, '..');
const force = process.argv.includes('--force');

/** @typedef {{ slug: string, title: string, component: string, routes: string[], auth: 'public'|'user'|'admin'|'mixed', apis: string[], batch: string, notes: string }} PageMod */

/** @type {PageMod[]} */
const PAGES = [
  // lote 1 already hand-written — listed for coverage matrix only
  { slug: 'landing', title: 'Landing / Index', component: 'LandingPage.tsx', routes: ['/', '/welcome'], auth: 'public', apis: [], batch: '1-auth', notes: 'Redirect se autenticado' },
  { slug: 'login', title: 'Login', component: 'LoginPage.tsx', routes: ['/login'], auth: 'public', apis: ['POST /v1/auth/login'], batch: '1-auth', notes: 'OTP + Turnstile' },
  { slug: 'register', title: 'Register', component: 'RegisterPage.tsx', routes: ['/register', '/r/:code'], auth: 'public', apis: ['POST /v1/auth/register'], batch: '1-auth', notes: 'Referral + terms' },

  // marketing
  { slug: 'features', title: 'Features', component: 'FeaturesPage.tsx', routes: ['/features'], auth: 'public', apis: [], batch: '2-marketing', notes: 'MarketingLayout' },
  { slug: 'coins', title: 'Coins', component: 'CoinsPage.tsx', routes: ['/coins'], auth: 'public', apis: [], batch: '2-marketing', notes: 'Lista COINS' },
  { slug: 'faq', title: 'FAQ', component: 'FaqPage.tsx', routes: ['/faq'], auth: 'public', apis: [], batch: '2-marketing', notes: 'i18n FAQ' },
  { slug: 'privacy', title: 'Privacy', component: 'PrivacyPage.tsx', routes: ['/privacy'], auth: 'public', apis: [], batch: '2-marketing', notes: 'Legal' },
  { slug: 'terms', title: 'Terms', component: 'TermsPage.tsx', routes: ['/terms'], auth: 'public', apis: [], batch: '2-marketing', notes: 'Legal' },
  { slug: 'cookies', title: 'Cookies', component: 'CookiesPage.tsx', routes: ['/cookies'], auth: 'public', apis: [], batch: '2-marketing', notes: 'Legal' },
  { slug: 'security-page', title: 'Security (marketing)', component: 'SecurityPage.tsx', routes: ['/security'], auth: 'public', apis: [], batch: '2-marketing', notes: 'Página pública security' },
  { slug: 'documentation', title: 'Documentation (in-app)', component: 'DocumentationPage.tsx', routes: ['/documentation'], auth: 'mixed', apis: [], batch: '2-marketing', notes: 'Público + autenticado' },
  { slug: 'guides', title: 'Guides', component: 'GuidesPage.tsx', routes: ['/guides'], auth: 'public', apis: [], batch: '2-marketing', notes: '' },
  { slug: 'sitemap', title: 'Sitemap', component: 'SitemapPage.tsx', routes: ['/sitemap'], auth: 'public', apis: [], batch: '2-marketing', notes: '' },
  { slug: 'status', title: 'Status', component: 'StatusPage.tsx', routes: ['/status'], auth: 'mixed', apis: ['GET /v1/status/nodes'], batch: '2-marketing', notes: 'Health checks' },
  { slug: 'llm', title: 'LLM', component: 'LlmPage.tsx', routes: ['/llm'], auth: 'public', apis: [], batch: '2-marketing', notes: '' },
  { slug: 'api-docs', title: 'API Docs', component: 'ApiDocsPage.tsx', routes: ['/api', '/docs', '/api-docs'], auth: 'mixed', apis: [], batch: '2-marketing', notes: 'Redirect /api-docs→/docs' },

  // app core
  { slug: 'dashboard', title: 'Dashboard', component: 'DashboardPage.tsx', routes: ['/dashboard'], auth: 'user', apis: ['GET /v1/wallet', 'GET /v1/swap/prices'], batch: '3-app', notes: 'Home autenticada' },
  { slug: 'wallets', title: 'Wallets', component: 'WalletsPage.tsx', routes: ['/wallets'], auth: 'user', apis: ['GET /v1/wallet'], batch: '3-app', notes: 'Saldos PERSONAL' },
  { slug: 'deposit', title: 'Deposit', component: 'DepositPage.tsx', routes: ['/deposit'], auth: 'user', apis: ['POST /v1/deposits/address', 'GET /v1/deposits'], batch: '3-app', notes: 'HD address' },
  { slug: 'withdraw', title: 'Withdraw', component: 'WithdrawPage.tsx', routes: ['/withdraw'], auth: 'user', apis: ['POST /v1/withdrawals'], batch: '3-app', notes: 'Idempotency' },
  { slug: 'faucet', title: 'Faucet', component: 'FaucetPage.tsx', routes: ['/faucet'], auth: 'user', apis: ['POST /v1/faucet/claim/:coin'], batch: '3-app', notes: 'Cooldown 11h + Turnstile' },
  { slug: 'swap', title: 'Swap', component: 'SwapPage.tsx', routes: ['/swap'], auth: 'user', apis: ['GET /v1/swap/quote', 'POST /v1/swap', 'GET /v1/swap/history'], batch: '3-app', notes: 'HOUSE; SwapKit off' },
  { slug: 'stake', title: 'Stake', component: 'StakePage.tsx', routes: ['/stake'], auth: 'user', apis: ['POST /v1/stake', 'POST /v1/stake/:id/claim'], batch: '3-app', notes: '' },
  { slug: 'lend', title: 'Lend', component: 'LendPage.tsx', routes: ['/lend'], auth: 'user', apis: ['POST /v1/lend/*'], batch: '3-app', notes: '' },
  { slug: 'analytics', title: 'Analytics', component: 'AnalyticsPage.tsx', routes: ['/analytics'], auth: 'user', apis: ['GET /v1/wallet', 'GET /v1/swap/prices'], batch: '3-app', notes: '' },
  { slug: 'settings', title: 'Settings', component: 'SettingsPage.tsx', routes: ['/settings'], auth: 'user', apis: ['GET /v1/auth/me', 'PATCH /v1/auth/username', 'GET /v1/auth/security-logs'], batch: '3-app', notes: '' },
  { slug: 'support', title: 'Support', component: 'SupportPage.tsx', routes: ['/support'], auth: 'user', apis: [], batch: '3-app', notes: '' },
  { slug: 'referrals', title: 'Referrals', component: 'ReferralPage.tsx', routes: ['/referrals'], auth: 'user', apis: ['GET /v1/referral/*'], batch: '3-app', notes: '' },
  { slug: 'airdrop', title: 'Airdrop', component: 'AirdropPage.tsx', routes: ['/airdrop'], auth: 'user', apis: ['GET /v1/airdrop/*'], batch: '3-app', notes: 'Tiers' },
  { slug: 'api-keys', title: 'API Keys', component: 'ApiKeysPage.tsx', routes: ['/api-keys'], auth: 'user', apis: ['GET/POST /v1/api-keys'], batch: '3-app', notes: 'HMAC public API' },
  { slug: 'developer-wallets', title: 'Developer Wallets', component: 'DeveloperWalletsPage.tsx', routes: ['/developer-wallets'], auth: 'user', apis: ['GET /v1/wallet'], batch: '3-app', notes: '' },

  // merchant / checkout / oauth
  { slug: 'checkout', title: 'Checkout (pay)', component: 'CheckoutPage.tsx', routes: ['/pay/:id'], auth: 'public', apis: ['GET /v1/public/pay/:id'], batch: '4-merchant', notes: 'Invoice pública' },
  { slug: 'merchant-dashboard', title: 'Merchant Dashboard', component: 'MerchantDashboardPage.tsx', routes: ['/merchant', '/merchant/dashboard'], auth: 'user', apis: ['GET /v1/merchant/*'], batch: '4-merchant', notes: '' },
  { slug: 'merchant-sites', title: 'Merchant Sites', component: 'MerchantSitesPage.tsx', routes: ['/merchant/sites'], auth: 'user', apis: ['GET /v1/merchant/*'], batch: '4-merchant', notes: '' },
  { slug: 'merchant-deposits', title: 'Merchant Deposits', component: 'MerchantDepositsPage.tsx', routes: ['/merchant/deposits'], auth: 'user', apis: ['GET /v1/merchant/deposits'], batch: '4-merchant', notes: '' },
  { slug: 'faucetlist', title: 'Faucetlist', component: 'FaucetListPage.tsx', routes: ['/faucetlist'], auth: 'user', apis: ['GET /v1/faucetlist'], batch: '4-merchant', notes: 'Diretório' },
  { slug: 'oauth-authorize', title: 'OAuth Authorize', component: 'OAuthAuthorizePage.tsx', routes: ['/oauth/authorize'], auth: 'mixed', apis: ['GET /v1/oauth/authorize/info', 'POST /v1/oauth/authorize'], batch: '4-merchant', notes: 'Consent' },
  { slug: 'oauth-bridge', title: 'OAuth Popup Bridge', component: 'OAuthPopupBridgePage.tsx', routes: ['/oauth/bridge', '/oauth/popup-done'], auth: 'public', apis: [], batch: '4-merchant', notes: 'Popup bridge' },
  { slug: 'oauth-apps', title: 'OAuth Apps', component: 'OAuthAppsPage.tsx', routes: ['/developer/apps', '/developer/oauth', '/oauth/apps'], auth: 'user', apis: ['GET/POST /v1/oauth/apps'], batch: '4-merchant', notes: '' },

  // admin
  { slug: 'admin-login', title: 'Admin Login', component: 'AdminLoginPage.tsx', routes: ['/admin/login'], auth: 'public', apis: ['POST /v1/auth/admin/login'], batch: '5-admin', notes: 'Role ADMIN' },
  { slug: 'admin-overview', title: 'Admin Overview', component: 'AdminOverviewPage.tsx', routes: ['/admin'], routeMatchers: ['path="/admin"', 'AdminOverviewPage'], auth: 'admin', apis: ['GET /v1/admin/*'], batch: '5-admin', notes: 'index route' },
  { slug: 'admin-telemetry', title: 'Admin Telemetry', component: 'AdminTelemetryPage.tsx', routes: ['/admin/telemetry'], routeMatchers: ['path="telemetry"', 'AdminTelemetryPage'], auth: 'admin', apis: ['GET /v1/admin/telemetry/*'], batch: '5-admin', notes: 'Nested under /admin' },
  { slug: 'admin-withdrawals', title: 'Admin Withdrawals', component: 'AdminWithdrawalsPage.tsx', routes: ['/admin/withdrawals'], routeMatchers: ['path="withdrawals"', 'AdminWithdrawalsPage'], auth: 'admin', apis: ['POST /v1/admin/withdrawals/:id/approve'], batch: '5-admin', notes: 'Nested under /admin' },
  { slug: 'admin-merchants', title: 'Admin Merchants', component: 'AdminMerchantsPage.tsx', routes: ['/admin/merchants'], routeMatchers: ['path="merchants"', 'AdminMerchantsPage'], auth: 'admin', apis: ['POST /v1/admin/merchants/:id/approve'], batch: '5-admin', notes: 'Nested under /admin' },
  { slug: 'admin-faucet-sites', title: 'Admin Faucet Sites', component: 'AdminFaucetSitesPage.tsx', routes: ['/admin/faucet-sites'], routeMatchers: ['path="faucet-sites"', 'AdminFaucetSitesPage'], auth: 'admin', apis: ['POST /v1/admin/faucetlist/:id/*'], batch: '5-admin', notes: 'Nested under /admin' },
  { slug: 'admin-stake', title: 'Admin Stake', component: 'AdminStakePage.tsx', routes: ['/admin/stake'], routeMatchers: ['path="stake"', 'AdminStakePage'], auth: 'admin', apis: ['GET /v1/admin/*'], batch: '5-admin', notes: 'Nested under /admin' },
];

const HAND_WRITTEN = new Set(['landing', 'login', 'register']);

function writeFile(filePath, content) {
  if (fs.existsSync(filePath) && !force) return false;
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, content);
  return true;
}

function docsFor(p) {
  const routesTable = p.routes.map((r) => `| \`${r}\` | \`${p.component}\` |`).join('\n');
  const apis = p.apis.length ? p.apis.map((a) => `- \`${a}\``).join('\n') : '- (nenhum endpoint específico no mount)';
  const readme = `# ${p.title}

## Rotas

| Path | Componente |
|------|------------|
${routesTable}

**Código:** [\`client/src/pages/${p.component}\`](../../../client/src/pages/${p.component})  
**Auth:** \`${p.auth}\` · **Lote:** \`${p.batch}\`

## Documentos

- [overview.md](overview.md)
- [routes-and-api.md](routes-and-api.md)
- [test-checklist.md](test-checklist.md)

${p.notes ? `## Notas\n\n${p.notes}\n` : ''}`;

  const overview = `# ${p.title} — Overview

## Papel

Página **${p.title}** (\`${p.component}\`).

- Auth gate: **${p.auth}**
- Rotas: ${p.routes.map((r) => `\`${r}\``).join(', ')}
${p.notes ? `- Nota: ${p.notes}` : ''}

## Comportamento esperado

1. Usuário navega para a rota.
2. Layout adequado renderiza (\`MarketingLayout\` / \`AppLayout\` / \`AdminLayout\` / standalone).
3. Dados carregam via React Query / fetch quando aplicável.
4. Erros de API passam por \`formatApiError\` / telemetria quando aplicável.

## i18n

Preferir chaves em \`client/src/i18n/locales/{pt,en}.json\` quando a página for traduzida.

## Segurança

- Respeitar gate \`${p.auth}\` (RequireAuth / RequireAdmin / público).
- Não persistir segredos em localStorage.
- Validar inputs antes de POST.
`;

  const routesApi = `# ${p.title} — Rotas e API

## Client

${p.routes.map((r) => `- \`${r}\``).join('\n')}

## Backend

${apis}

## Critérios mínimos

- Rota montada em \`App.tsx\`
- Componente exporta a page function
- Sem crash no mount sem dados (estado vazio / loading)
`;

  const checklist = `# ${p.title} — Checklist de testes

## Automatizado (lote coverage)

| Tipo | Local |
|------|-------|
| Unit — estrutura | \`client/tests/unit/pages/${p.slug}/${p.slug}.structure.test.ts\` |

## Planejado (aprofundar cobertura)

| Tipo | Caso |
|------|------|
| Unit | Regras de negócio / validação locais |
| API / smoke | Happy path + negativos |
| E2E | Fluxo crítico na UI |
| Security | Authz, IDOR, input validation |
| Observability | Erros reportados sem vazamento de segredo |

## Aceite

- [ ] Rota(s) registradas em \`App.tsx\`
- [ ] Página existe e exporta componente
- [ ] Docs overview + API atualizados
- [ ] Teste de estrutura passando
`;

  return { readme, overview, routesApi, checklist };
}

function testFor(p) {
  const matchers = p.routeMatchers?.length
    ? p.routeMatchers
    : [
        ...p.routes.filter((r) => !r.includes('*') && !r.includes(':')).map((r) => `path="${r}"`),
        ...p.routes.filter((r) => r.includes(':')),
        p.component.replace('.tsx', ''),
      ];

  const expects = matchers
    .map((m) => `    expect(app).toContain(${JSON.stringify(m)});`)
    .join('\n');

  return `import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../../');
const app = readFileSync(path.join(root, 'src/App.tsx'), 'utf8');
const pagePath = path.join(root, 'src/pages/${p.component}');

describe('${p.title} — structure', () => {
  it('page module file exists', () => {
    expect(existsSync(pagePath)).toBe(true);
  });

  it('is wired in App routes', () => {
${expects}
  });

  it('exports a React page component', () => {
    const src = readFileSync(pagePath, 'utf8');
    expect(src).toMatch(/export function \\w+/);
  });
});
`;
}

let createdDocs = 0;
let createdTests = 0;
let skipped = 0;

for (const p of PAGES) {
  if (HAND_WRITTEN.has(p.slug)) {
    skipped++;
    continue;
  }

  const docsDir = path.join(root, 'docs/pages', p.slug);
  const { readme, overview, routesApi, checklist } = docsFor(p);
  if (writeFile(path.join(docsDir, 'README.md'), readme)) createdDocs++;
  if (writeFile(path.join(docsDir, 'overview.md'), overview)) createdDocs++;
  if (writeFile(path.join(docsDir, 'routes-and-api.md'), routesApi)) createdDocs++;
  if (writeFile(path.join(docsDir, 'test-checklist.md'), checklist)) createdDocs++;

  const testPath = path.join(root, 'client/tests/unit/pages', p.slug, `${p.slug}.structure.test.ts`);
  if (writeFile(testPath, testFor(p))) createdTests++;
}

// Coverage matrix
const matrixRows = PAGES.map((p) => {
  const docsOk = fs.existsSync(path.join(root, 'docs/pages', p.slug, 'README.md'));
  const testOk =
    fs.existsSync(path.join(root, 'client/tests/unit/pages', p.slug, `${p.slug}.structure.test.ts`)) ||
    (p.slug === 'landing' && fs.existsSync(path.join(root, 'client/tests/unit/pages/landing/landing.structure.test.ts'))) ||
    (p.slug === 'login' && fs.existsSync(path.join(root, 'client/tests/unit/pages/login/loginPage.structure.test.ts'))) ||
    (p.slug === 'register' && fs.existsSync(path.join(root, 'client/tests/unit/pages/register/registerPage.structure.test.ts')));
  return `| ${p.slug} | ${p.title} | ${p.batch} | ${p.auth} | ${docsOk ? '✅' : '❌'} | ${testOk ? '✅' : '❌'} |`;
}).join('\n');

const covered = PAGES.filter((p) => {
  const docsOk = fs.existsSync(path.join(root, 'docs/pages', p.slug, 'README.md'));
  const testOk =
    fs.existsSync(path.join(root, 'client/tests/unit/pages', p.slug, `${p.slug}.structure.test.ts`)) ||
    ['landing', 'login', 'register'].includes(p.slug);
  return docsOk && testOk;
}).length;

const pct = ((covered / PAGES.length) * 100).toFixed(1);

const index = `# Documentação por página / recurso

Cobertura automática de módulos: **${covered}/${PAGES.length} (${pct}%)** com docs + teste de estrutura.

## Convenção

\`\`\`
docs/pages/<slug>/
  README.md
  overview.md
  routes-and-api.md
  test-checklist.md

client/tests/unit/pages/<slug>/
  <slug>.structure.test.ts
\`\`\`

Gerador: \`node scripts/generate_page_coverage.mjs\`

## Matriz

| Slug | Título | Lote | Auth | Docs | Teste |
|------|--------|------|------|------|-------|
${matrixRows}

## Lotes

1. \`1-auth\` — landing, login, register (mão)
2. \`2-marketing\` — features…api-docs
3. \`3-app\` — dashboard…developer-wallets
4. \`4-merchant\` — checkout, merchant, oauth, faucetlist
5. \`5-admin\` — admin/*

## Meta de cobertura

- **Mínimo:** 80% dos módulos com docs+structure test (atual: ${pct}%)
- **Ideal:** 97–98% + testes de regra/API/E2E nos fluxos críticos (auth, deposit, withdraw, swap, faucet, admin)

Catálogo de tipos: [\`../testing/types-catalog.md\`](../testing/types-catalog.md)
`;

writeFile(path.join(root, 'docs/pages/README.md'), index) || fs.writeFileSync(path.join(root, 'docs/pages/README.md'), index);
fs.writeFileSync(
  path.join(root, 'docs/pages/COVERAGE.generated.md'),
  `# Page module coverage (generated)

Generated by \`scripts/generate_page_coverage.mjs\`

- Modules: ${PAGES.length}
- Covered (docs+structure): ${covered} (${pct}%)
- Created docs files this run: ${createdDocs}
- Created test files this run: ${createdTests}
- Hand-written skipped: ${skipped}

See also [COVERAGE.md](COVERAGE.md) for quality metas beyond structure tests.
`,
);
console.log(JSON.stringify({ pages: PAGES.length, covered, pct, createdDocs, createdTests, skipped }, null, 2));
