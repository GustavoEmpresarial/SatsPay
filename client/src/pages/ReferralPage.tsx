import { useState, useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import QRCode from 'qrcode';
import { api } from '../lib/api.js';
import { useAuthStore } from '../stores/auth.js';

interface ReferralStats {
  referral_code: string;
  referral_link: string;
  total_referred: number;
  active_referred_24h: number;
  total_earned_usd: string;
  faucet_commission_pct: number;
  swap_commission_pct: number;
  merchant_commission_pct: number;
  earnings_by_coin: Array<{
    coin: string;
    total_amount: string;
    total_usd: string;
  }>;
}

interface CommissionItem {
  id: string;
  referred_id: string;
  activity_type: string;
  coin: string;
  amount: string;
  amount_usd: string;
  created_at: string;
}

interface ReferredUserItem {
  id: string;
  username: string;
  referral_code: string;
  is_active_24h: boolean;
  total_commissions_usd: string;
  joined_at: string;
}

export function ReferralPage() {
  const [copied, setCopied] = useState(false);
  const [activeTab, setActiveTab] = useState<'users' | 'commissions'>('users');
  const [qrDataUrl, setQrDataUrl] = useState<string | null>(null);
  const user = useAuthStore((s) => s.user);

  const { data: stats, isLoading: isStatsLoading } = useQuery<ReferralStats>({
    queryKey: ['referral-stats'],
    queryFn: async () => {
      const res = await api<ReferralStats>('/referral/stats');
      return res;
    },
  });

  const { data: usersData, isLoading: isUsersLoading } = useQuery<{ referred_users: ReferredUserItem[] }>({
    queryKey: ['referral-users'],
    queryFn: () => api<{ referred_users: ReferredUserItem[] }>('/referral/users'),
    refetchInterval: 10_000,
  });

  const { data: commissionsData, isLoading: isCommsLoading } = useQuery<{ commissions: CommissionItem[] }>({
    queryKey: ['referral-commissions'],
    queryFn: () => api<{ commissions: CommissionItem[] }>('/referral/commissions'),
    refetchInterval: 10_000,
  });

  const referredUsers = usersData?.referred_users ?? [];

  const activeCode = useMemo(() => {
    return stats?.referral_code || user?.username || user?.email?.split('@')[0] || user?.id?.substring(0, 8) || 'convite';
  }, [stats?.referral_code, user]);

  const liveLink = useMemo(() => {
    const origin = typeof window !== 'undefined' ? window.location.origin : 'https://www.satspay.pro';
    return `${origin}/r/${activeCode}`;
  }, [activeCode]);

  // Generate QR code when liveLink is ready
  useMemo(() => {
    if (liveLink) {
      QRCode.toDataURL(liveLink, { width: 220, margin: 2, color: { dark: '#0a0b0d', light: '#ffffff' } })
        .then(setQrDataUrl)
        .catch(() => {});
    }
  }, [liveLink]);

  const commissions = commissionsData?.commissions ?? [];

  const handleCopy = () => {
    navigator.clipboard.writeText(liveLink);
    setCopied(true);
    setTimeout(() => setCopied(false), 3000);
  };

  const shareText = encodeURIComponent(
    `Use a SatsPay para micropagamentos e swaps instantâneos de BTC, LTC, DOGE, SOL e USDT! Cadastre-se pelo meu link e ganhe bônus de entrada: ${liveLink}`,
  );

  return (
    <div className="space-y-8 max-w-6xl mx-auto pb-16">
      {/* HEADER */}
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <div className="inline-flex items-center gap-2 rounded-full border border-purple-500/30 bg-purple-500/10 px-3 py-1 text-xs font-black text-purple-600">
            <i className="bi bi-people-fill" /> Programa de Afiliados & Indicações
          </div>
          <h1 className="mt-1 text-2xl sm:text-3xl font-black text-ink tracking-tight">
            Ganhe Comissões Cripto em Tempo Real
          </h1>
          <p className="text-xs sm:text-sm text-ink-muted">
            Indique amigos, investidores e comerciantes. Receba comissões automáticas creditadas no mesmo segundo na sua carteira.
          </p>
        </div>
      </div>

      {/* REFERRAL LINK & SHARE CARD */}
      <div className="rounded-3xl border border-purple-500/20 bg-gradient-to-br from-purple-500/10 via-paper to-paper p-6 sm:p-8 shadow-xs">
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-center">
          <div className="lg:col-span-8 space-y-4">
            <div className="flex items-center gap-2">
              <span className="rounded-md bg-purple-600 text-white px-2 py-0.5 text-[10px] font-black uppercase tracking-wider">
                Seu Link Exclusivo
              </span>
              <span className="text-xs font-bold text-ink-muted">
                Código: <b className="text-ink font-mono">{activeCode}</b>
              </span>
            </div>

            {/* Copy input */}
            <div className="flex items-center gap-2 bg-surface p-2 rounded-2xl border border-border">
              <input
                type="text"
                readOnly
                value={liveLink}
                className="w-full bg-transparent px-3 py-1 text-xs font-mono font-bold text-ink outline-none"
              />
              <button
                type="button"
                onClick={handleCopy}
                className="shrink-0 flex items-center gap-2 rounded-xl bg-purple-600 px-5 py-2.5 text-xs font-black text-white hover:bg-purple-700 transition-all shadow-xs active:scale-95"
              >
                <i className={copied ? 'bi bi-check2' : 'bi bi-copy'} />
                <span>{copied ? 'Copiado!' : 'Copiar Link'}</span>
              </button>
            </div>

            {/* Social Share Buttons */}
            <div className="pt-2 flex items-center gap-2.5 flex-wrap">
              <span className="text-xs font-bold text-ink-muted mr-1">Compartilhar:</span>
              <a
                href={`https://api.whatsapp.com/send?text=${shareText}`}
                target="_blank"
                rel="noreferrer"
                className="flex items-center gap-1.5 rounded-xl border border-border bg-surface px-3 py-1.5 text-xs font-bold text-ink hover:bg-paper hover:text-emerald-600 transition-all shadow-xs"
              >
                <i className="bi bi-whatsapp text-emerald-600" /> WhatsApp
              </a>
              <a
                href={`https://t.me/share/url?url=${encodeURIComponent(liveLink)}&text=${shareText}`}
                target="_blank"
                rel="noreferrer"
                className="flex items-center gap-1.5 rounded-xl border border-border bg-surface px-3 py-1.5 text-xs font-bold text-ink hover:bg-paper hover:text-blue-500 transition-all shadow-xs"
              >
                <i className="bi bi-telegram text-blue-500" /> Telegram
              </a>
              <a
                href={`https://twitter.com/intent/tweet?text=${shareText}`}
                target="_blank"
                rel="noreferrer"
                className="flex items-center gap-1.5 rounded-xl border border-border bg-surface px-3 py-1.5 text-xs font-bold text-ink hover:bg-paper hover:text-ink transition-all shadow-xs"
              >
                <i className="bi bi-twitter-x" /> X (Twitter)
              </a>
            </div>
          </div>

          {/* QR Code */}
          <div className="lg:col-span-4 flex flex-col items-center justify-center border-t lg:border-t-0 lg:border-l border-border pt-4 lg:pt-0 lg:pl-6">
            {qrDataUrl ? (
              <div className="rounded-2xl border border-border bg-paper p-2.5 shadow-xs">
                <img src={qrDataUrl} alt="Referral QR Code" className="h-32 w-32 object-contain" />
              </div>
            ) : (
              <div className="h-32 w-32 rounded-2xl bg-surface animate-pulse" />
            )}
            <span className="mt-2 text-[10px] font-bold text-ink-muted">Escaneie para cadastrar</span>
          </div>
        </div>
      </div>

      {/* COMMISSION RATES BANNER */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
        <div className="rounded-2xl border border-border bg-paper p-5 shadow-xs space-y-1">
          <div className="flex items-center justify-between text-ink-muted text-xs font-bold">
            <span>Torneiras (Faucet)</span>
            <i className="bi bi-droplet-fill text-bitcoin" />
          </div>
          <div className="text-2xl font-black text-ink">{stats?.faucet_commission_pct ?? 10}% de Comissão</div>
          <p className="text-[11px] text-ink-muted">Você ganha {stats?.faucet_commission_pct ?? 10}% de cada claim feito pelos seus indicados.</p>
        </div>

        <div className="rounded-2xl border border-border bg-paper p-5 shadow-xs space-y-1">
          <div className="flex items-center justify-between text-ink-muted text-xs font-bold">
            <span>Câmbio & Swaps</span>
            <i className="bi bi-arrow-left-right text-emerald-600" />
          </div>
          <div className="text-2xl font-black text-ink">{stats?.swap_commission_pct ?? 10}% das Taxas</div>
          <p className="text-[11px] text-ink-muted">{stats?.swap_commission_pct ?? 10}% do fee de cada conversão é creditado na hora.</p>
        </div>

        <div className="rounded-2xl border border-border bg-paper p-5 shadow-xs space-y-1">
          <div className="flex items-center justify-between text-ink-muted text-xs font-bold">
            <span>Gateways de Lojas</span>
            <i className="bi bi-shop text-purple-600" />
          </div>
          <div className="text-2xl font-black text-ink">0.10% do Volume</div>
          <p className="text-[11px] text-ink-muted">Receba comissão perpétua sobre o fluxo de comerciantes indicados.</p>
        </div>
      </div>

      {/* STATS OVERVIEW */}
      <div className="grid grid-cols-1 sm:grid-cols-4 gap-4">
        <div className="rounded-2xl border border-border bg-paper p-5 shadow-xs space-y-2">
          <span className="text-xs font-bold text-ink-muted">Total de Indicados</span>
          <div className="text-3xl font-black text-ink">{stats?.total_referred ?? 0}</div>
          <span className="inline-block rounded-full bg-emerald-500/10 px-2 py-0.5 text-[10px] font-extrabold text-emerald-700">
            {stats?.active_referred_24h ?? 0} ativos hoje
          </span>
        </div>

        <div className="rounded-2xl border border-border bg-paper p-5 shadow-xs space-y-2">
          <span className="text-xs font-bold text-ink-muted">Ganhos em Comissões (USD)</span>
          <div className="text-3xl font-black text-emerald-600 font-mono">
            ${stats?.total_earned_usd ? Number(stats.total_earned_usd).toFixed(2) : '0.00'}
          </div>
          <p className="text-[11px] text-ink-muted">Crédito direto na carteira</p>
        </div>

        <div className="rounded-2xl border border-border bg-paper p-5 shadow-xs space-y-2">
          <span className="text-xs font-bold text-ink-muted">Pontos de Indicação ($SATS)</span>
          <div className="text-3xl font-black text-amber-500 font-mono">
            +{(stats?.total_referred ?? 0) * 50} pts
          </div>
          <p className="text-[11px] text-ink-muted">+50 pts por cada amigo cadastrado</p>
        </div>

        <div className="rounded-2xl border border-border bg-paper p-5 shadow-xs space-y-2">
          <span className="text-xs font-bold text-ink-muted">Comissões Recebidas</span>
          <div className="text-3xl font-black text-purple-600 font-mono">
            {commissions.length}
          </div>
          <p className="text-[11px] text-ink-muted">
            {commissions.length > 0 ? 'Transações creditadas' : 'Aguardando atividades'}
          </p>
        </div>
      </div>

      {/* TABS CONTAINER: REFERRED USERS & COMMISSIONS */}
      <div className="rounded-3xl border border-border bg-paper shadow-xs overflow-hidden">
        <div className="border-b border-border bg-surface/60 px-6 py-3 flex items-center justify-between flex-wrap gap-3">
          <div className="flex items-center gap-2">
            <button
              onClick={() => setActiveTab('users')}
              className={`flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-black transition-all ${
                activeTab === 'users'
                  ? 'bg-purple-600 text-white shadow-xs'
                  : 'text-ink-muted hover:text-ink hover:bg-surface'
              }`}
            >
              <i className="bi bi-people-fill" />
              <span>Seus Indicados Cadastrados ({referredUsers.length})</span>
            </button>

            <button
              onClick={() => setActiveTab('commissions')}
              className={`flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-black transition-all ${
                activeTab === 'commissions'
                  ? 'bg-purple-600 text-white shadow-xs'
                  : 'text-ink-muted hover:text-ink hover:bg-surface'
              }`}
            >
              <i className="bi bi-clock-history" />
              <span>Histórico de Comissões ({commissions.length})</span>
            </button>
          </div>

          <span className="text-[11px] text-ink-muted font-mono">Atualiza a cada 10s</span>
        </div>

        {/* TAB 1: REFERRED USERS LIST */}
        {activeTab === 'users' && (
          isUsersLoading ? (
            <div className="py-16 text-center text-ink-muted text-xs animate-pulse">
              Carregando lista de indicados...
            </div>
          ) : referredUsers.length === 0 ? (
            <div className="py-16 text-center space-y-2">
              <div className="flex h-12 w-12 items-center justify-center rounded-2xl bg-purple-500/10 text-purple-600 text-2xl mx-auto">
                <i className="bi bi-person-plus-fill" />
              </div>
              <p className="text-sm font-bold text-ink">Nenhum indicado cadastrado ainda</p>
              <p className="text-xs text-ink-muted max-w-sm mx-auto">
                Compartilhe seu link exclusivo para convidar amigos e acompanhar o cadastro de cada um aqui!
              </p>
            </div>
          ) : (
            <div className="overflow-x-auto [scrollbar-width:none]">
              <table className="w-full text-left text-xs">
                <thead className="border-b border-border bg-surface/50 text-[10px] uppercase tracking-wider text-ink-muted font-bold">
                  <tr>
                    <th className="p-4">Usuário Indicado</th>
                    <th className="p-4">Data de Cadastro</th>
                    <th className="p-4">Status de Atividade</th>
                    <th className="p-4">Bônus Concedido</th>
                    <th className="p-4 text-right">Comissões Geradas</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-border">
                  {referredUsers.map((u) => (
                    <tr key={u.id} className="hover:bg-surface/30">
                      <td className="p-4 font-bold text-ink">
                        <div className="flex items-center gap-2.5">
                          <div className="flex h-8 w-8 items-center justify-center rounded-full bg-purple-500/10 text-purple-600 font-black text-xs">
                            {u.username.charAt(0).toUpperCase()}
                          </div>
                          <div>
                            <div className="font-bold text-ink text-xs">@{u.username}</div>
                            <div className="text-[10px] text-ink-muted font-normal font-mono">ID: {u.id.substring(0, 8)}...</div>
                          </div>
                        </div>
                      </td>
                      <td className="p-4 font-mono text-ink-muted whitespace-nowrap">
                        {new Date(u.joined_at).toLocaleString('pt-BR', {
                          day: '2-digit',
                          month: '2-digit',
                          year: 'numeric',
                          hour: '2-digit',
                          minute: '2-digit',
                        })}
                      </td>
                      <td className="p-4 whitespace-nowrap">
                        {u.is_active_24h ? (
                          <span className="rounded-full bg-emerald-500/10 px-2.5 py-0.5 text-[10px] font-black text-emerald-700">
                            ● Ativo hoje
                          </span>
                        ) : (
                          <span className="rounded-full bg-surface px-2.5 py-0.5 text-[10px] font-bold text-ink-muted border border-border">
                            Cadastrado
                          </span>
                        )}
                      </td>
                      <td className="p-4 font-mono font-bold text-amber-600 whitespace-nowrap">
                        +50 pts $SATS
                      </td>
                      <td className="p-4 text-right font-mono font-black text-emerald-600 whitespace-nowrap">
                        ${Number(u.total_commissions_usd).toFixed(2)} USD
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )
        )}

        {/* TAB 2: COMMISSIONS HISTORY */}
        {activeTab === 'commissions' && (
          isCommsLoading ? (
            <div className="py-16 text-center text-ink-muted text-xs animate-pulse">
              Carregando comissões...
            </div>
          ) : commissions.length === 0 ? (
            <div className="py-16 text-center space-y-2">
              <div className="flex h-12 w-12 items-center justify-center rounded-2xl bg-purple-500/10 text-purple-600 text-2xl mx-auto">
                <i className="bi bi-gift" />
              </div>
              <p className="text-sm font-bold text-ink">Nenhuma comissão recebida ainda</p>
              <p className="text-xs text-ink-muted max-w-sm mx-auto">
                Quando seus amigos fizerem claims no Faucet ou Swaps, as comissões cairão aqui em tempo real!
              </p>
            </div>
          ) : (
            <div className="overflow-x-auto [scrollbar-width:none]">
              <table className="w-full text-left text-xs">
                <thead className="border-b border-border bg-surface/50 text-[10px] uppercase tracking-wider text-ink-muted font-bold">
                  <tr>
                    <th className="p-4">Horário</th>
                    <th className="p-4">Atividade Geradora</th>
                    <th className="p-4">Moeda</th>
                    <th className="p-4">Montante Creditado</th>
                    <th className="p-4 text-right">Status</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-border">
                  {commissions.map((c) => (
                    <tr key={c.id} className="hover:bg-surface/30">
                      <td className="p-4 font-mono text-ink-muted whitespace-nowrap">
                        {new Date(c.created_at).toLocaleString('pt-BR')}
                      </td>
                      <td className="p-4 font-bold text-ink">
                        <span className="rounded-md bg-surface px-2 py-0.5 border border-border">
                          {c.activity_type}
                        </span>
                      </td>
                      <td className="p-4">
                        <span className="rounded-md bg-bitcoin/10 px-2 py-0.5 font-mono font-black text-bitcoin-dark">
                          {c.coin}
                        </span>
                      </td>
                      <td className="p-4 font-mono font-bold text-emerald-600">
                        +{c.amount} {c.coin}
                      </td>
                      <td className="p-4 text-right">
                        <span className="rounded-full bg-emerald-500/10 px-2 py-0.5 text-[10px] font-black text-emerald-700">
                          Creditado no Saldo
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )
        )}
      </div>
    </div>
  );
}
