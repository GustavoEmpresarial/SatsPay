import { useState, useEffect } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { motion, AnimatePresence } from 'framer-motion';
import { clsx } from 'clsx';
import { COINS, COIN_CONFIG, INTERNAL_AMOUNT_DECIMALS, isDepositWithdrawPaused } from '@/shared';

type MainTab = 'deposits' | 'payouts' | 'oauth' | 'security' | 'simulator';
type CodeLang = 'curl' | 'js' | 'python' | 'php' | 'go' | 'rust';

/**
 * Canonical origin for every example on this page. The API is served from the
 * same origin as the app (`client/nginx.conf` proxies `/v1/`), so this is the
 * base for both the REST calls and the hosted checkout link.
 *
 * Keep it a single constant: the page used to mix `satspay.pro` and
 * `www.satspay.pro` across examples.
 */
const API_BASE = 'https://www.satspay.pro';

/** Ledger scale, shared with the backend (`Coin::onchain_decimals` docs). */
const UNITS_PER_COIN = 10 ** INTERNAL_AMOUNT_DECIMALS;

/** "25" USDT → "2500000000". Used in the unit examples below. */
function toLedgerUnits(coins: number): string {
  return BigInt(Math.round(coins * UNITS_PER_COIN)).toString();
}

interface ParamDoc {
  name: string;
  type: string;
  required: boolean;
  desc: string;
  example?: string;
}

function CodeBlock({ code, label }: { code: string; label?: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="relative overflow-hidden rounded-2xl border border-border/80 bg-[#090D16] text-white shadow-xl">
      {label && (
        <div className="flex items-center justify-between border-b border-white/10 bg-white/[0.04] px-4 py-2.5">
          <div className="flex items-center gap-2">
            <span className="flex h-2 w-2 rounded-full bg-emerald-500" />
            <span className="font-mono text-[11px] font-bold uppercase tracking-wider text-white/70">{label}</span>
          </div>
          <button
            type="button"
            onClick={() => {
              navigator.clipboard.writeText(code);
              setCopied(true);
              setTimeout(() => setCopied(false), 1500);
            }}
            className="flex items-center gap-1.5 rounded-lg bg-white/5 px-2.5 py-1 text-[11px] font-semibold text-white/80 hover:bg-white/15 hover:text-white transition-all active:scale-95"
          >
            <i className={`bi ${copied ? 'bi-check2 text-emerald-400 font-bold' : 'bi-clipboard'}`} />
            <span>{copied ? 'Copiado!' : 'Copiar'}</span>
          </button>
        </div>
      )}
      <pre className="overflow-x-auto p-4 sm:p-5 font-mono text-[12px] leading-relaxed text-emerald-400/95 [scrollbar-width:none]">
        {code}
      </pre>
    </div>
  );
}

function MultiLangCodeBlock({ snippets }: { snippets: Partial<Record<CodeLang, string>> }) {
  const [lang, setLang] = useState<CodeLang>('curl');

  const allLanguages: { id: CodeLang; label: string; icon: string }[] = [
    { id: 'curl', label: 'cURL', icon: 'bi-terminal-fill' },
    { id: 'js', label: 'Node.js', icon: 'bi-filetype-js' },
    { id: 'python', label: 'Python', icon: 'bi-filetype-py' },
    { id: 'php', label: 'PHP', icon: 'bi-filetype-php' },
    { id: 'go', label: 'Go', icon: 'bi-box-seam' },
    { id: 'rust', label: 'Rust', icon: 'bi-gear-fill' },
  ];
  const languages = allLanguages.filter((l) => Boolean(snippets[l.id]));

  const currentCode = snippets[lang] || snippets.curl || Object.values(snippets)[0] || '';

  return (
    <div className="space-y-2.5">
      <div className="flex items-center gap-1.5 overflow-x-auto pb-1 [scrollbar-width:none]">
        {languages.map((l) => (
          <button
            key={l.id}
            type="button"
            onClick={() => setLang(l.id)}
            className={clsx(
              'flex items-center gap-1.5 rounded-xl px-3.5 py-1.5 text-xs font-bold transition-all',
              lang === l.id
                ? 'bg-bitcoin text-white shadow-md shadow-bitcoin/25 scale-102'
                : 'bg-surface text-ink-muted hover:bg-paper hover:text-ink border border-border',
            )}
          >
            <i className={`bi ${l.icon}`} />
            <span>{l.label}</span>
          </button>
        ))}
      </div>
      <CodeBlock label={languages.find((l) => l.id === lang)?.label.toUpperCase()} code={currentCode} />
    </div>
  );
}

function MethodBadge({ method }: { method: 'GET' | 'POST' | 'DELETE' }) {
  const color =
    method === 'GET'
      ? 'bg-blue-500/15 text-blue-600 border-blue-500/30'
      : method === 'POST'
      ? 'bg-emerald-500/15 text-emerald-600 border-emerald-500/30'
      : 'bg-rose-500/15 text-rose-600 border-rose-500/30';
  return (
    <span className={clsx('rounded-lg border px-2.5 py-0.5 font-mono text-[11px] font-black tracking-wider', color)}>
      {method}
    </span>
  );
}

function EndpointHeader({ method, path, title, badge }: { method: 'GET' | 'POST' | 'DELETE'; path: string; title: string; badge?: string }) {
  return (
    <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 border-b border-border pb-4">
      <div className="space-y-1">
        <div className="flex items-center gap-2.5 flex-wrap">
          <MethodBadge method={method} />
          <code className="rounded-xl border border-border bg-surface px-3 py-1 font-mono text-xs sm:text-sm font-bold text-ink">
            {path}
          </code>
          {badge && (
            <span className="rounded-full bg-bitcoin/10 px-2.5 py-0.5 text-[10px] font-extrabold text-bitcoin-dark">
              {badge}
            </span>
          )}
        </div>
        <h3 className="text-lg sm:text-xl font-black text-ink">{title}</h3>
      </div>
    </div>
  );
}

function ParamsTable({ params }: { params: ParamDoc[] }) {
  return (
    <div className="overflow-x-auto rounded-2xl border border-border bg-paper shadow-xs">
      <table className="w-full text-left text-xs">
        <thead className="border-b border-border bg-surface/60 text-[10px] font-extrabold uppercase tracking-wider text-ink-muted">
          <tr>
            <th className="px-4 py-3">Parâmetro</th>
            <th className="px-4 py-3">Tipo</th>
            <th className="px-4 py-3">Obrigatório</th>
            <th className="px-4 py-3">Descrição</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-border">
          {params.map((p) => (
            <tr key={p.name} className="hover:bg-surface/30">
              <td className="px-4 py-3 font-mono font-bold text-ink">
                <div>{p.name}</div>
                {p.example && <div className="text-[10px] text-ink-muted font-normal">ex: {p.example}</div>}
              </td>
              <td className="px-4 py-3 font-mono text-ink-muted">{p.type}</td>
              <td className="px-4 py-3">
                {p.required ? (
                  <span className="rounded-full bg-rose-500/10 px-2 py-0.5 text-[10px] font-bold text-rose-700">
                    Obrigatório
                  </span>
                ) : (
                  <span className="rounded-full bg-slate-500/10 px-2 py-0.5 text-[10px] font-bold text-slate-600">
                    Opcional
                  </span>
                )}
              </td>
              <td className="px-4 py-3 text-ink-muted leading-relaxed">{p.desc}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// ---------------------------------------------------------------------------
// INTERACTIVE WEBHOOK SIMULATOR / SIGNATURE CALCULATOR
// ---------------------------------------------------------------------------
async function computeHmacSha256(secret: string, message: string): Promise<string> {
  if (!secret || !message) return '';
  try {
    const enc = new TextEncoder();
    const keyData = enc.encode(secret);
    const cryptoKey = await window.crypto.subtle.importKey(
      'raw',
      keyData,
      { name: 'HMAC', hash: 'SHA-256' },
      false,
      ['sign'],
    );
    const sigBuffer = await window.crypto.subtle.sign('HMAC', cryptoKey, enc.encode(message));
    const hashArray = Array.from(new Uint8Array(sigBuffer));
    return hashArray.map((b) => b.toString(16).padStart(2, '0')).join('');
  } catch {
    return 'Erro ao calcular HMAC';
  }
}

function WebhookSimulator() {
  const [secret, setSecret] = useState('');
  const [payload, setPayload] = useState(() =>
    JSON.stringify(
      {
        event: 'deposit.confirmed',
        invoiceId: '550e8400-e29b-41d4-a716-446655440000',
        orderId: 'ORD-99821',
        siteUserId: 'user_4412',
        coin: 'USDT',
        amount: '2500000000',
        fee: '12500000',
        netAmount: '2487500000',
        txHash: '0x4a8f9c2d1e0b3a7f8e6c5d4b3a2f1e0d9c8b7a6f5e4d3c2b1a0f9e8d7c6b5a4',
        status: 'CONFIRMED',
        paidAt: '2026-09-02T17:32:10.442Z',
        customerEmail: 'cliente@email.com',
        timestamp: Math.floor(Date.now() / 1000),
        attempt: 1,
      },
      null,
      2,
    ),
  );
  const [calculatedSig, setCalculatedSig] = useState('');
  const [verificationInput, setVerificationInput] = useState('');
  const [isValid, setIsValid] = useState<boolean | null>(null);

  useEffect(() => {
    let active = true;
    computeHmacSha256(secret, payload).then((res) => {
      if (active) setCalculatedSig(res);
    });
    return () => {
      active = false;
    };
  }, [secret, payload]);

  const handleTestVerify = () => {
    if (!verificationInput.trim()) {
      setIsValid(null);
      return;
    }
    // Aceita com ou sem o prefixo `sha256=`, como o header real envia.
    const clean = verificationInput.trim().replace(/^sha256=/i, '');
    setIsValid(clean.toLowerCase() === calculatedSig.toLowerCase());
  };

  return (
    <div className="rounded-3xl border border-border bg-paper p-6 sm:p-8 shadow-xs space-y-6">
      <div className="flex items-center gap-3 border-b border-border/80 pb-4">
        <div className="flex h-12 w-12 items-center justify-center rounded-2xl bg-indigo-500/10 text-indigo-600 text-2xl">
          <i className="bi bi-cpu-fill" />
        </div>
        <div>
          <h2 className="text-xl sm:text-2xl font-black text-ink">
            Simulador de Assinaturas Webhook & HMAC-SHA256
          </h2>
          <p className="text-xs sm:text-sm text-ink-muted">
            Reproduz exatamente o que o servidor envia: HMAC-SHA256 do corpo cru, com o prefixo <code className="font-mono">sha256=</code>.
          </p>
        </div>
      </div>

      <div className="rounded-2xl border border-amber-500/30 bg-amber-500/5 p-4 text-xs text-ink-muted leading-relaxed">
        <b className="text-amber-700">O segredo não é o secret da sua API Key.</b> A chave de assinatura do webhook é
        própria de cada comerciante e você a obtém autenticado no painel, em{' '}
        <code className="font-mono text-ink font-bold">GET /v1/merchant/webhook-signing-secret</code>. Cole o valor do
        campo <code className="font-mono text-ink font-bold">secret</code> abaixo.
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* INPUTS */}
        <div className="space-y-4">
          <div className="space-y-1.5">
            <label className="text-xs font-bold text-ink uppercase tracking-wider block">
              1. Segredo de assinatura do webhook (hex)
            </label>
            <input
              type="text"
              value={secret}
              onChange={(e) => setSecret(e.target.value)}
              placeholder="cole aqui o campo secret de /v1/merchant/webhook-signing-secret"
              className="w-full rounded-xl border border-border bg-surface px-3.5 py-2 font-mono text-xs text-ink focus:outline-none focus:ring-2 focus:ring-bitcoin/30"
            />
          </div>

          <div className="space-y-1.5">
            <label className="text-xs font-bold text-ink uppercase tracking-wider block">
              2. Corpo cru do webhook (raw body)
            </label>
            <textarea
              rows={14}
              value={payload}
              onChange={(e) => setPayload(e.target.value)}
              className="w-full rounded-xl border border-border bg-surface p-3 font-mono text-[11px] leading-relaxed text-ink focus:outline-none focus:ring-2 focus:ring-bitcoin/30 [scrollbar-width:none]"
            />
            <p className="text-[11px] text-ink-muted">
              Assine o corpo <b>exatamente como chegou</b> (bytes brutos). Reserializar o JSON muda a assinatura.
            </p>
          </div>
        </div>

        {/* OUTPUTS & VERIFIER */}
        <div className="space-y-4 flex flex-col justify-between">
          <div className="space-y-3">
            <label className="text-xs font-bold text-ink uppercase tracking-wider block">
              3. Cabeçalhos que a SatsPay envia
            </label>

            <div className="space-y-2 rounded-2xl bg-surface p-4 border border-border text-xs font-mono">
              <div>
                <span className="text-ink-muted font-bold block mb-0.5">X-SatsPay-Signature:</span>
                <code className="text-indigo-600 font-bold bg-paper px-2 py-1 rounded border border-border block break-all text-[11px]">
                  sha256={calculatedSig || '…'}
                </code>
              </div>

              <div>
                <span className="text-ink-muted font-bold block mb-0.5">X-SatsPay-Event:</span>
                <code className="text-emerald-600 font-bold bg-paper px-2 py-1 rounded border border-border block">
                  deposit.confirmed
                </code>
              </div>

              <div>
                <span className="text-ink-muted font-bold block mb-0.5">X-SatsPay-Timestamp / X-SatsPay-Delivery:</span>
                <code className="text-amber-600 font-bold bg-paper px-2 py-1 rounded border border-border block break-all text-[11px]">
                  {'<unix seconds>'} / {'<uuid por tentativa>'}
                </code>
              </div>

              <p className="text-[11px] text-ink-muted font-sans leading-relaxed pt-1">
                O <code className="font-mono">timestamp</code> também vai <b>dentro</b> do corpo assinado — valide-o contra o
                seu relógio (tolerância de 300s) para bloquear replay. Não existe header no formato{' '}
                <code className="font-mono">t=…,v1=…</code>.
              </p>
            </div>
          </div>

          <div className="rounded-2xl border border-border bg-paper p-4 space-y-3 shadow-xs">
            <span className="text-xs font-bold text-ink uppercase tracking-wider block">
              4. Testar a validação do seu servidor
            </span>
            <div className="flex gap-2">
              <input
                type="text"
                value={verificationInput}
                onChange={(e) => {
                  setVerificationInput(e.target.value);
                  setIsValid(null);
                }}
                placeholder="Cole a assinatura que seu código calculou..."
                className="flex-1 rounded-xl border border-border bg-surface px-3 py-1.5 font-mono text-xs text-ink focus:outline-none focus:ring-2 focus:ring-bitcoin/30"
              />
              <button
                type="button"
                onClick={handleTestVerify}
                className="rounded-xl bg-bitcoin hover:bg-bitcoin-dark text-white font-bold text-xs px-4 py-2 transition active:scale-95"
              >
                Verificar
              </button>
            </div>

            {isValid === true && (
              <div className="p-2.5 rounded-xl bg-emerald-500/10 border border-emerald-500/30 text-emerald-700 text-xs font-bold flex items-center gap-2">
                <i className="bi bi-check-circle-fill text-emerald-600 text-sm" />
                <span>Assinatura válida! Seu backend calculou exatamente o mesmo HMAC-SHA256.</span>
              </div>
            )}
            {isValid === false && (
              <div className="p-2.5 rounded-xl bg-rose-500/10 border border-rose-500/30 text-rose-700 text-xs font-bold flex items-center gap-2">
                <i className="bi bi-x-circle-fill text-rose-600 text-sm" />
                <span>Assinatura divergente! Confira o segredo, o prefixo sha256= e se você assinou o corpo cru.</span>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

export function ApiDocsPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const initialTab = (searchParams.get('tab') as MainTab) || 'deposits';
  const [activeTab, setActiveTab] = useState<MainTab>(
    ['deposits', 'payouts', 'oauth', 'security', 'simulator'].includes(initialTab) ? initialTab : 'deposits'
  );

  useEffect(() => {
    const tabParam = searchParams.get('tab') as MainTab;
    if (tabParam && ['deposits', 'payouts', 'oauth', 'security', 'simulator'].includes(tabParam)) {
      setActiveTab(tabParam);
    }
  }, [searchParams]);

  const handleTabChange = (tab: MainTab) => {
    setActiveTab(tab);
    setSearchParams({ tab });
  };

  return (
    <div className="space-y-6 max-w-6xl mx-auto pb-24">
      {/* HEADER PRINCIPAL */}
      <header className="rounded-3xl border border-border bg-paper p-6 sm:p-8 shadow-xs space-y-3">
        <div className="inline-flex items-center gap-2 rounded-full border border-bitcoin/30 bg-bitcoin/10 px-3.5 py-1 text-xs font-black text-bitcoin-dark">
          <i className="bi bi-shield-lock-fill text-emerald-600" /> SatsPay Developer API · v1.0 (Enterprise Gateway & Webhooks HMAC)
        </div>
        <h1 className="text-2xl sm:text-4xl font-black tracking-tight text-ink">
          Documentação da API SatsPay
        </h1>
        <p className="text-xs sm:text-sm text-ink-muted leading-relaxed">
          Infraestrutura segura para recebimento de depósitos em criptomoedas com checkout hospedado sob domínio oficial, assinaturas criptográficas HMAC-SHA256, whitelisting de IPs, micropagamentos em lote e Login com SatsPay (OAuth 2.0 / OIDC).
        </p>

        <div className="flex flex-wrap items-center gap-3 pt-2">
          <Link
            to="/api-keys"
            className="rounded-xl bg-bitcoin hover:bg-bitcoin-dark text-white font-bold text-xs px-4 py-2.5 shadow-md shadow-bitcoin/25 transition-all flex items-center gap-2"
          >
            <i className="bi bi-key-fill" /> Gerenciar Minhas Chaves & Whitelist
          </Link>
          <Link
            to="/deposit"
            className="rounded-xl border border-border bg-surface hover:bg-paper text-ink font-bold text-xs px-4 py-2.5 shadow-xs transition-all flex items-center gap-2"
          >
            <i className="bi bi-qr-code text-bitcoin" /> Meus endereços de depósito
          </Link>
        </div>
      </header>

      {/* SELETOR DE ABAS PRINCIPAIS */}
      <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-5 gap-2 p-1.5 rounded-2xl bg-surface border border-border">
        <button
          type="button"
          onClick={() => handleTabChange('deposits')}
          className={clsx(
            'flex items-center justify-center gap-2 py-3 px-3 rounded-xl font-bold text-xs sm:text-sm transition-all',
            activeTab === 'deposits'
              ? 'bg-paper text-emerald-600 shadow-sm border border-border'
              : 'text-ink-muted hover:text-ink',
          )}
        >
          <i className="bi bi-qr-code text-base" />
          <span>1. Depósitos & Invoices</span>
        </button>

        <button
          type="button"
          onClick={() => handleTabChange('payouts')}
          className={clsx(
            'flex items-center justify-center gap-2 py-3 px-3 rounded-xl font-bold text-xs sm:text-sm transition-all',
            activeTab === 'payouts'
              ? 'bg-paper text-bitcoin shadow-sm border border-border'
              : 'text-ink-muted hover:text-ink',
          )}
        >
          <i className="bi bi-send-fill text-base" />
          <span>2. Envios & Payouts</span>
        </button>

        <button
          type="button"
          onClick={() => handleTabChange('oauth')}
          className={clsx(
            'flex items-center justify-center gap-2 py-3 px-3 rounded-xl font-bold text-xs sm:text-sm transition-all',
            activeTab === 'oauth'
              ? 'bg-paper text-amber-500 shadow-sm border border-border'
              : 'text-ink-muted hover:text-ink',
          )}
        >
          <i className="bi bi-shield-lock-fill text-base text-amber-500" />
          <span>3. Login SSO (OAuth)</span>
        </button>

        <button
          type="button"
          onClick={() => handleTabChange('security')}
          className={clsx(
            'flex items-center justify-center gap-2 py-3 px-3 rounded-xl font-bold text-xs sm:text-sm transition-all',
            activeTab === 'security'
              ? 'bg-paper text-blue-600 shadow-sm border border-border'
              : 'text-ink-muted hover:text-ink',
          )}
        >
          <i className="bi bi-shield-check text-base" />
          <span>4. Segurança & Erros</span>
        </button>

        <button
          type="button"
          onClick={() => handleTabChange('simulator')}
          className={clsx(
            'flex items-center justify-center gap-2 py-3 px-3 rounded-xl font-bold text-xs sm:text-sm transition-all col-span-2 sm:col-span-1',
            activeTab === 'simulator'
              ? 'bg-paper text-purple-600 shadow-sm border border-border'
              : 'text-ink-muted hover:text-ink',
          )}
        >
          <i className="bi bi-cpu text-base" />
          <span>5. Simulador HMAC</span>
        </button>
      </div>

      {/* CONTEÚDO DA ABA SELECIONADA */}
      <AnimatePresence mode="wait">
        {activeTab === 'deposits' && (
          /* ========================================================================= */
          /* ABA 1: API DE DEPÓSITOS & GATEWAY DE COBRANÇAS */
          /* ========================================================================= */
          <motion.div
            key="deposits"
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            className="space-y-6"
          >
            <section className="rounded-3xl border border-border bg-paper p-6 sm:p-8 shadow-xs space-y-8">
              <div className="flex items-center gap-3 border-b border-border/80 pb-4">
                <div className="flex h-12 w-12 items-center justify-center rounded-2xl bg-emerald-500/10 text-emerald-600 text-2xl">
                  <i className="bi bi-qr-code" />
                </div>
                <div>
                  <h2 className="text-xl sm:text-2xl font-black text-ink">
                    Gateway de Depósitos & Checkout Hospedado
                  </h2>
                  <p className="text-xs sm:text-sm text-ink-muted">
                    Gere cobranças cripto com endereços exclusivos, QR codes dinâmicos e redirecionamento seguro para a infraestrutura oficial SatsPay.
                  </p>
                </div>
              </div>

              {/* FLUXO DE REDIRECIONAMENTO E SEGURANÇA */}
              <div className="rounded-2xl border border-emerald-500/20 bg-emerald-500/5 p-4 space-y-2">
                <div className="flex items-center gap-2 text-emerald-700 font-bold text-xs">
                  <i className="bi bi-shield-shaded text-base" />
                  <span>Como funciona o Checkout Hospedado Seguro:</span>
                </div>
                <p className="text-xs text-ink-muted leading-relaxed">
                  1. Seu backend faz uma requisição autenticada <code className="font-mono text-ink font-bold">POST /v1/merchant/deposits</code> usando sua API Key.<br />
                  2. A SatsPay responde <code className="font-mono text-ink font-bold">201 Created</code> com <code className="font-mono text-ink font-bold">checkoutUrl</code> (absoluto) e <code className="font-mono text-ink font-bold">payUrl</code> (relativo, <code>/pay/:id</code>).<br />
                  3. Você redireciona o cliente para o <code className="font-mono text-ink font-bold">checkoutUrl</code>, hospedado no domínio oficial SatsPay.<br />
                  4. Quando o pagamento é confirmado, disparamos o webhook assinado <code className="font-mono text-ink font-bold">deposit.confirmed</code> e o cliente volta para a sua <code className="font-mono text-ink font-bold">successUrl</code>.
                </p>
              </div>

              {/* REGRA DE UNIDADE — O ERRO MAIS CARO DA INTEGRAÇÃO */}
              <div className="rounded-2xl border border-rose-500/30 bg-rose-500/5 p-4 space-y-3">
                <div className="flex items-center gap-2 text-rose-700 font-bold text-xs">
                  <i className="bi bi-exclamation-octagon-fill text-base" />
                  <span>Leia antes de integrar: `amount` é em unidades de 1e-8</span>
                </div>
                <p className="text-xs text-ink-muted leading-relaxed">
                  Todos os valores da API (depósitos e envios) são <b>inteiros na menor fração interna</b>, com {INTERNAL_AMOUNT_DECIMALS} casas
                  decimais — nunca a quantidade decimal da moeda. Enviar <code className="font-mono text-ink font-bold">"25.00"</code> para USDT
                  não cobra 25 USDT: seriam 25 × 10<sup>-8</sup> USDT. Por isso a API <b>rejeita</b> valores com ponto ou vírgula
                  (<code className="font-mono text-ink font-bold">400 AMOUNT_NOT_INTEGER</code>).
                </p>
                <div className="overflow-x-auto rounded-xl border border-border bg-paper">
                  <table className="w-full text-left text-xs">
                    <thead className="border-b border-border bg-surface/60 text-[10px] font-extrabold uppercase tracking-wider text-ink-muted">
                      <tr>
                        <th className="px-3 py-2">Você quer cobrar</th>
                        <th className="px-3 py-2">Envie em `amount`</th>
                        <th className="px-3 py-2">Nunca envie</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-border font-mono">
                      <tr>
                        <td className="px-3 py-2 text-ink">25 USDT</td>
                        <td className="px-3 py-2 font-bold text-emerald-700">"{toLedgerUnits(25)}"</td>
                        <td className="px-3 py-2 text-rose-600">"25.00"</td>
                      </tr>
                      <tr>
                        <td className="px-3 py-2 text-ink">1 POL</td>
                        <td className="px-3 py-2 font-bold text-emerald-700">"{toLedgerUnits(1)}"</td>
                        <td className="px-3 py-2 text-rose-600">"1.0"</td>
                      </tr>
                      <tr>
                        <td className="px-3 py-2 text-ink">0,005 BCH</td>
                        <td className="px-3 py-2 font-bold text-emerald-700">"{toLedgerUnits(0.005)}"</td>
                        <td className="px-3 py-2 text-rose-600">"0.005"</td>
                      </tr>
                    </tbody>
                  </table>
                </div>
                <p className="text-[11px] text-ink-muted">
                  Valores muito pequenos também são recusados (<code className="font-mono text-ink font-bold">AMOUNT_BELOW_MINIMUM</code>) quando
                  arredondariam para zero na rede — USDT e USDC têm 6 casas on-chain, então o mínimo é 100 unidades internas.
                </p>
              </div>

              {/* ENDPOINT 1.1: CRIAR COBRANÇA */}
              <div className="space-y-4">
                <EndpointHeader
                  method="POST"
                  path="/v1/merchant/deposits"
                  title="1.1 Criar Cobrança de Depósito (Gerar Fatura / Invoice)"
                  badge="Idempotente por orderId"
                />
                <p className="text-xs sm:text-sm text-ink-muted">
                  Cria uma fatura com endereço de depósito dedicado, link público de checkout hospedado em{' '}
                  <code className="font-mono font-bold text-ink">{API_BASE}/pay/:id</code> e QR Code pronto.
                  Os caminhos <code className="font-mono font-bold text-ink">/v1/merchant/deposits/create</code> e{' '}
                  <code className="font-mono font-bold text-ink">/v1/merchant/invoices</code> são aliases do mesmo endpoint.
                </p>

                <ParamsTable
                  params={[
                    { name: 'coin', type: 'String', required: true, desc: 'Criptomoeda do depósito. Apenas moedas ativas (veja a tabela no fim desta página) — moeda pausada retorna 503 DEPOSIT_PAUSED.', example: '"USDT"' },
                    { name: 'amount', type: 'String', required: true, desc: `Inteiro na menor fração interna (${INTERNAL_AMOUNT_DECIMALS} decimais). 25 USDT = "${toLedgerUnits(25)}". Ponto/vírgula são rejeitados.`, example: `"${toLedgerUnits(25)}"` },
                    { name: 'orderId', type: 'String', required: true, desc: 'Identificador único do pedido no seu sistema (1-128 caracteres). É a chave de idempotência: repetir a mesma chamada devolve a mesma fatura.', example: '"ORD-99821"' },
                    { name: 'callbackUrl', type: 'String', required: true, desc: 'URL HTTPS pública do seu servidor para o webhook assinado. Endereços internos/localhost são recusados (INVALID_CALLBACK_URL).', example: '"https://meusite.com/webhook"' },
                    { name: 'successUrl', type: 'String', required: false, desc: 'URL de retorno após o cliente pagar com sucesso no checkout.', example: '"https://meusite.com/obrigado"' },
                    { name: 'cancelUrl', type: 'String', required: false, desc: 'URL de retorno se o cliente cancelar o pagamento.', example: '"https://meusite.com/carrinho"' },
                    { name: 'siteUserId', type: 'String', required: false, desc: 'Seu identificador interno do comprador. Volta igual no webhook, para você creditar o usuário certo.', example: '"user_4412"' },
                    { name: 'customerEmail', type: 'String', required: false, desc: 'E-mail do cliente para notificação de confirmação.', example: '"cliente@email.com"' },
                    { name: 'customerName', type: 'String', required: false, desc: 'Nome do cliente, exibido no checkout.', example: '"Maria Silva"' },
                    { name: 'siteName', type: 'String', required: false, desc: 'Nome da sua loja, exibido no topo da página de pagamento.', example: '"Minha Loja"' },
                    { name: 'description', type: 'String', required: false, desc: 'Descrição do pedido exibida no checkout.', example: '"Plano Pro - 1 mês"' },
                    { name: 'expiryMinutes', type: 'Number', required: false, desc: 'Validade da fatura em minutos. Padrão 60; limitado entre 5 e 1440.', example: '60' },
                  ]}
                />

                <div className="space-y-3">
                  <h4 className="text-xs font-bold uppercase tracking-wider text-ink-muted">
                    Exemplo de Requisição:
                  </h4>
                  <MultiLangCodeBlock
                    snippets={{
                      curl: `curl -X POST ${API_BASE}/v1/merchant/deposits \\
  -H "x-api-key: SUA_CHAVE_DE_API" \\
  -H "Content-Type: application/json" \\
  -d '{
    "coin": "USDT",
    "amount": "${toLedgerUnits(25)}",
    "orderId": "ORD-99821",
    "callbackUrl": "https://meusite.com/api/webhook",
    "customerEmail": "cliente@email.com",
    "siteName": "Minha Loja Online"
  }'`,
                      js: `const response = await fetch('${API_BASE}/v1/merchant/deposits', {
  method: 'POST',
  headers: {
    'x-api-key': process.env.SATSPAY_API_KEY,
    'Content-Type': 'application/json'
  },
  body: JSON.stringify({
    coin: 'USDT',
    amount: '${toLedgerUnits(25)}', // 25 USDT em unidades de 1e-8
    orderId: 'ORD-99821',
    callbackUrl: 'https://meusite.com/api/webhook',
    customerEmail: 'cliente@email.com',
    siteName: 'Minha Loja Online'
  })
});

const invoice = await response.json();
// 201 Created. Redirecione o cliente para o checkout hospedado:
window.location.href = invoice.checkoutUrl;`,
                      python: `import os
import requests

url = "${API_BASE}/v1/merchant/deposits"
headers = {
    "x-api-key": os.environ["SATSPAY_API_KEY"],
    "Content-Type": "application/json"
}
payload = {
    "coin": "USDT",
    "amount": "${toLedgerUnits(25)}",  # 25 USDT em unidades de 1e-8
    "orderId": "ORD-99821",
    "callbackUrl": "https://meusite.com/api/webhook",
    "customerEmail": "cliente@email.com",
    "siteName": "Minha Loja Online"
}

response = requests.post(url, json=payload, headers=headers)
response.raise_for_status()
invoice = response.json()
print("Redirecionar cliente para:", invoice["checkoutUrl"])`,
                      php: `<?php
$curl = curl_init();

$payload = [
  "coin" => "USDT",
  "amount" => "${toLedgerUnits(25)}", // 25 USDT em unidades de 1e-8
  "orderId" => "ORD-99821",
  "callbackUrl" => "https://meusite.com/api/webhook",
  "customerEmail" => "cliente@email.com",
  "siteName" => "Minha Loja Online"
];

curl_setopt_array($curl, [
  CURLOPT_URL => "${API_BASE}/v1/merchant/deposits",
  CURLOPT_RETURNTRANSFER => true,
  CURLOPT_POST => true,
  CURLOPT_POSTFIELDS => json_encode($payload),
  CURLOPT_HTTPHEADER => [
    "x-api-key: " . getenv("SATSPAY_API_KEY"),
    "Content-Type: application/json"
  ],
]);

$response = curl_exec($curl);
$invoice = json_decode($response, true);
header("Location: " . $invoice['checkoutUrl']);
exit;`,
                      go: `package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
)

func main() {
	payload, _ := json.Marshal(map[string]interface{}{
		"coin":          "USDT",
		"amount":        "${toLedgerUnits(25)}", // 25 USDT em unidades de 1e-8
		"orderId":       "ORD-99821",
		"callbackUrl":   "https://meusite.com/api/webhook",
		"customerEmail": "cliente@email.com",
		"siteName":      "Minha Loja Online",
	})

	req, _ := http.NewRequest("POST", "${API_BASE}/v1/merchant/deposits", bytes.NewBuffer(payload))
	req.Header.Set("x-api-key", os.Getenv("SATSPAY_API_KEY"))
	req.Header.Set("Content-Type", "application/json")

	resp, err := (&http.Client{}).Do(req)
	if err != nil {
		panic(err)
	}
	defer resp.Body.Close()

	var invoice struct {
		CheckoutURL string \`json:"checkoutUrl"\`
	}
	json.NewDecoder(resp.Body).Decode(&invoice)
	fmt.Println("Redirecionar cliente para:", invoice.CheckoutURL)
}`,
                      rust: `use reqwest::Client;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let invoice = client
        .post("${API_BASE}/v1/merchant/deposits")
        .header("x-api-key", std::env::var("SATSPAY_API_KEY")?)
        .json(&json!({
            "coin": "USDT",
            "amount": "${toLedgerUnits(25)}", // 25 USDT em unidades de 1e-8
            "orderId": "ORD-99821",
            "callbackUrl": "https://meusite.com/api/webhook",
            "customerEmail": "cliente@email.com",
            "siteName": "Minha Loja Online"
        }))
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    println!("Checkout URL: {}", invoice["checkoutUrl"]);
    Ok(())
}`,
                    }}
                  />
                </div>

                <div className="space-y-2">
                  <h4 className="text-xs font-bold uppercase tracking-wider text-ink-muted">
                    Resposta de Sucesso (201 Created):
                  </h4>
                  <CodeBlock
                    label="JSON RESPONSE"
                    code={`{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "PENDING",
  "coin": "USDT",
  "amount": "${toLedgerUnits(25)}",
  "feeAmount": "${toLedgerUnits(0.125)}",
  "netAmount": "${toLedgerUnits(24.875)}",
  "depositAddress": "0x71C6705624342490cf03323decB0C392A8892A88",
  "payUrl": "/pay/550e8400-e29b-41d4-a716-446655440000",
  "checkoutUrl": "${API_BASE}/pay/550e8400-e29b-41d4-a716-446655440000",
  "qrCode": "usdt:0x71C6705624342490cf03323decB0C392A8892A88?amount=${toLedgerUnits(25)}",
  "orderId": "ORD-99821",
  "expiresAt": "2026-09-02T18:00:00.000Z",
  "createdAt": "2026-09-02T17:00:00.000Z"
}`}
                  />
                  <p className="text-[11px] text-ink-muted leading-relaxed">
                    <b>qrCode</b> é uma URI de pagamento (<code className="font-mono">moeda:endereço?amount=…</code>) pronta para
                    virar QR no seu front — não é imagem base64. <b>checkoutUrl</b> já vem absoluto; <b>payUrl</b> é o mesmo
                    caminho relativo, caso você monte a URL por conta própria.
                  </p>
                </div>

                {/* TAXA */}
                <div className="rounded-2xl border border-border bg-surface p-4 space-y-2">
                  <div className="flex items-center gap-2 text-ink font-bold text-xs">
                    <i className="bi bi-percent text-bitcoin text-base" />
                    <span>Taxa do gateway: 0,5% por fatura</span>
                  </div>
                  <p className="text-xs text-ink-muted leading-relaxed">
                    O cliente paga <code className="font-mono text-ink font-bold">amount</code>; você é creditado em{' '}
                    <code className="font-mono text-ink font-bold">netAmount</code> (<code className="font-mono">amount − feeAmount</code>).
                    Para 25 USDT: taxa de {toLedgerUnits(0.125)} (0,125 USDT) e crédito líquido de {toLedgerUnits(24.875)} (24,875 USDT).
                  </p>
                </div>

                {/* IDEMPOTÊNCIA */}
                <div className="rounded-2xl border border-border bg-surface p-4 space-y-2">
                  <div className="flex items-center gap-2 text-ink font-bold text-xs">
                    <i className="bi bi-arrow-repeat text-purple-600 text-base" />
                    <span>Idempotência por orderId</span>
                  </div>
                  <p className="text-xs text-ink-muted leading-relaxed">
                    Repetir o POST com o mesmo <code className="font-mono text-ink font-bold">orderId</code> e os mesmos{' '}
                    <code className="font-mono text-ink font-bold">coin</code>/<code className="font-mono text-ink font-bold">amount</code>{' '}
                    devolve <b>200 OK</b> com a fatura original — nunca uma segunda cobrança. Reutilizar o mesmo{' '}
                    <code className="font-mono text-ink font-bold">orderId</code> para outro valor retorna{' '}
                    <b>409 <code className="font-mono">DUPLICATE_ORDER_ID</code></b>.
                  </p>
                </div>
              </div>

              {/* ENDPOINT 1.2: CONSULTAR FATURA */}
              <div className="space-y-4 pt-6 border-t border-border/80">
                <EndpointHeader
                  method="GET"
                  path="/v1/merchant/deposits/:id"
                  title="1.2 Consultar Status da Fatura de Depósito"
                />
                <p className="text-xs sm:text-sm text-ink-muted">
                  Retorna a fatura completa, incluindo <code className="font-mono font-bold text-ink">receivedAmount</code>{' '}
                  (quanto já chegou no endereço) e o estado de entrega do webhook.
                </p>

                <CodeBlock
                  label="CURL"
                  code={`curl -X GET ${API_BASE}/v1/merchant/deposits/550e8400-e29b-41d4-a716-446655440000 \\
  -H "x-api-key: SUA_CHAVE_DE_API"`}
                />

                <div className="overflow-x-auto rounded-2xl border border-border bg-surface">
                  <table className="w-full text-left text-xs">
                    <thead className="border-b border-border bg-paper text-[10px] font-extrabold uppercase tracking-wider text-ink-muted">
                      <tr>
                        <th className="px-4 py-2.5">Status</th>
                        <th className="px-4 py-2.5">Significado</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-border">
                      <tr>
                        <td className="px-4 py-2.5 font-mono font-bold text-ink">PENDING</td>
                        <td className="px-4 py-2.5 text-ink-muted">Fatura criada; nada recebido no endereço ainda.</td>
                      </tr>
                      <tr>
                        <td className="px-4 py-2.5 font-mono font-bold text-blue-600">DETECTED</td>
                        <td className="px-4 py-2.5 text-ink-muted">
                          Pagamento visto na rede, mas ainda sem confirmações suficientes <b>ou</b> abaixo do valor cobrado
                          (veja <code className="font-mono">receivedAmount</code>). Não credite nada aqui.
                        </td>
                      </tr>
                      <tr>
                        <td className="px-4 py-2.5 font-mono font-bold text-emerald-600">CONFIRMED</td>
                        <td className="px-4 py-2.5 text-ink-muted">Pago e creditado. É quando o webhook <code className="font-mono">deposit.confirmed</code> é enviado.</td>
                      </tr>
                      <tr>
                        <td className="px-4 py-2.5 font-mono font-bold text-amber-600">EXPIRED</td>
                        <td className="px-4 py-2.5 text-ink-muted">Passou de <code className="font-mono">expiresAt</code> sem pagamento completo.</td>
                      </tr>
                      <tr>
                        <td className="px-4 py-2.5 font-mono font-bold text-rose-600">CANCELLED</td>
                        <td className="px-4 py-2.5 text-ink-muted">Cancelada administrativamente.</td>
                      </tr>
                    </tbody>
                  </table>
                </div>
                <p className="text-[11px] text-ink-muted">
                  Não existe status <code className="font-mono">PAID</code>: o estado final de sucesso é{' '}
                  <code className="font-mono font-bold text-ink">CONFIRMED</code>.
                </p>
              </div>
            </section>
          </motion.div>
        )}

        {activeTab === 'payouts' && (
          /* ========================================================================= */
          /* ABA 2: API DE ENVIOS & SAQUES (PAYOUTS) */
          /* ========================================================================= */
          <motion.div
            key="payouts"
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            className="space-y-6"
          >
            <section className="rounded-3xl border border-border bg-paper p-6 sm:p-8 shadow-xs space-y-8">
              <div className="flex items-center gap-3 border-b border-border/80 pb-4">
                <div className="flex h-12 w-12 items-center justify-center rounded-2xl bg-bitcoin/10 text-bitcoin text-2xl">
                  <i className="bi bi-send-fill" />
                </div>
                <div>
                  <h2 className="text-xl sm:text-2xl font-black text-ink">
                    API de Envios & Micropagamentos (Payouts)
                  </h2>
                  <p className="text-xs sm:text-sm text-ink-muted">
                    Envie pagamentos, retiradas e recompensas diretamente para a conta SatsPay de qualquer usuário cadastrado com <b>zero taxas de rede e liquidação imediata (0ms)</b>.
                  </p>
                </div>
              </div>

              {/* ENDPOINT 2.1: CONSULTAR SALDO */}
              <div className="space-y-4">
                <EndpointHeader
                  method="GET"
                  path="/v1/public/balance"
                  title="2.1 Consultar Saldos da Tesouraria Comercial"
                />
                <p className="text-xs sm:text-sm text-ink-muted">
                  Retorna os saldos disponíveis em caixa em todas as 9 criptomoedas suportadas prontos para envio (representados em 8 decimais / satoshis).
                </p>

                <CodeBlock
                  label="CURL"
                  code={`curl -X GET ${API_BASE}/v1/public/balance \\
  -H "x-api-key: SUA_CHAVE_DE_API"`}
                />

                <CodeBlock
                  label="JSON RESPONSE (8 DECIMAIS: 100,000,000 = 1.0 MOEDA)"
                  code={`{
  "BTC": "500000",             // 0.005 BTC (8 decimais / satoshis)
  "USDT": "2500000000",        // 25.00 USDT (8 decimais: 100,000,000 = 1.0 USDT)
  "USDC": "1000000000",        // 10.00 USDC (8 decimais: 100,000,000 = 1.0 USDC)
  "SOL": "50000000",           // 0.50 SOL (8 decimais: 100,000,000 = 1.0 SOL)
  "POL": "100000000",          // 1.00 POL (8 decimais: 100,000,000 = 1.0 POL)
  "LTC": "25000000",           // 0.25 LTC (8 decimais: 100,000,000 = 1.0 LTC)
  "DOGE": "100000000",         // 1.00 DOGE (8 decimais: 100,000,000 = 1.0 DOGE)
  "BCH": "1500000",            // 0.015 BCH (8 decimais: 100,000,000 = 1.0 BCH)
  "DGB": "50000000"            // 0.50 DGB (8 decimais: 100,000,000 = 1.0 DGB)
}`}
                />
              </div>

              {/* ENDPOINT 2.2: ENVIAR PAGAMENTO */}
              <div className="space-y-4 pt-6 border-t border-border/80">
                <EndpointHeader
                  method="POST"
                  path="/v1/public/send"
                  title="2.2 Enviar Pagamento / Saque para Usuário SatsPay"
                  badge="Liquidação Instantânea (0ms)"
                />
                <p className="text-xs sm:text-sm text-ink-muted">
                  Transfere fundos da sua carteira comercial para a conta SatsPay do usuário indicado pelo e-mail com garantia anti-duplicação via <code className="font-mono font-bold text-ink">idempotencyKey</code>.
                </p>

                <div className="rounded-2xl border border-border bg-surface p-4 space-y-2">
                  <div className="flex items-center gap-2 text-ink font-bold text-xs">
                    <i className="bi bi-shield-lock text-bitcoin text-base" />
                    <span>Requisitos desta rota</span>
                  </div>
                  <ul className="text-xs text-ink-muted leading-relaxed list-disc pl-4 space-y-1">
                    <li>A chave precisa do escopo <code className="font-mono text-ink font-bold">send</code> — sem ele, <code className="font-mono text-ink font-bold">403 MISSING_SCOPE</code>.</li>
                    <li>Existe um limite diário de envios por chave; ao estourar, <code className="font-mono text-ink font-bold">DAILY_LIMIT_REACHED</code>.</li>
                    <li>O destinatário precisa ter conta SatsPay com o e-mail informado; não é envio on-chain.</li>
                    <li>Diferente do gateway de depósitos, <b>nenhuma moeda fica pausada</b> aqui — o envio é interno (ledger).</li>
                  </ul>
                </div>

                <ParamsTable
                  params={[
                    { name: 'coin', type: 'String', required: true, desc: 'Criptomoeda a transferir (USDT, USDC, BTC, SOL, POL, LTC, DOGE, BCH, DGB).', example: '"USDT"' },
                    { name: 'toEmail', type: 'String', required: true, desc: 'E-mail cadastrado na conta SatsPay do usuário destinatário.', example: '"usuario@email.com"' },
                    { name: 'amount', type: 'String', required: true, desc: `Inteiro na menor fração interna (${INTERNAL_AMOUNT_DECIMALS} decimais), igual ao gateway de depósitos: "${toLedgerUnits(1)}" = 1,00 moeda.`, example: `"${toLedgerUnits(10)}"` },
                    { name: 'idempotencyKey', type: 'String', required: true, desc: 'ID único da sua operação para evitar cobrança ou envio duplicado.', example: '"payout_ord_99812"' },
                  ]}
                />

                <MultiLangCodeBlock
                  snippets={{
                    curl: `curl -X POST ${API_BASE}/v1/public/send \\
  -H "x-api-key: SUA_CHAVE_DE_API" \\
  -H "Content-Type: application/json" \\
  -d '{
    "coin": "USDT",
    "toEmail": "cliente@email.com",
    "amount": "1000000000",
    "idempotencyKey": "payout_tx_99821"
  }'`,
                    js: `const res = await fetch('${API_BASE}/v1/public/send', {
  method: 'POST',
  headers: {
    'x-api-key': 'SUA_CHAVE_DE_API',
    'Content-Type': 'application/json'
  },
  body: JSON.stringify({
    coin: 'USDT',
    toEmail: 'cliente@email.com',
    amount: '1000000000', // 10.00 USDT (8 decimais: 100,000,000 = 1.0 USDT)
    idempotencyKey: 'payout_tx_99821'
  })
});

const data = await res.json();
console.log('ID da Transação:', data.referenceId);`,
                    python: `import requests

url = "${API_BASE}/v1/public/send"
headers = {
    "x-api-key": "SUA_CHAVE_DE_API",
    "Content-Type": "application/json"
}
payload = {
    "coin": "USDT",
    "toEmail": "cliente@email.com",
    "amount": "1000000000", # 10.00 USDT (8 decimais)
    "idempotencyKey": "payout_tx_99821"
}

response = requests.post(url, json=payload, headers=headers)
print(response.json())`,
                    php: `<?php
$curl = curl_init();

$payload = [
  "coin" => "USDT",
  "toEmail" => "cliente@email.com",
  "amount" => "1000000000", // 10.00 USDT (8 decimais)
  "idempotencyKey" => "payout_tx_99821"
];

curl_setopt_array($curl, [
  CURLOPT_URL => "${API_BASE}/v1/public/send",
  CURLOPT_RETURNTRANSFER => true,
  CURLOPT_POST => true,
  CURLOPT_POSTFIELDS => json_encode($payload),
  CURLOPT_HTTPHEADER => [
    "x-api-key: SUA_CHAVE_DE_API",
    "Content-Type: application/json"
  ],
]);

$response = curl_exec($curl);
echo $response;`,
                    go: `package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net/http"
)

func main() {
	payload, _ := json.Marshal(map[string]interface{}{
		"coin":           "USDT",
		"toEmail":        "cliente@email.com",
		"amount":         "1000000000", // 10.00 USDT (8 decimais)
		"idempotencyKey": "payout_tx_99821",
	})

	req, _ := http.NewRequest("POST", "${API_BASE}/v1/public/send", bytes.NewBuffer(payload))
	req.Header.Set("x-api-key", "SUA_CHAVE_DE_API")
	req.Header.Set("Content-Type", "application/json")

	client := &http.Client{}
	resp, err := client.Do(req)
	if err != nil {
		panic(err)
	}
	defer resp.Body.Close()
	fmt.Println("Status:", resp.Status)
}`,
                    rust: `use reqwest::Client;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let res = client
        .post("${API_BASE}/v1/public/send")
        .header("x-api-key", "SUA_CHAVE_DE_API")
        .json(&json!({
            "coin": "USDT",
            "toEmail": "cliente@email.com",
            "amount": "1000000000",
            "idempotencyKey": "payout_tx_99821"
        }))
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    println!("Reference ID: {}", res["referenceId"]);
    Ok(())
}`,
                  }}
                />

                <CodeBlock
                  label="JSON RESPONSE"
                  code={`{
  "referenceId": "550e8400-e29b-41d4-a716-446655440000"
}`}
                />
              </div>
            </section>
          </motion.div>
        )}

        {activeTab === 'oauth' && (
          /* ========================================================================= */
          /* ABA 3: LOGIN COM SATSPAY (OAUTH 2.0 & OPENID CONNECT SSO) */
          /* ========================================================================= */
          <motion.div
            key="oauth"
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            className="space-y-6"
          >
            <section className="rounded-3xl border border-border bg-paper p-6 sm:p-8 shadow-xs space-y-8">
              <div className="flex items-center gap-3 border-b border-border/80 pb-4">
                <div className="flex h-12 w-12 items-center justify-center rounded-2xl bg-amber-500/10 text-amber-500 text-2xl">
                  <i className="bi bi-shield-lock-fill" />
                </div>
                <div>
                  <h2 className="text-xl sm:text-2xl font-black text-ink">
                    Login com SatsPay (OAuth 2.0 & OpenID Connect SSO)
                  </h2>
                  <p className="text-xs sm:text-sm text-ink-muted">
                    Autentique usuários em seu site, aplicativo ou jogo com a conta SatsPay usando 2 linhas de código HTML ou o protocolo padrão <b>OAuth 2.0 / OIDC</b>, exatamente igual ao Google Sign-In.
                  </p>
                </div>
              </div>

              {/* PASSO A PASSO RÁPIDO */}
              <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <div className="rounded-2xl border border-border bg-surface p-4 space-y-2">
                  <div className="flex items-center gap-2 text-amber-500 font-bold text-xs">
                    <i className="bi bi-key-fill text-base" />
                    <span>1. Crie seu Aplicativo</span>
                  </div>
                  <p className="text-xs text-ink-muted leading-relaxed">
                    Acesse o <Link to="/developer/apps" className="font-bold text-bitcoin hover:underline">Painel OAuth</Link> e crie seu app para obter seu <b>Client ID</b> e <b>Client Secret</b> e registrar suas URLs de callback.
                  </p>
                </div>

                <div className="rounded-2xl border border-border bg-surface p-4 space-y-2">
                  <div className="flex items-center gap-2 text-amber-500 font-bold text-xs">
                    <i className="bi bi-code-slash text-base" />
                    <span>2. Adicione o Botão</span>
                  </div>
                  <p className="text-xs text-ink-muted leading-relaxed">
                    Inclua nosso SDK JavaScript e a tag HTML do botão. O SDK renderiza o botão oficial e gerencia o fluxo de autorização via popup ou redirecionamento.
                  </p>
                </div>

                <div className="rounded-2xl border border-border bg-surface p-4 space-y-2">
                  <div className="flex items-center gap-2 text-amber-500 font-bold text-xs">
                    <i className="bi bi-person-check-fill text-base" />
                    <span>3. Obtenha o Perfil</span>
                  </div>
                  <p className="text-xs text-ink-muted leading-relaxed">
                    Seu backend troca o código de autorização pelo <b>Access Token</b> e consulta o endpoint <code className="font-mono text-ink font-bold">/v1/oauth/userinfo</code> para receber o perfil verificado.
                  </p>
                </div>
              </div>

              {/* ENDPOINT 3.1: SDK FRONTEND */}
              <div className="space-y-4 pt-4 border-t border-border/80">
                <EndpointHeader
                  method="GET"
                  path={`${API_BASE}/sdk/satspay-auth.v2.js`}
                  title="3.1 Integração Frontend via JavaScript SDK"
                  badge="Recomendado"
                />
                <p className="text-xs sm:text-sm text-ink-muted">
                  Cole o código abaixo no seu HTML onde deseja exibir o botão de Login com SatsPay:
                </p>

                <CodeBlock
                  label="HTML SNIPPET"
                  code={`<!-- 1. Carregue o SDK JavaScript oficial do SatsPay -->
<script src="${API_BASE}/sdk/satspay-auth.v2.js" async defer></script>

<!-- 2. Adicione o elemento do botão na sua página de login -->
<div class="satspay-signin"
     data-client_id="sats_app_SEU_CLIENT_ID"
     data-redirect_uri="https://seusite.com/auth/callback"
     data-theme="bitcoin"
     data-size="large"
     data-onsuccess="onSatsPaySignIn">
</div>

<!-- 3. Capture a resposta de login -->
<script>
function onSatsPaySignIn(response) {
  console.log("Código de autorização OAuth recebido:", response.code);
  // Envie response.code para seu servidor efetuar a troca pelo Access Token!
  fetch('/api/auth/satspay', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ code: response.code })
  });
}
</script>`}
                />
              </div>

              {/* ENDPOINT 3.2: TROCAR CÓDIGO POR ACCESS TOKEN */}
              <div className="space-y-4 pt-4 border-t border-border/80">
                <EndpointHeader
                  method="POST"
                  path="/v1/oauth/token"
                  title="3.2 Trocar Código de Autorização por Access Token (Backend)"
                />
                <p className="text-xs sm:text-sm text-ink-muted">
                  Requisição do seu servidor para trocar o <code className="font-mono text-ink">code</code> retornado pelo usuário pelo Access Token oficial.
                </p>

                <MultiLangCodeBlock
                  snippets={{
                    curl: `curl -X POST ${API_BASE}/v1/oauth/token \\
  -H "Content-Type: application/x-www-form-urlencoded" \\
  -d "grant_type=authorization_code" \\
  -d "code=sats_code_xyz123abc456" \\
  -d "client_id=sats_app_SEU_CLIENT_ID" \\
  -d "client_secret=sats_sec_SEU_CLIENT_SECRET" \\
  -d "redirect_uri=https://seusite.com/auth/callback"`,

                    js: `const response = await fetch('${API_BASE}/v1/oauth/token', {
  method: 'POST',
  headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
  body: new URLSearchParams({
    grant_type: 'authorization_code',
    code: authCode,
    client_id: process.env.SATSPAY_CLIENT_ID,
    client_secret: process.env.SATSPAY_CLIENT_SECRET,
    redirect_uri: 'https://seusite.com/auth/callback'
  })
});
const { access_token } = await response.json();`,

                    python: `import requests

res = requests.post('${API_BASE}/v1/oauth/token', data={
    'grant_type': 'authorization_code',
    'code': auth_code,
    'client_id': SATSPAY_CLIENT_ID,
    'client_secret': SATSPAY_CLIENT_SECRET,
    'redirect_uri': 'https://seusite.com/auth/callback'
})
access_token = res.json()['access_token']`,

                    php: `<?php
$ch = curl_init('${API_BASE}/v1/oauth/token');
curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);
curl_setopt($ch, CURLOPT_POST, true);
curl_setopt($ch, CURLOPT_POSTFIELDS, http_build_query([
    'grant_type' => 'authorization_code',
    'code' => $authCode,
    'client_id' => $clientId,
    'client_secret' => $clientSecret,
    'redirect_uri' => 'https://seusite.com/auth/callback'
]));
$response = json_decode(curl_exec($ch), true);
$accessToken = $response['access_token'];`,

                    go: `package main

import (
    "net/http"
    "net/url"
    "encoding/json"
)

func exchangeCode(code, clientID, clientSecret, redirectURI string) (string, error) {
    formData := url.Values{
        "grant_type":    {"authorization_code"},
        "code":          {code},
        "client_id":     {clientID},
        "client_secret": {clientSecret},
        "redirect_uri":  {redirectURI},
    }
    resp, err := http.PostForm("${API_BASE}/v1/oauth/token", formData)
    if err != nil { return "", err }
    defer resp.Body.Close()

    var result struct {
        AccessToken string \`json:"access_token"\`
    }
    json.NewDecoder(resp.Body).Decode(&result)
    return result.AccessToken, nil
}`,
                    rust: `use reqwest::Client;
use std::collections::HashMap;

async fn exchange_token(code: &str, client_id: &str, secret: &str, redirect_uri: &str) -> Result<String, reqwest::Error> {
    let mut params = HashMap::new();
    params.insert("grant_type", "authorization_code");
    params.insert("code", code);
    params.insert("client_id", client_id);
    params.insert("client_secret", secret);
    params.insert("redirect_uri", redirect_uri);

    let client = Client::new();
    let res = client
        .post("${API_BASE}/v1/oauth/token")
        .form(&params)
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    Ok(res["access_token"].as_str().unwrap_or("").to_string())
}`,
                  }}
                />

                <CodeBlock
                  label="JSON RESPONSE (200 OK)"
                  code={`{
  "access_token": "sats_tok_a1b2c3d4e5f6g7h8i9j0",
  "token_type": "Bearer",
  "expires_in": 2592000,
  "scope": "openid profile email"
}`}
                />
              </div>

              {/* ENDPOINT 3.3: CONSULTAR DADOS DO USUÁRIO */}
              <div className="space-y-4 pt-4 border-t border-border/80">
                <EndpointHeader
                  method="GET"
                  path="/v1/oauth/userinfo"
                  title="3.3 Consultar Perfil & Dados do Usuário Autenticado (UserInfo)"
                />
                <p className="text-xs sm:text-sm text-ink-muted">
                  Utilize o Access Token recebido para consultar a identidade do usuário (ID, nome de usuário, e-mail verificado e avatar).
                </p>

                <MultiLangCodeBlock
                  snippets={{
                    curl: `curl -X GET ${API_BASE}/v1/oauth/userinfo \\
  -H "Authorization: Bearer sats_tok_a1b2c3d4e5f6g7h8i9j0"`,

                    js: `const userRes = await fetch('${API_BASE}/v1/oauth/userinfo', {
  headers: {
    'Authorization': \`Bearer \${accessToken}\`
  }
});
const user = await userRes.json();
console.log('ID:', user.sub, 'Username:', user.username, 'Email:', user.email);`,

                    python: `user_res = requests.get('${API_BASE}/v1/oauth/userinfo', headers={
    'Authorization': f'Bearer {access_token}'
})
user = user_res.json()
print(f"Logado como @{user['username']} ({user['email']})")`,

                    rust: `let user_info = reqwest::Client::new()
    .get("${API_BASE}/v1/oauth/userinfo")
    .bearer_auth(access_token)
    .send()
    .await?
    .json::<serde_json::Value>()
    .await?;

println!("Usuário: {}", user_info["username"]);`,
                  }}
                />

                <CodeBlock
                  label="JSON RESPONSE (200 OK)"
                  code={`{
  "sub": "018f3a9a-7c9b-7d12-8e34-56789abcdef0",
  "id": "018f3a9a-7c9b-7d12-8e34-56789abcdef0",
  "username": "elonmuskbr",
  "name": "elonmuskbr",
  "email": "user@example.com",
  "email_verified": true,
  "picture": "https://api.dicebear.com/7.x/identicon/svg?seed=demo",
  "created_at": "2026-03-01T12:00:00Z"
}`}
                />
              </div>

              {/* OPENID DISCOVERY */}
              <div className="rounded-2xl border border-border bg-surface p-4 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
                <div className="flex items-center gap-2.5">
                  <i className="bi bi-gear-wide-connected text-xl text-bitcoin" />
                  <div>
                    <span className="text-xs font-bold text-ink">OpenID Connect Discovery Endpoint</span>
                    <p className="text-[11px] text-ink-muted">Compatível com bibliotecas como NextAuth.js, Passport.js, Laravel Socialite, Auth0, etc.</p>
                  </div>
                </div>
                <code className="rounded-xl bg-paper px-3 py-1 font-mono text-xs font-bold text-bitcoin border border-border select-all">
                  {API_BASE}/.well-known/openid-configuration
                </code>
              </div>
            </section>
          </motion.div>
        )}

        {activeTab === 'security' && (
          /* ========================================================================= */
          /* ABA 4: SEGURANÇA, WEBHOOKS, ASSINATURAS HMAC E ERROS */
          /* ========================================================================= */
          <motion.div
            key="security"
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            className="space-y-6"
          >
            <section className="rounded-3xl border border-border bg-paper p-6 sm:p-8 shadow-xs space-y-8">
              <div className="flex items-center gap-3 border-b border-border/80 pb-4">
                <div className="flex h-12 w-12 items-center justify-center rounded-2xl bg-blue-500/10 text-blue-600 text-2xl">
                  <i className="bi bi-shield-check" />
                </div>
                <div>
                  <h2 className="text-xl sm:text-2xl font-black text-ink">
                    Arquitetura de Segurança Institucional & Blindagem Criptográfica
                  </h2>
                  <p className="text-xs sm:text-sm text-ink-muted">
                    Conheça os mecanismos de proteção avançada, assinaturas criptográficas HMAC-SHA256, políticas anti-replay e catálogo de erros.
                  </p>
                </div>
              </div>

              {/* AUTENTICAÇÃO */}
              <div className="space-y-4">
                <h3 className="text-lg font-black text-ink flex items-center gap-2">
                  <i className="bi bi-key-fill text-bitcoin" />
                  <span>Como autenticar suas chamadas</span>
                </h3>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div className="rounded-2xl border border-border bg-surface p-4 space-y-2">
                    <div className="flex items-center gap-2 text-emerald-600 font-bold text-xs">
                      <i className="bi bi-1-circle-fill text-base" />
                      <span>Header simples: `x-api-key`</span>
                    </div>
                    <p className="text-xs text-ink-muted leading-relaxed">
                      Envie sua chave de 64 caracteres hexadecimais no header{' '}
                      <code className="font-mono text-ink font-bold">x-api-key</code>. Simples e suficiente para a maioria
                      das integrações.
                    </p>
                  </div>

                  <div className="rounded-2xl border border-border bg-surface p-4 space-y-2">
                    <div className="flex items-center gap-2 text-blue-600 font-bold text-xs">
                      <i className="bi bi-2-circle-fill text-base" />
                      <span>Requisição assinada (HMAC)</span>
                    </div>
                    <p className="text-xs text-ink-muted leading-relaxed">
                      Envie <code className="font-mono text-ink font-bold">x-key-id</code>,{' '}
                      <code className="font-mono text-ink font-bold">x-timestamp</code> e{' '}
                      <code className="font-mono text-ink font-bold">x-signature</code>. A assinatura cobre{' '}
                      <code className="font-mono">timestamp + método + caminho + SHA-256 do corpo</code>, a janela é de 300s
                      e cada assinatura só pode ser usada uma vez (proteção contra replay).
                    </p>
                  </div>
                </div>

                <div className="rounded-2xl border border-amber-500/30 bg-amber-500/5 p-4 space-y-2">
                  <div className="flex items-center gap-2 text-amber-700 font-bold text-xs">
                    <i className="bi bi-exclamation-triangle-fill text-base" />
                    <span>Chave criada com “exigir assinatura”</span>
                  </div>
                  <p className="text-xs text-ink-muted leading-relaxed">
                    Uma chave emitida com <code className="font-mono text-ink font-bold">requireSignature: true</code> recusa o
                    caminho do <code className="font-mono text-ink font-bold">x-api-key</code> em <b>todos</b> os endpoints e
                    responde <code className="font-mono text-ink font-bold">401 KEY_REQUIRES_SIGNATURE</code>. Use requisições
                    assinadas com essa chave — ou emita uma chave sem essa opção.
                  </p>
                </div>

                <div className="overflow-x-auto rounded-2xl border border-border bg-surface">
                  <table className="w-full text-left text-xs">
                    <thead className="border-b border-border bg-paper text-[10px] font-extrabold uppercase tracking-wider text-ink-muted">
                      <tr>
                        <th className="px-4 py-2.5">Escopo</th>
                        <th className="px-4 py-2.5">Libera</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-border">
                      <tr>
                        <td className="px-4 py-2.5 font-mono font-bold text-ink">deposits</td>
                        <td className="px-4 py-2.5 text-ink-muted">Gateway de cobranças (<code className="font-mono">/v1/merchant/*</code>)</td>
                      </tr>
                      <tr>
                        <td className="px-4 py-2.5 font-mono font-bold text-ink">send</td>
                        <td className="px-4 py-2.5 text-ink-muted">Envios/payouts (<code className="font-mono">POST /v1/public/send</code>)</td>
                      </tr>
                      <tr>
                        <td className="px-4 py-2.5 font-mono font-bold text-ink">balance</td>
                        <td className="px-4 py-2.5 text-ink-muted">Consulta de saldos da tesouraria</td>
                      </tr>
                    </tbody>
                  </table>
                </div>
                <p className="text-[11px] text-ink-muted">
                  Escopos são <b>minúsculos</b> e comparados literalmente; <code className="font-mono">"*"</code> libera tudo.
                  Uma chave sem <code className="font-mono">deposits</code> recebe{' '}
                  <code className="font-mono font-bold text-ink">403 MISSING_SCOPE</code> ao criar faturas.
                  Se você cadastrar IPs na whitelist da chave, apenas o IP de origem real do seu servidor é aceito —
                  qualquer outro recebe <code className="font-mono font-bold text-ink">401 IP_NOT_ALLOWED</code>.
                </p>
              </div>

              {/* WEBHOOK: CONTRATO REAL */}
              <div className="space-y-4 pt-6 border-t border-border/80">
                <h3 className="text-lg font-black text-ink flex items-center gap-2">
                  <i className="bi bi-broadcast-pin text-emerald-600" />
                  <span>Webhook: contrato exato</span>
                </h3>
                <p className="text-xs sm:text-sm text-ink-muted">
                  Um único evento é emitido hoje: <code className="font-mono font-bold text-ink">deposit.confirmed</code>,
                  quando a fatura vira <code className="font-mono font-bold text-ink">CONFIRMED</code>. Ele é entregue por{' '}
                  <code className="font-mono font-bold text-ink">POST</code> na sua{' '}
                  <code className="font-mono font-bold text-ink">callbackUrl</code>, com até 8 tentativas e backoff
                  exponencial enquanto a resposta não for 2xx.
                </p>

                <CodeBlock
                  label="HTTP REQUEST"
                  code={`POST https://meusite.com/api/webhook
Content-Type: application/json
X-SatsPay-Signature: sha256=<hex HMAC-SHA256 do corpo cru>
X-SatsPay-Event: deposit.confirmed
X-SatsPay-Timestamp: 1789412330
X-SatsPay-Delivery: 7c6f1f0e-1d2a-4a55-9a1e-6b2c0d9f4e11`}
                />

                <CodeBlock
                  label="JSON BODY"
                  code={`{
  "event": "deposit.confirmed",
  "invoiceId": "550e8400-e29b-41d4-a716-446655440000",
  "orderId": "ORD-99821",
  "siteUserId": "user_4412",
  "coin": "USDT",
  "amount": "2500000000",
  "fee": "12500000",
  "netAmount": "2487500000",
  "txHash": "0x4a8f9c2d1e0b3a7f8e6c5d4b3a2f1e0d9c8b7a6f5e4d3c2b1a0f9e8d7c6b5a4",
  "status": "CONFIRMED",
  "paidAt": "2026-09-02T17:32:10.442Z",
  "customerEmail": "cliente@email.com",
  "timestamp": 1789412330,
  "attempt": 1
}`}
                />
                <p className="text-[11px] text-ink-muted leading-relaxed">
                  Note que o campo da taxa no webhook chama-se <code className="font-mono text-ink font-bold">fee</code>,
                  enquanto na resposta de criação da fatura ele é <code className="font-mono text-ink font-bold">feeAmount</code>.
                  Pagamentos feitos com saldo SatsPay chegam com{' '}
                  <code className="font-mono text-ink font-bold">txHash: "internal_satspay"</code>.
                </p>
              </div>

              {/* CHECKLIST DE SEGURANÇA EM PRODUÇÃO */}
              <div className="rounded-2xl border border-blue-500/20 bg-blue-500/5 p-5 space-y-3">
                <div className="flex items-center gap-2 text-blue-700 font-bold text-sm">
                  <i className="bi bi-check2-square text-base" />
                  <span>Checklist antes de publicar em produção:</span>
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5 text-xs text-ink">
                  <div className="flex items-start gap-2 bg-paper/60 p-2.5 rounded-xl border border-border">
                    <i className="bi bi-check-circle-fill text-emerald-600 shrink-0 mt-0.5" />
                    <span>Guardar API Key e segredo do webhook em variáveis de ambiente no backend — nunca no front.</span>
                  </div>
                  <div className="flex items-start gap-2 bg-paper/60 p-2.5 rounded-xl border border-border">
                    <i className="bi bi-check-circle-fill text-emerald-600 shrink-0 mt-0.5" />
                    <span>Cadastrar o IP fixo do seu servidor na whitelist da API Key.</span>
                  </div>
                  <div className="flex items-start gap-2 bg-paper/60 p-2.5 rounded-xl border border-border">
                    <i className="bi bi-check-circle-fill text-emerald-600 shrink-0 mt-0.5" />
                    <span>Comparar a assinatura com função de tempo constante (<code>timingSafeEqual</code> / <code>hash_equals</code>).</span>
                  </div>
                  <div className="flex items-start gap-2 bg-paper/60 p-2.5 rounded-xl border border-border">
                    <i className="bi bi-check-circle-fill text-emerald-600 shrink-0 mt-0.5" />
                    <span>Rejeitar webhooks cujo <code>timestamp</code> (no corpo) difira mais de 300s do seu relógio.</span>
                  </div>
                  <div className="flex items-start gap-2 bg-paper/60 p-2.5 rounded-xl border border-border">
                    <i className="bi bi-check-circle-fill text-emerald-600 shrink-0 mt-0.5" />
                    <span>Tratar entregas repetidas: credite por <code>invoiceId</code>/<code>orderId</code> uma única vez.</span>
                  </div>
                  <div className="flex items-start gap-2 bg-paper/60 p-2.5 rounded-xl border border-border">
                    <i className="bi bi-check-circle-fill text-emerald-600 shrink-0 mt-0.5" />
                    <span>Responder 2xx rápido; qualquer outra resposta agenda nova tentativa.</span>
                  </div>
                </div>
              </div>

              {/* CÓDIGO DE VALIDAÇÃO DO WEBHOOK */}
              <div className="space-y-4 pt-6 border-t border-border/80">
                <h3 className="text-lg font-black text-ink flex items-center gap-2">
                  <i className="bi bi-code-square text-blue-600" />
                  <span>Como Validar a Assinatura do Webhook no Seu Servidor</span>
                </h3>
                <p className="text-xs sm:text-sm text-ink-muted">
                  O header chega como <code className="font-mono font-bold text-ink">sha256=&lt;hex&gt;</code> — remova o
                  prefixo antes de comparar, e assine o <b>corpo cru</b>, não o JSON reserializado:
                </p>

                <MultiLangCodeBlock
                  snippets={{
                    js: `import crypto from 'crypto';

// Express: use express.raw({ type: 'application/json' }) nesta rota.
function verifySatsPayWebhook(rawBody, signatureHeader, webhookSecret) {
  const received = String(signatureHeader || '').replace(/^sha256=/i, '');
  const expected = crypto.createHmac('sha256', webhookSecret).update(rawBody).digest('hex');

  const a = Buffer.from(received, 'hex');
  const b = Buffer.from(expected, 'hex');
  if (a.length !== b.length) return false;              // timingSafeEqual exige mesmo tamanho
  if (!crypto.timingSafeEqual(a, b)) return false;

  const body = JSON.parse(rawBody.toString('utf8'));
  const ageSeconds = Math.abs(Date.now() / 1000 - body.timestamp);
  return ageSeconds <= 300;                              // anti-replay
}`,
                    python: `import hmac
import hashlib
import json
import time

def verify_satspay_webhook(raw_body: bytes, signature_header: str, webhook_secret: str) -> bool:
    received = (signature_header or "").removeprefix("sha256=")
    expected = hmac.new(webhook_secret.encode("utf-8"), raw_body, hashlib.sha256).hexdigest()
    if not hmac.compare_digest(received, expected):
        return False

    body = json.loads(raw_body)
    return abs(time.time() - body["timestamp"]) <= 300`,
                    php: `<?php
function verifySatsPayWebhook(string $rawBody, string $signatureHeader, string $webhookSecret): bool {
    $received = preg_replace('/^sha256=/i', '', $signatureHeader);
    $expected = hash_hmac('sha256', $rawBody, $webhookSecret);
    if (!hash_equals($expected, $received)) {
        return false;
    }

    $body = json_decode($rawBody, true);
    return abs(time() - $body['timestamp']) <= 300;
}`,
                    go: `package main

import (
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"math"
	"strings"
	"time"
)

func VerifyWebhook(rawBody []byte, signatureHeader, webhookSecret string) bool {
	received := strings.TrimPrefix(signatureHeader, "sha256=")
	mac := hmac.New(sha256.New, []byte(webhookSecret))
	mac.Write(rawBody)
	expected := hex.EncodeToString(mac.Sum(nil))
	if !hmac.Equal([]byte(received), []byte(expected)) {
		return false
	}

	var body struct {
		Timestamp int64 \`json:"timestamp"\`
	}
	if err := json.Unmarshal(rawBody, &body); err != nil {
		return false
	}
	return math.Abs(float64(time.Now().Unix()-body.Timestamp)) <= 300
}`,
                    rust: `use hmac::{Hmac, Mac};
use sha2::Sha256;

pub fn verify_webhook(raw_body: &[u8], signature_header: &str, webhook_secret: &str) -> bool {
    let received = signature_header.trim_start_matches("sha256=");
    let Ok(received) = hex::decode(received) else { return false };

    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(webhook_secret.as_bytes()) else { return false };
    mac.update(raw_body);
    // verify_slice já é comparação em tempo constante.
    mac.verify_slice(&received).is_ok()
}`,
                  }}
                />
              </div>

              {/* CATÁLOGO DE ERROS E CÓDIGOS HTTP */}
              <div className="space-y-4 pt-6 border-t border-border/80">
                <h3 className="text-lg font-black text-ink flex items-center gap-2">
                  <i className="bi bi-exclamation-triangle-fill text-amber-500" />
                  <span>Catálogo de Códigos de Erro & Resoluções</span>
                </h3>
                <p className="text-xs text-ink-muted">
                  Toda falha responde <code className="font-mono font-bold text-ink">{'{ "error": "...", "code": "..." }'}</code>.
                  Trate pelo <code className="font-mono font-bold text-ink">code</code>, nunca pela mensagem.
                </p>
                <div className="overflow-x-auto rounded-2xl border border-border bg-surface">
                  <table className="w-full text-left text-xs">
                    <thead className="border-b border-border bg-paper text-[10px] font-extrabold uppercase tracking-wider text-ink-muted">
                      <tr>
                        <th className="px-4 py-2.5">Código de Erro</th>
                        <th className="px-4 py-2.5">HTTP</th>
                        <th className="px-4 py-2.5">Causa Raiz</th>
                        <th className="px-4 py-2.5">Como Resolver</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-border font-mono text-[11px]">
                      {[
                        ['INVALID_API_KEY', '401', 'Chave inexistente, desativada ou com formato inválido.', 'Gere uma nova chave no menu API Keys.', 'rose'],
                        ['API_KEY_EXPIRED', '401', 'A chave passou de expiresAt.', 'Rotacione ou emita uma nova chave.', 'rose'],
                        ['IP_NOT_ALLOWED', '401', 'IP de origem fora da whitelist da chave.', 'Adicione o IP real do seu servidor na chave.', 'rose'],
                        ['KEY_REQUIRES_SIGNATURE', '401', 'Chave exige requisição assinada e veio x-api-key.', 'Assine a requisição ou use uma chave sem requireSignature.', 'rose'],
                        ['SIGNATURE_MISMATCH', '401', 'HMAC da requisição não confere.', 'Assine timestamp + método + caminho + SHA-256 do corpo.', 'amber'],
                        ['TIMESTAMP_OUT_OF_WINDOW', '401', 'x-timestamp fora da janela de 300s.', 'Sincronize o relógio do servidor (NTP).', 'amber'],
                        ['SIGNATURE_REPLAY', '401', 'Assinatura já usada antes.', 'Gere uma assinatura nova a cada requisição.', 'amber'],
                        ['MISSING_SCOPE', '403', 'A chave não tem o escopo exigido pela rota.', 'Emita a chave com deposits / send / balance conforme o uso.', 'rose'],
                        ['UNKNOWN_COIN', '400', 'Símbolo de moeda não suportado.', 'Use um dos símbolos da tabela de moedas.', 'amber'],
                        ['DEPOSIT_PAUSED', '503', 'Depósitos dessa moeda estão temporariamente pausados.', 'Ofereça outra moeda ativa no checkout.', 'amber'],
                        ['AMOUNT_NOT_INTEGER', '400', 'amount veio com ponto/vírgula (ex.: "25.00").', 'Envie inteiro em unidades de 1e-8: 25 USDT = "2500000000".', 'rose'],
                        ['AMOUNT_BELOW_MINIMUM', '400', 'Valor arredondaria para zero na rede.', 'Aumente o valor (USDT/USDC: mínimo 100 unidades).', 'amber'],
                        ['INVALID_AMOUNT', '400', 'amount ausente, zero, negativo ou não numérico.', 'Envie um inteiro positivo como string.', 'amber'],
                        ['INVALID_ORDER_ID', '400', 'orderId vazio ou acima de 128 caracteres.', 'Use o identificador do pedido no seu sistema.', 'amber'],
                        ['INVALID_CALLBACK_URL', '400', 'callbackUrl não é https público (localhost/IP interno).', 'Aponte para uma URL https acessível pela internet.', 'amber'],
                        ['DUPLICATE_ORDER_ID', '409', 'orderId já usado para outra cobrança.', 'Use um orderId único por pedido (repetir o mesmo pedido devolve 200).', 'purple'],
                        ['INVOICE_NOT_FOUND', '404', 'Fatura inexistente.', 'Confira o id retornado na criação.', 'amber'],
                        ['INVOICE_FORBIDDEN', '403', 'A fatura pertence a outro comerciante.', 'Use a chave do dono da fatura.', 'rose'],
                        ['INVOICE_INVALID_STATE', '400', 'Fatura já paga, expirada ou cancelada.', 'Crie uma nova fatura.', 'amber'],
                        ['INSUFFICIENT_BALANCE', '400', 'Saldo insuficiente para a operação.', 'Deposite na moeda correspondente.', 'amber'],
                        ['RATE_LIMITED', '429', 'Limite de requisições por IP excedido.', 'Use retry com backoff exponencial e jitter (veja Retry-After).', 'blue'],
                      ].map(([code, http, cause, fix, color]) => (
                        <tr key={code}>
                          <td className={clsx('px-4 py-2.5 font-bold', {
                            'text-rose-600': color === 'rose',
                            'text-amber-600': color === 'amber',
                            'text-purple-600': color === 'purple',
                            'text-blue-600': color === 'blue',
                          })}>
                            {code}
                          </td>
                          <td className="px-4 py-2.5 text-ink">{http}</td>
                          <td className="px-4 py-2.5 font-sans text-ink-muted">{cause}</td>
                          <td className="px-4 py-2.5 font-sans text-ink">{fix}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            </section>
          </motion.div>
        )}

        {activeTab === 'simulator' && (
          <motion.div
            key="simulator"
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            className="space-y-6"
          >
            <WebhookSimulator />
          </motion.div>
        )}
      </AnimatePresence>

      {/* REFERÊNCIA COMPARTILHADA — MOEDAS, UNIDADES E CONFIRMAÇÕES */}
      <section className="rounded-3xl border border-border bg-paper p-6 sm:p-8 shadow-xs space-y-6">
        <div className="flex items-center gap-2.5 border-b border-border/80 pb-3">
          <i className="bi bi-info-circle-fill text-bitcoin text-xl" />
          <h3 className="text-lg sm:text-xl font-black text-ink">Moedas, Unidades e Confirmações</h3>
        </div>

        <p className="text-xs text-ink-muted">
          Todos os valores da API usam o padrão interno de <b>{INTERNAL_AMOUNT_DECIMALS} casas decimais</b>{' '}
          (<code className="font-mono font-bold text-ink">10<sup>-{INTERNAL_AMOUNT_DECIMALS}</sup></code>), independente das
          casas decimais que a moeda tem na própria rede. <b>Confirmações</b> é o número de blocos exigidos antes de a
          fatura virar <code className="font-mono font-bold text-ink">CONFIRMED</code> e o webhook ser disparado.
        </p>

        <div className="overflow-x-auto rounded-2xl border border-border bg-surface shadow-xs">
          <table className="w-full text-left text-xs">
            <thead className="border-b border-border bg-paper text-[10px] font-extrabold uppercase tracking-wider text-ink-muted">
              <tr>
                <th className="px-4 py-3">Símbolo</th>
                <th className="px-4 py-3">Nome</th>
                <th className="px-4 py-3">1.0 moeda = `amount`</th>
                <th className="px-4 py-3">Confirmações</th>
                <th className="px-4 py-3">Gateway de depósito</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border">
              {COINS.map((c) => {
                const conf = COIN_CONFIG[c];
                const paused = isDepositWithdrawPaused(c);
                return (
                  <tr key={c} className={clsx('hover:bg-paper/50', paused && 'opacity-70')}>
                    <td className="px-4 py-3 font-mono font-bold text-ink">{c}</td>
                    <td className="px-4 py-3 text-ink-muted">{conf.name}</td>
                    <td className="px-4 py-3 font-mono text-ink">{toLedgerUnits(1)}</td>
                    <td className="px-4 py-3 font-mono text-ink-muted">{conf.minConfirmations}</td>
                    <td className="px-4 py-3">
                      {paused ? (
                        <span className="rounded-full bg-amber-500/10 px-2 py-0.5 text-[10px] font-bold text-amber-700">
                          Pausado · 503 DEPOSIT_PAUSED
                        </span>
                      ) : (
                        <span className="rounded-full bg-emerald-500/10 px-2 py-0.5 text-[10px] font-bold text-emerald-700">
                          Ativo
                        </span>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>

        <p className="text-[11px] text-ink-muted leading-relaxed">
          Moedas pausadas continuam listadas porque voltarão a ser aceitas, mas hoje{' '}
          <code className="font-mono text-ink font-bold">POST /v1/merchant/deposits</code> responde{' '}
          <code className="font-mono text-ink font-bold">503 DEPOSIT_PAUSED</code> para elas. A pausa{' '}
          <b>não</b> afeta <code className="font-mono text-ink font-bold">POST /v1/public/send</code> (payouts em ledger),
          que continua funcionando para todas as moedas.
        </p>
      </section>
    </div>
  );
}
