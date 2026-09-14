import { useState, useRef, useEffect, useMemo } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { motion, AnimatePresence } from 'framer-motion';
import { api } from '../lib/api.js';
import { useAuthStore } from '../stores/auth.js';
import { coinLogo } from '../lib/coinAssets.js';
import { AddressQr } from '../components/AddressQr.js';
import { COIN_CONFIG, COINS, defaultDepositWithdrawCoin, formatAmount, isCoin, isDepositWithdrawPaused, type Coin } from '@/shared';

interface AddressResp {
  address: string;
}

export interface DepositHistoryItem {
  id: string;
  coin: string;
  txHash: string;
  vout: number;
  amount: string;
  confirmations: number;
  minConfirmations: number;
  status: 'PENDING' | 'CONFIRMED' | 'CREDITED' | 'ORPHANED' | string;
  detectedAt: string;
  creditedAt?: string | null;
}

const NETWORK_NAMES: Record<Coin, { network: string; estTime: string; note: string }> = {
  BTC: { network: 'Bitcoin Mainnet', estTime: '~10 - 30 min', note: 'Envie apenas Bitcoin (BTC) para este endereço.' },
  LTC: { network: 'Litecoin Network', estTime: '~2 - 10 min', note: 'Envie apenas Litecoin (LTC) para este endereço.' },
  DOGE: { network: 'Dogecoin Network', estTime: '~5 - 15 min', note: 'Envie apenas Dogecoin (DOGE) para este endereço.' },
  BCH: { network: 'Bitcoin Cash Network', estTime: '~10 - 20 min', note: 'Envie apenas Bitcoin Cash (BCH) para este endereço.' },
  POL: { network: 'Polygon Mainnet (Native POL)', estTime: '~1 - 3 min', note: 'Envie apenas POL (nativo da rede Polygon) para este endereço.' },
  DGB: { network: 'DigiByte Network', estTime: '~1 - 5 min', note: 'Envie apenas DigiByte (DGB) para este endereço.' },
  SOL: { network: 'Solana Mainnet (SPL)', estTime: '~1 - 2 min', note: 'Envie apenas Solana (SOL) para este endereço.' },
  USDT: { network: 'Polygon Network (USDT)', estTime: '~1 - 3 min', note: 'Envie USDT via rede Polygon (ERC-20/Polygon) para este endereço.' },
  USDC: { network: 'Polygon Network (USDC)', estTime: '~1 - 3 min', note: 'Envie USDC via rede Polygon (ERC-20/Polygon) para este endereço.' },
};

function getExplorerTxUrl(coin: string, txHash: string): string {
  switch (coin.toUpperCase()) {
    case 'BTC':
      return `https://mempool.space/tx/${txHash}`;
    case 'LTC':
      return `https://litecoinspace.org/tx/${txHash}`;
    case 'DOGE':
      return `https://dogechain.info/tx/${txHash}`;
    case 'BCH':
      return `https://blockchair.com/bitcoin-cash/transaction/${txHash}`;
    case 'POL':
    case 'USDT':
    case 'USDC':
      return `https://polygonscan.com/tx/${txHash}`;
    case 'DGB':
      return `https://digiexplorer.info/tx/${txHash}`;
    case 'SOL':
      return `https://solscan.io/tx/${txHash}`;
    default:
      return `https://polygonscan.com/tx/${txHash}`;
  }
}

export function DepositPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  
  const rawInitialCoin = searchParams.get('coin')?.toUpperCase();
  const initialCoin = defaultDepositWithdrawCoin(rawInitialCoin);
  const [coin, setCoin] = useState<Coin>(initialCoin);
  const [activeTab, setActiveTab] = useState<'deposit' | 'history'>(
    searchParams.get('tab') === 'history' ? 'history' : 'deposit'
  );
  const [historyCoinFilter, setHistoryCoinFilter] = useState<string>('ALL');
  const [isPickerOpen, setIsPickerOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  const [copiedTx, setCopiedTx] = useState<string | null>(null);
  const pickerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const urlCoin = searchParams.get('coin')?.toUpperCase();
    if (urlCoin && isCoin(urlCoin) && urlCoin !== coin) {
      setCoin(urlCoin);
    }
    const urlTab = searchParams.get('tab');
    if (urlTab === 'history' || urlTab === 'deposit') {
      setActiveTab(urlTab);
    }
  }, [searchParams, coin]);

  const depositPaused = isDepositWithdrawPaused(coin);

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (pickerRef.current && !pickerRef.current.contains(event.target as Node)) {
        setIsPickerOpen(false);
      }
    }
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const user = useAuthStore((s) => s.user);

  const { data: walletsData } = useQuery({
    queryKey: ['wallets', 'PERSONAL'],
    queryFn: () => api<{ wallets?: Array<{ coin: string; address?: string | null }> } | Array<{ coin: string; address?: string | null }>>('/wallet?kind=PERSONAL'),
    enabled: Boolean(user),
    staleTime: 60_000,
  });

  const { data, isLoading, isError, refetch } = useQuery({
    queryKey: ['deposit-address', coin],
    queryFn: () => api<AddressResp>(`/deposits/address/${coin}`),
    enabled: Boolean(user) && !depositPaused,
    retry: 3,
  });

  const fallbackAddress = useMemo(() => {
    const list = Array.isArray(walletsData) ? walletsData : walletsData?.wallets ?? [];
    return list.find((w) => w.coin.toUpperCase() === coin.toUpperCase())?.address || undefined;
  }, [walletsData, coin]);

  const activeAddress = depositPaused ? undefined : data?.address || fallbackAddress;

  // Fetch real-time deposit history with 5s polling
  const {
    data: historyData,
    isLoading: isLoadingHistory,
    refetch: refetchHistory,
    isFetching: isFetchingHistory,
  } = useQuery({
    queryKey: ['deposits-history'],
    queryFn: () => api<{ deposits: DepositHistoryItem[] }>('/deposits/history'),
    enabled: Boolean(user),
    refetchInterval: (query) => (query.state.error ? false : 5000),
  });

  const deposits = historyData?.deposits || [];
  const activeCoinDeposits = deposits.filter((d) => d.coin.toUpperCase() === coin.toUpperCase());
  const filteredDeposits = historyCoinFilter === 'ALL'
    ? deposits
    : deposits.filter((d) => d.coin.toUpperCase() === historyCoinFilter.toUpperCase());

  const pendingCount = deposits.filter((d) => d.status === 'PENDING').length;

  const cfg = COIN_CONFIG[coin];
  const netInfo = NETWORK_NAMES[coin];

  const handleCopy = () => {
    if (!activeAddress) return;
    navigator.clipboard.writeText(activeAddress);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleCopyTx = (txHash: string) => {
    navigator.clipboard.writeText(txHash);
    setCopiedTx(txHash);
    setTimeout(() => setCopiedTx(null), 2000);
  };

  const switchTab = (tab: 'deposit' | 'history') => {
    setActiveTab(tab);
    setSearchParams((prev) => {
      const p = new URLSearchParams(prev);
      p.set('tab', tab);
      return p;
    });
  };

  return (
    <div className="space-y-6 pb-12">
      {/* HEADER WITH BACK BUTTON & TAB SELECTOR */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <button
            type="button"
            onClick={() => navigate('/wallets')}
            className="mb-3 inline-flex items-center gap-2 rounded-xl border border-border bg-surface px-3.5 py-2 text-xs font-semibold text-ink hover:bg-paper hover:text-bitcoin-dark hover:border-bitcoin/40 transition-all shadow-sm group"
          >
            <i className="bi bi-arrow-left text-sm text-bitcoin-dark group-hover:-translate-x-0.5 transition-transform" />
            <span>{t('wallets.backToWallets')}</span>
          </button>
          <h1 className="text-2xl font-bold tracking-tight md:text-3xl text-ink">
            {t('deposit.title') || 'Depositar Criptomoeda'}
          </h1>
          <p className="text-sm text-ink-muted">
            Cada conta tem endereços HD próprios. Envie só para o endereço desta sessão — crédito automático on-chain.
          </p>
        </div>

        {/* TOP CONTROLS */}
        <div className="flex flex-wrap items-center gap-2">
          {/* TAB BUTTONS */}
          <div className="inline-flex rounded-2xl border border-border bg-surface p-1 shadow-sm">
            <button
              type="button"
              onClick={() => switchTab('deposit')}
              className={`inline-flex items-center gap-2 rounded-xl px-4 py-2 text-xs font-bold transition-all ${
                activeTab === 'deposit'
                  ? 'bg-bitcoin text-white shadow-md shadow-bitcoin/20'
                  : 'text-ink-muted hover:text-ink hover:bg-paper'
              }`}
            >
              <i className="bi bi-qr-code text-sm" />
              <span>Gerar Depósito</span>
            </button>
            <button
              type="button"
              onClick={() => switchTab('history')}
              className={`inline-flex items-center gap-2 rounded-xl px-4 py-2 text-xs font-bold transition-all relative ${
                activeTab === 'history'
                  ? 'bg-bitcoin text-white shadow-md shadow-bitcoin/20'
                  : 'text-ink-muted hover:text-ink hover:bg-paper'
              }`}
            >
              <i className="bi bi-clock-history text-sm" />
              <span>Histórico</span>
              {pendingCount > 0 ? (
                <span className="flex h-5 w-5 items-center justify-center rounded-full bg-amber-500 text-[10px] font-black text-white animate-pulse">
                  {pendingCount}
                </span>
              ) : deposits.length > 0 ? (
                <span className="flex h-5 min-w-[20px] px-1.5 items-center justify-center rounded-full bg-paper text-[10px] font-bold text-ink-muted border border-border">
                  {deposits.length}
                </span>
              ) : null}
            </button>
          </div>

          <Link to="/wallets" className="btn-secondary text-xs">
            <i className="bi bi-wallet2 mr-1" /> {t('nav.wallets') || 'Carteiras'}
          </Link>
        </div>
      </div>

      {/* ACTIVE TAB CONTENT */}
      {activeTab === 'deposit' ? (
        <div className="space-y-6">
          {/* MAIN DEPOSIT LAYOUT */}
          <div className="grid gap-6 lg:grid-cols-12">
            {/* LEFT COLUMN: COIN SELECTION & DEPOSIT CARD */}
            <div className="space-y-4 lg:col-span-6">
              <div className="card p-6 space-y-5">
                {/* INTERACTIVE COIN SELECTOR WITH REAL LOGOS */}
                <div className="relative" ref={pickerRef}>
                  <label className="label font-semibold text-xs text-ink-muted mb-1.5 block">
                    Escolha a Criptomoeda para Depósito
                  </label>

                  {/* SELECTOR TRIGGER BUTTON */}
                  <button
                    type="button"
                    onClick={() => setIsPickerOpen((prev) => !prev)}
                    className={`flex w-full items-center justify-between rounded-2xl border p-3 text-left transition-all ${
                      isPickerOpen
                        ? 'border-bitcoin bg-paper shadow-md ring-2 ring-bitcoin/20'
                        : 'border-border bg-surface hover:border-bitcoin/50 hover:bg-paper'
                    }`}
                  >
                    <div className="flex items-center gap-3 min-w-0">
                      <img
                        src={coinLogo(coin)}
                        alt={coin}
                        className="h-9 w-9 rounded-full shrink-0 shadow-sm"
                      />
                      <div className="truncate">
                        <div className="flex items-center gap-2">
                          <span className="font-bold text-sm text-ink">{cfg.name}</span>
                          <span className="rounded-md bg-paper px-1.5 py-0.5 font-mono text-[11px] font-bold text-bitcoin-dark border border-border">
                            {coin}
                          </span>
                        </div>
                        <div className="text-xs text-ink-muted truncate">{netInfo.network}</div>
                      </div>
                    </div>

                    <div className="flex items-center gap-2 shrink-0 pl-2">
                      <span className="text-xs font-semibold text-bitcoin-dark hidden sm:inline">Alterar</span>
                      <div className={`flex h-7 w-7 items-center justify-center rounded-lg bg-paper text-ink-muted transition-transform ${isPickerOpen ? 'rotate-180 text-bitcoin' : ''}`}>
                        <i className="bi bi-chevron-down text-xs" />
                      </div>
                    </div>
                  </button>

                  {/* DROPDOWN POPOVER */}
                  <AnimatePresence>
                    {isPickerOpen && (
                      <motion.div
                        initial={{ opacity: 0, y: 8, scale: 0.98 }}
                        animate={{ opacity: 1, y: 0, scale: 1 }}
                        exit={{ opacity: 0, y: 8, scale: 0.98 }}
                        transition={{ duration: 0.15 }}
                        className="absolute left-0 right-0 top-full z-50 mt-2 max-h-80 overflow-y-auto rounded-2xl border border-border bg-paper p-2 shadow-2xl space-y-1"
                      >
                        <div className="px-2 py-1.5 text-[11px] font-semibold uppercase tracking-wider text-ink-muted">
                          Selecione um Ativo para Gerar Endereço
                        </div>
                        {COINS.map((c) => {
                          const isSelected = c === coin;
                          const cCfg = COIN_CONFIG[c];
                          const cNet = NETWORK_NAMES[c];
                          const paused = isDepositWithdrawPaused(c);
                          return (
                            <button
                              key={c}
                              type="button"
                              onClick={() => {
                                setCoin(c);
                                setSearchParams((prev) => {
                                  const p = new URLSearchParams(prev);
                                  p.set('coin', c);
                                  return p;
                                });
                                setIsPickerOpen(false);
                                setCopied(false);
                              }}
                              className={`flex w-full items-center justify-between rounded-xl px-3 py-2.5 text-left transition-all ${
                                isSelected
                                  ? 'bg-bitcoin/10 text-bitcoin-dark font-bold'
                                  : paused
                                    ? 'opacity-75 hover:bg-surface text-ink'
                                    : 'hover:bg-surface text-ink'
                              }`}
                            >
                              <div className="flex items-center gap-3 min-w-0">
                                <img
                                  src={coinLogo(c)}
                                  alt={c}
                                  className={`h-8 w-8 rounded-full shrink-0 shadow-xs ${paused ? 'grayscale' : ''}`}
                                />
                                <div className="truncate">
                                  <div className="flex items-center gap-2">
                                    <span className="text-sm font-semibold">{cCfg.name}</span>
                                    <span className="font-mono text-xs text-ink-muted">({c})</span>
                                    {paused && (
                                      <span className="rounded-md border border-amber-500/30 bg-amber-500/10 px-1.5 py-0.5 text-[10px] font-black uppercase tracking-wide text-amber-800">
                                        {t('deposit.pausedBadge')}
                                      </span>
                                    )}
                                  </div>
                                  <div className="text-[11px] text-ink-muted truncate">{cNet.network}</div>
                                </div>
                              </div>

                              {isSelected ? (
                                <i className="bi bi-check-circle-fill text-bitcoin text-base shrink-0" />
                              ) : (
                                <span className="text-[11px] text-ink-muted font-mono shrink-0">{cNet.estTime}</span>
                              )}
                            </button>
                          );
                        })}
                      </motion.div>
                    )}
                  </AnimatePresence>
                </div>

                {/* COIN ACTIVE NETWORK STATUS */}
                <div className="flex items-center justify-between rounded-xl border border-border bg-surface p-3.5">
                  <div className="flex items-center gap-3">
                    <img
                      src={coinLogo(coin)}
                      alt={coin}
                      className={`h-10 w-10 rounded-full shrink-0 shadow-sm ${depositPaused ? 'grayscale' : ''}`}
                    />
                    <div>
                      <div className="font-bold text-sm text-ink">{cfg.name} ({coin})</div>
                      <div className="text-xs text-ink-muted">{netInfo.network}</div>
                    </div>
                  </div>
                  {depositPaused ? (
                    <span className="inline-flex items-center gap-1.5 rounded-full bg-amber-500/10 px-2.5 py-1 text-[11px] font-semibold text-amber-800 border border-amber-500/25">
                      <i className="bi bi-pause-circle" />
                      {t('deposit.pausedBadge')}
                    </span>
                  ) : (
                    <span className="inline-flex items-center gap-1.5 rounded-full bg-emerald-500/10 px-2.5 py-1 text-[11px] font-semibold text-emerald-700 dark:text-emerald-400">
                      <span className="h-1.5 w-1.5 rounded-full bg-emerald-500 animate-pulse" />
                      Rede Ativa
                    </span>
                  )}
                </div>

                {depositPaused ? (
                  <div className="rounded-2xl border border-amber-500/30 bg-amber-500/10 p-5 text-center space-y-2">
                    <i className="bi bi-pause-circle text-2xl text-amber-700" />
                    <p className="text-sm font-bold text-ink">{t('deposit.pausedTitle', { coin })}</p>
                    <p className="text-xs text-ink-muted">{t('deposit.pausedHint')}</p>
                  </div>
                ) : (
                <>
                {/* QR CODE DISPLAY */}
                <div className="flex flex-col items-center justify-center space-y-3 py-1">
                  {isLoading && !activeAddress ? (
                    <div className="flex h-52 w-52 flex-col items-center justify-center rounded-2xl border-2 border-dashed border-border bg-surface p-6">
                      <i className="bi bi-arrow-repeat animate-spin text-3xl text-bitcoin" />
                      <span className="mt-3 text-xs text-ink-muted">{t('deposit.generating') || 'Gerando endereço...'}</span>
                    </div>
                  ) : activeAddress ? (
                    <AddressQr address={activeAddress} sizeClassName="h-48 w-48" />
                  ) : (
                    <div className="flex flex-col items-center justify-center gap-3 p-6 text-center">
                      <i className="bi bi-exclamation-triangle text-amber-500 text-2xl" />
                      <div className="text-xs text-rose-600 font-medium">Erro ao carregar endereço. Tente novamente.</div>
                      <button
                        type="button"
                        onClick={() => refetch()}
                        className="btn-secondary text-xs px-3.5 py-1.5 inline-flex items-center gap-1.5 rounded-xl shadow-xs"
                      >
                        <i className="bi bi-arrow-clockwise text-xs" />
                        <span>Tentar novamente</span>
                      </button>
                    </div>
                  )}
                  <span className="text-[11px] text-ink-muted">
                    Escaneie o QR Code no seu aplicativo de carteira
                  </span>
                </div>

                {/* ADDRESS INPUT & COPY ACTION */}
                <div className="space-y-2">
                  <div className="flex items-center justify-between text-xs">
                    <span className="font-semibold text-ink-muted">
                      {t('deposit.addressFor', { coin }) || `Endereço de Depósito ${coin}`}
                    </span>
                    <span className="text-[11px] font-semibold text-emerald-700">Exclusivo desta conta</span>
                  </div>

                  <div className="relative flex items-center rounded-xl border-2 border-border bg-surface p-1 focus-within:border-bitcoin transition-colors">
                    <input
                      type="text"
                      readOnly
                      value={isLoading && !activeAddress ? 'Carregando endereço...' : activeAddress || ''}
                      className="w-full bg-transparent px-3 py-2 font-mono text-xs text-ink outline-none"
                    />
                    <button
                      type="button"
                      onClick={handleCopy}
                      disabled={!activeAddress}
                      className={`flex shrink-0 items-center gap-1.5 rounded-lg px-4 py-2 text-xs font-semibold text-white transition-all shadow-sm ${
                        copied
                          ? 'bg-emerald-600 shadow-emerald-600/30'
                          : 'bg-bitcoin hover:bg-bitcoin-dark shadow-bitcoin/20'
                      }`}
                    >
                      <i className={`bi ${copied ? 'bi-check2-circle text-sm' : 'bi-clipboard'}`} />
                      <span>{copied ? (t('wallets.copied') || 'Copiado!') : (t('common.copy') || 'Copiar')}</span>
                    </button>
                  </div>

                  <AnimatePresence>
                    {copied && (
                      <motion.div
                        initial={{ opacity: 0, y: -4 }}
                        animate={{ opacity: 1, y: 0 }}
                        exit={{ opacity: 0 }}
                        className="flex items-center gap-1.5 text-xs font-medium text-emerald-700 dark:text-emerald-400"
                      >
                        <i className="bi bi-check-circle-fill" />
                        <span>Endereço copiado para a área de transferência!</span>
                      </motion.div>
                    )}
                  </AnimatePresence>
                </div>
                </>
                )}
              </div>
            </div>

            {/* RIGHT COLUMN: NETWORK SPECS & INSTRUCTIONS */}
            <div className="space-y-4 lg:col-span-6">
              {/* NETWORK SPECS CARD */}
              <div className="card p-6 space-y-4">
                <h3 className="flex items-center gap-2 text-sm font-bold uppercase tracking-wider text-ink-muted">
                  <i className="bi bi-hdd-network text-bitcoin-dark" /> Informações da Transação
                </h3>

                <div className="grid gap-3 sm:grid-cols-2">
                  <div className="rounded-xl border border-border bg-surface p-3.5 space-y-1">
                    <div className="text-[10px] font-semibold uppercase text-ink-muted">Rede Blockchain</div>
                    <div className="font-mono text-xs font-bold text-ink truncate">{netInfo.network}</div>
                  </div>

                  <div className="rounded-xl border border-border bg-surface p-3.5 space-y-1">
                    <div className="text-[10px] font-semibold uppercase text-ink-muted">Confirmações Necessárias</div>
                    <div className="text-xs font-bold text-ink">
                      {cfg.minConfirmations} confirmações
                    </div>
                  </div>

                  <div className="rounded-xl border border-border bg-surface p-3.5 space-y-1">
                    <div className="text-[10px] font-semibold uppercase text-ink-muted">Tempo Estimado</div>
                    <div className="text-xs font-bold text-emerald-700 dark:text-emerald-400">
                      {netInfo.estTime}
                    </div>
                  </div>

                  <div className="rounded-xl border border-border bg-surface p-3.5 space-y-1">
                    <div className="text-[10px] font-semibold uppercase text-ink-muted">Depósito Mínimo</div>
                    <div className="text-xs font-bold text-ink">
                      Sem valor mínimo
                    </div>
                  </div>
                </div>

                {/* SAFETY NOTICE */}
                <div className="rounded-xl border border-amber-300/40 bg-amber-500/10 p-4 text-xs text-amber-800 dark:text-amber-300 space-y-1.5">
                  <div className="flex items-center gap-2 font-bold">
                    <i className="bi bi-exclamation-triangle-fill text-amber-500 text-sm" />
                    Aviso de Segurança
                  </div>
                  <p className="leading-relaxed">
                    {netInfo.note} Transações enviadas para redes incompatíveis não podem ser recuperadas.
                  </p>
                </div>
              </div>

              {/* STEP BY STEP GUIDE */}
              <div className="card p-6 space-y-4">
                <h3 className="flex items-center gap-2 text-sm font-bold uppercase tracking-wider text-ink-muted">
                  <i className="bi bi-question-circle text-bitcoin-dark" /> Como Depositar na SatsPay
                </h3>

                <div className="space-y-3">
                  <div className="flex items-start gap-3">
                    <div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-bitcoin/15 text-xs font-bold text-bitcoin-dark">
                      1
                    </div>
                    <div className="text-xs text-ink leading-relaxed">
                      <strong>Copie o endereço</strong> ou escaneie o <strong>QR Code</strong> com o aplicativo da sua carteira ou exchange.
                    </div>
                  </div>

                  <div className="flex items-start gap-3">
                    <div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-bitcoin/15 text-xs font-bold text-bitcoin-dark">
                      2
                    </div>
                    <div className="text-xs text-ink leading-relaxed">
                      <strong>Envie a quantia</strong> desejada utilizando exclusivamente a <strong>{netInfo.network}</strong>.
                    </div>
                  </div>

                  <div className="flex items-start gap-3">
                    <div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-bitcoin/15 text-xs font-bold text-bitcoin-dark">
                      3
                    </div>
                    <div className="text-xs text-ink leading-relaxed">
                      <strong>Crédito Automático:</strong> O saldo é creditado em sua carteira após <strong>{cfg.minConfirmations} confirmações</strong> de rede.
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>

          {/* COMPACT RECENT DEPOSITS PREVIEW FOR ACTIVE COIN */}
          {activeCoinDeposits.length > 0 && (
            <div className="card p-4 sm:p-5 space-y-3">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <span className="h-2 w-2 rounded-full bg-emerald-500 animate-pulse" />
                  <h3 className="text-xs font-bold uppercase tracking-wider text-ink-muted">
                    Últimos Depósitos Detectados ({coin})
                  </h3>
                </div>
                <button
                  type="button"
                  onClick={() => switchTab('history')}
                  className="text-xs font-bold text-bitcoin hover:underline inline-flex items-center gap-1"
                >
                  Ver todos ({activeCoinDeposits.length}) <i className="bi bi-arrow-right" />
                </button>
              </div>

              <div className="divide-y divide-border/60 rounded-xl border border-border bg-surface">
                {activeCoinDeposits.slice(0, 3).map((dep) => (
                  <CompactDepositRow
                    key={dep.id}
                    deposit={dep}
                    copiedTx={copiedTx}
                    onCopyTx={handleCopyTx}
                  />
                ))}
              </div>
            </div>
          )}
        </div>
      ) : (
        /* HISTÓRICO DE DEPÓSITOS TAB */
        <div className="space-y-6">
          {/* FILTER BAR & MONITORING HEADER */}
          <div className="card p-4 sm:p-5 flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
            {/* COIN PILLS */}
            <div className="flex flex-wrap items-center gap-1.5">
              <button
                type="button"
                onClick={() => setHistoryCoinFilter('ALL')}
                className={`rounded-xl px-3 py-1.5 text-xs font-bold transition-all ${
                  historyCoinFilter === 'ALL'
                    ? 'bg-bitcoin text-white shadow-sm'
                    : 'bg-surface text-ink-muted hover:text-ink hover:bg-paper border border-border'
                }`}
              >
                Todas ({deposits.length})
              </button>
              {COINS.map((c) => {
                const count = deposits.filter((d) => d.coin.toUpperCase() === c).length;
                if (count === 0 && historyCoinFilter !== c) return null;
                return (
                  <button
                    key={c}
                    type="button"
                    onClick={() => setHistoryCoinFilter(c)}
                    className={`inline-flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-xs font-bold transition-all border ${
                      historyCoinFilter === c
                        ? 'bg-bitcoin text-white border-bitcoin shadow-sm'
                        : 'bg-surface text-ink border-border hover:bg-paper'
                    }`}
                  >
                    <img src={coinLogo(c)} alt={c} className="h-4 w-4 rounded-full" />
                    <span>{c}</span>
                    <span className="rounded-full bg-black/10 dark:bg-white/10 px-1.5 py-0.2 text-[10px]">
                      {count}
                    </span>
                  </button>
                );
              })}
            </div>

            {/* SYNC STATUS */}
            <div className="flex items-center gap-3 self-end sm:self-auto text-xs text-ink-muted">
              <div className="inline-flex items-center gap-2 rounded-full border border-emerald-500/30 bg-emerald-500/10 px-3 py-1 text-emerald-700 dark:text-emerald-400 font-semibold text-[11px]">
                <span className="h-2 w-2 rounded-full bg-emerald-500 animate-pulse" />
                <span>Monitoramento On-Chain (5s)</span>
              </div>
              <button
                type="button"
                onClick={() => refetchHistory()}
                className="btn-secondary px-2.5 py-1 text-xs"
                title="Atualizar agora"
              >
                <i className={`bi bi-arrow-clockwise ${isFetchingHistory ? 'animate-spin' : ''}`} />
              </button>
            </div>
          </div>

          {/* DEPOSIT LIST */}
          {isLoadingHistory ? (
            <div className="card p-12 text-center space-y-3">
              <i className="bi bi-arrow-repeat animate-spin text-3xl text-bitcoin inline-block" />
              <div className="text-sm font-bold text-ink">Buscando depósitos na blockchain...</div>
              <div className="text-xs text-ink-muted">Sincronizando com os nós das redes.</div>
            </div>
          ) : filteredDeposits.length === 0 ? (
            <div className="card p-12 text-center space-y-4">
              <div className="mx-auto flex h-14 w-14 items-center justify-center rounded-2xl bg-paper border border-border text-ink-muted">
                <i className="bi bi-inbox text-2xl text-bitcoin-dark" />
              </div>
              <div className="space-y-1">
                <h3 className="text-base font-bold text-ink">Nenhum depósito encontrado</h3>
                <p className="text-xs text-ink-muted max-w-md mx-auto">
                  {historyCoinFilter === 'ALL'
                    ? 'Assim que você enviar uma transação para qualquer um dos seus endereços SatsPay, ela será detectada e monitorada aqui em tempo real.'
                    : `Nenhum depósito detectado para ${historyCoinFilter}. Gere um endereço na aba de depósito para transferir.`}
                </p>
              </div>
              <button
                type="button"
                onClick={() => switchTab('deposit')}
                className="btn-primary text-xs mx-auto"
              >
                <i className="bi bi-qr-code mr-1.5" /> Ir para Depósito
              </button>
            </div>
          ) : (
            <div className="divide-y divide-border/60 rounded-2xl border border-border bg-paper shadow-sm overflow-hidden">
              {filteredDeposits.map((dep) => (
                <CompactDepositRow
                  key={dep.id}
                  deposit={dep}
                  copiedTx={copiedTx}
                  onCopyTx={handleCopyTx}
                />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function CompactDepositRow({
  deposit,
  copiedTx,
  onCopyTx,
}: {
  deposit: DepositHistoryItem;
  copiedTx: string | null;
  onCopyTx: (tx: string) => void;
}) {
  const rawCoin = deposit.coin.toUpperCase();
  const coinSymbol = isCoin(rawCoin) ? rawCoin : undefined;
  const cfg = coinSymbol ? COIN_CONFIG[coinSymbol] : undefined;
  const explorerUrl = getExplorerTxUrl(deposit.coin, deposit.txHash);
  const minConfs =
    deposit.minConfirmations > 0 ? deposit.minConfirmations : (cfg?.minConfirmations ?? 0);
  const confs = deposit.confirmations;
  const progressPercent =
    minConfs > 0 ? Math.min(100, Math.round((confs / minConfs) * 100)) : 0;

  const isCredited = deposit.status === 'CREDITED';
  const isConfirmed = deposit.status === 'CONFIRMED' || (confs >= minConfs && !isCredited);
  const isPending = deposit.status === 'PENDING' && !isConfirmed && !isCredited;
  const isOrphaned = deposit.status === 'ORPHANED';

  const formattedAmountStr = coinSymbol && cfg
    ? formatAmount(deposit.amount, coinSymbol)
    : deposit.amount;

  const dateStr = new Date(deposit.detectedAt).toLocaleString('pt-BR', {
    day: '2-digit',
    month: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });

  const shortTx = deposit.txHash.length > 16
    ? `${deposit.txHash.slice(0, 8)}...${deposit.txHash.slice(-6)}`
    : deposit.txHash;

  return (
    <div className="p-3.5 sm:px-5 sm:py-3.5 hover:bg-surface/80 transition-colors flex flex-col gap-2.5 sm:flex-row sm:items-center sm:justify-between">
      {/* LEFT: COIN ICON, AMOUNT, DATE */}
      <div className="flex items-center gap-3 min-w-0 sm:w-1/3">
        {coinSymbol ? (
          <img
            src={coinLogo(coinSymbol)}
            alt={deposit.coin}
            className="h-8 w-8 rounded-full shrink-0 shadow-xs"
          />
        ) : (
          <i className="bi bi-coin h-8 w-8 text-ink-muted" />
        )}
        <div className="min-w-0">
          <div className="flex items-center gap-1.5 font-bold text-sm text-ink truncate">
            <span className="text-emerald-600 dark:text-emerald-400">+{formattedAmountStr}</span>
            <span className="text-xs text-ink-muted">{deposit.coin}</span>
          </div>
          <div className="text-[11px] text-ink-muted truncate">
            {dateStr}
          </div>
        </div>
      </div>

      {/* MIDDLE: CONFIRMATIONS & PROGRESS BAR */}
      <div className="flex-1 max-w-xs space-y-1">
        <div className="flex items-center justify-between text-[11px] font-medium">
          <span className="text-ink-muted">
            {confs.toLocaleString()} / {minConfs} confs
          </span>
          <span className="font-mono text-ink text-[10px]">
            {progressPercent}%
          </span>
        </div>
        <div className="h-1.5 w-full overflow-hidden rounded-full bg-surface border border-border">
          <div
            className={`h-full transition-all duration-300 ${
              isCredited
                ? 'bg-emerald-500'
                : isConfirmed
                ? 'bg-teal-500'
                : 'bg-amber-500 animate-pulse'
            }`}
            style={{ width: `${progressPercent}%` }}
          />
        </div>
      </div>

      {/* RIGHT: STATUS & TX ACTIONS */}
      <div className="flex items-center justify-between sm:justify-end gap-2.5 shrink-0 pt-1 sm:pt-0 border-t border-border/40 sm:border-0">
        {/* STATUS PILL */}
        {isCredited ? (
          <span className="inline-flex items-center gap-1 rounded-md bg-emerald-500/10 px-2 py-0.5 text-[11px] font-semibold text-emerald-700 dark:text-emerald-400 border border-emerald-500/20">
            <i className="bi bi-check-circle-fill text-[10px] text-emerald-500" />
            <span>Creditado</span>
          </span>
        ) : isConfirmed ? (
          <span className="inline-flex items-center gap-1 rounded-md bg-teal-500/10 px-2 py-0.5 text-[11px] font-semibold text-teal-700 dark:text-teal-400 border border-teal-500/20">
            <i className="bi bi-shield-check text-[10px] text-teal-500" />
            <span>Confirmado</span>
          </span>
        ) : isOrphaned ? (
          <span className="inline-flex items-center gap-1 rounded-md bg-rose-500/10 px-2 py-0.5 text-[11px] font-semibold text-rose-700 dark:text-rose-400 border border-rose-500/20">
            <i className="bi bi-x-circle-fill text-[10px] text-rose-500" />
            <span>Órfão</span>
          </span>
        ) : isPending ? (
          <span className="inline-flex items-center gap-1 rounded-md bg-amber-500/10 px-2 py-0.5 text-[11px] font-semibold text-amber-700 dark:text-amber-400 border border-amber-500/20 animate-pulse">
            <i className="bi bi-arrow-repeat animate-spin text-[10px] text-amber-500" />
            <span>Confirmando...</span>
          </span>
        ) : (
          <span className="inline-flex items-center gap-1 rounded-md bg-amber-500/10 px-2 py-0.5 text-[11px] font-semibold text-amber-700 dark:text-amber-400 border border-amber-500/20">
            <i className="bi bi-clock text-[10px] text-amber-500" />
            <span>{deposit.status}</span>
          </span>
        )}

        {/* HASH BUTTON & EXPLORER LINK */}
        <div className="flex items-center gap-1.5 font-mono text-[11px]">
          <button
            type="button"
            onClick={() => onCopyTx(deposit.txHash)}
            className="inline-flex items-center gap-1 rounded-lg border border-border bg-paper hover:bg-surface px-2 py-0.5 text-[11px] text-ink transition-colors"
            title={deposit.txHash}
          >
            <span className="hidden md:inline">{shortTx}</span>
            <i className={`bi ${copiedTx === deposit.txHash ? 'bi-check2 text-emerald-600' : 'bi-clipboard'}`} />
          </button>

          <a
            href={explorerUrl}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center justify-center h-6 w-6 rounded-lg bg-paper hover:bg-bitcoin/10 text-ink-muted hover:text-bitcoin border border-border transition-colors"
            title="Abrir no Explorer"
          >
            <i className="bi bi-box-arrow-up-right text-[10px]" />
          </a>
        </div>
      </div>
    </div>
  );
}


