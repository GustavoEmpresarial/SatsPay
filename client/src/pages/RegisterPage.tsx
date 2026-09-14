import { useState, useRef } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router-dom';
import { Trans, useTranslation } from 'react-i18next';
import { AuthLayout } from '../components/AuthLayout.js';
import { PasswordInput } from '../components/PasswordInput.js';
import { Turnstile, type TurnstileRef } from '../components/Turnstile.js';
import { REGISTER_CAPTCHA_ACTION } from '../lib/captchaActions.js';
import { api } from '../lib/api.js';
import { formatApiError } from '../lib/formatError.js';
import { useAuthStore } from '../stores/auth.js';
import { reportAuthFailure } from '../lib/reportError.js';
import { resolveReturnTo } from '../lib/returnTo.js';
import { passwordIssues, usernameIssue } from '../lib/authValidation.js';
import type { AuthLoginResponse } from '@/shared';

export function RegisterPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const setSession = useAuthStore((s) => s.setSession);

  const initialRef = searchParams.get('r') || searchParams.get('ref') || '';
  const returnTo = resolveReturnTo(searchParams.get('return_to'));
  const [username, setUsername] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [referralCode, setReferralCode] = useState(initialRef);
  const [acceptTerms, setAcceptTerms] = useState(false);
  const [captchaToken, setCaptchaToken] = useState<string | null>(null);
  const turnstileRef = useRef<TurnstileRef>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);

    const userErr = usernameIssue(username);
    if (userErr) {
      setError(t(`auth.register.errors.${userErr}`));
      return;
    }
    const issues = passwordIssues(password);
    if (issues.length) {
      setError(issues.map((k) => t(`auth.register.errors.${k}`)).join(' · '));
      return;
    }
    if (password !== confirmPassword) {
      setError(t('auth.register.errors.mismatch'));
      return;
    }
    if (!acceptTerms) {
      setError(t('auth.register.errors.terms'));
      return;
    }

    setLoading(true);
    try {
      const res = await api<AuthLoginResponse>('/auth/register', {
        method: 'POST',
        skipAuth: true,
        json: {
          username: username.trim(),
          email,
          password,
          confirmPassword,
          acceptTerms: true,
          referralCode: referralCode.trim() || undefined,
          captchaToken: captchaToken || undefined,
        },
      });
      setSession(res);
      navigate(returnTo);
    } catch (err) {
      const msg = formatApiError(err);
      setError(msg);
      setCaptchaToken(null);
      turnstileRef.current?.reset();
      reportAuthFailure('auth', `Register failed: ${msg}`, {
        hasCaptcha: Boolean(captchaToken),
      });
    } finally {
      setLoading(false);
    }
  }

  return (
    <AuthLayout
      side="register"
      title={t('auth.register.title')}
      subtitle={t('auth.register.subtitle')}
      footer={
        <>
          {t('common.haveAccount')}?{' '}
          <Link to="/login" className="text-bitcoin-dark hover:underline">
            {t('common.signIn')}
          </Link>
        </>
      }
    >
      <form onSubmit={submit} className="space-y-4" noValidate>
        <div>
          <label className="label" htmlFor="register-username">
            {t('auth.register.username')}
          </label>
          <input
            id="register-username"
            className="input"
            type="text"
            required
            minLength={3}
            maxLength={24}
            autoComplete="username"
            spellCheck={false}
            value={username}
            onChange={(e) => setUsername(e.target.value)}
          />
          <p className="mt-1 text-xs text-ink-muted">{t('auth.register.usernameHint')}</p>
        </div>
        <div>
          <label className="label" htmlFor="register-email">
            {t('auth.login.email')}
          </label>
          <input
            id="register-email"
            className="input"
            type="email"
            required
            autoComplete="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
          />
        </div>
        <div>
          <label className="label" htmlFor="register-ref">
            Código de Indicação (Opcional)
          </label>
          <input
            id="register-ref"
            className="input font-mono text-xs"
            type="text"
            placeholder="Ex: elonmusk ou código do seu amigo"
            value={referralCode}
            onChange={(e) => setReferralCode(e.target.value)}
          />
        </div>
        <div>
          <PasswordInput
            id="register-password"
            label={t('auth.login.password')}
            required
            minLength={10}
            maxLength={128}
            autoComplete="new-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
          <p className="mt-1 text-xs text-ink-muted">{t('auth.register.passwordHint')}</p>
        </div>
        <PasswordInput
          id="register-confirm-password"
          label={t('auth.register.confirmPassword')}
          required
          minLength={10}
          maxLength={128}
          autoComplete="new-password"
          value={confirmPassword}
          onChange={(e) => setConfirmPassword(e.target.value)}
        />
        <label className="flex cursor-pointer items-start gap-2.5 text-sm text-ink-muted">
          <input
            id="register-accept-terms"
            type="checkbox"
            className="mt-0.5 h-4 w-4 shrink-0 rounded border-border text-bitcoin focus:ring-bitcoin"
            checked={acceptTerms}
            onChange={(e) => setAcceptTerms(e.target.checked)}
          />
          <span>
            <Trans
              i18nKey="auth.register.acceptTerms"
              components={{
                terms: (
                  <Link
                    to="/terms"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="font-medium text-bitcoin-dark underline-offset-2 hover:underline"
                  />
                ),
                privacy: (
                  <Link
                    to="/privacy"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="font-medium text-bitcoin-dark underline-offset-2 hover:underline"
                  />
                ),
              }}
            />
          </span>
        </label>
        {error && <div className="rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700">{error}</div>}
        <Turnstile
          ref={turnstileRef}
          action={REGISTER_CAPTCHA_ACTION}
          onVerify={(token) => setCaptchaToken(token)}
          onReset={() => setCaptchaToken(null)}
        />

        <button className="btn-primary w-full" disabled={loading || !acceptTerms}>
          {loading ? t('auth.register.submitting') : t('auth.register.submit')}
        </button>
      </form>
    </AuthLayout>
  );
}
