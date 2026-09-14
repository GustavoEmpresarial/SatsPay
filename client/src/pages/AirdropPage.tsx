import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import { api } from '../lib/api.js';

interface AirdropProfile {
  season_id: string;
  season_number: number;
  season_title: string;
  base_points: number;
  bonus_points: number;
  total_points: number;
  tier: string;
  multiplier: number;
  global_rank: number;
  total_participants: number;
  claimed: boolean;
  projected_reward_usd: string;
  days_remaining: number;
}

interface LeaderboardEntry {
  rank: number;
  user_id: string;
  username: string;
  tier: string;
  total_points: number;
  multiplier: number;
}

interface AirdropPointLog {
  id: string;
  activity_type: string;
  description: string;
  points: number;
  bonus_points: number;
  created_at: string;
}

interface TierInfo {
  name: string;
  minPoints: number;
  maxPoints: number | null;
  multiplier: number;
  badge: string;
  border: string;
  bg: string;
  textColor: string;
  icon: string;
  tag: string;
  perks: string[];
}

const DEFAULT_TIER: TierInfo = {
  name: 'BRONZE',
  minPoints: 0,
  maxPoints: 49999,
  multiplier: 1.0,
  badge: 'bg-amber-700 text-white',
  border: 'border-amber-700/30',
  bg: 'border-amber-700/20 bg-amber-700/5',
  textColor: 'text-amber-800',
  icon: 'bi-shield',
  tag: 'Nível Inicial',
  perks: ['1.00x Base em todos os pontos', 'Acesso ao Faucet a cada 11h', 'Alocação base no Airdrop'],
};

const TIERS_CONFIG: TierInfo[] = [
  DEFAULT_TIER,
  {
    name: 'SILVER',
    minPoints: 50000,
    maxPoints: 249999,
    multiplier: 1.25,
    badge: 'bg-slate-400 text-white',
    border: 'border-slate-400/40',
    bg: 'border-slate-400/20 bg-slate-400/5',
    textColor: 'text-slate-600',
    icon: 'bi-shield-shaded',
    tag: '+25% Bônus',
    perks: ['1.25x Multiplicador (+25% bônus)', 'Prioridade em Faucet Claims', 'Maior cota no Airdrop $SATS'],
  },
  {
    name: 'GOLD',
    minPoints: 250000,
    maxPoints: 999999,
    multiplier: 1.5,
    badge: 'bg-amber-500 text-white',
    border: 'border-amber-500/40',
    bg: 'border-amber-500/20 bg-amber-500/5',
    textColor: 'text-amber-600',
    icon: 'bi-shield-fill',
    tag: '+50% Bônus',
    perks: ['1.50x Multiplicador (+50% bônus)', 'Bônus em Swaps e Trades', 'Alocação Acelerada de Tokens'],
  },
  {
    name: 'PLATINUM',
    minPoints: 1000000,
    maxPoints: 4999999,
    multiplier: 2.0,
    badge: 'bg-indigo-500 text-white',
    border: 'border-indigo-500/40',
    bg: 'border-indigo-500/20 bg-indigo-500/5',
    textColor: 'text-indigo-600',
    icon: 'bi-gem',
    tag: '2x Dobro de Pontos',
    perks: ['2.00x Multiplicador (2x Pontos)', 'Taxas reduzidas na plataforma', 'Prioridade máxima no Snapshot'],
  },
  {
    name: 'DIAMOND',
    minPoints: 5000000,
    maxPoints: null,
    multiplier: 2.5,
    badge: 'bg-cyan-500 text-white',
    border: 'border-cyan-500/40',
    bg: 'border-cyan-500/20 bg-cyan-500/5',
    textColor: 'text-cyan-600',
    icon: 'bi-diamond-fill',
    tag: '2.5x Máximo VIP',
    perks: ['2.50x Multiplicador Máximo', 'Cota VIP da pool de $SATS', 'Acesso antecipado a novos recursos'],
  },
];

export function AirdropPage() {
  const { data: profile } = useQuery<AirdropProfile>({
    queryKey: ['airdrop-overview'],
    queryFn: () => api<AirdropProfile>('/airdrop/overview'),
    refetchInterval: 15_000,
  });

  const { data: lbData, isLoading: isLbLoading } = useQuery<{ leaderboard: LeaderboardEntry[] }>({
    queryKey: ['airdrop-leaderboard'],
    queryFn: () => api<{ leaderboard: LeaderboardEntry[] }>('/airdrop/leaderboard'),
    refetchInterval: 15_000,
  });

  const { data: logsData, isLoading: isLogsLoading } = useQuery<{ logs: AirdropPointLog[] }>({
    queryKey: ['airdrop-logs'],
    queryFn: () => api<{ logs: AirdropPointLog[] }>('/airdrop/logs'),
    refetchInterval: 10_000,
  });

  const leaderboard = lbData?.leaderboard ?? [];
  const pointLogs = logsData?.logs ?? [];

  const currentTier = profile?.tier || 'BRONZE';
  const currentTierConfig: TierInfo = TIERS_CONFIG.find((t) => t.name === currentTier) ?? DEFAULT_TIER;

  const currentTierIndex = TIERS_CONFIG.findIndex((t) => t.name === currentTier);
  const nextTier = (currentTierIndex >= 0 && currentTierIndex < TIERS_CONFIG.length - 1) ? TIERS_CONFIG[currentTierIndex + 1] : undefined;

  const userPoints = profile?.total_points ?? 0;
  const prevTierMin = currentTierConfig.minPoints;
  const nextTierMin = nextTier ? nextTier.minPoints : userPoints;
  const pointsRemaining = nextTier ? Math.max(0, nextTier.minPoints - userPoints) : 0;
  
  const progressPct = nextTier
    ? Math.min(100, Math.max(0, ((userPoints - prevTierMin) / (nextTierMin - prevTierMin)) * 100))
    : 100;

  return (
    <div className="space-y-8 max-w-6xl mx-auto pb-16">
      {/* HERO BANNER */}
      <div className="relative overflow-hidden rounded-3xl border border-amber-500/30 bg-gradient-to-br from-amber-500/20 via-paper to-paper p-6 sm:p-10 shadow-sm">
        <div className="flex flex-col lg:flex-row lg:items-center lg:justify-between gap-6 relative z-10">
          <div className="space-y-3 max-w-2xl">
            <div className="flex items-center gap-2 flex-wrap">
              <span className="rounded-full bg-amber-500 text-white px-3 py-1 text-xs font-black uppercase tracking-wider shadow-xs">
                🎁 Programa de Airdrop & Recompensas
              </span>
            </div>
            <h1 className="text-3xl sm:text-4xl font-black text-ink tracking-tight">
              SatsPay Airdrop & Points Engine
            </h1>
            <p className="text-xs sm:text-sm text-ink-muted leading-relaxed">
              Participe da distribuição oficial de tokens <b>$SATS</b> da plataforma. Acumule <b>SatsPoints</b> ao utilizar faucets, realizar swaps, manter saldos e convidar novos usuários.
            </p>
          </div>
        </div>
      </div>

      {/* USER PROFILE & TIER CARD */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {/* Tier & Multiplier */}
        <div className={`rounded-2xl border p-5 shadow-xs space-y-2 ${currentTierConfig.bg}`}>
          <div className="flex items-center justify-between">
            <span className="text-xs font-bold text-ink-muted">Seu Tier Atual</span>
            <span className={`rounded-md px-2 py-0.5 text-[10px] font-black uppercase ${currentTierConfig.badge}`}>
              {profile?.multiplier.toFixed(2)}x Boost
            </span>
          </div>
          <div className={`text-2xl font-black ${currentTierConfig.textColor} flex items-center gap-2`}>
            <i className={`bi ${currentTierConfig.icon}`} />
            <span>Tier {currentTier}</span>
          </div>
          <p className="text-[11px] text-ink-muted">Multiplicador de pontos ativo</p>
        </div>

        {/* Total Points */}
        <div className="rounded-2xl border border-border bg-paper p-5 shadow-xs space-y-2">
          <span className="text-xs font-bold text-ink-muted">Total de SatsPoints</span>
          <div className="text-2xl font-black text-amber-600 font-mono">
            {profile?.total_points.toLocaleString() ?? 0} pts
          </div>
          <p className="text-[11px] text-ink-muted">
            Base: {profile?.base_points ?? 0} • Bônus: {profile?.bonus_points ?? 0}
          </p>
        </div>

        {/* Global Rank */}
        <div className="rounded-2xl border border-border bg-paper p-5 shadow-xs space-y-2">
          <span className="text-xs font-bold text-ink-muted">Posição no Ranking Global</span>
          <div className="text-2xl font-black text-purple-600 font-mono">
            #{profile?.global_rank ?? 1}
          </div>
          <p className="text-[11px] text-ink-muted">
            Entre {profile?.total_participants ?? 1} participantes ativos
          </p>
        </div>
      </div>

      {/* USER NEXT TIER PROGRESS BAR CARD */}
      <div className="rounded-3xl border border-border bg-paper p-6 shadow-xs space-y-4">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2">
          <div className="flex items-center gap-2.5">
            <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-bitcoin/10 text-bitcoin font-bold">
              <i className="bi bi-speedometer" />
            </div>
            <div>
              <h2 className="text-sm font-black text-ink uppercase tracking-wider">
                Progresso para o Próximo Nível
              </h2>
              <p className="text-xs text-ink-muted">
                {nextTier ? (
                  <>
                    Você está no nível <b className="text-ink font-bold">{currentTier}</b> ({profile?.multiplier.toFixed(2)}x). Faltam <b className="text-bitcoin-dark font-mono font-bold">{pointsRemaining.toLocaleString()} pts</b> para desbloquear <b className="text-ink font-bold">{nextTier.name}</b> ({nextTier.multiplier.toFixed(2)}x Boost)!
                  </>
                ) : (
                  <>Você alcançou o nível máximo <b>DIAMOND</b> com o multiplicador máximo de 2.50x!</>
                )}
              </p>
            </div>
          </div>

          <div className="text-right shrink-0">
            <span className="font-mono text-sm font-black text-ink">
              {userPoints.toLocaleString()} {nextTier ? `/ ${nextTier.minPoints.toLocaleString()} pts` : 'pts (Nível Máximo)'}
            </span>
          </div>
        </div>

        {/* Progress bar */}
        <div className="relative h-3.5 w-full overflow-hidden rounded-full bg-surface border border-border">
          <div
            className="h-full rounded-full bg-gradient-to-r from-amber-500 via-bitcoin to-emerald-500 transition-all duration-500"
            style={{ width: `${Math.round(progressPct)}%` }}
          />
        </div>

        <div className="flex items-center justify-between text-[11px] font-bold text-ink-muted">
          <span className="flex items-center gap-1">
            <span className={`h-2 w-2 rounded-full ${currentTierConfig.badge.split(' ')[0]}`} />
            {currentTier} ({currentTierConfig.minPoints.toLocaleString()} pts)
          </span>
          <span className="font-mono">{Math.round(progressPct)}% concluído</span>
          {nextTier ? (
            <span className="flex items-center gap-1 text-ink">
              <span className={`h-2 w-2 rounded-full ${nextTier.badge.split(' ')[0]}`} />
              Próximo: {nextTier.name} ({nextTier.minPoints.toLocaleString()} pts)
            </span>
          ) : (
            <span className="text-emerald-600 font-bold">Nível Máximo Conquistado 👑</span>
          )}
        </div>
      </div>

      {/* DETAILED TIERS BREAKDOWN (TÊTE-À-TÊTE) */}
      <div className="rounded-3xl border border-border bg-paper shadow-xs overflow-hidden">
        <div className="border-b border-border bg-surface/60 px-6 py-5 flex flex-col sm:flex-row sm:items-center justify-between gap-4">
          <div>
            <div className="inline-flex items-center gap-1.5 rounded-full bg-purple-500/10 px-3 py-1 text-xs font-black text-purple-600 mb-1.5">
              <i className="bi bi-layers-fill" /> Estrutura de Níveis & Multiplicadores
            </div>
            <h2 className="text-xl font-black text-ink tracking-tight">
              Tabela Explicativa de Níveis (Tiers)
            </h2>
            <p className="text-xs text-ink-muted mt-0.5 max-w-2xl leading-relaxed">
              Cada ação na plataforma gera <b>SatsPoints</b>. Conforme você acumula pontos, você sobe automaticamente de nível e passa a multiplicar todas as suas recompensas e sua cota no Airdrop.
            </p>
          </div>
          <div className="shrink-0 flex items-center gap-2">
            <span className="text-xs font-bold text-ink-muted bg-paper px-3 py-1.5 rounded-xl border border-border">
              5 Níveis Progressivos
            </span>
          </div>
        </div>

        {/* DESKTOP / TABLET: SPACIOUS CLEAN TABLE */}
        <div className="hidden md:block overflow-x-auto [scrollbar-width:none]">
          <table className="w-full text-left text-xs">
            <thead className="border-b border-border bg-surface/50 text-[10px] uppercase tracking-wider text-ink-muted font-bold">
              <tr>
                <th className="p-4 pl-6">Nível (Tier)</th>
                <th className="p-4">Faixa de Pontos</th>
                <th className="p-4">Multiplicador</th>
                <th className="p-4">Benefícios & Vantagens</th>
                <th className="p-4 pr-6 text-right">Seu Status</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border">
              {TIERS_CONFIG.map((t) => {
                const isCurrent = t.name === currentTier;
                const isUnlocked = userPoints >= t.minPoints;

                return (
                  <tr
                    key={t.name}
                    className={`transition-colors ${
                      isCurrent
                        ? 'bg-bitcoin/5 hover:bg-bitcoin/10 font-medium'
                        : isUnlocked
                        ? 'hover:bg-surface/40'
                        : 'opacity-70 hover:opacity-90 hover:bg-surface/30'
                    }`}
                  >
                    {/* Tier Name & Badge */}
                    <td className="p-4 pl-6">
                      <div className="flex items-center gap-3">
                        <div className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border ${t.border} ${t.bg}`}>
                          <i className={`bi ${t.icon} text-lg ${t.textColor}`} />
                        </div>
                        <div>
                          <div className={`text-sm font-black ${t.textColor} flex items-center gap-2`}>
                            <span>{t.name}</span>
                            {isCurrent && (
                              <span className="rounded-full bg-bitcoin px-2 py-0.2 text-[9px] font-black uppercase text-white shadow-2xs">
                                Atual
                              </span>
                            )}
                          </div>
                          <span className="text-[10px] font-semibold text-ink-muted uppercase tracking-wider">
                            {t.tag}
                          </span>
                        </div>
                      </div>
                    </td>

                    {/* Point Range */}
                    <td className="p-4 font-mono font-bold text-ink whitespace-nowrap">
                      {t.maxPoints !== null
                        ? `${t.minPoints.toLocaleString()} – ${t.maxPoints.toLocaleString()} pts`
                        : `${t.minPoints.toLocaleString()}+ pts`}
                    </td>

                    {/* Multiplier */}
                    <td className="p-4 whitespace-nowrap">
                      <span className={`inline-flex items-center gap-1 rounded-lg px-2.5 py-1 text-xs font-black uppercase ${t.badge}`}>
                        <i className="bi bi-lightning-charge-fill text-[10px]" />
                        {t.multiplier.toFixed(2)}x Boost
                      </span>
                    </td>

                    {/* Perks */}
                    <td className="p-4">
                      <ul className="space-y-1 text-[11px] text-ink-muted">
                        {t.perks.map((perk, pIdx) => (
                          <li key={pIdx} className="flex items-center gap-1.5">
                            <i className={`bi bi-check2 text-xs shrink-0 ${isUnlocked ? 'text-emerald-600 font-bold' : 'text-ink-muted'}`} />
                            <span>{perk}</span>
                          </li>
                        ))}
                      </ul>
                    </td>

                    {/* Status Button / Indicator */}
                    <td className="p-4 pr-6 text-right whitespace-nowrap">
                      {isCurrent ? (
                        <span className="inline-flex items-center gap-1.5 rounded-xl border border-bitcoin/40 bg-bitcoin/15 px-3 py-1.5 text-xs font-black text-bitcoin-dark shadow-2xs">
                          <span className="h-2 w-2 rounded-full bg-bitcoin animate-ping" />
                          Seu Nível Ativo
                        </span>
                      ) : isUnlocked ? (
                        <span className="inline-flex items-center gap-1 rounded-xl bg-emerald-500/10 px-3 py-1.5 text-xs font-black text-emerald-700">
                          <i className="bi bi-check-lg" /> Conquistado
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1 rounded-xl border border-border bg-surface px-3 py-1.5 text-xs font-bold text-ink-muted">
                          Faltam {(t.minPoints - userPoints).toLocaleString()} pts
                        </span>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>

        {/* MOBILE: SPACIOUS CARDS STACK */}
        <div className="md:hidden divide-y divide-border">
          {TIERS_CONFIG.map((t) => {
            const isCurrent = t.name === currentTier;
            const isUnlocked = userPoints >= t.minPoints;

            return (
              <div
                key={t.name}
                className={`p-5 space-y-3.5 ${
                  isCurrent ? 'bg-bitcoin/5' : isUnlocked ? 'bg-paper' : 'bg-surface/30 opacity-75'
                }`}
              >
                {/* Header Mobile Card */}
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2.5">
                    <div className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border ${t.border} ${t.bg}`}>
                      <i className={`bi ${t.icon} text-base ${t.textColor}`} />
                    </div>
                    <div>
                      <div className={`text-sm font-black ${t.textColor} flex items-center gap-1.5`}>
                        <span>{t.name}</span>
                        {isCurrent && (
                          <span className="rounded-full bg-bitcoin px-2 py-0.2 text-[9px] font-black uppercase text-white">
                            Atual
                          </span>
                        )}
                      </div>
                      <div className="text-[10px] font-mono font-bold text-ink-muted">
                        {t.maxPoints !== null
                          ? `${t.minPoints.toLocaleString()} – ${t.maxPoints.toLocaleString()} pts`
                          : `${t.minPoints.toLocaleString()}+ pts`}
                      </div>
                    </div>
                  </div>

                  <span className={`rounded-lg px-2 py-1 text-[11px] font-black uppercase ${t.badge}`}>
                    {t.multiplier.toFixed(2)}x
                  </span>
                </div>

                {/* Perks Mobile */}
                <div className="rounded-xl border border-border/70 bg-surface/60 p-3 space-y-1.5 text-[11px] text-ink-muted">
                  {t.perks.map((perk, pIdx) => (
                    <div key={pIdx} className="flex items-start gap-1.5">
                      <i className={`bi bi-check2 text-xs shrink-0 mt-0.5 ${isUnlocked ? 'text-emerald-600 font-bold' : 'text-ink-muted'}`} />
                      <span>{perk}</span>
                    </div>
                  ))}
                </div>

                {/* Status Footer Mobile */}
                <div>
                  {isCurrent ? (
                    <div className="w-full text-center rounded-xl border border-bitcoin/40 bg-bitcoin/15 py-2 text-xs font-black text-bitcoin-dark">
                      ★ Seu Nível Ativo Agora
                    </div>
                  ) : isUnlocked ? (
                    <div className="w-full text-center rounded-xl bg-emerald-500/10 py-1.5 text-xs font-black text-emerald-700">
                      ✓ Nível Conquistado
                    </div>
                  ) : (
                    <div className="w-full text-center rounded-xl border border-border bg-surface py-1.5 text-xs font-bold text-ink-muted">
                      Faltam {(t.minPoints - userPoints).toLocaleString()} pts para desbloquear
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* MISSIONS & TASKS TO MULTIPLY POINTS */}
      <div className="rounded-3xl border border-border bg-paper p-6 shadow-xs space-y-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2.5">
            <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-amber-500/10 text-amber-600 font-bold">
              <i className="bi bi-trophy-fill" />
            </div>
            <div>
              <h2 className="text-sm font-black text-ink uppercase tracking-wider">
                Missões para Multiplicar seus SatsPoints
              </h2>
              <p className="text-xs text-ink-muted">
                Complete atividades diárias na plataforma para subir de Tier e acumular mais tokens $SATS.
              </p>
            </div>
          </div>
        </div>

        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3 pt-2">
          <Link
            to="/faucet"
            className="rounded-2xl border border-border bg-surface p-4 space-y-2 hover:border-amber-500/40 hover:bg-paper transition-all group"
          >
            <div className="flex items-center justify-between">
              <i className="bi bi-droplet-fill text-bitcoin text-lg" />
              <span className="rounded-md bg-amber-500/10 text-amber-700 px-2 py-0.5 text-[10px] font-black">
                +50 pts/claim
              </span>
            </div>
            <div className="text-xs font-bold text-ink group-hover:text-amber-600 transition-colors">
              Fazer Claim no Faucet
            </div>
            <p className="text-[11px] text-ink-muted">Reivindique cripto gratuita e ganhe pontos.</p>
          </Link>

          <Link
            to="/swap"
            className="rounded-2xl border border-border bg-surface p-4 space-y-2 hover:border-amber-500/40 hover:bg-paper transition-all group"
          >
            <div className="flex items-center justify-between">
              <i className="bi bi-arrow-left-right text-emerald-600 text-lg" />
              <span className="rounded-md bg-emerald-500/10 text-emerald-700 px-2 py-0.5 text-[10px] font-black">
                +100 pts/trade
              </span>
            </div>
            <div className="text-xs font-bold text-ink group-hover:text-emerald-600 transition-colors">
              Executar Câmbio (Swap)
            </div>
            <p className="text-[11px] text-ink-muted">Converta entre BTC, LTC, DOGE e SOL.</p>
          </Link>

          <Link
            to="/referrals"
            className="rounded-2xl border border-border bg-surface p-4 space-y-2 hover:border-amber-500/40 hover:bg-paper transition-all group"
          >
            <div className="flex items-center justify-between">
              <i className="bi bi-people-fill text-purple-600 text-lg" />
              <span className="rounded-md bg-purple-500/10 text-purple-700 px-2 py-0.5 text-[10px] font-black">
                +50 pts/amigo
              </span>
            </div>
            <div className="text-xs font-bold text-ink group-hover:text-purple-600 transition-colors">
              Convidar Usuários
            </div>
            <p className="text-[11px] text-ink-muted">Receba 50 pts por cada indicado cadastrado.</p>
          </Link>

          <Link
            to="/wallets"
            className="rounded-2xl border border-border bg-surface p-4 space-y-2 hover:border-amber-500/40 hover:bg-paper transition-all group"
          >
            <div className="flex items-center justify-between">
              <i className="bi bi-wallet2 text-blue-600 text-lg" />
              <span className="rounded-md bg-blue-500/10 text-blue-700 px-2 py-0.5 text-[10px] font-black">
                +10 pts / $10
              </span>
            </div>
            <div className="text-xs font-bold text-ink group-hover:text-blue-600 transition-colors">
              Manter Saldo em Carteira
            </div>
            <p className="text-[11px] text-ink-muted">Pontos diários de fidelidade por saldo.</p>
          </Link>
        </div>
      </div>

      {/* POINTS AUDIT TRAIL / ACTIVITY LOG */}
      <div className="rounded-3xl border border-border bg-paper shadow-xs overflow-hidden">
        <div className="border-b border-border bg-surface/60 px-6 py-4 flex items-center justify-between">
          <h2 className="text-sm font-black text-ink uppercase tracking-wider flex items-center gap-2">
            <i className="bi bi-clock-history text-purple-600" />
            <span>Extrato & Rastreamento Detalhado de Pontos ($SATS)</span>
          </h2>
          <span className="text-[11px] text-ink-muted font-mono">Auditoria em Tempo Real</span>
        </div>

        {isLogsLoading ? (
          <div className="py-12 text-center text-ink-muted text-xs animate-pulse">
            Carregando extrato de atividades...
          </div>
        ) : pointLogs.length === 0 ? (
          <div className="py-12 text-center space-y-2">
            <p className="text-sm font-bold text-ink">Nenhum evento individual detalhado nesta temporada</p>
            <p className="text-xs text-ink-muted max-w-md mx-auto">
              Seus próximos claims no faucet (+50), conversões swap (+100) e novos amigos indicados (+50) serão listados aqui linha por linha com data e hora.
            </p>
          </div>
        ) : (
          <div className="overflow-x-auto [scrollbar-width:none]">
            <table className="w-full text-left text-xs">
              <thead className="border-b border-border bg-surface/50 text-[10px] uppercase tracking-wider text-ink-muted font-bold">
                <tr>
                  <th className="p-4">Data / Hora</th>
                  <th className="p-4">Ação</th>
                  <th className="p-4">Descrição da Atividade</th>
                  <th className="p-4 text-right">Pontos</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {pointLogs.map((log) => (
                  <tr key={log.id} className="hover:bg-surface/30">
                    <td className="p-4 font-mono text-[11px] text-ink-muted whitespace-nowrap">
                      {new Date(log.created_at).toLocaleString('pt-BR', {
                        day: '2-digit',
                        month: '2-digit',
                        year: 'numeric',
                        hour: '2-digit',
                        minute: '2-digit',
                        second: '2-digit',
                      })}
                    </td>
                    <td className="p-4 whitespace-nowrap">
                      <span className="rounded-md bg-purple-500/10 text-purple-700 px-2 py-0.5 text-[10px] font-black uppercase">
                        {log.activity_type}
                      </span>
                    </td>
                    <td className="p-4 font-medium text-ink">
                      {log.description}
                    </td>
                    <td className="p-4 text-right font-mono font-black text-amber-600 text-sm whitespace-nowrap">
                      +{log.points.toLocaleString()} pts
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* LEADERBOARD TABLE */}
      <div className="rounded-3xl border border-border bg-paper shadow-xs overflow-hidden">
        <div className="border-b border-border bg-surface/60 px-6 py-4 flex items-center justify-between">
          <h2 className="text-sm font-black text-ink uppercase tracking-wider flex items-center gap-2">
            <i className="bi bi-award-fill text-amber-500" />
            <span>Leaderboard Global — Top Participantes</span>
          </h2>
          <span className="text-[11px] text-ink-muted font-mono">Ranking Oficial</span>
        </div>

        {isLbLoading ? (
          <div className="py-16 text-center text-ink-muted text-xs animate-pulse">
            Carregando ranking...
          </div>
        ) : leaderboard.length === 0 ? (
          <div className="py-16 text-center space-y-2">
            <p className="text-sm font-bold text-ink">Nenhum participante pontuou ainda</p>
            <p className="text-xs text-ink-muted">Seja o primeiro a realizar uma atividade e liderar o ranking!</p>
          </div>
        ) : (
          <div className="overflow-x-auto [scrollbar-width:none]">
            <table className="w-full text-left text-xs">
              <thead className="border-b border-border bg-surface/50 text-[10px] uppercase tracking-wider text-ink-muted font-bold">
                <tr>
                  <th className="p-4">Rank</th>
                  <th className="p-4">Usuário</th>
                  <th className="p-4">Tier</th>
                  <th className="p-4">Boost</th>
                  <th className="p-4 text-right">Total SatsPoints</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {leaderboard.map((u) => {
                  const tConfig: TierInfo = TIERS_CONFIG.find((t) => t.name === u.tier) ?? DEFAULT_TIER;
                  return (
                    <tr key={u.user_id} className="hover:bg-surface/30">
                      <td className="p-4 font-mono font-bold text-ink">
                        {u.rank === 1 ? '🥇 #1' : u.rank === 2 ? '🥈 #2' : u.rank === 3 ? '🥉 #3' : `#${u.rank}`}
                      </td>
                      <td className="p-4 font-bold text-ink">
                        @{u.username}
                      </td>
                      <td className="p-4">
                        <span className={`rounded-md px-2 py-0.5 text-[10px] font-black uppercase ${tConfig.badge}`}>
                          {u.tier}
                        </span>
                      </td>
                      <td className="p-4 font-mono font-bold text-ink-muted">
                        {u.multiplier.toFixed(2)}x
                      </td>
                      <td className="p-4 text-right font-mono font-black text-amber-600 text-sm">
                        {u.total_points.toLocaleString()} pts
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
