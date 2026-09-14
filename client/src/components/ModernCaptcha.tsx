import { useRef } from 'react';
import { Turnstile, type TurnstileRef } from './Turnstile.js';
import { FAUCET_CAPTCHA_ACTION } from '../lib/captchaActions.js';

export interface ModernCaptchaProps {
  onVerify: (token: string) => void;
  onReset?: () => void;
  verified: boolean;
  /** Cloudflare Turnstile action — must match backend expected_action (e.g. faucet_claim). */
  action?: string;
}

export function ModernCaptcha({
  onVerify,
  onReset,
  verified,
  action = FAUCET_CAPTCHA_ACTION,
}: ModernCaptchaProps) {
  const turnstileRef = useRef<TurnstileRef>(null);

  const handleReset = () => {
    turnstileRef.current?.reset();
    onReset?.();
  };

  return (
    <div className="w-full space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium text-ink-muted">Verificação</span>
        {verified && (
          <button
            type="button"
            onClick={handleReset}
            className="text-[11px] font-medium text-ink-muted hover:text-ink"
          >
            Refazer
          </button>
        )}
      </div>

      {verified ? (
        <div className="flex items-center gap-2 rounded-xl border border-border bg-surface px-3 py-2.5 text-sm text-ink">
          <i className="bi bi-check-circle-fill text-emerald-600" />
          <span>Pronto</span>
        </div>
      ) : (
        <div className="rounded-xl border border-border bg-surface/70 px-2 py-1">
          <Turnstile
            ref={turnstileRef}
            action={action}
            onVerify={onVerify}
            onReset={onReset}
            size="flexible"
          />
        </div>
      )}
    </div>
  );
}
