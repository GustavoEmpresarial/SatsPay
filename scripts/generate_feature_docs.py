#!/usr/bin/env python3
"""Generate brute, AI-searchable FEATURE.md + TC.md under docs/features/.

Run from repo root:
  python3 scripts/generate_feature_docs.py
"""
from __future__ import annotations

import re
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CLIENT = ROOT / "client" / "src"
PAGES = CLIENT / "pages"
APP = CLIENT / "App.tsx"
OUT = ROOT / "docs" / "features"
EXISTING_PAGES = ROOT / "docs" / "pages"

# Extra backend / cross-cutting domains (not a single React page).
DOMAINS: list[dict] = [
    {
        "slug": "domain-ledger",
        "title": "Ledger contábil (partidas dobradas)",
        "keywords": "ledger balance wallet_entries double-entry HOUSE PERSONAL DEVELOPER saldo",
        "files": [
            "crates/db/src/ledger.rs",
            "docs/architecture/ledger.md",
            "docs/security/BALANCE_SECURITY.md",
            "docs/database/ledger-invariants.md",
        ],
        "apis": [],
        "notes": "Saldos NÃO vivem em coluna mutável — sempre SUM(wallet_entries). Travas na linha wallets.",
    },
    {
        "slug": "domain-worker",
        "title": "Worker / jobs em background",
        "keywords": "worker deposit watcher withdrawal sweep outbox kafka jobs SKIP LOCKED",
        "files": [
            "crates/worker/",
            "docs/worker/background-jobs.md",
            "docs/architecture/events-and-jobs.md",
        ],
        "apis": [],
        "notes": "Daemon separado: depósitos, saques, sweeps, outbox→Kafka, preços, faucet HOUSE.",
    },
    {
        "slug": "domain-chain",
        "title": "Integração on-chain",
        "keywords": "BTC LTC DOGE BCH POL DGB SOL USDT USDC RPC HD xpub sweep hot wallet",
        "files": [
            "crates/chain/",
            "docs/architecture/chain-integration.md",
        ],
        "apis": [],
        "notes": "Clientes RPC / explorers; hot + deposit addresses; DGB tem fallback Cryptoid.",
    },
    {
        "slug": "domain-treasury-health",
        "title": "Tesouraria & saúde financeira (admin)",
        "keywords": "treasury-health solvency hot custody fee margin P&L break-even runway buffer FEE_MARGIN_HARD_BLOCK",
        "files": [
            "crates/db/src/treasury_health.rs",
            "crates/db/src/network_fees.rs",
            "client/src/pages/AdminStakePage.tsx",
            "docs/pages/admin-stake/",
        ],
        "apis": [
            "GET /v1/admin/treasury-wallets",
            "GET /v1/admin/treasury-health",
            "GET /v1/admin/economics",
        ],
        "notes": "9 painéis: P&L USD, break-even, runway HOUSE, passivos, sweeps, buffer hot, trava margem, série 7d, DGB OK.",
    },
    {
        "slug": "domain-gateway-merchant",
        "title": "Gateway merchant (invoices + HMAC)",
        "keywords": "merchant_deposit_invoices gateway HMAC api_keys webhook checkout order_id fee 0.5%",
        "files": [
            "crates/db/migrations/0010_merchant_deposit_invoices.sql",
            "docs/api/public-api-hmac.md",
            "client/src/pages/CheckoutPage.tsx",
            "client/src/pages/AdminMerchantsPage.tsx",
        ],
        "apis": [
            "POST /v1/public/pay",
            "GET /v1/public/pay/:id",
            "GET /v1/admin/merchants/stats",
        ],
        "notes": "Site X cria invoice → usuário paga on-chain → webhook. Taxa plataforma ~0,5%.",
    },
    {
        "slug": "domain-auth",
        "title": "Auth, sessão, 2FA, admin login",
        "keywords": "JWT refresh cookie login register 2FA admin RequireAuth RequireAdmin Turnstile",
        "files": [
            "crates/api-http/src/auth.rs",
            "client/src/lib/api.ts",
            "client/src/stores/auth.ts",
            "client/src/stores/admin.ts",
        ],
        "apis": [
            "POST /v1/auth/login",
            "POST /v1/auth/register",
            "GET /v1/auth/me",
            "POST /v1/auth/admin/login",
        ],
        "notes": "Access token em memória; refresh HttpOnly. RequireAdmin: NUNCA short-circuit useStore (React #311).",
    },
    {
        "slug": "domain-observability",
        "title": "Telemetria / erros cliente + servidor",
        "keywords": "system_error_logs telemetry reportClientError APM client-frontend FEE_MARGIN",
        "files": [
            "client/src/lib/reportError.ts",
            "client/src/pages/AdminTelemetryPage.tsx",
            "docs/quality/error-observability.md",
            "crates/db/migrations/0014_telemetry_and_error_logs.sql",
        ],
        "apis": [
            "POST /v1/telemetry/client-errors",
            "GET /v1/admin/telemetry/overview",
            "GET /v1/admin/telemetry/errors",
        ],
        "notes": "Filtros de ruído: inventory faucet, login 400, React #311 legado, CDN icons.",
    },
    {
        "slug": "domain-faucet-house",
        "title": "Faucet + inventário HOUSE",
        "keywords": "faucet claim HOUSE inventory cooldown Turnstile faucet_claim Sybil",
        "files": [
            "crates/api-http/src/faucet.rs",
            "client/src/pages/FaucetPage.tsx",
            "crates/db/src/house.rs",
        ],
        "apis": ["POST /v1/faucet/claim/:coin"],
        "notes": "Debita HOUSE. Erro 'platform inventory insufficient' é esperado quando caixa vazia.",
    },
]

AUTH_HINT = {
    "admin": "RequireAdmin — role ADMIN ou sessão admin store",
    "user": "RequireAuth — usuário logado",
    "public": "rota pública",
    "mixed": "pública ou autenticada conforme contexto",
}


def slug_from_component(comp: str) -> str:
    name = re.sub(r"Page$", "", comp)
    # AdminFoo -> admin-foo
    s1 = re.sub(r"(.)([A-Z][a-z]+)", r"\1-\2", name)
    s2 = re.sub(r"([a-z0-9])([A-Z])", r"\1-\2", s1)
    return s2.lower()


def guess_auth(paths: list[str]) -> str:
    joined = " ".join(paths)
    if "/admin" in joined or any(p.startswith("telemetry") or p in ("stake", "merchants", "withdrawals", "faucet-sites", "index") for p in paths if not p.startswith("/")):
        # admin nested paths without leading slash
        if any(
            x in joined
            for x in (
                "/admin",
                "telemetry",
                "merchants",
                "withdrawals",
                "faucet-sites",
            )
        ) or (paths == ["index"] or paths == ["stake"] and "Admin" in joined):
            pass
    if any(p.startswith("/admin") for p in paths) or any(
        p in ("telemetry", "merchants", "withdrawals", "faucet-sites", "index", "stake")
        for p in paths
    ):
        # Disambiguate user /stake vs admin stake: admin pages live under /admin
        return "admin" if any("admin" in p or p in ("telemetry", "merchants", "withdrawals", "faucet-sites", "index") for p in paths) or True else "user"
    if any(p.startswith(("/dashboard", "/wallet", "/deposit", "/withdraw", "/faucet", "/swap", "/lend", "/stake", "/settings", "/merchant", "/developer", "/api-keys", "/referrals", "/airdrop", "/analytics", "/support", "/faucetlist")) for p in paths):
        return "user"
    return "public"


def extract_apis(text: str) -> list[str]:
    found = re.findall(r"api(?:<[^>]+>)?\(\s*[`']([^`']+)", text)
    # normalize template bits
    out = []
    for a in found:
        a = a.split("?")[0]
        a = re.sub(r"\$\{[^}]+\}", ":id", a)
        if not a.startswith("/"):
            a = "/" + a
        out.append(a)
    return sorted(set(out))


def extract_tabs(text: str) -> list[str]:
    # label: 'Foo' inside tabs arrays
    labels = re.findall(r"label:\s*'([^']+)'", text)
    # also Tab type unions
    return labels[:20]


def read_existing_overview(slug: str) -> str:
    p = EXISTING_PAGES / slug / "overview.md"
    if p.exists():
        return p.read_text()[:2000]
    return ""


def write_feature(
    slug: str,
    title: str,
    *,
    component: str | None,
    page_file: str | None,
    routes: list[str],
    apis: list[str],
    auth: str,
    tabs: list[str],
    keywords: str,
    notes: str,
    related_files: list[str],
) -> None:
    d = OUT / slug
    d.mkdir(parents=True, exist_ok=True)

    route_lines = "\n".join(f"- `{r}`" for r in routes) or "- (sem rota SPA — domínio backend)"
    api_lines = "\n".join(f"- `{a}` (prefixo `/v1` no servidor)" for a in apis) or "- (sem chamadas `api()` na página / domínio)"
    tab_lines = "\n".join(f"- {t}" for t in tabs) or "- (página sem abas internas)"
    files = related_files[:]
    if page_file:
        files.insert(0, f"client/src/pages/{page_file}")
    pages_doc = f"docs/pages/{slug}/" if (EXISTING_PAGES / slug).exists() else ""
    if pages_doc:
        files.append(pages_doc)
    file_lines = "\n".join(f"- `{f}`" for f in files)

    existing = read_existing_overview(slug)

    feature = f"""# FEATURE — {title}

> Doc bruta para busca por IA/humanos. Atualizar quando a feature mudar.
> Gerado/atualizado por `scripts/generate_feature_docs.py`.

## Identidade

| Campo | Valor |
|-------|-------|
| Slug | `{slug}` |
| Título | {title} |
| Componente | `{component or "—"}` |
| Auth | **{auth}** — {AUTH_HINT.get(auth, auth)} |
| UI pt-BR | Admin sempre pt-BR hardcoded; app usuário usa i18n |

## Keywords (busca)

`{keywords}`

## Rotas

{route_lines}

## Abas / seções internas

{tab_lines}

## APIs usadas (client → `/v1…`)

{api_lines}

## Arquivos-chave

{file_lines}

## Comportamento (bruto)

{notes}

## Notas de overview legado

{existing or "_sem overview em docs/pages_"}

## Bugs / armadilhas conhecidas

- Não short-circuit hooks (`useA() || useB()`) — React #311.
- Admin: `AdminLayout` labels em pt-BR; ignore language switch do app.
- Erros esperados de produto (faucet inventory, login 400) não devem floodar telemetria.
- Saldos: nunca confiar em coluna `balance` mutável — usar ledger.

## Links relacionados

- Mapa geral: [`docs/README.md`](../../README.md)
- Índice features: [`../README.md`](../README.md)
- Testes: [`TC.md`](TC.md)
"""

    # Test cases — brute checklist
    tcs = [
        f"TC-{slug}-01 | smoke | Abrir rota(s) e renderizar sem crash",
        f"TC-{slug}-02 | auth | Gate {auth}: anônimo / usuário / admin conforme esperado",
        f"TC-{slug}-03 | api | Happy path das APIs listadas retorna 2xx com payload válido",
        f"TC-{slug}-04 | api-neg | 401/403/400 cobertos; mensagens via formatApiError",
        f"TC-{slug}-05 | i18n | Strings user-facing pt-BR (admin 100% pt-BR)",
        f"TC-{slug}-06 | obs | Erros inesperados reportados; ruído esperado filtrado",
        f"TC-{slug}-07 | security | Sem IDOR; sem vazar secrets em UI/logs",
        f"TC-{slug}-08 | structure | `client/tests/unit/pages/{slug}/` structure test se página SPA",
    ]
    if tabs:
        for i, t in enumerate(tabs[:8], start=9):
            tcs.append(f"TC-{slug}-{i:02d} | ui-tab | Aba/seção «{t}» carrega e exibe empty/loading/data")

    tc_body = f"""# TC — {title}

> Casos de teste brutos (aceite + regressão). Marque ao executar.

## Matriz

| ID | Tipo | Caso | Status |
|----|------|------|--------|
"""
    for line in tcs:
        parts = [p.strip() for p in line.split("|")]
        tc_body += f"| {parts[0]} | {parts[1]} | {parts[2]} | [ ] |\n"

    tc_body += f"""
## Automatizado

| Suite | Path |
|-------|------|
| Structure (se SPA) | `client/tests/unit/pages/{slug}/` |
| API HTTP (se admin/core) | `crates/api-http/tests/` |
| SQLx | `crates/db/tests/` |

## Dados / fixtures

- Preferir `client/tests/helpers/apiMock.ts` para unit.
- Integração: `DATABASE_URL` de teste + migrations.

## Critérios de aceite

- [ ] Rotas documentadas batem com `App.tsx`
- [ ] APIs documentadas batem com chamadas `api()` / handlers Axum
- [ ] Sem regressão de hooks (Rules of Hooks)
- [ ] Docs FEATURE.md + TC.md atualizados nesta pasta
"""

    (d / "FEATURE.md").write_text(feature)
    (d / "TC.md").write_text(tc_body)
    (d / "README.md").write_text(
        f"""# {title}

- [{title} — FEATURE](FEATURE.md)
- [Casos de teste — TC](TC.md)
"""
    )


def parse_app_routes() -> dict[str, list[str]]:
    app = APP.read_text()
    by_comp: dict[str, list[str]] = defaultdict(list)
    for m in re.finditer(r"<Route\s+([^>]*?)/?>", app, re.S):
        block = m.group(1)
        path_m = re.search(r'path="([^"]+)"', block)
        el_m = re.search(r"element=\{<(\w+)", block)
        if not el_m:
            continue
        comp = el_m.group(1)
        if comp in ("Navigate",):
            continue
        path = path_m.group(1) if path_m else "index"
        by_comp[comp].append(path)
    return by_comp


def find_page_file(comp: str) -> Path | None:
    for cand in PAGES.glob("*.tsx"):
        text = cand.read_text()
        if f"export function {comp}" in text:
            return cand
    return None


def enrich_notes(comp: str, text: str, apis: list[str], tabs: list[str]) -> str:
    bits = [
        f"Página React `{comp}`.",
        f"Chama {len(apis)} endpoint(s) via `api()`.",
    ]
    if tabs:
        bits.append("Abas/labels: " + ", ".join(tabs[:12]) + ".")
    # heuristics
    if "AdminStake" in comp:
        bits.append(
            "Tesouraria: hot vs custódia, economia, 9 painéis health, labels pt-BR "
            "(Resultado USD, Equilíbrio do saque, Autonomia HOUSE, Reserva da hot)."
        )
    if "AdminMerchants" in comp:
        bits.append(
            "3 abas: Visão geral · Estatísticas (funil, série 14d, volume, top, faturas recentes) · Comerciantes (moderação)."
        )
    if "AdminTelemetry" in comp:
        bits.append("Abas Saúde / Desempenho / Erros; autoatualização; resolve/ignore/clear.")
    if "FaucetPage" in comp:
        bits.append("Turnstile action `faucet_claim`; inventário HOUSE; cooldown.")
    if "Withdraw" in comp:
        bits.append("2FA / fee / min withdrawal; status PENDING→BROADCAST→CONFIRMED.")
    if "Deposit" in comp:
        bits.append("Endereço HD por coin; watcher no worker credita ledger.")
    if "RequireAdmin" in text or "admin" in comp.lower():
        bits.append("UI admin sempre pt-BR.")
    return " ".join(bits)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    by_comp = parse_app_routes()
    index_rows: list[tuple[str, str, str]] = []

    for comp, paths in sorted(by_comp.items()):
        pf = find_page_file(comp)
        if not pf:
            continue
        text = pf.read_text()
        slug = slug_from_component(comp)
        # Fix admin nested: AdminStakePage under /admin/stake -> keep admin-stake
        title = re.sub(r"([a-z])([A-Z])", r"\1 \2", re.sub(r"Page$", "", comp))
        apis = extract_apis(text)
        tabs = extract_tabs(text)
        auth = "admin" if comp.startswith("Admin") else guess_auth(paths)
        # normalize admin routes
        routes = []
        for p in paths:
            if comp.startswith("Admin") and not p.startswith("/"):
                routes.append("/admin" if p == "index" else f"/admin/{p}")
            else:
                routes.append(p)
        keywords = " ".join(
            [
                slug.replace("-", " "),
                comp,
                " ".join(routes),
                " ".join(apis),
                " ".join(tabs),
                auth,
            ]
        )
        notes = enrich_notes(comp, text, apis, tabs)
        write_feature(
            slug,
            title,
            component=comp,
            page_file=pf.name,
            routes=routes,
            apis=apis,
            auth=auth,
            tabs=tabs,
            keywords=keywords,
            notes=notes,
            related_files=[],
        )
        index_rows.append((slug, title, auth))

    for dom in DOMAINS:
        write_feature(
            dom["slug"],
            dom["title"],
            component=None,
            page_file=None,
            routes=[],
            apis=dom["apis"],
            auth="mixed",
            tabs=[],
            keywords=dom["keywords"],
            notes=dom["notes"],
            related_files=dom["files"],
        )
        index_rows.append((dom["slug"], dom["title"], "domain"))

    # Master README for AI
    lines = [
        "# Features — índice para IA / humanos",
        "",
        "> **Regra:** antes de implementar ou debugar uma aba/feature, busque aqui",
        "> (`docs/features/<slug>/FEATURE.md` + `TC.md`) e em `docs/pages/<slug>/`.",
        "",
        "Gerador: `python3 scripts/generate_feature_docs.py`",
        "",
        "## Como pesquisar",
        "",
        "1. `rg -n \"keyword\" docs/features`",
        "2. Abrir `FEATURE.md` da pasta batida",
        "3. Validar com `TC.md`",
        "4. Código: caminhos listados em «Arquivos-chave»",
        "",
        "## Convenção de pasta",
        "",
        "```",
        "docs/features/<slug>/",
        "  README.md      # links",
        "  FEATURE.md     # o quê / onde / APIs / armadilhas",
        "  TC.md          # casos de teste brutos",
        "```",
        "",
        "Slugs de domínio backend usam prefixo `domain-`.",
        "",
        "## Matriz",
        "",
        "| Slug | Título | Auth/tipo |",
        "|------|--------|-----------|",
    ]
    for slug, title, auth in sorted(index_rows, key=lambda x: x[0]):
        lines.append(f"| [`{slug}`]({slug}/FEATURE.md) | {title} | {auth} |")

    lines += [
        "",
        "## Domínios críticos (comece por estes)",
        "",
        "- [`domain-ledger`](domain-ledger/FEATURE.md)",
        "- [`domain-treasury-health`](domain-treasury-health/FEATURE.md)",
        "- [`domain-gateway-merchant`](domain-gateway-merchant/FEATURE.md)",
        "- [`domain-auth`](domain-auth/FEATURE.md)",
        "- [`domain-faucet-house`](domain-faucet-house/FEATURE.md)",
        "- [`domain-observability`](domain-observability/FEATURE.md)",
        "- [`domain-worker`](domain-worker/FEATURE.md)",
        "- [`domain-chain`](domain-chain/FEATURE.md)",
        "- Admin UI: `admin-overview`, `admin-stake`, `admin-merchants`, `admin-telemetry`, `admin-withdrawals`",
        "",
        "## Relação com docs/pages",
        "",
        "`docs/pages/<slug>/` = docs por página (overview, routes-and-api, test-checklist).",
        "`docs/features/<slug>/` = FEATURE + TC densos para busca por IA (esta árvore).",
        "",
    ]
    (OUT / "README.md").write_text("\n".join(lines) + "\n")
    print(f"Wrote {len(index_rows)} feature folders → {OUT}")


if __name__ == "__main__":
    main()
