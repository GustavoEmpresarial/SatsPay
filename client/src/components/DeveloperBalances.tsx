import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '../lib/api.js';
import { useAuthStore } from '../stores/auth.js';
import { formatApiError } from '../lib/formatError.js';
import { coinLogo } from '../lib/coinAssets.js';
import {
  COIN_CONFIG,
  COINS,
  formatAmount,
  parseAmount,
  safeBigInt,
  getCoinUsdValue,
  type Coin,
  type WalletBalance,
} from '@/shared';
import { Modal } from './Modal.js';

type Direction = 'toDeveloper' | 'toPersonal';

interface WalletsResp {
  wallets: WalletBalance[];
}

export function DeveloperBalances() {
  const { t } = useTranslation();
  const user = useAuthStore((s) => s.user);
  const qc = useQueryClient();
  const [modal, setModal] = useState<{ coin: Coin; direction: Direction } | null>(null);

  const personalQ = useQuery({
    queryKey: ['wallets', 'PERSONAL'],
    queryFn: () => api<WalletsResp>('/wallet?kind=PERSONAL'),
    enabled: Boolean(user),
  });
  const devQ = useQuery({
    queryKey: ['wallets', 'DEVELOPER'],
    queryFn: () => api<WalletsResp>('/wallet?kind=DEVELOPER'),
    enabled: Boolean(user),
  });
  const pricingQ = useQuery({
    queryKey: ['pricing'],
    queryFn: () =>
      api<{ prices: Record<string, string>; priceDecimals: number }>('/swap/prices', { skipAuth: true }),
    refetchInterval: 30_000,
  });
  const prices = pricingQ.data?.prices;
  const priceDecimals = pricingQ.data?.priceDecimals;

  const personalMap = useMemo(() => {
    const m: Partial<Record<Coin, WalletBalance>> = {};
    const list = Array.isArray(personalQ.data) ? personalQ.data : personalQ.data?.wallets ?? [];
    list.forEach((w) => (m[w.coin as Coin] = w));
    return m;
  }, [personalQ.data]);

  const devMap = useMemo(() => {
    const m: Partial<Record<Coin, WalletBalance>> = {};
    const list = Array.isArray(devQ.data) ? devQ.data : devQ.data?.wallets ?? [];
    list.forEach((w) => (m[w.coin as Coin] = w));
    return m;
  }, [devQ.data]);

  return (
    <>
      <section className="rounded-3xl border border-border bg-paper shadow-xs overflow-hidden">
        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border/80 px-6 py-5 bg-surface/30">
          <div>
            <div className="flex items-center gap-2 mb-1">
              <span className="flex h-2 w-2 rounded-full bg-emerald-500 animate-pulse" />
              <span className="text-xs font-bold uppercase tracking-wider text-emerald-600">
                {t('developer.balances')}
              </span>
            </div>
            <p className="text-xs text-ink-muted">{t('developer.balancesHint')}</p>
          </div>
        </div>

        <div className="grid gap-4 p-5 sm:grid-cols-2 xl:grid-cols-3">
          {COINS.map((c) => {
            const cfg = COIN_CONFIG[c] || COIN_CONFIG.BTC;
            const devBal = devMap[c] ? safeBigInt(devMap[c]!.balance) : 0n;
            const perBal = personalMap[c] ? safeBigInt(personalMap[c]!.balance) : 0n;
            const devUsd = getCoinUsdValue(devBal, c, prices, priceDecimals);
            const perUsd = getCoinUsdValue(perBal, c, prices, priceDecimals);

            return (
              <div
                key={c}
                className="rounded-2xl border border-border bg-surface/50 p-4 space-y-3.5 hover:border-border/80 transition-all shadow-xs"
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2.5">
                    <img src={coinLogo(c)} alt={c} className="h-8 w-8 rounded-full object-contain shrink-0" />
                    <div>
                      <div className="text-xs font-bold text-ink leading-tight">{cfg.name}</div>
                      <div className="text-[10px] font-mono font-semibold text-ink-muted uppercase">{c}</div>
                    </div>
                  </div>
                  <div className="rounded-full bg-paper border border-border/80 px-2.5 py-0.5 font-mono text-[11px] font-bold text-emerald-700">
                    ${devUsd.toFixed(2)} USD
                  </div>
                </div>

                {/* Developer / Merchant Balance */}
                <div className="rounded-xl border border-border bg-paper p-3 space-y-1">
                  <div className="text-[10px] uppercase font-bold tracking-wider text-ink-muted flex items-center justify-between">
                    <span>{t('developer.developerBalance')}</span>
                    <span className="text-[9px] bg-emerald-500/10 text-emerald-700 px-1.5 py-0.2 rounded font-bold">API / Caixa</span>
                  </div>
                  <div className="flex items-baseline justify-between">
                    <div className="font-mono text-lg font-black text-ink">
                      {formatAmount(devBal, c)} <span className="text-xs font-bold text-ink-muted">{c}</span>
                    </div>
                    <div className="font-mono text-xs text-ink-muted font-medium">
                      ≈ ${devUsd.toFixed(2)}
                    </div>
                  </div>
                </div>

                {/* Personal Balance */}
                <div className="flex items-center justify-between rounded-xl bg-surface/80 border border-border/60 px-3 py-2 text-xs">
                  <div className="flex items-center gap-1.5 text-ink-muted font-semibold text-[11px]">
                    <i className="bi bi-person-fill text-bitcoin" />
                    <span>{t('developer.personalBalance')}:</span>
                  </div>
                  <div className="text-right">
                    <span className="font-mono font-bold text-ink">{formatAmount(perBal, c)} {c}</span>
                    <span className="text-[10px] text-ink-muted ml-1">(${perUsd.toFixed(2)})</span>
                  </div>
                </div>

                {/* Action Buttons */}
                <div className="grid grid-cols-2 gap-2 pt-0.5">
                  <button
                    type="button"
                    onClick={() => setModal({ coin: c, direction: 'toDeveloper' })}
                    disabled={perBal === 0n}
                    className="rounded-xl bg-emerald-600 hover:bg-emerald-700 active:scale-[0.98] text-white py-2 px-3 text-xs font-bold transition-all shadow-xs disabled:opacity-40 disabled:pointer-events-none flex items-center justify-center gap-1.5"
                  >
                    <i className="bi bi-arrow-down" />
                    <span>{t('developer.topUp')}</span>
                  </button>
                  <button
                    type="button"
                    onClick={() => setModal({ coin: c, direction: 'toPersonal' })}
                    disabled={devBal === 0n}
                    className="rounded-xl bg-paper hover:bg-surface active:scale-[0.98] border border-border text-ink py-2 px-3 text-xs font-bold transition-all shadow-xs disabled:opacity-40 disabled:pointer-events-none flex items-center justify-center gap-1.5"
                  >
                    <i className="bi bi-arrow-up" />
                    <span>{t('developer.withdrawToPersonal')}</span>
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      </section>

      {modal && (
          <TransferModal
            coin={modal.coin}
            direction={modal.direction}
            available={
              modal.direction === 'toDeveloper'
                ? (personalMap[modal.coin] ? safeBigInt(personalMap[modal.coin]!.balance) : 0n)
                : (devMap[modal.coin] ? safeBigInt(devMap[modal.coin]!.balance) : 0n)
            }
            prices={prices}
            priceDecimals={priceDecimals}
            onClose={() => setModal(null)}
            onDone={() => {
              qc.invalidateQueries({ queryKey: ['wallets', 'PERSONAL'] });
              qc.invalidateQueries({ queryKey: ['wallets', 'DEVELOPER'] });
              qc.invalidateQueries({ queryKey: ['wallets'] });
              setModal(null);
            }}
          />
      )}
    </>
  );
}

function TransferModal({
  coin,
  direction,
  available,
  prices,
  priceDecimals = 8,
  onClose,
  onDone,
}: {
  coin: Coin;
  direction: Direction;
  available: bigint;
  prices?: Record<string, string | number>;
  priceDecimals?: number;
  onClose: () => void;
  onDone: () => void;
}) {
  const { t } = useTranslation();
  const [inputVal, setInputVal] = useState('');
  const [err, setErr] = useState<string | null>(null);

  const cfg = COIN_CONFIG[coin] || COIN_CONFIG.BTC;

  const handleInputChange = (val: string) => {
    setErr(null);
    const clean = val.replace(',', '.').replace(/[^0-9.]/g, '');
    const dots = clean.split('.').length - 1;
    if (dots <= 1) {
      setInputVal(clean);
    }
  };

  const parsedAmount = useMemo(() => {
    return parseAmount(inputVal, coin);
  }, [inputVal, coin]);

  const setPercent = (pct: number) => {
    setErr(null);
    if (available <= 0n) return;
    if (pct === 100) {
      setInputVal(formatAmount(available, coin));
      return;
    }
    const fracUnits = (available * BigInt(pct)) / 100n;
    setInputVal(formatAmount(fracUnits, coin));
  };

  const isExceeded = parsedAmount > available;
  const isZero = parsedAmount <= 0n;

  const approxUsd = useMemo(() => {
    return getCoinUsdValue(parsedAmount, coin, prices, priceDecimals);
  }, [parsedAmount, coin, prices, priceDecimals]);

  const availableUsd = useMemo(() => {
    return getCoinUsdValue(available, coin, prices, priceDecimals);
  }, [available, coin, prices, priceDecimals]);

  const mutation = useMutation({
    mutationFn: () =>
      api('/wallet/transfer', {
        method: 'POST',
        json: {
          coin,
          amount: parsedAmount.toString(),
          toDeveloper: direction === 'toDeveloper',
        },
      }),
    onSuccess: onDone,
    onError: (e) => setErr(formatApiError(e)),
  });

  const label =
    direction === 'toDeveloper'
      ? t('developer.topUp')
      : t('developer.withdrawToPersonal');

  return (
    <Modal open onClose={onClose} panelClassName="max-w-md" aria-label={label}>
      <div className="rounded-3xl border border-border bg-paper w-full p-6 shadow-2xl space-y-5">
        {/* Header */}
        <div className="flex items-start justify-between border-b border-border pb-4">
          <div className="flex items-center gap-3">
            <img src={coinLogo(coin)} alt={coin} className="h-10 w-10 rounded-full object-contain" />
            <div>
              <h3 className="text-base font-black text-ink">
                {label} · {coin}
              </h3>
              <div className="text-xs text-ink-muted font-medium">
                {cfg.name}
              </div>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-xl p-1.5 text-ink-muted hover:bg-surface hover:text-ink transition-colors"
          >
            <i className="bi bi-x-lg text-sm" />
          </button>
        </div>

        {/* Transfer Path / Direction Indicator */}
        <div className="rounded-2xl border border-border bg-surface/60 p-3 flex items-center justify-between text-xs">
          <div className="text-left space-y-0.5">
            <div className="text-[10px] uppercase font-bold text-ink-muted">Origem</div>
            <div className="font-bold text-ink">
              {direction === 'toDeveloper' ? 'Carteira Pessoal' : 'Saldo Comerciante'}
            </div>
          </div>
          <div className="flex h-7 w-7 items-center justify-center rounded-full bg-paper border border-border text-ink-muted">
            <i className="bi bi-arrow-right" />
          </div>
          <div className="text-right space-y-0.5">
            <div className="text-[10px] uppercase font-bold text-ink-muted">Destino</div>
            <div className="font-bold text-emerald-600">
              {direction === 'toDeveloper' ? 'Saldo Comerciante' : 'Carteira Pessoal'}
            </div>
          </div>
        </div>

        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (isZero || isExceeded) return;
            mutation.mutate();
          }}
          className="space-y-4"
        >
          {/* Amount input */}
          <div className="space-y-2">
            <div className="flex items-center justify-between text-xs">
              <label className="font-bold text-ink">{t('developer.amount')}</label>
              <div className="text-[11px] text-ink-muted font-medium">
                Disponível: <span className="font-mono font-bold text-ink">{formatAmount(available, coin)} {coin}</span>
                <span className="text-[10px] ml-1">(${availableUsd.toFixed(2)})</span>
              </div>
            </div>

            <div className="relative">
              <input
                className="w-full rounded-2xl border border-border bg-surface px-4 py-3 font-mono text-lg font-black text-ink placeholder:text-ink-muted/50 focus:border-bitcoin focus:outline-none focus:ring-2 focus:ring-bitcoin/20 transition-all pr-24"
                required
                type="text"
                inputMode="decimal"
                placeholder="0.00"
                value={inputVal}
                onChange={(e) => handleInputChange(e.target.value)}
              />
              <div className="absolute right-3 top-1/2 -translate-y-1/2 flex items-center gap-1.5">
                <span className="font-bold text-xs font-mono text-ink-muted">{coin}</span>
              </div>
            </div>

            {/* Quick percentage buttons */}
            <div className="grid grid-cols-4 gap-1.5 pt-1">
              {[25, 50, 75, 100].map((pct) => (
                <button
                  key={pct}
                  type="button"
                  onClick={() => setPercent(pct)}
                  className="rounded-xl border border-border bg-paper hover:bg-surface active:scale-95 py-1.5 text-xs font-bold text-ink transition-all shadow-2xs"
                >
                  {pct === 100 ? 'Máx' : `${pct}%`}
                </button>
              ))}
            </div>

            {/* Approx USD conversion badge */}
            <div className="flex items-center justify-between text-[11px] text-ink-muted pt-0.5">
              <span>Valor aproximado:</span>
              <span className="font-mono font-bold text-ink">≈ ${approxUsd.toFixed(2)} USD</span>
            </div>
          </div>

          {/* Validation Warnings */}
          {isExceeded && (
            <div className="rounded-xl bg-rose-500/10 border border-rose-500/20 p-3 text-xs text-rose-600 flex items-center gap-2">
              <i className="bi bi-exclamation-triangle-fill shrink-0" />
              <span>O valor informado ultrapassa o saldo disponível ({formatAmount(available, coin)} {coin}).</span>
            </div>
          )}

          {err && (
            <div className="rounded-xl bg-rose-500/10 border border-rose-500/20 p-3 text-xs text-rose-600 flex items-center gap-2">
              <i className="bi bi-exclamation-circle-fill shrink-0" />
              <span>{err}</span>
            </div>
          )}

          {/* Action buttons */}
          <div className="flex gap-2.5 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="flex-1 rounded-2xl border border-border bg-paper hover:bg-surface py-3 text-xs font-bold text-ink transition-all shadow-xs"
            >
              {t('developer.close')}
            </button>
            <button
              type="submit"
              disabled={mutation.isPending || isZero || isExceeded}
              className="flex-1 rounded-2xl bg-emerald-600 hover:bg-emerald-700 active:scale-[0.98] py-3 text-xs font-black text-white transition-all shadow-xs disabled:opacity-40 disabled:pointer-events-none flex items-center justify-center gap-2"
            >
              {mutation.isPending ? (
                <>
                  <span className="h-4 w-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                  <span>{t('developer.transferring')}</span>
                </>
              ) : (
                <>
                  <i className="bi bi-check2-circle text-sm" />
                  <span>{t('developer.transfer')}</span>
                </>
              )}
            </button>
          </div>
        </form>
      </div>
    </Modal>
  );
}
