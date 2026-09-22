import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api, ApiError } from '../lib/api.js';
import { formatApiError } from '../lib/formatError.js';
import { coinLogo } from '../lib/coinAssets.js';
import { useAuthStore } from '../stores/auth.js';
import { ModernCaptcha } from '../components/ModernCaptcha.js';
import { FAUCET_CAPTCHA_ACTION } from '../lib/captchaActions.js';
import { COIN_CONFIG, COINS, formatAmount, safeBigInt, type Coin } from '@/shared';

/** Parse API timestamps (RFC3339 or legacy "YYYY-MM-DD HH:MM:SS.ssssss UTC"). */
function parseClaimAt(raw?: string | null): number | null {
  if (!raw) return null;
  let t = Date.parse(raw);
  if (Number.isFinite(t)) return t;
  const normalized = raw
    .trim()
    .replace(' ', 'T')
    .replace(/ UTC$/i, 'Z')
    .replace(/(\.\d{3})\d+/, '$1');
  t = Date.parse(normalized);
  return Number.isFinite(t) ? t : null;
}

function formatRemainingTime(secs: number) {
  const hours = Math.floor(secs / 3600);
  const minutes = Math.floor((secs % 3600) / 60);
  const seconds = secs % 60;
  return `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}`;
}

type ClaimResp = {
  amount: string;
  nextClaimAt?: string;
  next_claim_at?: string;
  pointsAwarded?: boolean;
  seasonActive?: boolean;
};

type FaucetStatus = {
  cooldownMinutes?: number;
  coins?: { coin: string; nextClaimAt?: string | null }[];
};

function cooldownsFromStatus(data: FaucetStatus | undefined): Record<string, number> {
  const map: Record<string, number> = {};
  for (const row of data?.coins ?? []) {
    const until = parseClaimAt(row.nextClaimAt);
    if (until) map[row.coin] = until;
  }
  return map;
}

export function FaucetPage() {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const user = useAuthStore((s) => s.user);

  const [selectedCoin, setSelectedCoin] = useState<Coin>('BTC');
  const [msg, setMsg] = useState<{ type: 'success' | 'error' | 'info'; text: string } | null>(null);

  const [captchaToken, setCaptchaToken] = useState<string | null>(null);
  const [captchaVerified, setCaptchaVerified] = useState(false);
  const [honeypot, setHoneypot] = useState('');
  const [mountedAt] = useState(() => Date.now());

  const [now, setNow] = useState(Date.now());

  // Server is the only clock: user + IP cooldown from faucet_claims.
  const statusQ = useQuery({
    queryKey: ['faucet-status'],
    queryFn: () => api<FaucetStatus>('/faucet/status'),
    enabled: !!user,
    staleTime: 15_000,
    refetchOnWindowFocus: true,
  });
  const statusReady = statusQ.isSuccess;
  const cooldowns = useMemo(() => cooldownsFromStatus(statusQ.data), [statusQ.data]);

  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);

  const applyServerCooldown = (coin: Coin, untilIso: string) => {
    qc.setQueryData<FaucetStatus>(['faucet-status'], (old) => {
      const base = old?.coins?.length
        ? old.coins
        : COINS.map((c) => ({ coin: c, nextClaimAt: null as string | null }));
      return {
        cooldownMinutes: old?.cooldownMinutes ?? 660,
        coins: base.map((row) => (row.coin === coin ? { ...row, nextClaimAt: untilIso } : row)),
      };
    });
  };

  const isCoinCooling = (coin: Coin) => statusReady && now < (cooldowns[coin] || 0);

  const availableCoins = useMemo(() => {
    if (!statusReady) return [];
    return COINS.filter((c) => !isCoinCooling(c));
    // eslint-disable-next-line react-hooks/exhaustive-deps -- isCoinCooling closes over now/cooldowns/statusReady
  }, [now, cooldowns, statusReady]);

  useEffect(() => {
    if (!statusReady) return;
    if (!isCoinCooling(selectedCoin)) return;
    const next = availableCoins[0];
    if (next && next !== selectedCoin) {
      setSelectedCoin(next);
      setMsg((m) => (m?.type === 'info' ? null : m));
    }
  }, [selectedCoin, availableCoins, now, cooldowns, statusReady]);

  const coinCooldownUntil = cooldowns[selectedCoin] || 0;
  const isCoolingDown = statusReady && now < coinCooldownUntil;
  const remainingSeconds = Math.max(0, Math.ceil((coinCooldownUntil - now) / 1000));
  const canClaim =
    statusReady && captchaVerified && !isCoolingDown && availableCoins.length > 0 && !statusQ.isFetching;

  const handleCaptchaVerify = (token: string) => {
    setCaptchaToken(token);
    setCaptchaVerified(true);
    setMsg(null);
  };

  const claimMut = useMutation({
    mutationFn: async (coin: Coin) => {
      if (!statusReady) {
        throw new Error(t('faucet.statusLoading', { defaultValue: 'Carregando cooldown do servidor…' }));
      }
      if (honeypot.trim() !== '') {
        throw new Error('Bot detectado.');
      }
      if (Date.now() - mountedAt < 800) {
        throw new Error('Ação muito rápida. Verifique a tela.');
      }
      if (!captchaVerified || !captchaToken) {
        throw new Error(t('faucet.captchaRequired', { defaultValue: 'Conclua a verificação primeiro.' }));
      }
      if (now < (cooldowns[coin] || 0)) {
        throw new Error(t('faucet.cooldownActive', { defaultValue: 'Aguarde o cooldown do faucet.' }));
      }

      return api<ClaimResp>(`/faucet/claim/${coin}`, {
        method: 'POST',
        json: { coin, captchaToken },
      });
    },
    onSuccess: (data, coin) => {
      const untilIso = data.nextClaimAt ?? data.next_claim_at;
      if (untilIso) applyServerCooldown(coin, untilIso);
      void qc.invalidateQueries({ queryKey: ['faucet-status'] });

      setCaptchaVerified(false);
      setCaptchaToken(null);

      void qc.invalidateQueries({ queryKey: ['wallets', 'PERSONAL'] });
      void qc.invalidateQueries({ queryKey: ['wallets'] });
      void qc.invalidateQueries({ queryKey: ['ledger'] });
      void qc.invalidateQueries({ queryKey: ['airdrop-overview'] });
      void qc.invalidateQueries({ queryKey: ['airdrop-logs'] });

      const units = formatAmount(safeBigInt(data.amount), coin);
      let text = `${t('faucet.received', { defaultValue: 'Reivindicado' })}: +${units} ${coin}`;
      if (data.pointsAwarded) {
        text += ` (+50 SatsPoints)`;
      } else if (data.seasonActive === false) {
        text += `. ${t('faucet.seasonInactive', { defaultValue: 'Temporada de airdrop inativa — pontos não creditados.' })}`;
      }
      setMsg({ type: 'success', text });

      const nextAvail = COINS.find((c) => c !== coin && Date.now() >= (cooldowns[c] || 0));
      if (nextAvail) setSelectedCoin(nextAvail);
    },
    onError: (err, coin) => {
      setCaptchaVerified(false);
      setCaptchaToken(null);
      // Always re-read the server clock after a rejection.
      void qc.invalidateQueries({ queryKey: ['faucet-status'] });

      if (err instanceof ApiError && (err.code === 'FAUCET_COOLDOWN' || /next claim available/i.test(err.message))) {
        const details = err.details as { nextClaimAt?: string } | undefined;
        const until = parseClaimAt(details?.nextClaimAt);
        if (until && until > Date.now()) {
          applyServerCooldown(coin, new Date(until).toISOString());
          const stillAvail = COINS.find((c) => c !== coin && Date.now() >= (cooldowns[c] || 0));
          if (stillAvail) {
            setSelectedCoin(stillAvail);
            setMsg({
              type: 'info',
              text: `${coin}: cooldown ativo no servidor. Tente ${stillAvail}.`,
            });
            return;
          }
          const secs = Math.ceil((until - Date.now()) / 1000);
          setMsg({
            type: 'info',
            text: `${coin}: ${t('faucet.cooldownActive', { defaultValue: 'Aguarde o cooldown.' })} Recarga em ${formatRemainingTime(secs)}.`,
          });
          return;
        }
        setMsg({ type: 'info', text: t('faucet.cooldownActive', { defaultValue: 'Aguarde o cooldown do faucet.' }) });
        return;
      }

      setMsg({ type: 'error', text: formatApiError(err) });
    },
  });

  const onClaimClick = () => {
    if (!statusReady) {
      setMsg({ type: 'info', text: t('faucet.statusLoading', { defaultValue: 'Carregando cooldown do servidor…' }) });
      return;
    }
    const coin = !isCoolingDown ? selectedCoin : availableCoins[0];
    if (!coin) {
      setMsg({ type: 'info', text: t('faucet.cooldownActive', { defaultValue: 'Nenhuma moeda disponível agora.' }) });
      return;
    }
    if (coin !== selectedCoin) setSelectedCoin(coin);
    claimMut.mutate(coin);
  };

  return (
    <div className="mx-auto max-w-2xl space-y-6">
      <input
        type="text"
        name="faucet_bot_trap"
        value={honeypot}
        onChange={(e) => setHoneypot(e.target.value)}
        tabIndex={-1}
        autoComplete="off"
        className="hidden"
        style={{ display: 'none' }}
      />

      <header>
        <h1 className="text-2xl md:text-3xl font-bold tracking-tight">{t('faucet.title', { defaultValue: 'Faucet' })}</h1>
        <p className="text-sm text-ink-muted mt-1">{t('faucet.subtitle', { defaultValue: 'Pequenas quantias a cada 11 horas' })}</p>
      </header>

      {statusQ.isError && (
        <div className="rounded-2xl border border-rose-200 bg-rose-50 p-4 text-sm text-rose-800">
          Não foi possível ler o cooldown no servidor. Recarregue a página.
        </div>
      )}

      {!statusReady && !statusQ.isError && (
        <div className="rounded-2xl border border-border bg-surface/80 p-4 text-sm text-ink-muted flex items-center gap-2">
          <i className="bi bi-arrow-repeat animate-spin" />
          Carregando cooldown do servidor…
        </div>
      )}

      {msg && (
        <div
          className={`rounded-2xl p-4 text-sm border flex items-center gap-3 shadow-xs ${
            msg.type === 'success'
              ? 'bg-emerald-50 text-emerald-800 border-emerald-200'
              : msg.type === 'error'
                ? 'bg-rose-50 text-rose-800 border-rose-200'
                : 'bg-amber-50 text-amber-900 border-amber-200'
          }`}
        >
          <i
            className={`bi text-lg ${
              msg.type === 'success'
                ? 'bi-check-circle-fill text-emerald-600'
                : msg.type === 'error'
                  ? 'bi-exclamation-triangle-fill text-rose-600'
                  : 'bi-info-circle-fill text-amber-600'
            }`}
          />
          <span className="leading-relaxed font-medium">{msg.text}</span>
        </div>
      )}

      <div className="card p-6 space-y-6">
        <div>
          <label className="mb-3 block text-xs font-semibold uppercase tracking-wider text-ink-muted">
            {t('common.selectCoin', { defaultValue: 'Selecione a moeda' })}
          </label>
          <div className="grid grid-cols-2 gap-2.5 sm:grid-cols-3">
            {COINS.map((c) => {
              const active = selectedCoin === c;
              const cooling = isCoinCooling(c);
              const secs = Math.max(0, Math.ceil(((cooldowns[c] || 0) - now) / 1000));
              const pending = !statusReady;

              return (
                <button
                  key={c}
                  type="button"
                  disabled={pending}
                  onClick={() => {
                    setSelectedCoin(c);
                    setMsg(null);
                  }}
                  className={`flex items-center gap-3 rounded-xl border p-3 text-left transition-all ${
                    active
                      ? 'border-bitcoin bg-bitcoin/5 text-ink shadow-sm ring-1 ring-bitcoin/30'
                      : cooling || pending
                        ? 'border-border bg-surface/50 text-ink-muted opacity-70'
                        : 'border-border bg-surface hover:bg-paper text-ink-muted'
                  }`}
                >
                  <img src={coinLogo(c)} alt={c} className="h-7 w-7 rounded-full shadow-xs" />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center justify-between gap-1">
                      <span className="text-xs font-semibold text-ink">{c}</span>
                      {pending ? (
                        <span className="rounded bg-surface px-1.5 py-0.5 text-[10px] font-medium text-ink-muted">…</span>
                      ) : cooling ? (
                        <span className="rounded bg-amber-50 px-1.5 py-0.5 font-mono text-[10px] font-medium text-amber-700">
                          {formatRemainingTime(secs)}
                        </span>
                      ) : (
                        <span className="rounded bg-emerald-50 px-1.5 py-0.5 text-[10px] font-semibold text-emerald-700">
                          OK
                        </span>
                      )}
                    </div>
                    <div className="mt-0.5 text-[11px] text-ink-muted">+{formatAmount(COIN_CONFIG[c].faucetReward, c)}</div>
                  </div>
                </button>
              );
            })}
          </div>
        </div>

        <ModernCaptcha
          action={FAUCET_CAPTCHA_ACTION}
          verified={captchaVerified}
          onVerify={handleCaptchaVerify}
          onReset={() => {
            setCaptchaVerified(false);
            setCaptchaToken(null);
          }}
        />

        <div className="flex flex-col gap-4 rounded-2xl border border-border bg-surface/80 p-4 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <div className="text-xs text-ink-muted">{t('faucet.claimAmount', { defaultValue: 'Valor' })}</div>
            <div className="mt-0.5 flex items-center gap-2 font-mono text-xl font-bold text-ink">
              <img src={coinLogo(selectedCoin)} alt={selectedCoin} className="h-5 w-5 rounded-full" />
              <span>
                +{formatAmount(COIN_CONFIG[selectedCoin].faucetReward, selectedCoin)} {selectedCoin}
              </span>
            </div>
            {!statusReady ? (
              <div className="mt-1 text-xs font-medium text-ink-muted">Sincronizando com o servidor…</div>
            ) : isCoolingDown ? (
              <div className="mt-1 flex items-center gap-1.5 text-xs font-medium text-amber-700">
                <i className="bi bi-clock-history" />
                <span>
                  Recarga em: <strong className="font-mono">{formatRemainingTime(remainingSeconds)}</strong>
                </span>
              </div>
            ) : (
              <div className="mt-1 text-xs font-medium text-emerald-700">Disponível agora</div>
            )}
          </div>

          <button
            type="button"
            disabled={claimMut.isPending || !canClaim}
            onClick={onClaimClick}
            className={`btn-primary flex items-center justify-center gap-2 px-7 py-3 text-sm font-semibold shadow-md ${
              !canClaim ? 'cursor-not-allowed opacity-50' : 'hover:scale-[1.02] active:scale-[0.98]'
            }`}
          >
            {claimMut.isPending ? (
              <>
                <i className="bi bi-arrow-repeat animate-spin text-base" />
                <span>{t('faucet.claiming', { defaultValue: 'Processando...' })}</span>
              </>
            ) : !statusReady ? (
              <>
                <i className="bi bi-arrow-repeat animate-spin text-base" />
                <span>Sincronizando…</span>
              </>
            ) : !captchaVerified ? (
              <>
                <i className="bi bi-shield-check text-base" />
                <span>Verifique primeiro</span>
              </>
            ) : availableCoins.length === 0 ? (
              <>
                <i className="bi bi-hourglass-split text-base" />
                <span>{formatRemainingTime(remainingSeconds)}</span>
              </>
            ) : (
              <>
                <i className="bi bi-droplet-fill text-base text-white" />
                <span>
                  {t('faucet.claim', { defaultValue: 'Reivindicar' })} {selectedCoin}
                </span>
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
}
