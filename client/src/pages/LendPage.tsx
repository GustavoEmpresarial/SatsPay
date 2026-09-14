import { useMemo, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '../lib/api.js';
import { useAuthStore } from '../stores/auth.js';
import { formatApiError } from '../lib/formatError.js';
import { coinLogo } from '../lib/coinAssets.js';
import { COIN_CONFIG, formatAmount, safeBigInt, type Coin, type WalletBalance } from '@/shared';
import { clsx } from 'clsx';
import { Modal } from '../components/Modal.js';

type LendAction = 'supply' | 'withdraw' | 'borrow' | 'repay';

// Strictly official Aave V3 Polygon assets
const AAVE_V3_COINS: Coin[] = ['BTC', 'USDT', 'USDC', 'POL'];

interface Market {
  coin: Coin;
  total_supply?: string | number;
  totalSupply?: string | number;
  total_debt?: string | number;
  totalDebt?: string | number;
  available?: string | number;
  utilization_bps?: number;
  utilizationBps?: number;
  supply_apy_bps?: number;
  supplyApyBps?: number;
  borrow_apy_bps?: number;
  borrowApyBps?: number;
  collateral_factor_bps?: number;
  collateralFactorBps?: number;
  liquidation_threshold_bps?: number;
  liquidationThresholdBps?: number;
  borrow_enabled?: boolean;
  borrowEnabled?: boolean;
  can_be_collateral?: boolean;
  canBeCollateral?: boolean;
}

interface PositionRow {
  coin: Coin;
  supplied: string;
  debt: string;
  useAsCollateral: boolean;
}

interface PositionsResp {
  positions: PositionRow[];
  debtUsd: string;
  borrowPowerUsd: string;
  availableBorrowUsd: string;
  liquidationCollateralUsd: string;
  healthFactorBps: number | null;
}

// Live baseline stats from Aave Polygon Market V3
const AAVE_V3_MARKET_DATA: Record<
  Coin,
  {
    displayName: string;
    symbol: string;
    supplyApy: number;
    borrowApy: number;
    totalSuppliedUsd: string;
    totalBorrowedUsd: string;
    ltv: number;
  }
> = {
  BTC: {
    displayName: 'Wrapped BTC',
    symbol: 'WBTC',
    supplyApy: 0.01,
    borrowApy: 0.71,
    totalSuppliedUsd: '$66.12M',
    totalBorrowedUsd: '$2.79M',
    ltv: 75,
  },
  USDT: {
    displayName: 'Tether USD',
    symbol: 'USDT',
    supplyApy: 3.37,
    borrowApy: 6.87,
    totalSuppliedUsd: '$32.35M',
    totalBorrowedUsd: '$21.49M',
    ltv: 80,
  },
  USDC: {
    displayName: 'USD Coin',
    symbol: 'USDC',
    supplyApy: 2.92,
    borrowApy: 6.19,
    totalSuppliedUsd: '$29.71M',
    totalBorrowedUsd: '$17.83M',
    ltv: 85,
  },
  POL: {
    displayName: 'Polygon',
    symbol: 'POL',
    supplyApy: 0.05,
    borrowApy: 0.63,
    totalSuppliedUsd: '$9.01M',
    totalBorrowedUsd: '$849.91K',
    ltv: 70,
  },
  LTC: { displayName: 'Litecoin', symbol: 'LTC', supplyApy: 0.0, borrowApy: 0.0, totalSuppliedUsd: '$0', totalBorrowedUsd: '$0', ltv: 70 },
  DOGE: { displayName: 'Dogecoin', symbol: 'DOGE', supplyApy: 0.0, borrowApy: 0.0, totalSuppliedUsd: '$0', totalBorrowedUsd: '$0', ltv: 50 },
  BCH: { displayName: 'Bitcoin Cash', symbol: 'BCH', supplyApy: 0.0, borrowApy: 0.0, totalSuppliedUsd: '$0', totalBorrowedUsd: '$0', ltv: 70 },
  DGB: { displayName: 'DigiByte', symbol: 'DGB', supplyApy: 0.0, borrowApy: 0.0, totalSuppliedUsd: '$0', totalBorrowedUsd: '$0', ltv: 50 },
  SOL: { displayName: 'Solana', symbol: 'SOL', supplyApy: 0.0, borrowApy: 0.0, totalSuppliedUsd: '$0', totalBorrowedUsd: '$0', ltv: 75 },
};

const getSupplyApy = (m?: Market, coin?: Coin): number => {
  if (m) {
    const bps = m.supply_apy_bps ?? m.supplyApyBps;
    if (typeof bps === 'number' && bps > 0) return bps / 100;
  }
  return coin ? AAVE_V3_MARKET_DATA[coin]?.supplyApy ?? 2.92 : 2.92;
};

const getBorrowApy = (m?: Market, coin?: Coin): number => {
  if (m) {
    const bps = m.borrow_apy_bps ?? m.borrowApyBps;
    if (typeof bps === 'number' && bps > 0) return bps / 100;
  }
  return coin ? AAVE_V3_MARKET_DATA[coin]?.borrowApy ?? 6.19 : 6.19;
};

const getLtv = (m?: Market, coin?: Coin): number => {
  if (m) {
    const bps = m.collateral_factor_bps ?? m.collateralFactorBps;
    if (typeof bps === 'number' && bps > 0) return bps / 100;
  }
  return coin ? AAVE_V3_MARKET_DATA[coin]?.ltv ?? 75 : 75;
};

const formatUsd = (scaled: string | number | undefined | null) => {
  if (!scaled) return '$0.00';
  try {
    const raw = safeBigInt(scaled);
    return `$${(Number(raw) / 1e8).toLocaleString(undefined, {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    })}`;
  } catch {
    return '$0.00';
  }
};

export function LendPage() {
  const qc = useQueryClient();

  // Modal State
  const [modalOpen, setModalOpen] = useState(false);
  const [modalCoin, setModalCoin] = useState<Coin>('USDT');
  const [modalAction, setModalAction] = useState<LendAction>('supply');
  const [inputVal, setInputVal] = useState('');
  const [msg, setMsg] = useState<{ type: 'ok' | 'err'; text: string } | null>(null);

  const user = useAuthStore((s) => s.user);

  const marketsQ = useQuery({
    queryKey: ['lend', 'markets'],
    queryFn: () => api<{ markets: Market[] }>('/lend/markets', { skipAuth: true }),
    refetchInterval: 30_000,
  });

  const positionsQ = useQuery({
    queryKey: ['lend', 'positions'],
    queryFn: () => api<PositionsResp>('/lend/positions'),
    enabled: Boolean(user),
    refetchInterval: 30_000,
  });

  const walletsQ = useQuery({
    queryKey: ['wallets', 'PERSONAL'],
    queryFn: () => api<{ wallets: WalletBalance[] }>('/wallet?kind=PERSONAL'),
    enabled: Boolean(user),
  });

  const walletsMap = useMemo(() => {
    const m: Partial<Record<Coin, WalletBalance>> = {};
    const list = Array.isArray(walletsQ.data) ? walletsQ.data : walletsQ.data?.wallets ?? [];
    list.forEach((w) => (m[w.coin as Coin] = w));
    return m;
  }, [walletsQ.data]);

  const cfg = COIN_CONFIG[modalCoin] || COIN_CONFIG.BTC;

  // Convert human float to BigInt base units
  const smallestAmount = useMemo(() => {
    if (!inputVal || isNaN(Number(inputVal)) || Number(inputVal) <= 0) return 0n;
    try {
      const parts = inputVal.split('.');
      const whole = safeBigInt(parts[0] || '0') * (10n ** BigInt(cfg.decimals));
      if (parts.length > 1) {
        const fracStr = (parts[1] ?? '').slice(0, cfg.decimals).padEnd(cfg.decimals, '0');
        return whole + safeBigInt(fracStr);
      }
      return whole;
    } catch {
      return 0n;
    }
  }, [inputVal, cfg.decimals]);

  // Position for modal coin
  const currentPos = useMemo(() => {
    return positionsQ.data?.positions.find((p) => p.coin === modalCoin);
  }, [positionsQ.data, modalCoin]);

  // Max available for action in modal
  const maxAvailable = useMemo(() => {
    if (modalAction === 'supply') {
      return safeBigInt(walletsMap[modalCoin]?.balance);
    }
    if (modalAction === 'withdraw') {
      return safeBigInt(currentPos?.supplied);
    }
    if (modalAction === 'repay') {
      const debt = safeBigInt(currentPos?.debt);
      const bal = safeBigInt(walletsMap[modalCoin]?.balance);
      return debt < bal ? debt : bal;
    }
    return 0n;
  }, [modalAction, modalCoin, walletsMap, currentPos]);

  const mutation = useMutation({
    mutationFn: (vars: { action: LendAction; coin: Coin; amount: string }) =>
      api('/lend/action', {
        method: 'POST',
        json: {
          action: vars.action.toUpperCase(),
          coin: vars.coin,
          amount: vars.amount,
        },
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['lend', 'positions'] });
      qc.invalidateQueries({ queryKey: ['wallets', 'PERSONAL'] });
      qc.invalidateQueries({ queryKey: ['wallets'] });
      setMsg({ type: 'ok', text: 'Operação executada com sucesso no protocolo!' });
      setTimeout(() => {
        setModalOpen(false);
        setMsg(null);
      }, 1500);
    },
    onError: (err) => setMsg({ type: 'err', text: formatApiError(err) }),
  });

  const hf = positionsQ.data?.healthFactorBps;
  const hfLabel = hf == null ? '∞' : (hf / 10_000).toFixed(2);
  const hfSafe = hf == null || hf >= 15_000;
  const hfWarning = hf != null && hf < 15_000 && hf >= 11_000;

  // Calculate borrow power used %
  const borrowPowerRaw = safeBigInt(positionsQ.data?.borrowPowerUsd);
  const debtRaw = safeBigInt(positionsQ.data?.debtUsd);
  const borrowPowerUsedPct =
    borrowPowerRaw > 0n ? Math.min(100, Number((debtRaw * 10000n) / borrowPowerRaw) / 100) : 0;

  const openActionModal = (c: Coin, act: LendAction) => {
    setModalCoin(c);
    setModalAction(act);
    setInputVal('');
    setMsg(null);
    setModalOpen(true);
  };

  const setPercent = (pct: number) => {
    if (maxAvailable <= 0n) return;
    const target = (maxAvailable * BigInt(pct)) / 100n;
    setInputVal(formatAmount(target, modalCoin));
  };

  const submit = () => {
    setMsg(null);
    if (smallestAmount <= 0n) {
      setMsg({ type: 'err', text: 'Informe um valor válido maior que zero.' });
      return;
    }
    mutation.mutate({ action: modalAction, coin: modalCoin, amount: smallestAmount.toString() });
  };

  const markets = marketsQ.data?.markets ?? [];
  const userPositions = (positionsQ.data?.positions ?? []).filter((p) =>
    AAVE_V3_COINS.includes(p.coin),
  );
  const suppliedPositions = userPositions.filter((p) => safeBigInt(p.supplied) > 0n);
  const borrowedPositions = userPositions.filter((p) => safeBigInt(p.debt) > 0n);

  return (
    <div className="space-y-6 max-w-6xl mx-auto pb-12">
      {/* AAVE V3 HEADER & PROTOCOL STATS BAR */}
      <div className="flex flex-col md:flex-row md:items-center md:justify-between gap-4 border-b border-border/80 pb-4">
        <div>
          <div className="flex items-center gap-2">
            <span className="flex h-2.5 w-2.5 rounded-full bg-emerald-500 animate-pulse" />
            <span className="text-xs font-bold uppercase tracking-widest text-emerald-600">
              AAVE · Polygon Market V3
            </span>
          </div>
          <h1 className="text-2xl sm:text-3xl font-black tracking-tight text-ink mt-0.5">
            Mercado de Liquidez Aave V3
          </h1>
          <p className="text-xs text-ink-muted mt-1">
            Deposite ativos para render juros automaticamente ou tome empréstimos instantâneos sem pagar taxas de gás.
          </p>
        </div>

        {/* Global Aave Polygon Market V3 Stats */}
        <div className="flex items-center gap-2 flex-wrap">
          <div className="rounded-xl border border-border bg-paper px-3 py-1.5 shadow-xs">
            <div className="text-[10px] uppercase font-bold text-ink-muted">Tamanho Total</div>
            <div className="text-xs font-black text-ink">$190.05M</div>
          </div>
          <div className="rounded-xl border border-border bg-paper px-3 py-1.5 shadow-xs">
            <div className="text-[10px] uppercase font-bold text-ink-muted">Disponível</div>
            <div className="text-xs font-black text-emerald-600">$132.59M</div>
          </div>
          <div className="rounded-xl border border-border bg-paper px-3 py-1.5 shadow-xs">
            <div className="text-[10px] uppercase font-bold text-ink-muted">Empréstimos</div>
            <div className="text-xs font-black text-amber-600">$54.93M</div>
          </div>
        </div>
      </div>

      {/* USER ACCOUNT METRICS DASHBOARD */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-2.5 sm:gap-4">
        {/* Total Debt */}
        <div className="rounded-2xl border border-border bg-paper p-3 sm:p-4 space-y-0.5 sm:space-y-1 shadow-xs">
          <div className="text-[10px] sm:text-xs font-bold uppercase tracking-wider text-ink-muted truncate">
            Dívida Total
          </div>
          <div className="text-lg sm:text-2xl font-black text-ink">
            {formatUsd(positionsQ.data?.debtUsd)}
          </div>
          <div className="text-[9px] sm:text-[10px] text-ink-muted truncate">Soma de empréstimos</div>
        </div>

        {/* Borrow Power */}
        <div className="rounded-2xl border border-border bg-paper p-3 sm:p-4 space-y-0.5 sm:space-y-1 shadow-xs">
          <div className="text-[10px] sm:text-xs font-bold uppercase tracking-wider text-ink-muted truncate">
            Poder de Empréstimo
          </div>
          <div className="text-lg sm:text-2xl font-black text-bitcoin-dark">
            {formatUsd(positionsQ.data?.borrowPowerUsd)}
          </div>
          <div className="text-[9px] sm:text-[10px] text-ink-muted truncate">Disp: {formatUsd(positionsQ.data?.availableBorrowUsd)}</div>
        </div>

        {/* Borrow Power Used % */}
        <div className="rounded-2xl border border-border bg-paper p-3 sm:p-4 space-y-0.5 sm:space-y-1 shadow-xs">
          <div className="flex justify-between items-center text-[10px] sm:text-xs font-bold uppercase tracking-wider text-ink-muted">
            <span className="truncate">Uso Limite</span>
            <span className="font-mono text-ink text-[11px] sm:text-xs">{borrowPowerUsedPct.toFixed(1)}%</span>
          </div>
          <div className="text-lg sm:text-2xl font-black text-ink">{borrowPowerUsedPct.toFixed(1)}%</div>
          <div className="h-1.5 w-full bg-surface rounded-full overflow-hidden border border-border mt-1">
            <div
              className={clsx(
                'h-full rounded-full transition-all duration-300',
                borrowPowerUsedPct > 80
                  ? 'bg-rose-500'
                  : borrowPowerUsedPct > 50
                  ? 'bg-amber-500'
                  : 'bg-emerald-500',
              )}
              style={{ width: `${Math.min(100, Math.max(0, borrowPowerUsedPct))}%` }}
            />
          </div>
        </div>

        {/* Health Factor */}
        <div className="rounded-2xl border border-border bg-paper p-3 sm:p-4 space-y-0.5 sm:space-y-1 shadow-xs">
          <div className="flex items-center justify-between text-[10px] sm:text-xs font-bold uppercase tracking-wider text-ink-muted">
            <span className="truncate">Fator Saúde</span>
            <span
              className={clsx(
                'rounded-full px-1.5 sm:px-2 py-0.5 text-[8px] sm:text-[9px] font-bold',
                hfSafe
                  ? 'bg-emerald-500/15 text-emerald-700'
                  : hfWarning
                  ? 'bg-amber-500/15 text-amber-700'
                  : 'bg-rose-500/15 text-rose-700',
              )}
            >
              {hfSafe ? 'Seguro' : hfWarning ? 'Moderado' : 'Risco'}
            </span>
          </div>
          <div
            className={clsx(
              'text-lg sm:text-2xl font-black font-mono',
              hfSafe ? 'text-emerald-600' : hfWarning ? 'text-amber-600' : 'text-rose-600',
            )}
          >
            {hfLabel}
          </div>
          <div className="text-[9px] sm:text-[10px] text-ink-muted truncate">Liquidação HF &lt; 1.0</div>
        </div>
      </div>

      {/* AAVE V3 2-COLUMN SPLIT (SUPPLY vs BORROW) */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 items-start">
        {/* ================= LEFT COLUMN: SUPPLY ================= */}
        <div className="space-y-6">
          {/* 1. YOUR SUPPLIES */}
          <div className="rounded-2xl border border-border bg-paper overflow-hidden shadow-xs">
            <div className="p-4 border-b border-border flex items-center justify-between bg-surface/50">
              <h2 className="text-sm font-bold text-ink flex items-center gap-2">
                <i className="bi bi-box-arrow-in-down text-emerald-600" />
                <span>Seus Fornecimentos (Your Supplies)</span>
              </h2>
              <span className="text-[11px] font-bold text-emerald-700">
                {suppliedPositions.length} ativos
              </span>
            </div>

            {suppliedPositions.length === 0 ? (
              <div className="p-8 text-center text-ink-muted space-y-1">
                <div className="text-xs font-semibold text-ink">Nenhum ativo fornecido ainda</div>
                <div className="text-[11px]">Forneça liquidez na lista abaixo para render juros.</div>
              </div>
            ) : (
              <div className="overflow-x-auto">
                <table className="w-full text-left text-xs">
                  <thead className="bg-surface text-[10px] uppercase font-bold text-ink-muted border-b border-border">
                    <tr>
                      <th className="p-3">Ativo</th>
                      <th className="p-3">Saldo Fornecido</th>
                      <th className="p-3">APY</th>
                      <th className="p-3">Colateral</th>
                      <th className="p-3 text-right">Ações</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-border font-medium">
                    {suppliedPositions.map((p) => {
                      const m = markets.find((item) => item.coin === p.coin);
                      const apy = getSupplyApy(m, p.coin);
                      const meta = AAVE_V3_MARKET_DATA[p.coin];
                      return (
                        <tr key={p.coin} className="hover:bg-surface/50">
                          <td className="p-3 font-bold text-ink">
                            <div className="flex items-center gap-2">
                              <img
                                src={coinLogo(p.coin)}
                                alt={p.coin}
                                className="h-5 w-5 rounded-full object-contain"
                              />
                              <span>{meta?.symbol || p.coin}</span>
                            </div>
                          </td>
                          <td className="p-3 font-mono font-bold text-emerald-600">
                            {formatAmount(safeBigInt(p.supplied), p.coin)}
                          </td>
                          <td className="p-3 font-mono text-emerald-700 font-bold">
                            {apy.toFixed(2)}%
                          </td>
                          <td className="p-3">
                            <span className="inline-flex items-center gap-1 rounded-full bg-emerald-500/15 px-2 py-0.5 text-[9px] font-bold text-emerald-700">
                              ✓ Sim
                            </span>
                          </td>
                          <td className="p-3 text-right space-x-1">
                            <button
                              type="button"
                              onClick={() => openActionModal(p.coin, 'supply')}
                              className="rounded-lg bg-emerald-50 text-emerald-700 border border-emerald-200 px-2 py-1 text-[10px] font-bold hover:bg-emerald-100"
                            >
                              + Fornecer
                            </button>
                            <button
                              type="button"
                              onClick={() => openActionModal(p.coin, 'withdraw')}
                              className="rounded-lg bg-surface border border-border px-2 py-1 text-[10px] font-bold text-ink hover:bg-paper"
                            >
                              Resgatar
                            </button>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            )}
          </div>

          {/* 2. ASSETS TO SUPPLY */}
          <div className="rounded-2xl border border-border bg-paper overflow-hidden shadow-xs">
            <div className="p-4 border-b border-border flex items-center justify-between bg-surface/50">
              <h2 className="text-sm font-bold text-ink flex items-center gap-2">
                <i className="bi bi-wallet2 text-bitcoin" />
                <span>Ativos para Fornecer (Assets to Supply)</span>
              </h2>
              <span className="text-[11px] text-ink-muted">Renda juros automáticos</span>
            </div>

            <div className="overflow-x-auto">
              <table className="w-full text-left text-xs">
                <thead className="bg-surface text-[10px] uppercase font-bold text-ink-muted border-b border-border">
                  <tr>
                    <th className="p-3">Ativo</th>
                    <th className="p-3">Saldo Carteira</th>
                    <th className="p-3">APY Fornecimento</th>
                    <th className="p-3 text-right">Ação</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-border font-medium">
                  {AAVE_V3_COINS.map((c) => {
                    const m = markets.find((item) => item.coin === c);
                    const apy = getSupplyApy(m, c);
                    const meta = AAVE_V3_MARKET_DATA[c];
                    const bal = safeBigInt(walletsMap[c]?.balance);

                    return (
                      <tr key={c} className="hover:bg-surface/50 transition-colors">
                        <td className="p-3">
                          <div className="flex items-center gap-2.5">
                            <img
                              src={coinLogo(c)}
                              alt={c}
                              className="h-6 w-6 rounded-full object-contain shrink-0"
                            />
                            <div>
                              <div className="font-bold text-ink flex items-center gap-1.5">
                                <span>{meta?.displayName}</span>
                                <span className="rounded bg-surface border border-border px-1.5 py-0.2 text-[9px] font-bold text-ink-muted font-mono">
                                  {meta?.symbol}
                                </span>
                              </div>
                              <div className="text-[10px] text-ink-muted">
                                Total Fornecido: {meta?.totalSuppliedUsd}
                              </div>
                            </div>
                          </div>
                        </td>
                        <td className="p-3 font-mono font-bold text-ink">
                          {formatAmount(bal, c)} <span className="text-[10px] text-ink-muted">{meta?.symbol}</span>
                        </td>
                        <td className="p-3 font-mono font-bold text-emerald-600">
                          <span className="bg-emerald-500/10 px-2 py-0.5 rounded-md">
                            +{apy.toFixed(2)}%
                          </span>
                        </td>
                        <td className="p-3 text-right">
                          <button
                            type="button"
                            onClick={() => openActionModal(c, 'supply')}
                            className="rounded-xl bg-emerald-600 text-white font-bold px-3.5 py-1.5 text-xs shadow-xs hover:bg-emerald-700 active:scale-95 transition-all"
                          >
                            Fornecer
                          </button>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </div>
        </div>

        {/* ================= RIGHT COLUMN: BORROW ================= */}
        <div className="space-y-6">
          {/* 1. YOUR BORROWS */}
          <div className="rounded-2xl border border-border bg-paper overflow-hidden shadow-xs">
            <div className="p-4 border-b border-border flex items-center justify-between bg-surface/50">
              <h2 className="text-sm font-bold text-ink flex items-center gap-2">
                <i className="bi bi-cash-coin text-amber-600" />
                <span>Seus Empréstimos (Your Borrows)</span>
              </h2>
              <span className="text-[11px] font-bold text-amber-700">
                {borrowedPositions.length} dívidas
              </span>
            </div>

            {borrowedPositions.length === 0 ? (
              <div className="p-8 text-center text-ink-muted space-y-1">
                <div className="text-xs font-semibold text-ink">Nenhuma dívida ativa</div>
                <div className="text-[11px]">Tome empréstimo contra seu colateral na lista abaixo.</div>
              </div>
            ) : (
              <div className="overflow-x-auto">
                <table className="w-full text-left text-xs">
                  <thead className="bg-surface text-[10px] uppercase font-bold text-ink-muted border-b border-border">
                    <tr>
                      <th className="p-3">Ativo</th>
                      <th className="p-3">Dívida Atual</th>
                      <th className="p-3">APY</th>
                      <th className="p-3">Tipo</th>
                      <th className="p-3 text-right">Ações</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-border font-medium">
                    {borrowedPositions.map((p) => {
                      const m = markets.find((item) => item.coin === p.coin);
                      const apy = getBorrowApy(m, p.coin);
                      const meta = AAVE_V3_MARKET_DATA[p.coin];
                      return (
                        <tr key={p.coin} className="hover:bg-surface/50">
                          <td className="p-3 font-bold text-ink">
                            <div className="flex items-center gap-2">
                              <img
                                src={coinLogo(p.coin)}
                                alt={p.coin}
                                className="h-5 w-5 rounded-full object-contain"
                              />
                              <span>{meta?.symbol || p.coin}</span>
                            </div>
                          </td>
                          <td className="p-3 font-mono font-bold text-amber-600">
                            {formatAmount(safeBigInt(p.debt), p.coin)}
                          </td>
                          <td className="p-3 font-mono text-amber-700 font-bold">
                            {apy.toFixed(2)}%
                          </td>
                          <td className="p-3 text-ink-muted text-[10px]">Variável</td>
                          <td className="p-3 text-right space-x-1">
                            <button
                              type="button"
                              onClick={() => openActionModal(p.coin, 'borrow')}
                              className="rounded-lg bg-blue-50 text-blue-700 border border-blue-200 px-2 py-1 text-[10px] font-bold hover:bg-blue-100"
                            >
                              + Tomar
                            </button>
                            <button
                              type="button"
                              onClick={() => openActionModal(p.coin, 'repay')}
                              className="rounded-lg bg-indigo-600 text-white px-2 py-1 text-[10px] font-bold hover:bg-indigo-700"
                            >
                              Pagar
                            </button>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            )}
          </div>

          {/* 2. ASSETS TO BORROW */}
          <div className="rounded-2xl border border-border bg-paper overflow-hidden shadow-xs">
            <div className="p-4 border-b border-border flex items-center justify-between bg-surface/50">
              <h2 className="text-sm font-bold text-ink flex items-center gap-2">
                <i className="bi bi-bank text-blue-600" />
                <span>Ativos para Tomar Emprestado (Assets to Borrow)</span>
              </h2>
              <span className="text-[11px] text-ink-muted">Liquidez imediata</span>
            </div>

            <div className="overflow-x-auto">
              <table className="w-full text-left text-xs">
                <thead className="bg-surface text-[10px] uppercase font-bold text-ink-muted border-b border-border">
                  <tr>
                    <th className="p-3">Ativo</th>
                    <th className="p-3">LTV Máx</th>
                    <th className="p-3">APY Empréstimo</th>
                    <th className="p-3 text-right">Ação</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-border font-medium">
                  {AAVE_V3_COINS.map((c) => {
                    const m = markets.find((item) => item.coin === c);
                    const apy = getBorrowApy(m, c);
                    const ltv = getLtv(m, c);
                    const meta = AAVE_V3_MARKET_DATA[c];

                    return (
                      <tr key={c} className="hover:bg-surface/50 transition-colors">
                        <td className="p-3">
                          <div className="flex items-center gap-2.5">
                            <img
                              src={coinLogo(c)}
                              alt={c}
                              className="h-6 w-6 rounded-full object-contain shrink-0"
                            />
                            <div>
                              <div className="font-bold text-ink flex items-center gap-1.5">
                                <span>{meta?.displayName}</span>
                                <span className="rounded bg-surface border border-border px-1.5 py-0.2 text-[9px] font-bold text-ink-muted font-mono">
                                  {meta?.symbol}
                                </span>
                              </div>
                              <div className="text-[10px] text-ink-muted">
                                Total Tomado: {meta?.totalBorrowedUsd}
                              </div>
                            </div>
                          </div>
                        </td>
                        <td className="p-3 font-mono font-semibold text-ink">
                          {ltv.toFixed(0)}%
                        </td>
                        <td className="p-3 font-mono font-bold text-amber-700">
                          <span className="bg-amber-500/10 px-2 py-0.5 rounded-md">
                            {apy.toFixed(2)}%
                          </span>
                        </td>
                        <td className="p-3 text-right">
                          <button
                            type="button"
                            onClick={() => openActionModal(c, 'borrow')}
                            className="rounded-xl bg-blue-600 text-white font-bold px-3.5 py-1.5 text-xs shadow-xs hover:bg-blue-700 active:scale-95 transition-all"
                          >
                            Tomar
                          </button>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      </div>

      {/* AAVE V3 INTERACTIVE TRANSACTION MODAL */}
      <Modal
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        panelClassName="max-w-md"
        aria-label="Aave transaction"
      >
          <div className="w-full max-h-[92vh] overflow-y-auto [scrollbar-width:none] rounded-3xl border border-border bg-paper p-5 sm:p-6 shadow-2xl space-y-4 sm:space-y-5">
            {/* Modal Header */}
            <div className="flex items-center justify-between border-b border-border pb-3">
              <div className="flex items-center gap-3">
                <img
                  src={coinLogo(modalCoin)}
                  alt={modalCoin}
                  className="h-8 w-8 rounded-full object-contain"
                />
                <div>
                  <h3 className="text-base font-bold text-ink">
                    {modalAction === 'supply'
                      ? `Fornecer ${AAVE_V3_MARKET_DATA[modalCoin]?.symbol || modalCoin}`
                      : modalAction === 'withdraw'
                      ? `Resgatar ${AAVE_V3_MARKET_DATA[modalCoin]?.symbol || modalCoin}`
                      : modalAction === 'borrow'
                      ? `Tomar Empréstimo em ${AAVE_V3_MARKET_DATA[modalCoin]?.symbol || modalCoin}`
                      : `Pagar Dívida de ${AAVE_V3_MARKET_DATA[modalCoin]?.symbol || modalCoin}`}
                  </h3>
                  <div className="text-[11px] text-ink-muted font-medium">
                    Aave Polygon Market V3 · {AAVE_V3_MARKET_DATA[modalCoin]?.displayName}
                  </div>
                </div>
              </div>

              <button
                type="button"
                onClick={() => setModalOpen(false)}
                className="rounded-lg p-1.5 text-ink-muted hover:text-ink text-lg"
              >
                <i className="bi bi-x-lg" />
              </button>
            </div>

            {/* Input Form */}
            <div className="space-y-4">
              <div>
                <div className="flex justify-between text-xs font-semibold text-ink-muted mb-1.5">
                  <span>Quantia</span>
                  <span>
                    {modalAction === 'supply'
                      ? `Saldo Carteira: ${formatAmount(safeBigInt(walletsMap[modalCoin]?.balance), modalCoin)} ${AAVE_V3_MARKET_DATA[modalCoin]?.symbol || modalCoin}`
                      : modalAction === 'withdraw'
                      ? `Fornecido: ${formatAmount(safeBigInt(currentPos?.supplied), modalCoin)} ${AAVE_V3_MARKET_DATA[modalCoin]?.symbol || modalCoin}`
                      : modalAction === 'repay'
                      ? `Dívida: ${formatAmount(safeBigInt(currentPos?.debt), modalCoin)} ${AAVE_V3_MARKET_DATA[modalCoin]?.symbol || modalCoin}`
                      : `Margem Livre: ${formatUsd(positionsQ.data?.availableBorrowUsd)}`}
                  </span>
                </div>

                <div className="relative">
                  <input
                    type="text"
                    value={inputVal}
                    onChange={(e) => setInputVal(e.target.value.replace(/[^0-9.]/g, ''))}
                    placeholder="0.00"
                    autoFocus
                    className="input w-full pr-28 font-mono text-base font-bold"
                  />
                  <div className="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-1">
                    {[25, 50, 75, 100].map((p) => (
                      <button
                        key={p}
                        type="button"
                        onClick={() => setPercent(p)}
                        className="rounded-lg bg-surface border border-border px-1.5 py-1 text-[10px] font-bold text-ink-muted hover:text-ink hover:bg-paper active:scale-95"
                      >
                        {p === 100 ? 'MÁX' : `${p}%`}
                      </button>
                    ))}
                  </div>
                </div>
              </div>

              {/* Transaction Overview Card */}
              <div className="rounded-2xl border border-border bg-surface p-3.5 space-y-2 text-xs">
                <div className="flex justify-between">
                  <span className="text-ink-muted">Taxa Anual (APY):</span>
                  <span className="font-mono font-bold text-emerald-600">
                    {modalAction === 'supply' || modalAction === 'withdraw'
                      ? `+${getSupplyApy(undefined, modalCoin).toFixed(2)}% a.a.`
                      : `${getBorrowApy(undefined, modalCoin).toFixed(2)}% a.a.`}
                  </span>
                </div>

                <div className="flex justify-between">
                  <span className="text-ink-muted">Taxa de Rede (Gás):</span>
                  <span className="font-bold text-emerald-600 flex items-center gap-1">
                    <span>Grátis</span>
                    <span className="rounded bg-emerald-500/15 text-[9px] px-1 font-bold">
                      Zero Gas
                    </span>
                  </span>
                </div>

                <div className="flex justify-between pt-1 border-t border-border">
                  <span className="text-ink-muted">Fator de Saúde Atual:</span>
                  <span className="font-mono font-bold text-ink">{hfLabel}</span>
                </div>
              </div>

              {/* Feedback Msg */}
              {msg && (
                <div
                  className={clsx(
                    'rounded-xl p-3 text-xs font-medium border flex items-center gap-2',
                    msg.type === 'ok'
                      ? 'bg-emerald-50 text-emerald-800 border-emerald-200'
                      : 'bg-rose-50 text-rose-800 border-rose-200',
                  )}
                >
                  <i
                    className={clsx(
                      'bi',
                      msg.type === 'ok' ? 'bi-check-circle-fill text-emerald-600' : 'bi-exclamation-triangle-fill text-rose-600',
                    )}
                  />
                  <span>{msg.text}</span>
                </div>
              )}

              {/* Confirm Button */}
              <button
                type="button"
                onClick={submit}
                disabled={mutation.isPending || !inputVal || Number(inputVal) <= 0}
                className={clsx(
                  'w-full rounded-2xl py-3 text-xs font-bold text-white shadow-md transition-all active:scale-95 disabled:opacity-50',
                  modalAction === 'supply'
                    ? 'bg-emerald-600 hover:bg-emerald-700 shadow-emerald-600/25'
                    : modalAction === 'withdraw'
                    ? 'bg-amber-600 hover:bg-amber-700 shadow-amber-600/25'
                    : modalAction === 'borrow'
                    ? 'bg-blue-600 hover:bg-blue-700 shadow-blue-600/25'
                    : 'bg-indigo-600 hover:bg-indigo-700 shadow-indigo-600/25',
                )}
              >
                {mutation.isPending
                  ? 'Processando...'
                  : modalAction === 'supply'
                  ? `Fornecer ${AAVE_V3_MARKET_DATA[modalCoin]?.symbol || modalCoin}`
                  : modalAction === 'withdraw'
                  ? `Resgatar ${AAVE_V3_MARKET_DATA[modalCoin]?.symbol || modalCoin}`
                  : modalAction === 'borrow'
                  ? `Tomar ${AAVE_V3_MARKET_DATA[modalCoin]?.symbol || modalCoin}`
                  : `Pagar ${AAVE_V3_MARKET_DATA[modalCoin]?.symbol || modalCoin}`}
              </button>
            </div>
          </div>
      </Modal>
    </div>
  );
}
