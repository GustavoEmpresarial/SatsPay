import { useState, useMemo, useEffect } from 'react';
import { useSearchParams, useNavigate, Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { motion, AnimatePresence } from 'framer-motion';
import { api } from '../lib/api.js';
import { formatApiError } from '../lib/formatError.js';
import { coinLogo } from '../lib/coinAssets.js';
import { useAuthStore } from '../stores/auth.js';
import { COIN_CONFIG, COINS, defaultDepositWithdrawCoin, depositWithdrawActiveCoins, depositWithdrawPausedCoinList, formatAmount, isCoin, isDepositWithdrawPaused, safeBigInt, type Coin, type WalletBalance } from '@/shared';
import { Modal } from '../components/Modal.js';
import { getExplorerTxUrl } from '../lib/chainExplorers.js';
import { canWithdraw, isPlausibleAddress, parseHumanAmount } from '../lib/amountInput.js';

interface WalletsResp {
  wallets: WalletBalance[];
}

interface SavedAddress {
  id: string;
  coin: Coin;
  label: string;
  address: string;
}

export interface WithdrawalHistoryItem {
  id: string;
  coin: string;
  toAddress: string;
  amount: string;
  feeAmount: string;
  status: 'PENDING' | 'QUEUED' | 'BROADCASTING' | 'BROADCASTED' | 'CONFIRMED' | 'FAILED' | 'REJECTED' | 'CANCELLED' | string;
  txHash?: string | null;
  requiresApproval: boolean;
  createdAt: string;
  updatedAt: string;
}

export function WithdrawPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const qc = useQueryClient();
  const user = useAuthStore((s) => s.user);
  const [searchParams, setSearchParams] = useSearchParams();

  // Initialize coin from URL param if valid and not paused
  const rawInitialCoin = searchParams.get('coin')?.toUpperCase();
  const initialCoin = defaultDepositWithdrawCoin(rawInitialCoin);
  const [coin, setCoin] = useState<Coin>(initialCoin);
  const [activeTab, setActiveTab] = useState<'withdraw' | 'history'>(
    searchParams.get('tab') === 'history' ? 'history' : 'withdraw'
  );
  const [historyCoinFilter, setHistoryCoinFilter] = useState<string>('ALL');
  const [isCoinSelectorOpen, setIsCoinSelectorOpen] = useState(false);
  const [copiedTx, setCopiedTx] = useState<string | null>(null);

  useEffect(() => {
    const urlCoin = searchParams.get('coin')?.toUpperCase();
    const next = defaultDepositWithdrawCoin(urlCoin);
    if (next !== coin) {
      setCoin(next);
    }
    if (urlCoin && isCoin(urlCoin) && isDepositWithdrawPaused(urlCoin) && next !== urlCoin) {
      setSearchParams((prev) => {
        const p = new URLSearchParams(prev);
        p.set('coin', next);
        return p;
      }, { replace: true });
    }
    const urlTab = searchParams.get('tab');
    if (urlTab === 'history' || urlTab === 'withdraw') {
      setActiveTab(urlTab);
    }
  }, [searchParams, coin, setSearchParams]);

  const [address, setAddress] = useState('');
  const [inputVal, setInputVal] = useState('');
  const [emailCode, setEmailCode] = useState('');
  const [needsEmailCode, setNeedsEmailCode] = useState(false);
  const [msg, setMsg] = useState<{ type: 'success' | 'error'; text: string } | null>(null);
  const [showAllFeesModal, setShowAllFeesModal] = useState(false);

  // Address book / Whitelist state
  const [showAddressBook, setShowAddressBook] = useState(false);
  const [newLabel, setNewLabel] = useState('');

  const addressBookQ = useQuery({
    queryKey: ['withdrawal-addresses'],
    queryFn: () => api<{ addresses: SavedAddress[] }>('/withdrawals/addresses'),
    enabled: Boolean(user),
  });
  const savedAddresses = addressBookQ.data?.addresses ?? [];

  const saveAddressMut = useMutation({
    mutationFn: (item: { coin: Coin; label: string; address: string }) =>
      api<SavedAddress>('/withdrawals/addresses', { method: 'POST', json: item }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['withdrawal-addresses'] });
      setNewLabel('');
      setShowAddressBook(false);
    },
    onError: (err) => setMsg({ type: 'error', text: formatApiError(err) }),
  });

  const deleteAddressMut = useMutation({
    mutationFn: (id: string) => api(`/withdrawals/addresses/${id}`, { method: 'DELETE' }),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ['withdrawal-addresses'] }),
    onError: (err) => setMsg({ type: 'error', text: formatApiError(err) }),
  });

  const walletsQ = useQuery({
    queryKey: ['wallets', 'PERSONAL'],
    queryFn: () => api<WalletsResp>('/wallet?kind=PERSONAL'),
    enabled: Boolean(user),
  });
  // Fetch real-time withdrawal history with 5s polling
  const {
    data: historyData,
    isLoading: isLoadingHistory,
    refetch: refetchHistory,
    isFetching: isFetchingHistory,
  } = useQuery({
    queryKey: ['withdrawals-history'],
    queryFn: () => api<{ withdrawals: WithdrawalHistoryItem[] }>('/withdrawals/history'),
    enabled: Boolean(user),
    refetchInterval: 5000,
  });

  const withdrawals = historyData?.withdrawals || [];
  const activeCoinWithdrawals = withdrawals.filter((w) => w.coin.toUpperCase() === coin.toUpperCase());
  const filteredWithdrawals = historyCoinFilter === 'ALL'
    ? withdrawals
    : withdrawals.filter((w) => w.coin.toUpperCase() === historyCoinFilter.toUpperCase());

  const pendingCount = withdrawals.filter((w) => w.status === 'PENDING' || w.status === 'QUEUED' || w.status === 'BROADCASTING').length;

  const walletMap = useMemo(() => {
    return (walletsQ.data?.wallets ?? []).reduce<Record<string, WalletBalance>>((acc, w) => {
      acc[w.coin] = w;
      return acc;
    }, {});
  }, [walletsQ.data]);
  // This page is personal-only. Merchant caixa lives in the merchant panel and
  // is never read here.
  const currentBal = walletMap[coin] ? safeBigInt(walletMap[coin]!.balance) : 0n;

  const cfg = COIN_CONFIG[coin] || COIN_CONFIG.BTC;
  const fee = cfg.withdrawalFee || 0n;
  const faucetFee = cfg.faucetPayFee || 0n;
  const withdrawPaused = isDepositWithdrawPaused(coin);

  // Sync URL search params — only active (non-paused) coins are selectable
  const handleSelectCoin = (newCoin: Coin) => {
    if (isDepositWithdrawPaused(newCoin)) return;
    setCoin(newCoin);
    setSearchParams((prev) => {
      const p = new URLSearchParams(prev);
      p.set('coin', newCoin);
      return p;
    });
    setInputVal('');
    setMsg(null);
    setIsCoinSelectorOpen(false);
  };

  const selectableCoins = useMemo(() => depositWithdrawActiveCoins(), []);
  const pausedCoins = useMemo(() => depositWithdrawPausedCoinList(), []);

  const switchTab = (tab: 'withdraw' | 'history') => {
    setActiveTab(tab);
    setSearchParams((prev) => {
      const p = new URLSearchParams(prev);
      p.set('tab', tab);
      return p;
    });
  };

  const handleCopyTx = (txHash: string) => {
    navigator.clipboard.writeText(txHash);
    setCopiedTx(txHash);
    setTimeout(() => setCopiedTx(null), 2000);
  };

  const smallestAmount = useMemo(() => parseHumanAmount(inputVal, coin), [inputVal, coin]);

  const totalDebit = smallestAmount > 0n ? smallestAmount + fee : 0n;

  const handleSaveCurrentAddress = () => {
    if (!address.trim() || !newLabel.trim()) return;
    saveAddressMut.mutate({
      coin,
      label: newLabel.trim(),
      address: address.trim(),
    });
  };

  const handleDeleteSavedAddress = (id: string) => {
    deleteAddressMut.mutate(id);
  };

  const withdrawMut = useMutation({
    mutationFn: () =>
      api<{ codeSent?: boolean }>('/withdrawals', {
        method: 'POST',
        json: {
          coin,
          toAddress: address.trim(),
          amount: smallestAmount.toString(),
          ...(emailCode ? { emailCode: emailCode.trim() } : {}),
        },
      }),
    onSuccess: (res) => {
      if (res && typeof res === 'object' && res.codeSent) {
        setNeedsEmailCode(true);
        setMsg({
          type: 'success',
          text: 'Enviamos um código para o seu e-mail. Informe-o para confirmar o saque.',
        });
        return;
      }
      qc.invalidateQueries({ queryKey: ['wallets'] });
      qc.invalidateQueries({ queryKey: ['wallets', 'PERSONAL'] });
      qc.invalidateQueries({ queryKey: ['ledger'] });
      qc.invalidateQueries({ queryKey: ['withdrawals-history'] });
      setAddress('');
      setInputVal('');
      setEmailCode('');
      setNeedsEmailCode(false);
      setMsg({
        type: 'success',
        text: `Saque de ${formatAmount(smallestAmount, coin)} ${coin} solicitado com sucesso!`,
      });
    },
    onError: (err) => {
      setMsg({ type: 'error', text: formatApiError(err) });
    },
  });

  const setPercentage = (pct: number) => {
    if (currentBal <= 0n) return;
    if (pct === 100) {
      const maxWithdrawable = currentBal > fee ? currentBal - fee : 0n;
      setInputVal(formatAmount(maxWithdrawable, coin));
      return;
    }
    const target = (currentBal * BigInt(pct)) / 100n;
    setInputVal(formatAmount(target, coin));
  };

  const filteredSavedAddresses = savedAddresses.filter((a) => a.coin === coin);
  const isBelowMin = smallestAmount > 0n && smallestAmount < cfg.minWithdrawal;
  const insufficient = totalDebit > currentBal && currentBal > 0n;
  const canSubmit =
    !withdrawPaused &&
    canWithdraw({
      amount: smallestAmount,
      balance: currentBal,
      fee,
      minWithdrawal: cfg.minWithdrawal,
    }).ok &&
    isPlausibleAddress(address) &&
    !withdrawMut.isPending;

  return (
    <div className="mx-auto max-w-5xl space-y-6 pb-12">
      {/* HEADER WITH BACK BUTTON & TAB SELECTOR */}
      <header className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 border-b border-border/60 pb-5">
        <div>
          <button
            type="button"
            onClick={() => navigate('/wallets')}
            className="mb-3 inline-flex items-center gap-2 rounded-xl border border-border bg-surface px-3.5 py-2 text-xs font-semibold text-ink hover:bg-paper hover:text-bitcoin-dark hover:border-bitcoin/40 transition-all shadow-xs group"
          >
            <i className="bi bi-arrow-left text-sm text-bitcoin-dark group-hover:-translate-x-0.5 transition-transform" />
            <span>Voltar para Carteiras</span>
          </button>
          <div className="flex items-center gap-2 text-xs font-bold uppercase tracking-wider text-bitcoin-dark mb-1">
            <i className="bi bi-shield-check" />
            <span>Saques On-Chain Diretos</span>
          </div>
          <h1 className="text-2xl sm:text-3xl font-extrabold tracking-tight text-ink">
            Sacar {cfg.name} ({coin})
          </h1>
          <p className="text-xs sm:text-sm text-ink-muted mt-1">
            Envio direto para qualquer carteira blockchain externa com taxa reduzida e monitoramento em tempo real.
          </p>
        </div>

        {/* TOP CONTROLS & TABS */}
        <div className="flex flex-wrap items-center gap-2 self-start sm:self-auto">
          {/* TAB BUTTONS */}
          <div className="inline-flex rounded-2xl border border-border bg-surface p-1 shadow-xs">
            <button
              type="button"
              onClick={() => switchTab('withdraw')}
              className={`inline-flex items-center gap-2 rounded-xl px-4 py-2 text-xs font-bold transition-all ${
                activeTab === 'withdraw'
                  ? 'bg-bitcoin text-white shadow-md shadow-bitcoin/20'
                  : 'text-ink-muted hover:text-ink hover:bg-paper'
              }`}
            >
              <i className="bi bi-box-arrow-up-right text-sm" />
              <span>Solicitar Saque</span>
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
              ) : withdrawals.length > 0 ? (
                <span className="flex h-5 min-w-[20px] px-1.5 items-center justify-center rounded-full bg-paper text-[10px] font-bold text-ink-muted border border-border">
                  {withdrawals.length}
                </span>
              ) : null}
            </button>
          </div>

          <Link to="/wallets" className="btn-secondary text-xs px-3.5 py-2 flex items-center gap-1.5">
            <i className="bi bi-wallet2 text-bitcoin" />
            <span>Minhas Carteiras</span>
          </Link>
          <button
            type="button"
            onClick={() => setShowAllFeesModal(true)}
            className="btn-secondary text-xs px-3.5 py-2 flex items-center gap-2"
          >
            <i className="bi bi-tag-fill text-bitcoin" />
            <span>Tabela de Taxas</span>
          </button>
        </div>
      </header>

      {/* FEEDBACK MESSAGE */}
      {msg && (
        <div
          className={`rounded-2xl p-4 text-sm border flex items-center gap-3 shadow-xs ${
            msg.type === 'success'
              ? 'bg-emerald-50 text-emerald-800 border-emerald-200'
              : 'bg-rose-50 text-rose-800 border-rose-200'
          }`}
        >
          <i
            className={`bi ${
              msg.type === 'success'
                ? 'bi-check-circle-fill text-emerald-600'
                : 'bi-exclamation-triangle-fill text-rose-600'
            } text-xl shrink-0`}
          />
          <span className="font-medium">{msg.text}</span>
        </div>
      )}

      {/* ACTIVE TAB: WITHDRAW FORM */}
      {activeTab === 'withdraw' ? (
        <div className="space-y-6">
          <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
            {/* LEFT COLUMN: FORM (7/12 on Desktop) */}
            <div className="lg:col-span-7 space-y-5">
              <form
                onSubmit={(e) => {
                  e.preventDefault();
                  if (canSubmit) withdrawMut.mutate();
                }}
                className="card p-5 sm:p-7 space-y-6 shadow-sm"
              >
                {/* COIN SELECTOR */}
                <div>
                  <label className="block text-xs font-bold uppercase tracking-wider text-ink-muted mb-2">
                    Criptomoeda Selecionada
                  </label>

                  <div className="relative">
                    <button
                      type="button"
                      onClick={() => setIsCoinSelectorOpen(!isCoinSelectorOpen)}
                      className="w-full flex items-center justify-between p-3.5 rounded-2xl bg-surface hover:bg-paper border border-border transition-all hover:border-bitcoin/50 group text-left"
                    >
                      <div className="flex items-center gap-3.5">
                        <div className="h-10 w-10 rounded-xl bg-paper p-1.5 shadow-sm border border-border flex items-center justify-center shrink-0">
                          <img src={coinLogo(coin)} alt={coin} className="h-full w-full object-contain" />
                        </div>
                        <div>
                          <div className="font-bold text-base text-ink flex items-center gap-2">
                            <span>{cfg.name}</span>
                            <span className="text-xs font-mono font-bold text-bitcoin-dark bg-bitcoin/10 px-2 py-0.5 rounded-md">
                              {coin}
                            </span>
                          </div>
                          <div className="text-xs text-ink-muted mt-0.5">
                            {t('withdraw.personalBalance')}: <strong className="font-mono text-ink">{formatAmount(currentBal, coin)} {coin}</strong>
                          </div>
                        </div>
                      </div>

                      <div className="flex items-center gap-1.5 text-xs font-semibold text-bitcoin-dark group-hover:translate-x-0.5 transition-transform pr-2">
                        <span>Alterar</span>
                        <i className="bi bi-chevron-down text-xs" />
                      </div>
                    </button>

                    {/* COIN DROPDOWN / POPUP */}
                    <AnimatePresence>
                      {isCoinSelectorOpen && (
                        <motion.div
                          initial={{ opacity: 0, y: 6 }}
                          animate={{ opacity: 1, y: 0 }}
                          exit={{ opacity: 0, y: 4 }}
                          className="absolute z-30 left-0 right-0 top-full mt-2 rounded-2xl border border-border bg-paper p-2 shadow-xl max-h-[28rem] overflow-y-auto"
                        >
                          <div className="px-3 py-1.5 text-[11px] font-bold uppercase tracking-wider text-ink-muted border-b border-border/50 mb-1">
                            Escolha uma moeda para sacar:
                          </div>
                          <div className="grid grid-cols-1 sm:grid-cols-2 gap-1">
                            {selectableCoins.map((c) => {
                              const conf = COIN_CONFIG[c];
                              const bal = walletMap[c] ? safeBigInt(walletMap[c]!.balance) : 0n;
                              const isSelected = c === coin;
                              return (
                                <button
                                  key={c}
                                  type="button"
                                  onClick={() => handleSelectCoin(c)}
                                  className={`flex items-center justify-between p-2.5 rounded-xl text-left transition-colors ${
                                    isSelected
                                      ? 'bg-bitcoin/10 font-bold text-bitcoin-dark border border-bitcoin/30'
                                      : 'hover:bg-surface text-ink'
                                  }`}
                                >
                                  <div className="flex items-center gap-2.5 truncate">
                                    <img
                                      src={coinLogo(c)}
                                      alt={c}
                                      className="h-6 w-6 rounded-full shrink-0"
                                    />
                                    <div className="truncate">
                                      <div className="text-xs font-semibold leading-tight">{conf.name}</div>
                                      <div className="text-[10px] text-ink-muted">{c}</div>
                                    </div>
                                  </div>
                                  <div className="font-mono text-xs text-right shrink-0">
                                    {formatAmount(bal, c)}
                                  </div>
                                </button>
                              );
                            })}
                          </div>
                          {pausedCoins.length > 0 && (
                            <div className="mt-2 border-t border-border/50 pt-2 space-y-1">
                              <div className="px-3 py-1 text-[11px] font-bold uppercase tracking-wider text-ink-muted">
                                {t('withdraw.pausedSection')}
                              </div>
                              <div className="grid grid-cols-1 sm:grid-cols-2 gap-1">
                                {pausedCoins.map((c) => {
                                  const conf = COIN_CONFIG[c];
                                  return (
                                    <button
                                      key={c}
                                      type="button"
                                      disabled
                                      aria-disabled="true"
                                      className="flex cursor-not-allowed items-center justify-between rounded-xl p-2.5 text-left opacity-60"
                                    >
                                      <div className="flex items-center gap-2.5 truncate">
                                        <img
                                          src={coinLogo(c)}
                                          alt=""
                                          className="h-6 w-6 rounded-full shrink-0 grayscale"
                                        />
                                        <div className="truncate">
                                          <div className="text-xs font-semibold leading-tight text-ink-muted">{conf.name}</div>
                                          <div className="text-[10px] text-ink-muted">{c}</div>
                                        </div>
                                      </div>
                                      <span className="inline-flex shrink-0 items-center gap-1 rounded-full border border-amber-500/25 bg-amber-500/10 px-2 py-0.5 text-[10px] font-semibold text-amber-800">
                                        <i className="bi bi-pause-circle" />
                                        {t('withdraw.pausedBadge')}
                                      </span>
                                    </button>
                                  );
                                })}
                              </div>
                            </div>
                          )}
                        </motion.div>
                      )}
                    </AnimatePresence>
                  </div>
                </div>

                {withdrawPaused && (
                  <div className="rounded-2xl border border-amber-500/30 bg-amber-500/10 p-4 text-center space-y-1">
                    <p className="text-sm font-bold text-ink">Saques de {coin} temporariamente pausados</p>
                    <p className="text-xs text-ink-muted">
                      A moeda continua visível. Depósitos e saques desta rede estão desabilitados por enquanto.
                    </p>
                  </div>
                )}

                {/* DESTINATION ADDRESS WITH SAVED ADDRESSES */}
                <div className="space-y-2">
                  <div className="flex items-center justify-between">
                    <label className="text-xs font-bold uppercase tracking-wider text-ink-muted">
                      Endereço {cfg.name} de Destino
                    </label>
                    <button
                      type="button"
                      onClick={() => setShowAddressBook(!showAddressBook)}
                      className="text-xs font-semibold text-bitcoin-dark hover:underline flex items-center gap-1.5"
                    >
                      <i className="bi bi-bookmark-star" />
                      <span>{showAddressBook ? 'Ocultar Caderno' : 'Endereços Salvos'}</span>
                    </button>
                  </div>

                  <div className="relative">
                    <input
                      type="text"
                      required
                      value={address}
                      onChange={(e) => setAddress(e.target.value)}
                      className="input w-full font-mono text-xs sm:text-sm py-3 px-3.5 pr-20"
                      placeholder={`Cole o endereço ${coin} de destino...`}
                    />
                    {address.trim().length > 10 && (
                      <button
                        type="button"
                        onClick={() => setShowAddressBook(true)}
                        className="absolute right-2.5 top-1/2 -translate-y-1/2 text-[11px] font-bold text-bitcoin-dark hover:bg-bitcoin/10 px-2 py-1 rounded-md transition-colors"
                      >
                        + Salvar
                      </button>
                    )}
                  </div>

                  {/* WHITELIST ADDRESS BOOK DRAWER */}
                  {showAddressBook && (
                    <div className="rounded-2xl border border-border bg-surface p-4 space-y-3">
                      <div className="text-xs font-bold text-ink flex items-center justify-between">
                        <span>Whitelist de Endereços ({coin})</span>
                        <span className="text-[10px] text-ink-muted">Anti-Clipboard Hijacking</span>
                      </div>

                      {filteredSavedAddresses.length === 0 ? (
                        <p className="text-xs text-ink-muted py-2">
                          Nenhum endereço de {coin} salvo na whitelist ainda.
                        </p>
                      ) : (
                        <ul className="space-y-1.5 max-h-40 overflow-y-auto">
                          {filteredSavedAddresses.map((sa) => (
                            <li
                              key={sa.id}
                              className="flex items-center justify-between rounded-xl bg-paper p-2.5 text-xs border border-border/60"
                            >
                              <button
                                type="button"
                                onClick={() => {
                                  setAddress(sa.address);
                                  setShowAddressBook(false);
                                }}
                                className="text-left font-mono truncate flex-1 hover:text-bitcoin-dark pr-2"
                              >
                                <div className="font-bold text-ink">{sa.label}</div>
                                <div className="text-[11px] text-ink-muted truncate">{sa.address}</div>
                              </button>
                              <button
                                type="button"
                                onClick={() => handleDeleteSavedAddress(sa.id)}
                                className="text-rose-600 hover:text-rose-700 p-1.5 rounded hover:bg-rose-50"
                                title="Remover"
                              >
                                <i className="bi bi-trash3 text-xs" />
                              </button>
                            </li>
                          ))}
                        </ul>
                      )}

                      {address.trim().length > 10 && (
                        <div className="flex gap-2 pt-2 border-t border-border/80">
                          <input
                            type="text"
                            placeholder="Apelido (ex: Minha Carteira Fria)"
                            className="input text-xs py-1.5 flex-1"
                            value={newLabel}
                            onChange={(e) => setNewLabel(e.target.value)}
                          />
                          <button
                            type="button"
                            onClick={handleSaveCurrentAddress}
                            className="btn-secondary text-xs px-3 py-1.5 font-bold"
                          >
                            Salvar na Whitelist
                          </button>
                        </div>
                      )}
                    </div>
                  )}
                </div>

                {/* AMOUNT INPUT */}
                <div className="space-y-2">
                  <div className="flex justify-between items-center">
                    <label className="text-xs font-bold uppercase tracking-wider text-ink-muted">
                      Quantia do Saque
                    </label>
                    <span className="text-xs text-ink-muted">
                      Mínimo: <strong className="font-mono text-ink">{cfg.minWithdrawal <= 1n ? 'Sem valor mínimo' : `${formatAmount(cfg.minWithdrawal, coin)} ${coin}`}</strong>
                    </span>
                  </div>

                  <div className="rounded-2xl border border-border bg-surface p-4 focus-within:ring-2 focus-within:ring-bitcoin/30 focus-within:border-bitcoin transition-all">
                    <div className="flex items-center justify-between gap-3">
                      <input
                        type="text"
                        required
                        value={inputVal}
                        onChange={(e) => {
                          const val = e.target.value.replace(/[^0-9.]/g, '');
                          if ((val.match(/\./g) || []).length <= 1) {
                            setInputVal(val);
                          }
                        }}
                        className="w-full border-none bg-transparent font-mono text-2xl sm:text-3xl font-extrabold outline-none text-ink placeholder:text-ink-muted"
                        placeholder="0.00"
                      />
                      <div className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-paper border border-border shrink-0">
                        <img src={coinLogo(coin)} alt={coin} className="h-5 w-5 rounded-full" />
                        <span className="font-bold text-xs text-ink">{coin}</span>
                      </div>
                    </div>

                    {/* PERCENTAGE QUICK SHORTCUTS */}
                    <div className="mt-3.5 flex items-center justify-between gap-2 pt-3 border-t border-border/50">
                      <div className="text-[11px] text-ink-muted font-medium">
                        {t('withdraw.personalBalance')}: <span className="font-mono font-bold text-ink">{formatAmount(currentBal, coin)}</span>
                      </div>
                      <div className="flex items-center gap-1.5">
                        {[25, 50, 75, 100].map((pct) => (
                          <button
                            key={pct}
                            type="button"
                            onClick={() => setPercentage(pct)}
                            className={`rounded-lg px-2.5 py-1 text-xs font-bold transition-all border ${
                              pct === 100
                                ? 'bg-bitcoin/10 text-bitcoin-dark border-bitcoin/30 hover:bg-bitcoin/20'
                                : 'bg-paper text-ink hover:bg-surface border-border'
                            }`}
                          >
                            {pct === 100 ? 'MÁXIMO' : `${pct}%`}
                          </button>
                        ))}
                      </div>
                    </div>
                  </div>
                </div>

                {/* Email OTP (SMTP step-up or 2FA enabled) */}
                {needsEmailCode || user?.twoFactorEnabled ? (
                  <div className="space-y-1.5">
                    <label className="block text-xs font-bold uppercase tracking-wider text-ink-muted flex items-center justify-between">
                      <span>Código de autenticação (e-mail)</span>
                      <span className="text-[10px] font-bold text-emerald-600 bg-emerald-50 px-2 py-0.5 rounded-md border border-emerald-200">
                        <i className="bi bi-shield-check mr-1" /> Proteção Ativa
                      </span>
                    </label>
                    <input
                      type="text"
                      required
                      value={emailCode}
                      onChange={(e) => setEmailCode(e.target.value.replace(/\D/g, ''))}
                      maxLength={6}
                      className="input w-full font-mono text-base tracking-widest text-center py-2.5"
                      placeholder="000000"
                    />
                  </div>
                ) : (
                  <div className="rounded-2xl border border-border/80 bg-surface/60 p-3.5 text-xs text-ink-muted flex items-center justify-between">
                    <span className="flex items-center gap-2">
                      <i className="bi bi-shield text-ink-muted text-sm" />
                      <span>2FA Opcional não está ativo nesta conta.</span>
                    </span>
                    <a href="/settings" className="text-bitcoin-dark hover:underline font-bold text-xs">
                      Ativar 2FA →
                    </a>
                  </div>
                )}

                {/* ERROR ALERTS */}
                {isBelowMin && (
                  <div className="rounded-xl bg-amber-50 p-3 text-xs font-medium text-amber-800 border border-amber-200 flex items-center gap-2">
                    <i className="bi bi-exclamation-triangle-fill text-amber-600 shrink-0" />
                    <span>O valor mínimo de saque para {coin} é de <strong>{formatAmount(cfg.minWithdrawal, coin)} {coin}</strong>.</span>
                  </div>
                )}

                {insufficient && (
                  <div className="rounded-xl bg-rose-50 p-3 text-xs font-medium text-rose-700 border border-rose-200 flex items-center gap-2">
                    <i className="bi bi-x-circle-fill text-rose-600 shrink-0" />
                    <span>Saldo insuficiente para cobrir o saque e a taxa de rede ({formatAmount(totalDebit, coin)} {coin} necessários).</span>
                  </div>
                )}

                {/* SUBMIT BUTTON */}
                <button
                  type="submit"
                  disabled={!canSubmit}
                  className={`btn-primary w-full py-4 text-base font-bold flex items-center justify-center gap-2 shadow-lg ${
                    !canSubmit ? 'opacity-50 cursor-not-allowed' : 'hover:scale-[1.01] active:scale-[0.99]'
                  }`}
                >
                  {withdrawMut.isPending ? (
                    <>
                      <i className="bi bi-arrow-repeat animate-spin text-lg" />
                      <span>Transmitindo Transação...</span>
                    </>
                  ) : (
                    <>
                      <i className="bi bi-box-arrow-up-right text-lg" />
                      <span>Confirmar Saque de {coin}</span>
                    </>
                  )}
                </button>
              </form>
            </div>

            {/* RIGHT COLUMN: FINANCIAL SUMMARY & NETWORK METRICS (5/12 on Desktop) */}
            <div className="lg:col-span-5 space-y-5 lg:sticky lg:top-6">
              {/* FINANCIAL SUMMARY CARD */}
              <div className="card p-5 sm:p-6 space-y-4 shadow-sm border-bitcoin/20 bg-gradient-to-b from-surface to-paper">
                <div className="flex items-center justify-between border-b border-border/60 pb-3">
                  <h3 className="font-extrabold text-sm uppercase tracking-wider text-ink flex items-center gap-2">
                    <i className="bi bi-receipt text-bitcoin" />
                    <span>Resumo da Transação</span>
                  </h3>
                  <span className="text-[10px] font-bold text-emerald-700 bg-emerald-100 px-2 py-0.5 rounded-full">
                    -25% em Taxas
                  </span>
                </div>

                <div className="space-y-3 text-xs">
                  <div className="flex justify-between items-center">
                    <span className="text-ink-muted">Quantia a Enviar (On-Chain):</span>
                    <span className="font-mono font-bold text-sm text-ink">
                      {formatAmount(smallestAmount, coin)} {coin}
                    </span>
                  </div>

                  {/* FOCUSED FEE COMPARISON FOR SELECTED COIN */}
                  <div className="rounded-xl bg-emerald-50/80 border border-emerald-200/80 p-3 space-y-1.5">
                    <div className="flex justify-between items-center">
                      <span className="text-emerald-900 font-bold flex items-center gap-1.5">
                        <span>Taxa de Rede SatsPay:</span>
                      </span>
                      <span className="font-mono font-black text-emerald-800 text-sm">
                        {formatAmount(fee, coin)} {coin}
                      </span>
                    </div>

                    <div className="flex justify-between items-center text-[11px] text-emerald-700 pt-1 border-t border-emerald-200/50">
                      <span className="line-through text-ink-muted">
                        A Concorrência: {formatAmount(faucetFee, coin)} {coin}
                      </span>
                      <span className="font-bold bg-emerald-200/60 px-1.5 py-0.5 rounded text-[10px]">
                        Economia de 25%
                      </span>
                    </div>
                  </div>

                  <div className="pt-2 border-t border-border/80 flex justify-between items-center">
                    <div>
                      <div className="text-ink font-bold">Total Debitado da Conta:</div>
                      <div className="text-[10px] text-ink-muted">
                        {t('withdraw.debitedFrom')} · Montante + Taxa de rede
                      </div>
                    </div>
                    <div className="font-mono text-lg font-black text-bitcoin-dark">
                      {formatAmount(totalDebit, coin)} {coin}
                    </div>
                  </div>
                </div>

                {/* NETWORK TELEMETRY BOX */}
                <div className="rounded-xl bg-surface p-3.5 border border-border space-y-2 text-xs">
                  <div className="text-[11px] font-bold uppercase tracking-wider text-ink-muted">
                    Parâmetros da Rede ({coin})
                  </div>
                  <div className="grid grid-cols-2 gap-2 text-ink">
                    <div>
                      <span className="text-[10px] text-ink-muted block">Confirmações:</span>
                      <strong className="font-mono">{cfg.minConfirmations} blocos</strong>
                    </div>
                    <div>
                      <span className="text-[10px] text-ink-muted block">Processamento:</span>
                      <strong className="text-emerald-700">Imediato</strong>
                    </div>
                  </div>
                </div>

                {/* SECURITY WARNING */}
                <div className="rounded-xl border border-amber-200/80 bg-amber-50/70 p-3 text-[11px] text-amber-800 space-y-1">
                  <div className="font-bold flex items-center gap-1.5">
                    <i className="bi bi-shield-exclamation text-amber-600" />
                    <span>Segurança de Envio</span>
                  </div>
                  <p className="leading-relaxed text-amber-700">
                    Certifique-se de que o endereço pertence à rede <strong>{coin === 'PEPE' ? 'BNB Smart Chain (BEP-20)' : cfg.name}</strong>.
                    {coin === 'PEPE' ? ' Não envie o PEPE da Ethereum. A taxa de rede (gas) é paga em BNB pela plataforma.' : ''} Transações em blockchain são irreversíveis.
                  </p>
                </div>
              </div>
            </div>
          </div>

          {/* COMPACT RECENT WITHDRAWALS PREVIEW FOR ACTIVE COIN */}
          {activeCoinWithdrawals.length > 0 && (
            <div className="card p-4 sm:p-5 space-y-3">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <span className="h-2 w-2 rounded-full bg-emerald-500 animate-pulse" />
                  <h3 className="text-xs font-bold uppercase tracking-wider text-ink-muted">
                    Últimos Saques Solicitados ({coin})
                  </h3>
                </div>
                <button
                  type="button"
                  onClick={() => switchTab('history')}
                  className="text-xs font-bold text-bitcoin hover:underline inline-flex items-center gap-1"
                >
                  Ver histórico completo ({activeCoinWithdrawals.length}) <i className="bi bi-arrow-right" />
                </button>
              </div>

              <div className="divide-y divide-border/60 rounded-xl border border-border bg-surface">
                {activeCoinWithdrawals.slice(0, 3).map((w) => (
                  <CompactWithdrawalRow
                    key={w.id}
                    withdrawal={w}
                    copiedTx={copiedTx}
                    onCopyTx={handleCopyTx}
                  />
                ))}
              </div>
            </div>
          )}
        </div>
      ) : (
        /* HISTÓRICO DE SAQUES TAB */
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
                Todas ({withdrawals.length})
              </button>
              {COINS.map((c) => {
                const count = withdrawals.filter((w) => w.coin.toUpperCase() === c).length;
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

          {/* WITHDRAWAL LIST */}
          {isLoadingHistory ? (
            <div className="card p-12 text-center space-y-3">
              <i className="bi bi-arrow-repeat animate-spin text-3xl text-bitcoin inline-block" />
              <div className="text-sm font-bold text-ink">Buscando saques na plataforma...</div>
              <div className="text-xs text-ink-muted">Sincronizando com a blockchain.</div>
            </div>
          ) : filteredWithdrawals.length === 0 ? (
            <div className="card p-12 text-center space-y-4">
              <div className="mx-auto flex h-14 w-14 items-center justify-center rounded-2xl bg-paper border border-border text-ink-muted">
                <i className="bi bi-inbox text-2xl text-bitcoin-dark" />
              </div>
              <div className="space-y-1">
                <h3 className="text-base font-bold text-ink">Nenhum saque encontrado</h3>
                <p className="text-xs text-ink-muted max-w-md mx-auto">
                  {historyCoinFilter === 'ALL'
                    ? 'Assim que você solicitar um saque de qualquer moeda, a transmissão on-chain e o hash serão monitorados aqui em tempo real.'
                    : `Nenhum saque registrado para ${historyCoinFilter}. Solicite um saque na aba anterior.`}
                </p>
              </div>
              <button
                type="button"
                onClick={() => switchTab('withdraw')}
                className="btn-primary text-xs mx-auto"
              >
                <i className="bi bi-box-arrow-up-right mr-1.5" /> Ir para Solicitar Saque
              </button>
            </div>
          ) : (
            <div className="divide-y divide-border/60 rounded-2xl border border-border bg-paper shadow-sm overflow-hidden">
              {filteredWithdrawals.map((w) => (
                <CompactWithdrawalRow
                  key={w.id}
                  withdrawal={w}
                  copiedTx={copiedTx}
                  onCopyTx={handleCopyTx}
                />
              ))}
            </div>
          )}
        </div>
      )}

      {/* MODAL: ALL COINS FEE SCHEDULE */}
      <Modal
        open={showAllFeesModal}
        onClose={() => setShowAllFeesModal(false)}
        panelClassName="max-w-2xl"
        backdropClassName="bg-black/50 backdrop-blur-xs"
        aria-label="Tabela Comparativa de Taxas On-Chain"
      >
            <div className="card w-full max-h-[85vh] overflow-y-auto p-6 space-y-5 bg-paper shadow-2xl">
              <div className="flex items-center justify-between border-b border-border/80 pb-3">
                <div className="flex items-center gap-2.5">
                  <div className="h-8 w-8 rounded-xl bg-emerald-600 text-white flex items-center justify-center font-bold text-sm">
                    %
                  </div>
                  <div>
                    <h3 className="text-base font-bold text-ink">Tabela Comparativa de Taxas On-Chain</h3>
                    <p className="text-xs text-ink-muted">SatsPay vs A Concorrência (-25% em todas as moedas)</p>
                  </div>
                </div>
                <button
                  type="button"
                  onClick={() => setShowAllFeesModal(false)}
                  className="rounded-lg p-2 text-ink-muted hover:bg-surface hover:text-ink text-sm"
                >
                  <i className="bi bi-x-lg" />
                </button>
              </div>

              <div className="overflow-x-auto">
                <table className="w-full text-xs text-left">
                  <thead>
                    <tr className="border-b border-border text-ink-muted">
                      <th className="py-2.5 pr-2 font-bold uppercase tracking-wider">Moeda</th>
                      <th className="py-2.5 px-2 font-bold uppercase tracking-wider text-emerald-700">Taxa SatsPay</th>
                      <th className="py-2.5 px-2 font-bold uppercase tracking-wider line-through text-ink-muted">A Concorrência</th>
                      <th className="py-2.5 pl-2 font-bold uppercase tracking-wider">Saque Mínimo</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-border/60">
                    {COINS.map((c) => {
                      const conf = COIN_CONFIG[c];
                      const isCurrent = c === coin;
                      return (
                        <tr
                          key={c}
                          onClick={() => {
                            handleSelectCoin(c);
                            setShowAllFeesModal(false);
                          }}
                          className={`cursor-pointer transition-colors ${
                            isCurrent ? 'bg-bitcoin/10 font-bold' : 'hover:bg-surface'
                          }`}
                        >
                          <td className="py-2.5 pr-2 flex items-center gap-2">
                            <img src={coinLogo(c)} alt={c} className="h-5 w-5 rounded-full" />
                            <span className="font-bold text-ink">{conf.name}</span>
                            <span className="text-[10px] text-ink-muted font-mono uppercase">({c})</span>
                          </td>
                          <td className="py-2.5 px-2 font-mono font-bold text-emerald-700">
                            {formatAmount(conf.withdrawalFee, c)} {c}
                          </td>
                          <td className="py-2.5 px-2 font-mono text-ink-muted line-through">
                            {formatAmount(conf.faucetPayFee, c)} {c}
                          </td>
                          <td className="py-2.5 pl-2 font-mono text-ink-muted">
                            {conf.minWithdrawal <= 1n ? 'Sem mínimo' : `${formatAmount(conf.minWithdrawal, c)} ${c}`}
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>

              <div className="flex justify-end pt-2 border-t border-border/80">
                <button
                  type="button"
                  onClick={() => setShowAllFeesModal(false)}
                  className="btn-primary text-xs px-5 py-2.5 font-bold"
                >
                  Fechar
                </button>
              </div>
            </div>
      </Modal>
    </div>
  );
}

function CompactWithdrawalRow({
  withdrawal,
  copiedTx,
  onCopyTx,
}: {
  withdrawal: WithdrawalHistoryItem;
  copiedTx: string | null;
  onCopyTx: (tx: string) => void;
}) {
  const rawCoin = withdrawal.coin.toUpperCase();
  const coinSymbol = isCoin(rawCoin) ? rawCoin : undefined;
  const cfg = coinSymbol ? COIN_CONFIG[coinSymbol] : undefined;
  const explorerUrl = withdrawal.txHash ? getExplorerTxUrl(withdrawal.coin, withdrawal.txHash) : null;

  const isConfirmed = withdrawal.status === 'CONFIRMED';
  const isBroadcasted = withdrawal.status === 'BROADCASTED';
  const isBroadcasting = withdrawal.status === 'BROADCASTING';
  const isQueued = withdrawal.status === 'QUEUED';
  const isPendingApproval = withdrawal.status === 'PENDING';
  const isFailed = withdrawal.status === 'FAILED';
  const isRejected = withdrawal.status === 'REJECTED';
  const isCancelled = withdrawal.status === 'CANCELLED';

  const formattedAmountStr = coinSymbol && cfg
    ? formatAmount(withdrawal.amount, coinSymbol)
    : withdrawal.amount;

  const formattedFeeStr = coinSymbol && cfg
    ? formatAmount(withdrawal.feeAmount, coinSymbol)
    : withdrawal.feeAmount;

  const dateStr = new Date(withdrawal.createdAt).toLocaleString('pt-BR', {
    day: '2-digit',
    month: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });

  const shortAddr = withdrawal.toAddress.length > 18
    ? `${withdrawal.toAddress.slice(0, 8)}...${withdrawal.toAddress.slice(-6)}`
    : withdrawal.toAddress;

  const shortTx = withdrawal.txHash
    ? withdrawal.txHash.length > 16
      ? `${withdrawal.txHash.slice(0, 8)}...${withdrawal.txHash.slice(-6)}`
      : withdrawal.txHash
    : null;

  return (
    <div className="p-3.5 sm:px-5 sm:py-3.5 hover:bg-surface/80 transition-colors flex flex-col gap-2.5 sm:flex-row sm:items-center sm:justify-between">
      {/* LEFT: COIN ICON, AMOUNT, DATE */}
      <div className="flex items-center gap-3 min-w-0 sm:w-1/3">
        {coinSymbol ? (
          <img
            src={coinLogo(coinSymbol)}
            alt={withdrawal.coin}
            className="h-8 w-8 rounded-full shrink-0 shadow-xs"
          />
        ) : (
          <i className="bi bi-coin h-8 w-8 text-ink-muted" />
        )}
        <div className="min-w-0">
          <div className="flex items-center gap-1.5 font-bold text-sm text-ink truncate">
            <span className="text-rose-600 dark:text-rose-400">-{formattedAmountStr}</span>
            <span className="text-xs text-ink-muted">{withdrawal.coin}</span>
          </div>
          <div className="text-[11px] text-ink-muted truncate">
            {dateStr} • Taxa: {formattedFeeStr} {withdrawal.coin}
          </div>
        </div>
      </div>

      {/* MIDDLE: DESTINATION ADDRESS */}
      <div className="flex-1 max-w-xs space-y-0.5">
        <div className="text-[10px] uppercase font-bold text-ink-muted tracking-wider">
          Destino:
        </div>
        <div
          className="font-mono text-xs text-ink truncate bg-surface px-2 py-1 rounded-md border border-border/80 inline-block max-w-full"
          title={withdrawal.toAddress}
        >
          {shortAddr}
        </div>
      </div>

      {/* RIGHT: STATUS & TX ACTIONS */}
      <div className="flex items-center justify-between sm:justify-end gap-2.5 shrink-0 pt-1 sm:pt-0 border-t border-border/40 sm:border-0">
        {/* STATUS PILL */}
        {isConfirmed ? (
          <span className="inline-flex items-center gap-1 rounded-md bg-emerald-500/10 px-2 py-0.5 text-[11px] font-semibold text-emerald-700 dark:text-emerald-400 border border-emerald-500/20">
            <i className="bi bi-shield-check text-[10px] text-emerald-500" />
            <span>Confirmado</span>
          </span>
        ) : isBroadcasted ? (
          <span className="inline-flex items-center gap-1 rounded-md bg-teal-500/10 px-2 py-0.5 text-[11px] font-semibold text-teal-700 dark:text-teal-400 border border-teal-500/20">
            <i className="bi bi-send-check text-[10px] text-teal-500" />
            <span>Enviado On-Chain</span>
          </span>
        ) : isBroadcasting ? (
          <span className="inline-flex items-center gap-1 rounded-md bg-indigo-500/10 px-2 py-0.5 text-[11px] font-semibold text-indigo-700 dark:text-indigo-400 border border-indigo-500/20 animate-pulse">
            <i className="bi bi-arrow-repeat animate-spin text-[10px] text-indigo-500" />
            <span>Transmitindo...</span>
          </span>
        ) : isQueued ? (
          <span className="inline-flex items-center gap-1 rounded-md bg-sky-500/10 px-2 py-0.5 text-[11px] font-semibold text-sky-700 dark:text-sky-400 border border-sky-500/20">
            <i className="bi bi-hourglass-split text-[10px] text-sky-500" />
            <span>Fila de Envio</span>
          </span>
        ) : isPendingApproval ? (
          <span className="inline-flex items-center gap-1 rounded-md bg-amber-500/10 px-2 py-0.5 text-[11px] font-semibold text-amber-700 dark:text-amber-400 border border-amber-500/20 animate-pulse">
            <i className="bi bi-clock-history text-[10px] text-amber-500" />
            <span>Aguardando Aprovação</span>
          </span>
        ) : isFailed ? (
          <span className="inline-flex items-center gap-1 rounded-md bg-rose-500/10 px-2 py-0.5 text-[11px] font-semibold text-rose-700 dark:text-rose-400 border border-rose-500/20">
            <i className="bi bi-arrow-counterclockwise text-[10px] text-rose-500" />
            <span>Falha (Reembolsado)</span>
          </span>
        ) : isRejected ? (
          <span className="inline-flex items-center gap-1 rounded-md bg-rose-500/10 px-2 py-0.5 text-[11px] font-semibold text-rose-700 dark:text-rose-400 border border-rose-500/20">
            <i className="bi bi-x-circle-fill text-[10px] text-rose-500" />
            <span>Rejeitado</span>
          </span>
        ) : isCancelled ? (
          <span className="inline-flex items-center gap-1 rounded-md bg-slate-500/10 px-2 py-0.5 text-[11px] font-semibold text-slate-700 dark:text-slate-400 border border-slate-500/20">
            <i className="bi bi-slash-circle text-[10px] text-slate-500" />
            <span>Cancelado</span>
          </span>
        ) : (
          <span className="inline-flex items-center gap-1 rounded-md bg-amber-500/10 px-2 py-0.5 text-[11px] font-semibold text-amber-700 dark:text-amber-400 border border-amber-500/20">
            <span>{withdrawal.status}</span>
          </span>
        )}

        {/* HASH BUTTON & EXPLORER LINK */}
        {withdrawal.txHash ? (
          <div className="flex items-center gap-1.5 font-mono text-[11px]">
            <button
              type="button"
              onClick={() => onCopyTx(withdrawal.txHash!)}
              className="inline-flex items-center gap-1 rounded-lg border border-border bg-paper hover:bg-surface px-2 py-0.5 text-[11px] text-ink transition-colors"
              title={withdrawal.txHash}
            >
              <span className="hidden md:inline">{shortTx}</span>
              <i className={`bi ${copiedTx === withdrawal.txHash ? 'bi-check2 text-emerald-600' : 'bi-clipboard'}`} />
            </button>

            {explorerUrl && (
              <a
                href={explorerUrl}
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex items-center justify-center h-6 w-6 rounded-lg bg-paper hover:bg-bitcoin/10 text-ink-muted hover:text-bitcoin border border-border transition-colors"
                title="Abrir no Explorer"
              >
                <i className="bi bi-box-arrow-up-right text-[10px]" />
              </a>
            )}
          </div>
        ) : withdrawal.status === 'FAILED' ? (
          <span className="text-[11px] text-emerald-600 font-mono inline-flex items-center gap-1">
            <i className="bi bi-arrow-counterclockwise" /> Estornado
          </span>
        ) : withdrawal.status === 'REJECTED' || withdrawal.status === 'CANCELLED' ? (
          <span className="text-[11px] text-ink-muted font-mono">Cancelado</span>
        ) : (
          <span className="text-[11px] text-ink-muted italic font-mono">Aguardando TXID...</span>
        )}
      </div>
    </div>
  );
}


