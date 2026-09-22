import { useState } from 'react';
import { Link } from 'react-router-dom';

type DocSection = 'overview' | 'ledger' | 'faucet' | 'swap' | 'stake' | 'lend' | 'merchant' | 'oauth_sso' | 'security';

const DOC_TABS: { id: DocSection; label: string; icon: string }[] = [
  { id: 'overview', label: 'Visão Geral & Moedas', icon: 'bi-grid-fill' },
  { id: 'oauth_sso', label: 'Login com SatsPay (SSO)', icon: 'bi-shield-lock-fill' },
  { id: 'merchant', label: 'Comerciantes & APIs', icon: 'bi-shop' },
  { id: 'ledger', label: 'Ledger Contábil', icon: 'bi-journal-check' },
  { id: 'faucet', label: 'Faucet & Faucetlist', icon: 'bi-droplet-fill' },
  { id: 'swap', label: 'Câmbio (Swap)', icon: 'bi-arrow-left-right' },
  { id: 'stake', label: 'Staking', icon: 'bi-graph-up-arrow' },
  { id: 'lend', label: 'Mercado de Empréstimos', icon: 'bi-bank' },
  { id: 'security', label: 'Segurança & Custódia', icon: 'bi-shield-check' },
];

export function DocumentationPage() {
  const [activeSection, setActiveSection] = useState<DocSection>('overview');

  return (
    <div className="mx-auto max-w-6xl px-4 py-8 md:px-6 md:py-10">
      {/* Header */}
      <div className="mb-8 border-b border-border pb-6">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <h1 className="text-3xl font-bold tracking-tight text-ink">Documentação da Plataforma</h1>
            <p className="mt-1 text-sm text-ink-muted">
              Guia completo de ponta a ponta sobre o funcionamento, contabilidade, APIs e integrações do BitcoSats.
            </p>
          </div>
          <div className="flex gap-2">
            <Link to="/docs" className="btn-primary text-xs flex items-center gap-1.5">
              <i className="bi bi-code-slash" />
              <span>Docs da API REST</span>
            </Link>
            <Link to="/status" className="btn-secondary text-xs flex items-center gap-1.5">
              <i className="bi bi-activity text-emerald-500" />
              <span>Uptime & Status</span>
            </Link>
          </div>
        </div>

        {/* Section Navigation Tabs */}
        <div className="mt-6 flex flex-wrap gap-2">
          {DOC_TABS.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveSection(tab.id)}
              className={`flex items-center gap-2 rounded-lg px-3.5 py-2 text-xs font-semibold transition-all ${
                activeSection === tab.id
                  ? 'bg-bitcoin text-white shadow-md shadow-bitcoin/20'
                  : 'bg-paper text-ink-muted hover:bg-surface hover:text-ink border border-border'
              }`}
            >
              <i className={`bi ${tab.icon}`} />
              <span>{tab.label}</span>
            </button>
          ))}
        </div>
      </div>

      {/* Content Area */}
      <div className="grid gap-8 lg:grid-cols-4">
        {/* Main Content */}
        <div className="lg:col-span-3 space-y-6">
          {activeSection === 'overview' && (
            <div className="card p-6 space-y-6">
              <div className="flex items-center gap-3 border-b border-border pb-4">
                <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-bitcoin/10 text-bitcoin-dark text-xl font-bold">
                  <i className="bi bi-grid-fill" />
                </div>
                <div>
                  <h2 className="text-xl font-bold text-ink">Visão Geral da Plataforma</h2>
                  <p className="text-xs text-ink-muted">Custódia multi-moeda e utilidades cripto</p>
                </div>
              </div>

              <div className="prose text-sm text-ink-muted space-y-4">
                <p>
                  O <strong>BitcoSats</strong> é uma plataforma moderna de custódia e utilidades cripto, desenvolvida com foco em integridade matemática, alta velocidade e proteção contra condições de corrida. A plataforma suporta nativamente 5 redes principais:
                </p>

                <div className="grid gap-3 sm:grid-cols-2 not-prose my-4">
                  <div className="rounded-xl border border-border bg-surface p-4">
                    <div className="flex items-center gap-2 font-bold text-ink text-sm">
                      <span className="flex h-6 w-6 items-center justify-center rounded-full bg-amber-500/10 text-amber-600">₿</span>
                      Bitcoin (BTC)
                    </div>
                    <p className="mt-1 text-xs text-ink-muted">Rede principal e SegWit Nativo (Bech32). Menor unidade: 1 Satoshi.</p>
                  </div>
                  <div className="rounded-xl border border-border bg-surface p-4">
                    <div className="flex items-center gap-2 font-bold text-ink text-sm">
                      <span className="flex h-6 w-6 items-center justify-center rounded-full bg-blue-500/10 text-blue-600">Ł</span>
                      Litecoin (LTC)
                    </div>
                    <p className="mt-1 text-xs text-ink-muted">Transações rápidas com SegWit. Menor unidade: 1 Litoshi.</p>
                  </div>
                  <div className="rounded-xl border border-border bg-surface p-4">
                    <div className="flex items-center gap-2 font-bold text-ink text-sm">
                      <span className="flex h-6 w-6 items-center justify-center rounded-full bg-amber-400/10 text-amber-500">Ð</span>
                      Dogecoin (DOGE)
                    </div>
                    <p className="mt-1 text-xs text-ink-muted">Taxas ultra-baixas e alta liquidez. Menor unidade: 1 Koinu.</p>
                  </div>
                  <div className="rounded-xl border border-border bg-surface p-4">
                    <div className="flex items-center gap-2 font-bold text-ink text-sm">
                      <span className="flex h-6 w-6 items-center justify-center rounded-full bg-emerald-500/10 text-emerald-600">Ƀ</span>
                      Bitcoin Cash (BCH)
                    </div>
                    <p className="mt-1 text-xs text-ink-muted">Formato CashAddr com suporte a SIGHASH_FORKID.</p>
                  </div>
                  <div className="rounded-xl border border-border bg-surface p-4 sm:col-span-2">
                    <div className="flex items-center gap-2 font-bold text-ink text-sm">
                      <span className="flex h-6 w-6 items-center justify-center rounded-full bg-purple-500/10 text-purple-600">⬡</span>
                      Polygon (POL)
                    </div>
                    <p className="mt-1 text-xs text-ink-muted">Camada EVM com Smart Contracts e taxas mínimas em padrão EIP-155 / EIP-55.</p>
                  </div>
                </div>

                <h3 className="text-base font-bold text-ink pt-2">Estrutura de Contas</h3>
                <p>
                  Cada usuário possui carteiras segregadas por moeda e propósito:
                </p>
                <ul className="list-disc pl-5 space-y-1">
                  <li><strong>Carteiras Pessoais (PERSONAL):</strong> Para custódia individual, transferências gratuitas instantâneas, saques on-chain e depósitos.</li>
                  <li><strong>Carteiras de Desenvolvedor (DEVELOPER):</strong> Isoladas para recebimento de taxas de sites do Faucet e integrações via API B2B.</li>
                </ul>
              </div>
            </div>
          )}

          {activeSection === 'ledger' && (
            <div className="card p-6 space-y-6">
              <div className="flex items-center gap-3 border-b border-border pb-4">
                <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-emerald-500/10 text-emerald-600 text-xl font-bold">
                  <i className="bi bi-journal-check" />
                </div>
                <div>
                  <h2 className="text-xl font-bold text-ink">Arquitetura do Ledger Contábil</h2>
                  <p className="text-xs text-ink-muted">Razão contábil imutável e modelo Zero-Balance Column</p>
                </div>
              </div>

              <div className="prose text-sm text-ink-muted space-y-4">
                <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4 text-xs text-emerald-800">
                  <strong>Regra de Ouro Contábil:</strong> Saldos não são colunas editáveis. O saldo de cada carteira é sempre o somatório dos lançamentos imutáveis no razão (partidas dobradas).
                </div>

                <h3 className="text-base font-bold text-ink pt-2">Garantia contra Race Conditions</h3>
                <p>
                  Antes de qualquer movimentação de fundos (saque, swap, transferências), a plataforma serializa operações na mesma carteira no banco de dados. Isso impede gasto duplo (*double-spend*) e condições de corrida entre requisições paralelas.
                </p>

                <h3 className="text-base font-bold text-ink pt-2">Precisão Absoluta</h3>
                <p>
                  Valores monetários usam aritmética decimal de precisão arbitrária de ponta a ponta — sem ponto flutuante — para não perder satoshis por arredondamento.
                </p>
              </div>
            </div>
          )}

          {activeSection === 'faucet' && (
            <div className="card p-6 space-y-6">
              <div className="flex items-center gap-3 border-b border-border pb-4">
                <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-cyan-500/10 text-cyan-600 text-xl font-bold">
                  <i className="bi bi-droplet-fill" />
                </div>
                <div>
                  <h2 className="text-xl font-bold text-ink">Faucet & Diretório Faucetlist</h2>
                  <p className="text-xs text-ink-muted">Distribuição gratuita de satoshis e curadoria de sites parceiros</p>
                </div>
              </div>

              <div className="prose text-sm text-ink-muted space-y-4">
                <p>
                  O <strong>Faucet</strong> permite que usuários resgatem frações gratuitas de criptomoedas periodicamente. As recompensas saem do inventário operacional da plataforma, com limites por conta.
                </p>
                
                <h3 className="text-base font-bold text-ink pt-2">Proteção Anti-Sybil e Automação</h3>
                <ul className="list-disc pl-5 space-y-1">
                  <li><strong>Captcha:</strong> proteção anti-bot na solicitação de resgate.</li>
                  <li><strong>Anti-replay:</strong> tokens de desafio não podem ser reutilizados.</li>
                  <li><strong>Cooldown:</strong> intervalo mínimo entre resgates por conta (e controles adicionais contra abuso).</li>
                </ul>

                <h3 className="text-base font-bold text-ink pt-2">Faucetlist Pública</h3>
                <p>
                  Comerciantes aprovados podem cadastrar seus próprios sites no diretório global de faucets. Cada clique é contabilizado e direcionado com verificação de segurança contra domínios maliciosos.
                </p>
              </div>
            </div>
          )}

          {activeSection === 'swap' && (
            <div className="card p-6 space-y-6">
              <div className="flex items-center gap-3 border-b border-border pb-4">
                <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-bitcoin/10 text-bitcoin-dark text-xl font-bold">
                  <i className="bi bi-arrow-left-right" />
                </div>
                <div>
                  <h2 className="text-xl font-bold text-ink">Swap Custodial Cross-Chain</h2>
                  <p className="text-xs text-ink-muted">Liquidez SwapKit (THORChain, Chainflip, Jupiter…) + pool SatsPay</p>
                </div>
              </div>

              <div className="prose text-sm text-ink-muted space-y-4">
                <p>
                  O <strong>Swap</strong> é custodial: você troca saldo na conta; a plataforma executa a rota on-chain via{' '}
                  <strong>1inch / SwapKit</strong>, <strong>Relay</strong> (Polygon ↔ SOL) ou <strong>ChangeNOW</strong> (L1).
                </p>
                <h3 className="text-base font-bold text-ink pt-2">Swap vs Bridge</h3>
                <ul className="list-disc pl-5 space-y-1">
                  <li><strong>Swap</strong> (aba Swap): mesma rede — só POL ↔ USDT ↔ USDC na Polygon (DEX / 1inch).</li>
                  <li><strong>Bridge</strong> (aba Bridge): redes distintas — Solana ↔ Polygon (Relay) ou L1 nativa ↔ outra moeda (ChangeNOW).</li>
                </ul>
                <h3 className="text-base font-bold text-ink pt-2">Rede de cada moeda na SatsPay</h3>
                <ul className="list-disc pl-5 space-y-1">
                  <li><strong>POL, USDT, USDC:</strong> Polygon PoS (não Ethereum).</li>
                  <li><strong>SOL:</strong> Solana.</li>
                  <li><strong>BTC, LTC, DOGE, BCH, DGB:</strong> rede nativa de cada uma (ChangeNOW).</li>
                  <li><strong>PEPE:</strong> BNB Smart Chain (BEP-20). Depósito/saque ativos; gas em BNB. Swap cross-chain em breve.</li>
                </ul>
                <h3 className="text-base font-bold text-ink pt-2">Taxas transparentes</h3>
                <ul className="list-disc pl-5 space-y-1">
                  <li><strong>Taxa de rede / provedor:</strong> inbound, network, outbound, liquidity — exibidas no quote.</li>
                  <li><strong>Taxa SatsPay:</strong> 0,25% (same-chain e cross-chain) — diferencial da plataforma.</li>
                </ul>
                <h3 className="text-base font-bold text-ink pt-2">Idempotência</h3>
                <p>
                  Toda solicitação aceita chave de idempotência. Cliques duplos retornam a ordem original sem débitos duplicados.
                </p>
              </div>
            </div>
          )}

          {activeSection === 'stake' && (
            <div className="card p-6 space-y-6">
              <div className="flex items-center gap-3 border-b border-border pb-4">
                <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-purple-500/10 text-purple-600 text-xl font-bold">
                  <i className="bi bi-graph-up-arrow" />
                </div>
                <div>
                  <h2 className="text-xl font-bold text-ink">Contratos de Staking</h2>
                  <p className="text-xs text-ink-muted">Rendimento sobre saldo com bloqueio de prazos flexíveis</p>
                </div>
              </div>

              <div className="prose text-sm text-ink-muted space-y-4">
                <p>
                  Usuários podem alocar saldos em contratos de staking com prazos predefinidos (ex: 30, 60 ou 90 dias) para auferir rendimentos competitivos em taxa anual (APY).
                </p>
                <ul className="list-disc pl-5 space-y-1">
                  <li><strong>Bloqueio Seguro:</strong> O principal é debitado da carteira pessoal (<code>STAKE_LOCK</code>) e mantido sob custódia protegida.</li>
                  <li><strong>Resgate no Vencimento:</strong> Ao atingir a data de maturação (<code>matures_at</code>), o usuário resgata o principal mais a recompensa acumulada em um único clique (<code>STAKE_REWARD</code>).</li>
                  <li><strong>Cancelamento Antecipado:</strong> Permite devolver o principal integral à carteira a qualquer momento antes do vencimento (sem pagamento do rendimento proporcional).</li>
                </ul>
              </div>
            </div>
          )}

          {activeSection === 'lend' && (
            <div className="card p-6 space-y-6">
              <div className="flex items-center gap-3 border-b border-border pb-4">
                <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-indigo-500/10 text-indigo-600 text-xl font-bold">
                  <i className="bi bi-bank" />
                </div>
                <div>
                  <h2 className="text-xl font-bold text-ink">Mercado Monetário de Empréstimos (Lending)</h2>
                  <p className="text-xs text-ink-muted">Fornecimento de liquidez colateralizada e empréstimos descentralizados</p>
                </div>
              </div>

              <div className="prose text-sm text-ink-muted space-y-4">
                <p>
                  O módulo de <strong>Lending</strong> opera de forma similar aos protocolos Aave e Compound, utilizando índices de liquidez cumulativos com precisão Ray ($10^{18}$).
                </p>
                <h3 className="text-base font-bold text-ink pt-2">Fator de Saúde (Health Factor)</h3>
                <p>
                  O índice de saúde financeira mede a segurança da sua posição colateralizada:
                </p>
                <pre className="rounded-lg bg-surface p-3 text-xs text-ink overflow-x-auto border border-border">
                  <code>Health Factor = (Total Colateral em USD * LTV Médio) / Total Dívida em USD</code>
                </pre>
                <ul className="list-disc pl-5 space-y-1">
                  <li><strong>HF &gt; 1.5:</strong> Posição saudável com folga confortável contra flutuações de mercado.</li>
                  <li><strong>1.0 &lt; HF &le; 1.5:</strong> Risco moderado. Recomendado aportar colateral ou amortizar dívida.</li>
                  <li><strong>HF &lt; 1.0:</strong> Posição sujeita à liquidação automática do colateral para quitação do débito.</li>
                </ul>
              </div>
            </div>
          )}

          {activeSection === 'merchant' && (
            <div className="card p-6 space-y-6">
              <div className="flex items-center gap-3 border-b border-border pb-4">
                <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-emerald-500/10 text-emerald-600 text-xl font-bold">
                  <i className="bi bi-shop" />
                </div>
                <div>
                  <h2 className="text-xl font-bold text-ink">Sistemas para Comerciantes & Desenvolvedores</h2>
                  <p className="text-xs text-ink-muted">Gateway de cobranças, checkout hospedado, chaves de API e webhooks assinados</p>
                </div>
              </div>

              <div className="prose text-sm text-ink-muted space-y-4">
                <p>
                  O SatsPay entrega duas coisas diferentes para quem vende: um <strong>gateway de
                  cobranças</strong> com checkout hospedado, e uma <strong>API de envios</strong> para
                  automatizar pagamentos. As duas usam a mesma credencial de servidor.
                </p>

                <h3 className="text-base font-bold text-ink pt-2">Enviar para a conta SatsPay do seu cliente</h3>
                <p>
                  <code>POST /v1/public/send</code> debita a sua carteira comercial e credita a conta
                  SatsPay do e-mail em <code>toEmail</code>. É transferência interna: sem taxa de rede
                  e sem endereço de carteira. A chave precisa do escopo <code>send</code>.
                </p>
                <p>
                  Esse e-mail é o da conta que <strong>recebe</strong>. No seu produto você pode pegá-lo
                  dos dois jeitos abaixo. Os dois são válidos, e pode oferecer os dois ao mesmo tempo.
                  O valor que você colocar em <code>toEmail</code> é o que a SatsPay usa.
                </p>
                <ol className="list-decimal pl-5 space-y-1.5 text-sm">
                  <li>
                    <strong>O cliente digita.</strong> Um campo de texto no seu site ou app. Ele escreve
                    o e-mail da conta SatsPay dele. Você envia esse texto em <code>toEmail</code>.
                  </li>
                  <li>
                    <strong>O cliente entra com SatsPay.</strong> O botão Login com SatsPay. Depois da
                    troca do código, <code>GET /v1/oauth/userinfo</code> devolve <code>email</code> com{' '}
                    <code>email_verified: true</code>. Esse e-mail também vai em <code>toEmail</code>.
                  </li>
                </ol>
                <p>
                  Não coloque o e-mail da sua própria chave de API, e não coloque endereço{' '}
                  <code>bc1…</code>, <code>t1…</code> ou qualquer outra carteira: esta rota não saca
                  on-chain. Se ninguém tiver conta SatsPay com aquele e-mail, a chamada falha com{' '}
                  <code>TARGET_INELIGIBLE</code> e <strong>nada é debitado</strong>.
                </p>
                <p>
                  O <code>toEmail</code> também não pode ser a conta que emitiu a chave. A API responde{' '}
                  <code>400</code> com <code>code: SEND_TO_SELF</code> e o texto{' '}
                  <code>cannot send to the SatsPay account that owns this API key</code>. Nada é debitado.
                  Não é falta de saldo. Acontece quando quem recebe é o mesmo e-mail do comerciante — para
                  testar, use outra conta SatsPay. Leia o campo <code>error</code> e o <code>code</code>;
                  não substitua por uma mensagem genérica.
                </p>

                <h3 className="text-base font-bold text-ink pt-2">Gateway de cobranças</h3>
                <p>
                  O seu backend cria a fatura, o cliente paga numa página sob o domínio oficial, e você
                  recebe um webhook assinado quando o pagamento confirma na blockchain. Em resumo:
                </p>
                <ol className="list-decimal pl-5 space-y-1.5 text-sm">
                  <li>
                    <strong>Credenciamento</strong> — <code>POST /v1/merchant/apply</code>. Enquanto o
                    pedido não for aprovado, criar fatura responde <code>403</code>.
                  </li>
                  <li>
                    <strong>Chave de API</strong> — emitida com escopo <code>deposits</code>. O segredo
                    aparece uma única vez; ele vive no seu servidor e <strong>nunca</strong> no navegador.
                  </li>
                  <li>
                    <strong>Cobrança</strong> — <code>POST /v1/merchant/deposits</code>. Você informa a
                    moeda e a quantia, ou apenas <code>amountUsd</code> e deixa o cliente escolher a moeda.
                  </li>
                  <li>
                    <strong>Checkout</strong> — redirecione para o <code>checkoutUrl</code> da resposta, ou
                    use o botão oficial (<code>/sdk/satspay-pay.js</code>), que só navega até esse link.
                  </li>
                  <li>
                    <strong>Webhook</strong> — <code>deposit.confirmed</code>, assinado em
                    <code> X-SatsPay-Signature</code>. Confirme sempre pela assinatura, nunca pelo
                    redirecionamento do navegador.
                  </li>
                </ol>

                <h3 className="text-base font-bold text-ink pt-2">O cliente escolhe como pagar</h3>
                <p>
                  Você define quais moedas aceita no painel; quem paga escolhe entre elas no checkout. A
                  cotação trava no instante da escolha e vale até a fatura vencer. Se você cobrou em
                  dólar, a variação de preço entre a trava e o pagamento é sua — a plataforma não absorve
                  nem repassa nada além da taxa.
                </p>
                <p>
                  A <strong>taxa do gateway é de 0,25%</strong>, igual para todo comerciante, descontada
                  da fatura: <code>feeAmount + netAmount</code> é exatamente o valor cobrado.
                </p>

                <h3 className="text-base font-bold text-ink pt-2">Quantias são inteiros</h3>
                <p>
                  Todo campo <code>amount</code> da API é um <strong>inteiro em unidades de 1e-8</strong>,
                  em qualquer moeda: 25 USDT é <code>"2500000000"</code>, não <code>"25.00"</code>. A
                  única exceção é <code>amountUsd</code>, que é decimal porque é dinheiro fiat. Mandar
                  decimal onde se espera inteiro responde <code>400 AMOUNT_NOT_INTEGER</code> em vez de
                  cobrar o valor errado.
                </p>

                <h3 className="text-base font-bold text-ink pt-2">Chaves de API e assinatura HMAC</h3>
                <p>
                  Uma chave pode ser restrita por escopo, por lista de IPs e por validade. Com{' '}
                  <code>requireSignature = true</code>, cada requisição precisa vir assinada em
                  HMAC-SHA256 sobre <code>timestamp + método + caminho + hash do corpo</code> — e a
                  própria assinatura é reservada como uso único, o que impede repetição sem precisar de
                  um nonce separado.
                </p>

                <div className="flex flex-wrap gap-3 pt-2">
                  <Link to="/docs?tab=start" className="btn-primary text-xs">
                    Começar do zero →
                  </Link>
                  <Link to="/api-keys" className="btn-secondary text-xs">
                    Gerenciar API Keys →
                  </Link>
                  <Link to="/docs?tab=deposits" className="btn-secondary text-xs">
                    Gateway de cobranças →
                  </Link>
                  <Link to="/docs?tab=payouts" className="btn-secondary text-xs">
                    API de envios →
                  </Link>
                  <Link to="/pay/demo" className="rounded-xl border border-emerald-500/30 bg-emerald-500/10 hover:bg-emerald-500/20 text-emerald-600 font-bold text-xs px-3.5 py-2 flex items-center gap-1.5 transition-all">
                    <i className="bi bi-qr-code" />
                    <span>Ver o checkout de demonstração</span>
                  </Link>
                  <Link to="/docs?tab=simulator" className="rounded-xl border border-purple-500/30 bg-purple-500/10 hover:bg-purple-500/20 text-purple-600 font-bold text-xs px-3.5 py-2 flex items-center gap-1.5 transition-all">
                    <i className="bi bi-cpu" />
                    <span>Testar Simulador HMAC Webhook</span>
                  </Link>
                </div>
              </div>
            </div>
          )}

          {activeSection === 'oauth_sso' && (
            <div className="card p-6 space-y-6">
              <div className="flex items-center gap-3 border-b border-border pb-4">
                <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-amber-500/10 text-amber-500 text-xl font-bold">
                  <i className="bi bi-shield-lock-fill" />
                </div>
                <div>
                  <h2 className="text-xl font-bold text-ink">Login com SatsPay (OAuth 2.0 & OpenID Connect SSO)</h2>
                  <p className="text-xs text-ink-muted">
                    Transforme o SatsPay no provedor de login do seu site, loja ou aplicativo
                  </p>
                </div>
              </div>

              <div className="prose text-sm text-ink-muted space-y-6">
                <p>
                  O <strong>Login com SatsPay</strong> funciona de maneira análoga ao <em>Google Sign-In</em> ou <em>Apple ID</em>. Seus clientes e jogadores podem se cadastrar e efetuar login em sua plataforma com 1 clique utilizando a conta SatsPay verificada, sem necessidade de gerenciar senhas ou expor credenciais sensíveis.
                </p>
                <p>
                  O <code>email</code> que volta em <code>/v1/oauth/userinfo</code> já está verificado. Ele serve como <code>toEmail</code> em <code>POST /v1/public/send</code> quando você for pagar essa conta. Se preferir, o cliente também pode digitar o e-mail da conta SatsPay dele num campo seu — os dois caminhos mandam para o mesmo campo. Veja a <Link to="/docs?tab=payouts" className="font-bold text-bitcoin hover:underline">API de envios</Link>.
                </p>

                {/* 3 STEPS GRID */}
                <div className="grid gap-3 sm:grid-cols-3 not-prose">
                  <div className="rounded-2xl border border-border bg-surface p-4 space-y-2">
                    <span className="flex h-7 w-7 items-center justify-center rounded-xl bg-bitcoin font-black text-white text-xs">
                      1
                    </span>
                    <h3 className="font-bold text-ink text-sm">Obtenha as Chaves</h3>
                    <p className="text-xs text-ink-muted">
                      Acesse o <Link to="/developer/apps" className="font-bold text-bitcoin hover:underline">Painel de Aplicações</Link>, crie seu app e obtenha seu <code>Client ID</code> e <code>Client Secret</code>.
                    </p>
                  </div>

                  <div className="rounded-2xl border border-border bg-surface p-4 space-y-2">
                    <span className="flex h-7 w-7 items-center justify-center rounded-xl bg-bitcoin font-black text-white text-xs">
                      2
                    </span>
                    <h3 className="font-bold text-ink text-sm">Adicione o Botão</h3>
                    <p className="text-xs text-ink-muted">
                      Insira o <code>satspay-auth.v2.js</code> no seu site e renderize o botão oficial
                      com a logo SatsPay (<code>/sdk/satspay-logo.png</code>).
                    </p>
                  </div>

                  <div className="rounded-2xl border border-border bg-surface p-4 space-y-2">
                    <span className="flex h-7 w-7 items-center justify-center rounded-xl bg-bitcoin font-black text-white text-xs">
                      3
                    </span>
                    <h3 className="font-bold text-ink text-sm">Valide no Backend</h3>
                    <p className="text-xs text-ink-muted">
                      Seu servidor troca o código de autorização pelo <code>access_token</code> e busca o perfil em <code>/v1/oauth/userinfo</code>.
                    </p>
                  </div>
                </div>

                {/* EXEMPLO PRÁTICO FRONTEND */}
                <div className="not-prose space-y-2">
                  <h3 className="text-sm font-bold uppercase tracking-wider text-ink">
                    Código Frontend (HTML / JavaScript)
                  </h3>
                  <div className="overflow-hidden rounded-2xl border border-border bg-[#090D16] p-4 text-white">
                    <pre className="overflow-x-auto font-mono text-xs text-emerald-400/90 leading-relaxed [scrollbar-width:none]">
{`<!-- 1. Importar SDK -->
<script src="https://www.satspay.pro/sdk/satspay-auth.v2.js" async defer></script>

<!-- 2. Botão oficial (redirect = padrão: usuário vai ao SatsPay e volta no callback com ?code=) -->
<div class="satspay-signin"
     data-client_id="sats_app_SEU_CLIENT_ID"
     data-redirect_uri="https://seusite.com/auth/callback"
     data-mode="redirect"
     data-theme="bitcoin"
     data-size="large"
     data-onsuccess="onSatsPaySignIn">
</div>

<!-- No callback https://seusite.com/auth/callback?code=...&state=...
     troque o code por access_token no backend (nunca no browser). -->

<script>
function onSatsPaySignIn(response) {
  // Apenas no modo popup. No redirect, leia a query string no servidor.
  console.log("Código OAuth:", response.code);
}
</script>`}
                    </pre>
                  </div>
                </div>

                {/* EXEMPLO PRÁTICO BACKEND */}
                <div className="not-prose space-y-2">
                  <h3 className="text-sm font-bold uppercase tracking-wider text-ink">
                    Troca do Código por Token & Perfil no Backend (Node.js / Express)
                  </h3>
                  <div className="overflow-hidden rounded-2xl border border-border bg-[#090D16] p-4 text-white">
                    <pre className="overflow-x-auto font-mono text-xs text-emerald-400/90 leading-relaxed [scrollbar-width:none]">
{`app.post('/api/auth/satspay', async (req, res) => {
  const { code } = req.body;

  // 1. Troca o código pelo Access Token
  const tokenRes = await fetch('https://www.satspay.pro/v1/oauth/token', {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({
      grant_type: 'authorization_code',
      code,
      client_id: process.env.SATSPAY_CLIENT_ID,
      client_secret: process.env.SATSPAY_CLIENT_SECRET,
      redirect_uri: 'https://seusite.com/auth/callback'
    })
  });
  const { access_token } = await tokenRes.json();

  // 2. Consulta os dados do usuário autenticado
  const userRes = await fetch('https://www.satspay.pro/v1/oauth/userinfo', {
    headers: { Authorization: \`Bearer \${access_token}\` }
  });
  const profile = await userRes.json();

  // profile contém: { sub, id, username, email, email_verified, picture }
  // profile.email (email_verified === true) pode ir em toEmail no POST /v1/public/send.
  // O cliente também pode digitar outro e-mail de conta SatsPay. Os dois servem.
  console.log('Usuário autenticado:', profile.username, profile.email);
  res.json({ success: true, user: profile });
});`}
                    </pre>
                  </div>
                </div>

                {/* BOTÕES DE AÇÃO */}
                <div className="flex flex-wrap gap-3 pt-4 border-t border-border">
                  <Link to="/developer/apps" className="btn-primary text-xs flex items-center gap-1.5">
                    <i className="bi bi-plus-circle" />
                    <span>Criar Aplicativo no Painel OAuth</span>
                  </Link>
                  <Link to="/docs" className="btn-secondary text-xs flex items-center gap-1.5">
                    <i className="bi bi-book-half" />
                    <span>Ver Especificação Completa da API</span>
                  </Link>
                </div>
              </div>
            </div>
          )}

          {activeSection === 'security' && (
            <div className="card p-6 sm:p-8 space-y-8">
              <div className="flex items-center gap-3 border-b border-border pb-4">
                <div className="flex h-12 w-12 items-center justify-center rounded-2xl bg-emerald-500/10 text-emerald-600 text-2xl font-bold">
                  <i className="bi bi-shield-check" />
                </div>
                <div>
                  <h2 className="text-xl sm:text-2xl font-black text-ink">Segurança & Custódia</h2>
                  <p className="text-xs sm:text-sm text-ink-muted">
                    Como protegemos contas, sessões e fundos — sem expor detalhes internos de infraestrutura
                  </p>
                </div>
              </div>

              <div className="space-y-6 text-sm text-ink-muted leading-relaxed">
                <p>
                  A <strong>SatsPay</strong> opera com defesa em camadas: autenticação forte, contabilidade auditável e custódia com o mínimo necessário em carteiras quentes. Detalhes de topologia, nós, redes internas e material criptográfico <strong>não são publicados</strong> nesta documentação.
                </p>

                {/* 4 PILARES INSTITUCIONAIS */}
                <div className="grid gap-4 sm:grid-cols-2 not-prose">
                  <div className="rounded-2xl border border-border bg-surface p-5 space-y-2.5">
                    <div className="flex items-center gap-2 text-emerald-600 font-bold text-xs uppercase tracking-wider">
                      <i className="bi bi-journal-check text-base" />
                      <span>1. Imutabilidade Contábil (Ledger)</span>
                    </div>
                    <p className="text-xs text-ink-muted leading-relaxed">
                      Saldos <b>não são valores editáveis</b>. Cada movimento vira lançamento no razão (partidas dobradas). Transferências e saques só avançam se o saldo derivado continuar consistente.
                    </p>
                  </div>

                  <div className="rounded-2xl border border-border bg-surface p-5 space-y-2.5">
                    <div className="flex items-center gap-2 text-blue-600 font-bold text-xs uppercase tracking-wider">
                      <i className="bi bi-safe2-fill text-base" />
                      <span>2. Custódia e Chaves</span>
                    </div>
                    <p className="text-xs text-ink-muted leading-relaxed">
                      Endereços de depósito são derivados sem expor material de assinatura na camada web pública. Assinatura on-chain e tesouraria ficam em componentes separados da API de usuário. Movimentações grandes passam por revisão operacional.
                    </p>
                  </div>

                  <div className="rounded-2xl border border-border bg-surface p-5 space-y-2.5">
                    <div className="flex items-center gap-2 text-amber-500 font-bold text-xs uppercase tracking-wider">
                      <i className="bi bi-key-fill text-base" />
                      <span>3. Contas e Sessões</span>
                    </div>
                    <p className="text-xs text-ink-muted leading-relaxed">
                      Senhas com hash resistente a força bruta; 2FA para ações sensíveis (ex.: saques); sessões com tokens de curta duração e refresh protegido (cookie HttpOnly). Segredos de API são mostrados uma vez na criação.
                    </p>
                  </div>

                  <div className="rounded-2xl border border-border bg-surface p-5 space-y-2.5">
                    <div className="flex items-center gap-2 text-purple-600 font-bold text-xs uppercase tracking-wider">
                      <i className="bi bi-shield-slash-fill text-base" />
                      <span>4. Proteção contra Abuso</span>
                    </div>
                    <p className="text-xs text-ink-muted leading-relaxed">
                      Limitação de taxa, mitigação de enumeração em login, captcha em fluxos públicos e assinaturas HMAC com janela de tempo nas APIs de comerciante — para reduzir replay e automação abusiva.
                    </p>
                  </div>
                </div>

                {/* DETALHES TÉCNICOS & POLÍTICAS */}
                <div className="space-y-4 pt-4 border-t border-border">
                  <h3 className="text-base font-bold text-ink flex items-center gap-2">
                    <i className="bi bi-info-circle text-bitcoin" />
                    <span>O que esta página não lista</span>
                  </h3>
                  <ul className="list-disc pl-5 space-y-2 text-xs">
                    <li>IPs, hosts, VMs, portas ou provedores de RPC internos.</li>
                    <li>Nomes de tabelas, SQL, variáveis de ambiente ou caminhos no servidor.</li>
                    <li>Algoritmos/parâmetros de cifra além do necessário para o usuário confiar no produto.</li>
                    <li>Arquitetura de rede privada, workers ou inventário de hot/cold wallets.</li>
                  </ul>
                  <p className="text-xs text-ink-muted">
                    Para reportar vulnerabilidades, use a página{' '}
                    <Link to="/security" className="font-semibold text-bitcoin hover:underline">
                      Segurança
                    </Link>
                    .
                  </p>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Sidebar Info Cards */}
        <div className="space-y-6">
          <div className="card p-5 space-y-3">
            <h3 className="font-bold text-ink text-sm flex items-center gap-2">
              <i className="bi bi-link-45deg text-bitcoin" />
              Links Rápidos
            </h3>
            <div className="space-y-2 text-xs">
              <Link to="/docs" className="flex items-center justify-between p-2 rounded-lg bg-surface hover:bg-border/50 text-ink transition-colors">
                <span>Docs da API REST</span>
                <i className="bi bi-arrow-right text-ink-muted" />
              </Link>
              <Link to="/status" className="flex items-center justify-between p-2 rounded-lg bg-surface hover:bg-border/50 text-ink transition-colors">
                <span>Status do sistema</span>
                <i className="bi bi-arrow-right text-ink-muted" />
              </Link>
              <Link to="/api-keys" className="flex items-center justify-between p-2 rounded-lg bg-surface hover:bg-border/50 text-ink transition-colors">
                <span>Minhas Chaves de API</span>
                <i className="bi bi-arrow-right text-ink-muted" />
              </Link>
            </div>
          </div>

          <div className="rounded-2xl border border-bitcoin/20 bg-gradient-to-br from-bitcoin/5 to-transparent p-5 space-y-2">
            <div className="flex items-center gap-2 text-xs font-bold text-bitcoin-dark">
              <i className="bi bi-shield-check" />
              Segurança pública
            </div>
            <p className="text-xs text-ink-muted leading-relaxed">
              Documentação de produto e API — sem topologia de rede, credenciais ou inventário de custódia.
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
