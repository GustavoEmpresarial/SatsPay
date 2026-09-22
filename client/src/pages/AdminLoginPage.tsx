import { useState, useRef } from 'react';
import { Link, Navigate, useNavigate } from 'react-router-dom';
import { motion } from 'framer-motion';
import { api } from '../lib/api.js';
import { formatApiError } from '../lib/formatError.js';
import { useAdminStore } from '../stores/admin.js';
import { useAuthStore } from '../stores/auth.js';
import { ThemeToggle } from '../components/ThemeToggle.js';
import { Turnstile, type TurnstileRef } from '../components/Turnstile.js';
import { ADMIN_LOGIN_CAPTCHA_ACTION } from '../lib/captchaActions.js';

interface AdminLoginResp {
  user: { id: string; email: string; role: string };
  accessToken: string;
  refreshToken?: string;
}

/** Login do admin sempre em pt-BR. */
export function AdminLoginPage() {
  const navigate = useNavigate();
  const { admin, setSession } = useAdminStore();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [emailCode, setEmailCode] = useState('');
  const [codeSent, setCodeSent] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [captchaToken, setCaptchaToken] = useState<string | null>(null);
  const turnstileRef = useRef<TurnstileRef>(null);

  if (admin) return <Navigate to="/admin" replace />;

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setLoading(true);
    try {
      const res = await api<AdminLoginResp | { codeSent: true }>('/auth/admin/login', {
        method: 'POST',
        skipAuth: true,
        json: {
          email,
          password,
          emailCode: emailCode || undefined,
          ...(captchaToken ? { captchaToken } : {}),
        },
      });
      if ('codeSent' in res) {
        setCodeSent(true);
        setCaptchaToken(null);
        turnstileRef.current?.reset();
      } else {
        setSession({
          admin: { id: res.user.id, email: res.user.email },
          accessToken: res.accessToken,
        });
        useAuthStore.getState().setSession({
          user: {
            id: res.user.id,
            email: res.user.email,
            role: res.user.role as any,
            twoFactorEnabled: false,
            balance: 0,
            createdAt: new Date().toISOString(),
          } as any,
          accessToken: res.accessToken,
        });
        navigate('/admin');
      }
    } catch (err) {
      setError(formatApiError(err));
      setCaptchaToken(null);
      turnstileRef.current?.reset();
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="min-h-screen bg-canvas text-ink antialiased">
      <div className="absolute right-4 top-4 flex items-center gap-2">
        <ThemeToggle />
      </div>
      <div className="flex min-h-screen items-center justify-center p-4">
        <motion.div
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          className="w-full max-w-md"
        >
          <div className="mb-8 flex flex-col items-center gap-2">
            <div className="flex h-12 w-12 items-center justify-center rounded-2xl bg-gradient-to-br from-rose-500 to-rose-700 text-xl text-white shadow-xl shadow-rose-500/40">
              <i className="bi bi-shield-lock-fill" />
            </div>
            <div className="text-center">
              <div className="text-2xl font-bold">Painel Admin</div>
              <div className="text-xs uppercase tracking-widest text-ink-muted">SatsPay interno</div>
            </div>
          </div>

          <div className="rounded-2xl border border-border bg-paper p-8 shadow-card">
            <form onSubmit={submit} className="space-y-4">
              <div>
                <label className="mb-1 block text-sm font-medium text-ink">E-mail do admin</label>
                <input
                  type="email"
                  required
                  className="w-full rounded-lg border border-border bg-surface px-3 py-2 text-ink placeholder:text-ink-muted focus:border-rose-500 focus:outline-none"
                  placeholder="admin@empresa.com"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                />
              </div>
              <div>
                <label className="mb-1 block text-sm font-medium text-ink">Senha</label>
                <div className="relative">
                  <input
                    type={showPassword ? 'text' : 'password'}
                    required
                    className="w-full rounded-lg border border-border bg-surface px-3 py-2 pr-11 text-ink placeholder:text-ink-muted focus:border-rose-500 focus:outline-none"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                  />
                  <button
                    type="button"
                    tabIndex={-1}
                    aria-label={showPassword ? 'Ocultar senha' : 'Mostrar senha'}
                    onClick={() => setShowPassword((v) => !v)}
                    className="absolute inset-y-0 right-0 flex w-11 items-center justify-center text-ink-muted transition hover:text-ink"
                  >
                    <i className={`bi ${showPassword ? 'bi-eye-slash' : 'bi-eye'}`} aria-hidden />
                  </button>
                </div>
              </div>

              {codeSent && (
                <div>
                  <label className="mb-1 block text-sm font-medium text-ink">Código do email</label>
                  <input
                    inputMode="numeric"
                    maxLength={6}
                    required
                    className="w-full rounded-lg border border-border bg-surface px-3 py-2 font-mono tracking-widest text-ink placeholder:text-ink-muted focus:border-rose-500 focus:outline-none"
                    placeholder="000000"
                    value={emailCode}
                    onChange={(e) => setEmailCode(e.target.value.replace(/\D/g, ''))}
                  />
                  <p className="mt-1 text-xs text-ink-muted">
                    <i className="bi bi-envelope-check-fill mr-1 text-rose-400" /> Confira sua caixa de entrada
                  </p>
                </div>
              )}

              {error && (
                <div className="rounded-lg border border-rose-500/40 bg-rose-500/10 px-3 py-2 text-sm text-rose-600">
                  <i className="bi bi-exclamation-triangle-fill mr-1" />
                  {error}
                </div>
              )}

              <Turnstile
                ref={turnstileRef}
                action={ADMIN_LOGIN_CAPTCHA_ACTION}
                onVerify={(token) => setCaptchaToken(token)}
                onReset={() => setCaptchaToken(null)}
              />

              <button
                disabled={loading}
                className="w-full rounded-lg bg-gradient-to-br from-rose-500 to-rose-700 px-4 py-2.5 font-semibold text-white shadow-lg shadow-rose-500/30 transition-all hover:shadow-rose-500/50 disabled:opacity-50"
              >
                <i className="bi bi-shield-check mr-1.5" />
                {loading ? 'Autenticando…' : 'Entrar no admin'}
              </button>
            </form>

            <div className="mt-6 border-t border-border pt-4 text-center text-xs text-ink-muted">
              Não é admin?{' '}
              <Link to="/login" className="text-rose-400 hover:underline">
                Ir para login de usuário
              </Link>
            </div>
          </div>

          <p className="mt-4 text-center text-[10px] uppercase tracking-widest text-ink-muted">
            <i className="bi bi-lock-fill" /> Acesso restrito · Todas as ações são registradas
          </p>
        </motion.div>
      </div>
    </div>
  );
}
