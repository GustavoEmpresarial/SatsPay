import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { api, ApiError } from '../lib/api.js';
import { formatApiError } from '../lib/formatError.js';
import { coinLogo } from '../lib/coinAssets.js';
import { useAuthStore } from '../stores/auth.js';
import { ModernCaptcha } from '../components/ModernCaptcha.js';
import { FAUCET_CAPTCHA_ACTION } from '../lib/captchaActions.js';
import { COIN_CONFIG, COINS, formatAmount, safeBigInt, type Coin } from '@/shared';

const COOLDOWN_MS = 11 * 60 * 60 * 1000; // fallback; must match FAUCET_COOLDOWN_MINUTES=660 (server)

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

type ClaimResp = { amount: string; nextClaimAt?: string; next_claim_at?: string };

export function FaucetPage() {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const user = useAuthStore((s) => s.user);
  const storageKey = `bitcosats_faucet_v2_${user?.id || 'guest'}`;

  const [selectedCoin, setSelectedCoin] = useState<Coin>('BTC');
  const [msg, setMsg] = useState<{ type: 'success' | 'error' | 'info'; text: string } | null>(null);

  const [captchaToken, setCaptchaToken] = useState<string | null>(null);
  const [captchaVerified, setCaptchaVerified] = useState(false);
  const [honeypot, setHoneypot] = useState('');
  const [mountedAt] = useState(() => Date.now());

  const [now, setNow] = useState(Date.now());
  const [cooldowns, setCooldowns] = useState<Record<string, number>>(() => {
    try {
      const stored = localStorage.getItem(storageKey);
      return stored ? JSON.parse(stored) : {};
    } catch {
      return {};
    }
  });

  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);

  const persistCooldowns = (next: Record<string, number>) => {
    setCooldowns(next);
    try {
      localStorage.setItem(storageKey, JSON.stringify(next));
    } catch {
      /* ignore */
    }
  };

  const isCoinCooling = (coin: Coin) => now < (cooldowns[coin] || 0);

  const availableCoins = useMemo(
    () => COINS.filter((c) => !isCoinCooling(c)),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- isCoinCooling closes over now/cooldowns
    [now, cooldowns],
  );

  // If the selected coin is on cooldown, jump to the first available one.
  useEffect(() => {
    if (!isCoinCooling(selectedCoin)) return;
    const next = availableCoins[0];
    if (next && next !== selectedCoin) {
      setSelectedCoin(next);
      // Don't leave a stale cooldown banner for the previous coin.
      setMsg((m) => (m?.type === 'info' ? null : m));
    }
  }, [selectedCoin, availableCoins, now, cooldowns]);

  const coinCooldownUntil = cooldowns[selectedCoin] || 0;
  const isCoolingDown = now < coinCooldownUntil;
  const remainingSeconds = Math.max(0, Math.ceil((coinCooldownUntil - now) / 1000));
  const canClaim = captchaVerified && !isCoolingDown && availableCoins.length > 0;

  const handleCaptchaVerify = (token: string) => {
    setCaptchaToken(token);
    setCaptchaVerified(true);
    setMsg(null);
  };

  const claimMut = useMutation({
    mutationFn: async (coin: Coin) => {
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
      const until = parseClaimAt(data.nextClaimAt ?? data.next_claim_at) ?? Date.now() + COOLDOWN_MS;
      persistCooldowns({ ...cooldowns, [coin]: until });

      setCaptchaVerified(false);
      setCaptchaToken(null);

      qc.invalidateQueries({ queryKey: ['wallets'] });
      qc.invalidateQueries({ queryKey: ['ledger'] });

      setMsg({
        type: 'success',
        text: `${t('faucet.received', { defaultValue: 'Reivindicado' })}: +${formatAmount(safeBigInt(data.amount), coin)} ${coin}.`,
      });

      const nextAvail = COINS.find((c) => c !== coin && Date.now() >= (cooldowns[c] || 0));
      if (nextAvail) setSelectedCoin(nextAvail);
    },
    onError: (err, coin) => {
      setCaptchaVerified(false);
      setCaptchaToken(null);

      if (err instanceof ApiError && (err.code === 'FAUCET_COOLDOWN' || /next claim available/i.test(err.message))) {
        const details = err.details as { nextClaimAt?: string } | undefined;
        const until = parseClaimAt(details?.nextClaimAt);
        if (until && until > Date.now()) {
          const updated = { ...cooldowns, [coin]: until };
          persistCooldowns(updated);
          const stillAvail = COINS.find((c) => Date.now() >= (updated[c] || 0));
          if (stillAvail) {
            setSelectedCoin(stillAvail);
            // Don't leave an amber banner over an available coin — user should claim that one.
            setMsg(null);
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

              return (
                <button
                  key={c}
                  type="button"
                  onClick={() => {
                    setSelectedCoin(c);
                    // Clear banners when picking another coin (esp. cooldown info).
                    setMsg(null);
                  }}
                  className={`flex items-center gap-3 rounded-xl border p-3 text-left transition-all ${
                    active
                      ? 'border-bitcoin bg-bitcoin/5 text-ink shadow-sm ring-1 ring-bitcoin/30'
                      : cooling
                        ? 'border-border bg-surface/50 text-ink-muted opacity-70'
                        : 'border-border bg-surface hover:bg-paper text-ink-muted'
                  }`}
                >
                  <img src={coinLogo(c)} alt={c} className="h-7 w-7 rounded-full shadow-xs" />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center justify-between gap-1">
                      <span className="text-xs font-semibold text-ink">{c}</span>
                      {cooling ? (
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
            {isCoolingDown ? (
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
