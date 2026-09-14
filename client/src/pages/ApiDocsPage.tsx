import { useState, useEffect } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { motion, AnimatePresence } from 'framer-motion';
import { clsx } from 'clsx';
import { COINS, COIN_CONFIG } from '@/shared';

type MainTab = 'deposits' | 'payouts' | 'oauth' | 'security' | 'simulator';
type CodeLang = 'curl' | 'js' | 'python' | 'php' | 'go' | 'rust';

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
  const [secret, setSecret] = useState('sats_sec_demo_98234791823791823');
  const [timestamp, setTimestamp] = useState(() => Math.floor(Date.now() / 1000).toString());
  const [payload, setPayload] = useState(() =>
    JSON.stringify(
      {
        event: 'invoice.paid',
        invoiceId: '550e8400-e29b-41d4-a716-446655440000',
        orderId: 'ORD-99821',
        coin: 'USDT',
        amount: '25.00',
        status: 'CONFIRMED',
        depositAddress: '0x71C6705624342490cf03323decB0C392A8892A88',
        txHash: '0x4a8f9c2d1e0b3a7f8e6c5d4b3a2f1e0d9c8b7a6f5e4d3c2b1a0f9e8d7c6b5a4',
        timestamp: Math.floor(Date.now() / 1000),
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
    const clean = verificationInput.replace(/^v1=/, '').trim();
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
            Calcule e depure em tempo real a validação de assinaturas para testar seu endpoint receptor antes de ir para produção.
          </p>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* INPUTS */}
        <div className="space-y-4">
          <div className="space-y-1.5">
            <label className="text-xs font-bold text-ink uppercase tracking-wider block">
              1. API Secret da sua Chave (Chave Secreta)
            </label>
            <input
              type="text"
              value={secret}
              onChange={(e) => setSecret(e.target.value)}
              placeholder="sats_sec_..."
              className="w-full rounded-xl border border-border bg-surface px-3.5 py-2 font-mono text-xs text-ink focus:outline-none focus:ring-2 focus:ring-bitcoin/30"
            />
          </div>

          <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <label className="text-xs font-bold text-ink uppercase tracking-wider block">
                2. Payload JSON do Webhook (Body Raw)
              </label>
              <button
                type="button"
                onClick={() => setTimestamp(Math.floor(Date.now() / 1000).toString())}
                className="text-[11px] text-bitcoin font-bold hover:underline"
              >
                Atualizar Timestamp
              </button>
            </div>
            <textarea
              rows={10}
              value={payload}
              onChange={(e) => setPayload(e.target.value)}
              className="w-full rounded-xl border border-border bg-surface p-3 font-mono text-[11px] leading-relaxed text-ink focus:outline-none focus:ring-2 focus:ring-bitcoin/30 [scrollbar-width:none]"
            />
          </div>
        </div>

        {/* OUTPUTS & VERIFIER */}
        <div className="space-y-4 flex flex-col justify-between">
          <div className="space-y-3">
            <label className="text-xs font-bold text-ink uppercase tracking-wider block">
              3. Cabeçalhos HTTP Gerados pela SatsPay
            </label>

            <div className="space-y-2 rounded-2xl bg-surface p-4 border border-border text-xs font-mono">
              <div>
                <span className="text-ink-muted font-bold block mb-0.5">X-SatsPay-Timestamp:</span>
                <code className="text-emerald-600 font-bold bg-paper px-2 py-1 rounded border border-border block truncate">
                  {timestamp}
                </code>
              </div>

              <div>
                <span className="text-ink-muted font-bold block mb-0.5">X-SatsPay-Signature:</span>
                <code className="text-indigo-600 font-bold bg-paper px-2 py-1 rounded border border-border block break-all text-[11px]">
                  t={timestamp},v1={calculatedSig}
                </code>
              </div>

              <div>
                <span className="text-ink-muted font-bold block mb-0.5">Digest HMAC-SHA256 Puro (v1):</span>
                <code className="text-amber-600 font-bold bg-paper px-2 py-1 rounded border border-border block break-all text-[11px]">
                  {calculatedSig}
                </code>
              </div>
            </div>
          </div>

          <div className="rounded-2xl border border-border bg-paper p-4 space-y-3 shadow-xs">
            <span className="text-xs font-bold text-ink uppercase tracking-wider block">
              4. Testar Validação no seu Servidor
            </span>
            <div className="flex gap-2">
              <input
                type="text"
                value={verificationInput}
                onChange={(e) => {
                  setVerificationInput(e.target.value);
                  setIsValid(null);
                }}
                placeholder="Cole aqui a assinatura gerada pelo seu código..."
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
                <span>Assinatura divergente! Verifique o Secret ou a codificação UTF-8 do body.</span>
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
                  2. A SatsPay retorna uma fatura com o link oficial <code className="font-mono text-ink font-bold">checkoutUrl: "https://satspay.pro/pay/:invoice_id"</code>.<br />
                  3. Você redireciona o usuário para este link sob o domínio seguro SatsPay (com TLS 1.3 e proteção anti-tampering).<br />
                  4. Quando o pagamento for liquidado, nosso gateway dispara uma notificação assinada via Webhook e redireciona o cliente para sua <code className="font-mono text-ink font-bold">successUrl</code>.
                </p>
              </div>

              {/* ENDPOINT 1.1: CRIAR COBRANÇA */}
              <div className="space-y-4">
                <EndpointHeader
                  method="POST"
                  path="/v1/merchant/deposits"
                  title="1.1 Criar Cobrança de Depósito (Gerar Fatura / Invoice)"
                  badge="Idempotente"
                />
                <p className="text-xs sm:text-sm text-ink-muted">
                  Cria uma fatura segura com endereço de depósito dedicado, link público de checkout hospedado no domínio oficial <code className="font-mono font-bold text-ink">https://satspay.pro/pay/:id</code> e QR Code pronto.
                </p>

                <ParamsTable
                  params={[
                    { name: 'coin', type: 'String', required: true, desc: 'Criptomoeda do depósito (BTC, LTC, DOGE, BCH, POL, DGB, SOL, USDT, USDC).', example: '"USDT"' },
                    { name: 'amount', type: 'String', required: true, desc: 'Quantidade exata de criptoativos a receber no depósito (ex: "25.00").', example: '"25.00"' },
                    { name: 'orderId', type: 'String', required: true, desc: 'Identificador único do pedido no seu sistema para conciliação.', example: '"ORD-99821"' },
                    { name: 'callbackUrl', type: 'String', required: true, desc: 'URL HTTPS do seu servidor onde o webhook assinado com HMAC será disparado.', example: '"https://meusite.com/webhook"' },
                    { name: 'successUrl', type: 'String', required: false, desc: 'URL de retorno após o cliente pagar com sucesso no checkout.', example: '"https://meusite.com/obrigado"' },
                    { name: 'customerEmail', type: 'String', required: false, desc: 'E-mail do cliente para notificações e recibo.', example: '"cliente@email.com"' },
                    { name: 'siteName', type: 'String', required: false, desc: 'Nome da sua loja para exibir no topo da página de pagamento.', example: '"Minha Loja"' },
                  ]}
                />

                <div className="space-y-3">
                  <h4 className="text-xs font-bold uppercase tracking-wider text-ink-muted">
                    Exemplo de Requisição:
                  </h4>
                  <MultiLangCodeBlock
                    snippets={{
                      curl: `curl -X POST https://satspay.pro/v1/merchant/deposits \\
  -H "x-api-key: SUA_CHAVE_DE_API" \\
  -H "Content-Type: application/json" \\
  -d '{
    "coin": "USDT",
    "amount": "25.00",
    "orderId": "ORD-99821",
    "callbackUrl": "https://meusite.com/api/webhook",
    "customerEmail": "cliente@email.com",
    "siteName": "Minha Loja Online"
  }'`,
                      js: `const response = await fetch('https://satspay.pro/v1/merchant/deposits', {
  method: 'POST',
  headers: {
    'x-api-key': 'SUA_CHAVE_DE_API',
    'Content-Type': 'application/json'
  },
  body: JSON.stringify({
    coin: 'USDT',
    amount: '25.00',
    orderId: 'ORD-99821',
    callbackUrl: 'https://meusite.com/api/webhook',
    customerEmail: 'cliente@email.com',
    siteName: 'Minha Loja Online'
  })
});

const data = await response.json();
// Redirecione seu cliente com segurança para o checkout hospedado na infraestrutura SatsPay:
window.location.href = data.checkoutUrl;`,
                      python: `import requests

url = "https://satspay.pro/v1/merchant/deposits"
headers = {
    "x-api-key": "SUA_CHAVE_DE_API",
    "Content-Type": "application/json"
}
payload = {
    "coin": "USDT",
    "amount": "25.00",
    "orderId": "ORD-99821",
    "callbackUrl": "https://meusite.com/api/webhook",
    "customerEmail": "cliente@email.com",
    "siteName": "Minha Loja Online"
}

response = requests.post(url, json=payload, headers=headers)
invoice = response.json()
print("Redirecionar Cliente Para:", invoice.get("checkoutUrl"))`,
                      php: `<?php
$curl = curl_init();

$payload = [
  "coin" => "USDT",
  "amount" => "25.00",
  "orderId" => "ORD-99821",
  "callbackUrl" => "https://meusite.com/api/webhook",
  "customerEmail" => "cliente@email.com",
  "siteName" => "Minha Loja Online"
];

curl_setopt_array($curl, [
  CURLOPT_URL => "https://satspay.pro/v1/merchant/deposits",
  CURLOPT_RETURNTRANSFER => true,
  CURLOPT_POST => true,
  CURLOPT_POSTFIELDS => json_encode($payload),
  CURLOPT_HTTPHEADER => [
    "x-api-key: SUA_CHAVE_DE_API",
    "Content-Type: application/json"
  ],
]);

$response = curl_exec($curl);
$data = json_decode($response, true);
header("Location: " . $data['checkoutUrl']);
exit;`,
                      go: `package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net/http"
)

func main() {
	payload, _ := json.Marshal(map[string]interface{}{
		"coin":          "USDT",
		"amount":        "25.00",
		"orderId":       "ORD-99821",
		"callbackUrl":   "https://meusite.com/api/webhook",
		"customerEmail": "cliente@email.com",
		"siteName":      "Minha Loja Online",
	})

	req, _ := http.NewRequest("POST", "https://satspay.pro/v1/merchant/deposits", bytes.NewBuffer(payload))
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
        .post("https://satspay.pro/v1/merchant/deposits")
        .header("x-api-key", "SUA_CHAVE_DE_API")
        .json(&json!({
            "coin": "USDT",
            "amount": "25.00",
            "orderId": "ORD-99821",
            "callbackUrl": "https://meusite.com/api/webhook",
            "customerEmail": "cliente@email.com",
            "siteName": "Minha Loja Online"
        }))
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    println!("Checkout URL: {}", res["checkoutUrl"]);
    Ok(())
}`,
                    }}
                  />
                </div>

                <div className="space-y-2">
                  <h4 className="text-xs font-bold uppercase tracking-wider text-ink-muted">
                    Resposta de Sucesso (200 OK):
                  </h4>
                  <CodeBlock
                    label="JSON RESPONSE"
                    code={`{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "orderId": "ORD-99821",
  "coin": "USDT",
  "amount": "25.00",
  "depositAddress": "0x71C6705624342490cf03323decB0C392A8892A88",
  "qrCode": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAMgAAADICAY...",
  "checkoutUrl": "https://satspay.pro/pay/550e8400-e29b-41d4-a716-446655440000",
  "status": "PENDING",
  "expiresAt": "2026-09-02T18:00:00.000Z",
  "createdAt": "2026-09-02T16:00:00.000Z"
}`}
                  />
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
                  Retorna o estado em tempo real da fatura (PENDING, DETECTED, CONFIRMED, PAID, EXPIRED).
                </p>

                <CodeBlock
                  label="CURL"
                  code={`curl -X GET https://satspay.pro/v1/merchant/deposits/550e8400-e29b-41d4-a716-446655440000 \\
  -H "x-api-key: SUA_CHAVE_DE_API"`}
                />
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
                  code={`curl -X GET https://satspay.pro/v1/public/balance \\
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

                <ParamsTable
                  params={[
                    { name: 'coin', type: 'String', required: true, desc: 'Criptomoeda a transferir (USDT, USDC, BTC, SOL, POL, LTC, DOGE, BCH, DGB).', example: '"USDT"' },
                    { name: 'toEmail', type: 'String', required: true, desc: 'E-mail cadastrado na conta SatsPay do usuário destinatário.', example: '"usuario@email.com"' },
                    { name: 'amount', type: 'String', required: true, desc: 'Quantidade na menor fração / 8 decimais (ex: "100000000" = 1.00 moeda).', example: '"1000000000"' },
                    { name: 'idempotencyKey', type: 'String', required: true, desc: 'ID único da sua operação para evitar cobrança ou envio duplicado.', example: '"payout_ord_99812"' },
                  ]}
                />

                <MultiLangCodeBlock
                  snippets={{
                    curl: `curl -X POST https://satspay.pro/v1/public/send \\
  -H "x-api-key: SUA_CHAVE_DE_API" \\
  -H "Content-Type: application/json" \\
  -d '{
    "coin": "USDT",
    "toEmail": "cliente@email.com",
    "amount": "1000000000",
    "idempotencyKey": "payout_tx_99821"
  }'`,
                    js: `const res = await fetch('https://satspay.pro/v1/public/send', {
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

url = "https://satspay.pro/v1/public/send"
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
  CURLOPT_URL => "https://satspay.pro/v1/public/send",
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

	req, _ := http.NewRequest("POST", "https://satspay.pro/v1/public/send", bytes.NewBuffer(payload))
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
        .post("https://satspay.pro/v1/public/send")
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
                  path="https://www.satspay.pro/sdk/satspay-auth.v2.js"
                  title="3.1 Integração Frontend via JavaScript SDK"
                  badge="Recomendado"
                />
                <p className="text-xs sm:text-sm text-ink-muted">
                  Cole o código abaixo no seu HTML onde deseja exibir o botão de Login com SatsPay:
                </p>

                <CodeBlock
                  label="HTML SNIPPET"
                  code={`<!-- 1. Carregue o SDK JavaScript oficial do SatsPay -->
<script src="https://www.satspay.pro/sdk/satspay-auth.v2.js" async defer></script>

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
                    curl: `curl -X POST https://www.satspay.pro/v1/oauth/token \\
  -H "Content-Type: application/x-www-form-urlencoded" \\
  -d "grant_type=authorization_code" \\
  -d "code=sats_code_xyz123abc456" \\
  -d "client_id=sats_app_SEU_CLIENT_ID" \\
  -d "client_secret=sats_sec_SEU_CLIENT_SECRET" \\
  -d "redirect_uri=https://seusite.com/auth/callback"`,

                    js: `const response = await fetch('https://www.satspay.pro/v1/oauth/token', {
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

res = requests.post('https://www.satspay.pro/v1/oauth/token', data={
    'grant_type': 'authorization_code',
    'code': auth_code,
    'client_id': SATSPAY_CLIENT_ID,
    'client_secret': SATSPAY_CLIENT_SECRET,
    'redirect_uri': 'https://seusite.com/auth/callback'
})
access_token = res.json()['access_token']`,

                    php: `<?php
$ch = curl_init('https://www.satspay.pro/v1/oauth/token');
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
    resp, err := http.PostForm("https://www.satspay.pro/v1/oauth/token", formData)
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
        .post("https://www.satspay.pro/v1/oauth/token")
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
                    curl: `curl -X GET https://www.satspay.pro/v1/oauth/userinfo \\
  -H "Authorization: Bearer sats_tok_a1b2c3d4e5f6g7h8i9j0"`,

                    js: `const userRes = await fetch('https://www.satspay.pro/v1/oauth/userinfo', {
  headers: {
    'Authorization': \`Bearer \${accessToken}\`
  }
});
const user = await userRes.json();
console.log('ID:', user.sub, 'Username:', user.username, 'Email:', user.email);`,

                    python: `user_res = requests.get('https://www.satspay.pro/v1/oauth/userinfo', headers={
    'Authorization': f'Bearer {access_token}'
})
user = user_res.json()
print(f"Logado como @{user['username']} ({user['email']})")`,

                    rust: `let user_info = reqwest::Client::new()
    .get("https://www.satspay.pro/v1/oauth/userinfo")
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
                  https://www.satspay.pro/.well-known/openid-configuration
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

              {/* 4 PILARES DE SEGURANÇA */}
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className="rounded-2xl border border-border bg-surface p-4 space-y-2">
                  <div className="flex items-center gap-2 text-emerald-600 font-bold text-xs">
                    <i className="bi bi-link-45deg text-base" />
                    <span>1. Checkout Hospedado Seguro (Isolamento de Credenciais)</span>
                  </div>
                  <p className="text-xs text-ink-muted leading-relaxed">
                    O seu cliente é redirecionado com segurança para o domínio oficial <code className="font-mono text-ink font-bold">https://satspay.pro/pay/:id</code>. Suas chaves de API <b>nunca são expostas</b> no navegador do cliente nem no código HTML do seu site.
                  </p>
                </div>

                <div className="rounded-2xl border border-border bg-surface p-4 space-y-2">
                  <div className="flex items-center gap-2 text-blue-600 font-bold text-xs">
                    <i className="bi bi-shield-lock-fill text-base" />
                    <span>2. Assinatura HMAC-SHA256 & Anti-Replay</span>
                  </div>
                  <p className="text-xs text-ink-muted leading-relaxed">
                    Todas as notificações enviadas para a sua URL de Webhook contêm a assinatura <code className="font-mono text-ink font-bold">x-satspay-signature</code> gerada com seu Secret e timestamp. Isso impede fraudes e ataques de repetição.
                  </p>
                </div>

                <div className="rounded-2xl border border-border bg-surface p-4 space-y-2">
                  <div className="flex items-center gap-2 text-bitcoin text-xs font-bold">
                    <i className="bi bi-fingerprint text-base" />
                    <span>3. Restrição por Whitelist de IPs</span>
                  </div>
                  <p className="text-xs text-ink-muted leading-relaxed">
                    Ao cadastrar suas chaves na aba <b>API Keys</b>, você pode restringir o acesso exclusivamente aos endereços IP dos seus servidores. Qualquer requisição vinda de outro IP é bloqueada pelo firewall do gateway.
                  </p>
                </div>

                <div className="rounded-2xl border border-border bg-surface p-4 space-y-2">
                  <div className="flex items-center gap-2 text-purple-600 font-bold text-xs">
                    <i className="bi bi-arrow-repeat text-base" />
                    <span>4. Chave de Idempotência Obrigatória</span>
                  </div>
                  <p className="text-xs text-ink-muted leading-relaxed">
                    Envios e saques exigem o parâmetro <code className="font-mono text-ink font-bold">idempotencyKey</code>. Em caso de oscilações de rede ou reenvios automáticos, a operação é executada exatamente uma única vez, prevenindo transferências duplicadas.
                  </p>
                </div>
              </div>

              {/* CHECKLIST DE SEGURANÇA EM PRODUÇÃO */}
              <div className="rounded-2xl border border-blue-500/20 bg-blue-500/5 p-5 space-y-3">
                <div className="flex items-center gap-2 text-blue-700 font-bold text-sm">
                  <i className="bi bi-check2-square text-base" />
                  <span>Checklist de Segurança Antes de Publicar em Produção:</span>
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5 text-xs text-ink">
                  <div className="flex items-start gap-2 bg-paper/60 p-2.5 rounded-xl border border-border">
                    <i className="bi bi-check-circle-fill text-emerald-600 shrink-0 mt-0.5" />
                    <span>Guardar API Key e Secret exclusivamente em variáveis de ambiente (<code>.env</code>) no backend.</span>
                  </div>
                  <div className="flex items-start gap-2 bg-paper/60 p-2.5 rounded-xl border border-border">
                    <i className="bi bi-check-circle-fill text-emerald-600 shrink-0 mt-0.5" />
                    <span>Cadastrar o IP fixo do seu servidor na Whitelist da API Key no painel.</span>
                  </div>
                  <div className="flex items-start gap-2 bg-paper/60 p-2.5 rounded-xl border border-border">
                    <i className="bi bi-check-circle-fill text-emerald-600 shrink-0 mt-0.5" />
                    <span>Comparar a assinatura HMAC com funções de tempo constante (<code>timingSafeEqual</code> / <code>hash_equals</code>).</span>
                  </div>
                  <div className="flex items-start gap-2 bg-paper/60 p-2.5 rounded-xl border border-border">
                    <i className="bi bi-check-circle-fill text-emerald-600 shrink-0 mt-0.5" />
                    <span>Validar que o timestamp do webhook não difira mais de 300 segundos do horário atual.</span>
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
                  Sempre valide o cabeçalho <code className="font-mono font-bold text-ink">x-satspay-signature</code> antes de liberar produtos ou creditar o usuário no seu banco de dados:
                </p>

                <MultiLangCodeBlock
                  snippets={{
                    js: `import crypto from 'crypto';

// No seu endpoint de Webhook (Express / Next.js / Fastify):
function verifySatsPayWebhook(rawBodyString, signatureHeader, apiSecret) {
  const expectedSignature = crypto
    .createHmac('sha256', apiSecret)
    .update(rawBodyString)
    .digest('hex');

  // Comparação segura contra Timing Attacks:
  return crypto.timingSafeEqual(
    Buffer.from(signatureHeader, 'hex'),
    Buffer.from(expectedSignature, 'hex')
  );
}`,
                    python: `import hmac
import hashlib

def verify_satspay_webhook(raw_body_bytes, signature_header, api_secret):
    expected = hmac.new(
        api_secret.encode('utf-8'),
        raw_body_bytes,
        hashlib.sha256
    ).hexdigest()
    
    # Comparação timing-safe:
    return hmac.compare_digest(signature_header, expected)`,
                    php: `<?php
function verifySatsPayWebhook($rawBody, $signatureHeader, $apiSecret) {
    $expected = hash_hmac('sha256', $rawBody, $apiSecret);
    return hash_equals($signatureHeader, $expected);
}`,
                    go: `package main

import (
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
)

func VerifyWebhook(rawBody []byte, signatureHeader string, apiSecret string) bool {
	mac := hmac.New(sha256.New, []byte(apiSecret))
	mac.Write(rawBody)
	expected := hex.EncodeToString(mac.Sum(nil))
	return hmac.Equal([]byte(signatureHeader), []byte(expected))
}`,
                    rust: `use hmac::{Hmac, Mac};
use sha2::Sha256;

pub fn verify_webhook(raw_body: &[u8], signature_hex: &str, secret: &str) -> bool {
    let mut mac = match Hmac::<Sha256>::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(raw_body);
    let expected = hex::encode(mac.finalize().into_bytes());
    // Comparação timing-safe via constante:
    subtle::ConstantTimeEq::ct_eq(signature_hex.as_bytes(), expected.as_bytes()).into()
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
                      <tr>
                        <td className="px-4 py-2.5 font-bold text-rose-600">INVALID_API_KEY</td>
                        <td className="px-4 py-2.5 text-ink">401</td>
                        <td className="px-4 py-2.5 font-sans text-ink-muted">Chave inexistente ou desativada.</td>
                        <td className="px-4 py-2.5 font-sans text-ink">Gere uma nova chave no menu API Keys.</td>
                      </tr>
                      <tr>
                        <td className="px-4 py-2.5 font-bold text-rose-600">IP_NOT_WHITELISTED</td>
                        <td className="px-4 py-2.5 text-ink">401 / 403</td>
                        <td className="px-4 py-2.5 font-sans text-ink-muted">IP do servidor não está na lista permitida.</td>
                        <td className="px-4 py-2.5 font-sans text-ink">Adicione o IP do seu host na configuração da chave.</td>
                      </tr>
                      <tr>
                        <td className="px-4 py-2.5 font-bold text-amber-600">INSUFFICIENT_FUNDS</td>
                        <td className="px-4 py-2.5 text-ink">400</td>
                        <td className="px-4 py-2.5 font-sans text-ink-muted">Saldo da carteira comercial insuficiente.</td>
                        <td className="px-4 py-2.5 font-sans text-ink">Efetue um depósito na moeda correspondente.</td>
                      </tr>
                      <tr>
                        <td className="px-4 py-2.5 font-bold text-purple-600">DUPLICATE_ORDER_ID</td>
                        <td className="px-4 py-2.5 text-ink">409</td>
                        <td className="px-4 py-2.5 font-sans text-ink-muted">O orderId já foi utilizado em fatura anterior.</td>
                        <td className="px-4 py-2.5 font-sans text-ink">Envie um identificador único para cada pedido.</td>
                      </tr>
                      <tr>
                        <td className="px-4 py-2.5 font-bold text-blue-600">RATE_LIMIT_EXCEEDED</td>
                        <td className="px-4 py-2.5 text-ink">429</td>
                        <td className="px-4 py-2.5 font-sans text-ink-muted">Limite de requisições por minuto excedido.</td>
                        <td className="px-4 py-2.5 font-sans text-ink">Adote retry com exponential backoff e jitter.</td>
                      </tr>
                      <tr>
                        <td className="px-4 py-2.5 font-bold text-amber-600">SIGNATURE_MISMATCH</td>
                        <td className="px-4 py-2.5 text-ink">401</td>
                        <td className="px-4 py-2.5 font-sans text-ink-muted">Assinatura HMAC incorreta.</td>
                        <td className="px-4 py-2.5 font-sans text-ink">Utilize o Simulador HMAC da aba 5 para depurar.</td>
                      </tr>
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

      {/* REFERÊNCIA COMPARTILHADA (MOEDAS 100% 8 DECIMAIS) */}
      <section className="rounded-3xl border border-border bg-paper p-6 sm:p-8 shadow-xs space-y-6">
        <div className="flex items-center gap-2.5 border-b border-border/80 pb-3">
          <i className="bi bi-info-circle-fill text-bitcoin text-xl" />
          <h3 className="text-lg sm:text-xl font-black text-ink">Moedas Suportadas & Decimais (Padrão 8 Decimais)</h3>
        </div>

        <p className="text-xs text-ink-muted">
          Na plataforma SatsPay, todas as 9 criptomoedas operam sob o padrão universal de <b>8 casas decimais</b> (<code className="font-mono font-bold text-ink">10^-8</code> / escala Satoshi), simplificando cálculos contábeis e garantindo precisão em micropagamentos.
        </p>

        <div className="overflow-x-auto rounded-2xl border border-border bg-surface shadow-xs">
          <table className="w-full text-left text-xs">
            <thead className="border-b border-border bg-paper text-[10px] font-extrabold uppercase tracking-wider text-ink-muted">
              <tr>
                <th className="px-4 py-3">Símbolo</th>
                <th className="px-4 py-3">Nome</th>
                <th className="px-4 py-3">Decimais</th>
                <th className="px-4 py-3">Menor Unidade</th>
                <th className="px-4 py-3">Exemplo (1.0 Moeda)</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border">
              {COINS.map((c) => {
                const conf = COIN_CONFIG[c];
                const factor = Math.pow(10, conf.decimals);
                return (
                  <tr key={c} className="hover:bg-paper/50">
                    <td className="px-4 py-3 font-mono font-bold text-ink">{c}</td>
                    <td className="px-4 py-3 text-ink-muted">{conf.name}</td>
                    <td className="px-4 py-3 font-mono text-ink-muted">{conf.decimals}</td>
                    <td className="px-4 py-3 font-mono text-xs text-bitcoin-dark">10^-{conf.decimals}</td>
                    <td className="px-4 py-3 font-mono text-ink">{factor.toLocaleString('en-US')}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  );
}
