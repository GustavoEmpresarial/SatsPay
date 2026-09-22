import { useState, useRef } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { AuthLayout } from '../components/AuthLayout.js';
import { PasswordInput } from '../components/PasswordInput.js';
import { Turnstile, type TurnstileRef } from '../components/Turnstile.js';
import { LOGIN_CAPTCHA_ACTION } from '../lib/captchaActions.js';
import { api } from '../lib/api.js';
import { formatApiError } from '../lib/formatError.js';
import { useAuthStore } from '../stores/auth.js';
import { reportAuthFailure } from '../lib/reportError.js';
import { resolveReturnTo } from '../lib/returnTo.js';
import type { AuthLoginResponse } from '@/shared';

export function LoginPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const setSession = useAuthStore((s) => s.setSession);
  const returnTo = resolveReturnTo(searchParams.get('return_to'));
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [emailCode, setEmailCode] = useState('');
  const [needsCode, setNeedsCode] = useState(false);
  const [captchaToken, setCaptchaToken] = useState<string | null>(null);
  const turnstileRef = useRef<TurnstileRef>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setLoading(true);

    try {
      const res = await api<AuthLoginResponse>('/auth/login', {
        method: 'POST',
        skipAuth: true,
        json: {
          email: email.trim(),
          password,
          ...(needsCode && emailCode ? { emailCode: emailCode.trim() } : {}),
          ...(captchaToken ? { captchaToken } : {}),
        },
      });

      if (res.codeSent) {
        setNeedsCode(true);
        setInfo(res.message || t('auth.login.codeSent'));
        setCaptchaToken(null);
        turnstileRef.current?.reset();
      } else if (res.user && res.accessToken) {
        setSession(res);
        navigate(returnTo);
      }
    } catch (err) {
      const msg = formatApiError(err);
      setError(msg);
      setCaptchaToken(null);
      turnstileRef.current?.reset();
      reportAuthFailure('auth', `Login failed: ${msg}`, {
        hasCaptcha: Boolean(captchaToken),
        needsCode,
      });
    } finally {
      setLoading(false);
    }
  }

  return (
    <AuthLayout
      side="default"
      title={t('auth.login.title')}
      subtitle={t('auth.login.subtitle')}
      footer={
        <>
          {t('common.noAccount')}?{' '}
          <Link to="/register" className="text-bitcoin-dark hover:underline font-medium">
            {t('common.signUp')}
          </Link>
        </>
      }
    >
      <form onSubmit={submit} className="space-y-4" noValidate>
        {error && (
          <div className="rounded-lg bg-rose-50 p-3 text-sm text-rose-700 border border-rose-200">
            {error}
          </div>
        )}
        {info && (
          <div className="rounded-lg bg-emerald-50 p-3 text-sm text-emerald-700 border border-emerald-200">
            {info}
          </div>
        )}

        <div>
          <label className="label" htmlFor="login-email">
            {t('auth.login.email')}
          </label>
          <input
            id="login-email"
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            disabled={needsCode || loading}
            required
            autoComplete="email"
            className="input"
            placeholder="name@example.com"
          />
        </div>

        <div>
          <PasswordInput
            id="login-password"
            label={t('auth.login.password')}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            disabled={needsCode || loading}
            required
            autoComplete="current-password"
            placeholder="••••••••"
          />
        </div>

        {needsCode && (
          <div>
            <label className="label" htmlFor="login-code">
              {t('auth.login.verificationCode')}
            </label>
            <input
              id="login-code"
              type="text"
              value={emailCode}
              onChange={(e) => setEmailCode(e.target.value)}
              disabled={loading}
              required
              className="input text-center tracking-widest font-mono text-lg"
              placeholder="000000"
              maxLength={6}
            />
          </div>
        )}

        <Turnstile
          ref={turnstileRef}
          action={LOGIN_CAPTCHA_ACTION}
          onVerify={(token) => setCaptchaToken(token)}
          onReset={() => setCaptchaToken(null)}
        />

        <button type="submit" disabled={loading} className="btn-primary w-full py-2.5 mt-2">
          {loading ? t('common.loading') : needsCode ? t('auth.login.verify') : t('common.signIn')}
        </button>
      </form>
    </AuthLayout>
  );
}
