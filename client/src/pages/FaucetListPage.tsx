import { useState, useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { api } from '../lib/api.js';
import { COINS, isCoin } from '../shared/coins.js';
import { coinLogo } from '../lib/coinAssets.js';

interface FaucetSite {
  id: string;
  name: string;
  url: string;
  description: string;
  coins: string[];
  rewardInfo?: string;
  status: string;
  clicks: number;
  category?: string;
  timerInterval?: string;
  rating?: number;
  isOfficial?: boolean;
}

// Curated verified partners list to ensure the directory is always rich and active
const CURATED_DEFAULT_FAUCETS: FaucetSite[] = [
  {
    id: 'official-satspay',
    name: 'SatsPay Official Faucet',
    url: '/faucet',
    description: 'Torneira oficial e nativa da plataforma SatsPay. Créditos instantâneos sem intermediários.',
    coins: ['BTC', 'LTC', 'DOGE', 'BCH', 'POL', 'DGB', 'SOL', 'USDT', 'USDC', 'ZER'],
    rewardInfo: 'Micro-pagamentos instantâneos',
    timerInterval: 'A cada 11h',
    status: 'APPROVED',
    clicks: 1420,
    category: 'Faucets Clássicos',
    rating: 100,
    isOfficial: true,
  },
  {
    id: 'freebitcoin-partner',
    name: 'FreeBitco.in',
    url: 'https://freebitco.in',
    description: 'Uma das torneiras de Bitcoin mais antigas e confiáveis do mundo com sorteios e multiplicadores.',
    coins: ['BTC'],
    rewardInfo: 'Até $200 em Bitcoin grátis',
    timerInterval: 'A cada 60 min',
    status: 'APPROVED',
    clicks: 980,
    category: 'Faucets Clássicos',
    rating: 99,
  },
  {
    id: 'coinpayu-partner',
    name: 'CoinPayU Rewards',
    url: 'https://www.coinpayu.com',
    description: 'Ganhe criptomoedas visualizando anúncios PTC, completando pesquisas e reivindicando faucets.',
    coins: ['BTC', 'LTC', 'DOGE', 'SOL', 'USDT'],
    rewardInfo: 'Recompensas diárias múltiplas',
    timerInterval: 'A cada 60 min',
    status: 'APPROVED',
    clicks: 750,
    category: 'Micro-Tarefas & PTC',
    rating: 98,
  },
  {
    id: 'firefaucet-partner',
    name: 'FireFaucet Auto-Claim',
    url: 'https://firefaucet.win',
    description: 'Auto-faucet avançado com suporte a múltiplos ativos, níveis de fidelidade e bônus diários.',
    coins: ['BTC', 'LTC', 'DOGE', 'BCH', 'DGB', 'USDT'],
    rewardInfo: 'Auto-claim contínuo',
    timerInterval: 'Auto / Contínuo',
    status: 'APPROVED',
    clicks: 620,
    category: 'Auto-Faucets',
    rating: 97,
  },
  {
    id: 'faucetcrypto-partner',
    name: 'FaucetCrypto Gamified',
    url: 'https://faucetcrypto.com',
    description: 'Torneira gamificada com sistema de itens de RPG, níveis de experiência e multiplicadores.',
    coins: ['BTC', 'LTC', 'DOGE', 'POL', 'SOL', 'USDC'],
    rewardInfo: 'Pontos conversíveis em cripto',
    timerInterval: 'A cada 20 min',
    status: 'APPROVED',
    clicks: 580,
    category: 'Jogos & Recompensas',
    rating: 96,
  },
  {
    id: 'cointiply-partner',
    name: 'Cointiply Bitcoin Rewards',
    url: 'https://cointiply.com',
    description: 'Ganhe Bitcoin completando ofertas, jogando jogos, instalando apps e rodando a roleta da torneira.',
    coins: ['BTC', 'LTC', 'DOGE'],
    rewardInfo: 'Recompensas de alto valor',
    timerInterval: 'A cada 60 min',
    status: 'APPROVED',
    clicks: 510,
    category: 'Micro-Tarefas & PTC',
    rating: 98,
  },
  {
    id: 'dogebits-partner',
    name: 'DogeBits Instant Faucet',
    url: 'https://doge.satspay.pro',
    description: 'Torneira direta de Dogecoin integrada com a API SatsPay para pagamentos sem taxas de saque.',
    coins: ['DOGE', 'LTC'],
    rewardInfo: '0.001 - 0.05 DOGE / claim',
    timerInterval: 'A cada 15 min',
    status: 'APPROVED',
    clicks: 430,
    category: 'Faucets Clássicos',
    rating: 95,
  },
  {
    id: 'solanadrop-partner',
    name: 'Solana & Polygon MicroDrop',
    url: 'https://sol.satspay.pro',
    description: 'Torneira comunitária de alta velocidade focada em moedas de baixa taxa como SOL e POL.',
    coins: ['SOL', 'POL', 'USDC'],
    rewardInfo: 'Micro-frações instantâneas',
    timerInterval: 'A cada 30 min',
    status: 'APPROVED',
    clicks: 390,
    category: 'Faucets Clássicos',
    rating: 96,
  },
];

type SortBy = 'popular' | 'rating' | 'name';

function isSortBy(value: string): value is SortBy {
  return value === 'popular' || value === 'rating' || value === 'name';
}

function getSiteCoins(coins: unknown): string[] {
  if (!coins) return [];
  if (Array.isArray(coins)) return coins.map(String);
  if (typeof coins === 'string') {
    return coins.split(',').map((s) => s.trim()).filter(Boolean);
  }
  return [];
}

function faucetSitesFromPayload(data: { sites: FaucetSite[] } | FaucetSite[] | undefined): FaucetSite[] {
  if (!data) return [];
  const rawList: FaucetSite[] = Array.isArray(data) ? data : Array.isArray(data.sites) ? data.sites : [];
  return rawList.map((s) => ({
    ...s,
    coins: getSiteCoins(s.coins),
  }));
}

const CATEGORIES = [
  'Todos',
  'Faucets Clássicos',
  'Auto-Faucets',
  'Micro-Tarefas & PTC',
  'Jogos & Recompensas',
];

export function FaucetListPage() {
  const { t } = useTranslation();
  const [selectedCoin, setSelectedCoin] = useState<string>('ALL');
  const [selectedCategory, setSelectedCategory] = useState<string>('Todos');
  const [searchQuery, setSearchQuery] = useState('');
  const [sortBy, setSortBy] = useState<SortBy>('popular');

  const { data: dbData, isLoading } = useQuery({
    queryKey: ['faucet-sites-public'],
    queryFn: () => api<{ sites: FaucetSite[] } | FaucetSite[]>('/faucetlist'),
    staleTime: 30_000,
  });

  const allSites = useMemo(() => {
    const apiSites = faucetSitesFromPayload(dbData);
    const existingUrls = new Set(apiSites.map((s) => s.url.toLowerCase()));
    const nonDuplicateCurated = CURATED_DEFAULT_FAUCETS.filter(
      (c) => !existingUrls.has(c.url.toLowerCase()),
    );
    return [...apiSites, ...nonDuplicateCurated];
  }, [dbData]);

  // Filter and sort
  const filteredSites = useMemo(() => {
    return allSites
      .filter((site) => {
        // Coin filter
        if (selectedCoin !== 'ALL') {
          const hasCoin = site.coins?.some(
            (c) => c.toUpperCase() === selectedCoin.toUpperCase(),
          );
          if (!hasCoin) return false;
        }

        // Category filter
        if (selectedCategory !== 'Todos') {
          if (site.category && site.category !== selectedCategory) return false;
        }

        // Search query
        if (searchQuery.trim()) {
          const q = searchQuery.toLowerCase();
          const matchName = site.name.toLowerCase().includes(q);
          const matchDesc = site.description?.toLowerCase().includes(q);
          const matchCoins = site.coins?.some((c) => c.toLowerCase().includes(q));
          if (!matchName && !matchDesc && !matchCoins) return false;
        }

        return true;
      })
      .sort((a, b) => {
        if (a.isOfficial) return -1;
        if (b.isOfficial) return 1;

        if (sortBy === 'popular') {
          return (b.clicks || 0) - (a.clicks || 0);
        }
        if (sortBy === 'rating') {
          return (b.rating || 95) - (a.rating || 95);
        }
        if (sortBy === 'name') {
          return a.name.localeCompare(b.name);
        }
        return 0;
      });
  }, [allSites, selectedCoin, selectedCategory, searchQuery, sortBy]);

  return (
    <div className="space-y-8 max-w-6xl mx-auto pb-16">
      {/* HERO BANNER */}
      <div className="relative overflow-hidden rounded-3xl border border-blue-500/30 bg-gradient-to-br from-blue-500/15 via-paper to-paper p-6 sm:p-10 shadow-sm">
        <div className="flex flex-col lg:flex-row lg:items-center lg:justify-between gap-6 relative z-10">
          <div className="space-y-3 max-w-2xl">
            <div className="flex items-center gap-2 flex-wrap">
              <span className="rounded-full bg-blue-600 text-white px-3 py-1 text-xs font-black uppercase tracking-wider shadow-xs">
                {t('faucetlist.badge')}
              </span>
              <span className="rounded-full border border-border bg-surface/80 px-3 py-1 text-xs font-bold text-ink">
                {t('faucetlist.verifiedCount', { n: allSites.length })}
              </span>
            </div>
            <h1 className="text-3xl sm:text-4xl font-black text-ink tracking-tight">
              {t('faucetlist.heroTitle')}
            </h1>
            <p
              className="text-xs sm:text-sm text-ink-muted leading-relaxed"
              dangerouslySetInnerHTML={{ __html: t('faucetlist.heroBody') }}
            />
          </div>
        </div>
      </div>

      {/* FILTER & SEARCH TOOLBAR */}
      <div className="space-y-4 rounded-3xl border border-border bg-paper p-6 shadow-xs">
        {/* Row 1: Search & Sort */}
        <div className="flex flex-col sm:flex-row gap-3 items-center justify-between">
          <div className="relative w-full sm:max-w-md">
            <i className="bi bi-search absolute left-3.5 top-1/2 -translate-y-1/2 text-ink-muted text-sm" />
            <input
              type="text"
              placeholder={t('faucetlist.searchPlaceholder')}
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full rounded-xl border border-border bg-surface pl-10 pr-4 py-2.5 text-xs text-ink outline-none focus:border-blue-500 transition-colors"
            />
            {searchQuery && (
              <button
                type="button"
                onClick={() => setSearchQuery('')}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-xs text-ink-muted hover:text-ink"
              >
                <i className="bi bi-x-circle-fill" />
              </button>
            )}
          </div>

          <div className="flex items-center gap-2.5 w-full sm:w-auto justify-between sm:justify-end">
            <span className="text-xs font-bold text-ink-muted whitespace-nowrap">Ordenar:</span>
            <select
              value={sortBy}
              onChange={(e) => {
                if (isSortBy(e.target.value)) setSortBy(e.target.value);
              }}
              className="rounded-xl border border-border bg-surface px-3 py-2 text-xs font-bold text-ink outline-none cursor-pointer focus:border-blue-500"
            >
              <option value="popular">🔥 Mais Populares (Cliques)</option>
              <option value="rating">⭐ Maior Confiabilidade</option>
              <option value="name">🔤 Ordem Alfabética (A-Z)</option>
            </select>
          </div>
        </div>

        {/* Row 2: Coin Selector Pills */}
        <div>
          <div className="text-[11px] font-bold uppercase tracking-wider text-ink-muted mb-2">
            Filtrar por Criptomoeda
          </div>
          <div className="flex items-center gap-1.5 flex-wrap">
            <button
              type="button"
              onClick={() => setSelectedCoin('ALL')}
              className={`rounded-xl px-3 py-1.5 text-xs font-black transition-all ${
                selectedCoin === 'ALL'
                  ? 'bg-blue-600 text-white shadow-xs'
                  : 'border border-border bg-surface text-ink-muted hover:text-ink hover:bg-paper'
              }`}
            >
              Todas as Moedas
            </button>
            {COINS.map((c) => {
              const isSelected = selectedCoin === c;
              return (
                <button
                  key={c}
                  type="button"
                  onClick={() => setSelectedCoin(c)}
                  className={`flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-xs font-bold transition-all ${
                    isSelected
                      ? 'border-2 border-blue-500 bg-blue-500/10 text-blue-700 shadow-xs font-black'
                      : 'border border-border bg-surface text-ink hover:text-blue-600 hover:bg-paper'
                  }`}
                >
                  <img src={coinLogo(c)} alt={c} className="h-4 w-4 rounded-full object-contain" />
                  <span>{c}</span>
                </button>
              );
            })}
          </div>
        </div>

        {/* Row 3: Category Pills */}
        <div className="pt-2 border-t border-border/70 flex items-center gap-1.5 flex-wrap">
          {CATEGORIES.map((cat) => (
            <button
              key={cat}
              type="button"
              onClick={() => setSelectedCategory(cat)}
              className={`rounded-lg px-2.5 py-1 text-[11px] font-bold transition-colors ${
                selectedCategory === cat
                  ? 'bg-ink text-paper'
                  : 'text-ink-muted hover:text-ink bg-surface/60 hover:bg-surface'
              }`}
            >
              {cat}
            </button>
          ))}
        </div>
      </div>

      {/* SITES GRID */}
      {isLoading ? (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {[1, 2, 3, 4, 5, 6].map((i) => (
            <div key={i} className="rounded-3xl border border-border bg-paper p-6 h-52 animate-pulse" />
          ))}
        </div>
      ) : filteredSites.length === 0 ? (
        <div className="rounded-3xl border border-border bg-paper p-12 text-center space-y-3">
          <div className="flex h-12 w-12 items-center justify-center rounded-2xl bg-blue-500/10 text-blue-600 text-2xl mx-auto">
            <i className="bi bi-funnel" />
          </div>
          <p className="text-base font-bold text-ink">Nenhum faucet encontrado com esses filtros</p>
          <p className="text-xs text-ink-muted max-w-sm mx-auto">
            Tente remover os filtros de moeda ou limpar o termo de busca para visualizar todos os parceiros.
          </p>
          <button
            type="button"
            onClick={() => {
              setSelectedCoin('ALL');
              setSelectedCategory('Todos');
              setSearchQuery('');
            }}
            className="btn-secondary text-xs px-4 py-2"
          >
            Limpar Filtros
          </button>
        </div>
      ) : (
        <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
          {filteredSites.map((site) => {
            const isInternal = site.url.startsWith('/');

            return (
              <div
                key={site.id}
                className={`relative flex flex-col justify-between rounded-3xl border p-6 transition-all hover:shadow-md ${
                  site.isOfficial
                    ? 'border-bitcoin/50 bg-gradient-to-b from-bitcoin/10 via-paper to-paper ring-1 ring-bitcoin/30'
                    : 'border-border bg-paper hover:border-blue-500/40'
                }`}
              >
                {site.isOfficial && (
                  <div className="absolute -top-3 left-6 rounded-full bg-bitcoin px-3 py-0.5 text-[9px] font-black uppercase tracking-wider text-white shadow-xs">
                    👑 Torneira Oficial
                  </div>
                )}

                <div className="space-y-3.5">
                  {/* Top Header */}
                  <div className="flex items-start justify-between gap-3">
                    <div>
                      <h3 className="font-black text-base text-ink flex items-center gap-1.5">
                        <span>{site.name}</span>
                        <i className="bi bi-patch-check-fill text-blue-500 text-sm" title="Parceiro Verificado" />
                      </h3>
                      {site.category && (
                        <span className="inline-block mt-0.5 text-[10px] font-bold text-ink-muted">
                          {site.category}
                        </span>
                      )}
                    </div>

                    <span className="shrink-0 inline-flex items-center gap-1 rounded-full bg-emerald-500/10 px-2 py-0.5 text-[10px] font-black text-emerald-600">
                      <span className="h-1.5 w-1.5 rounded-full bg-emerald-500 animate-pulse" />
                      Pagando
                    </span>
                  </div>

                  {/* Description */}
                  <p className="text-xs text-ink-muted leading-relaxed line-clamp-2">
                    {site.description}
                  </p>

                  {/* Reward & Interval Badge */}
                  <div className="flex items-center gap-2 flex-wrap pt-1">
                    {site.timerInterval && (
                      <span className="rounded-lg border border-border bg-surface px-2 py-1 text-[10px] font-bold text-ink flex items-center gap-1">
                        <i className="bi bi-clock-history text-bitcoin" />
                        {site.timerInterval}
                      </span>
                    )}
                    {site.rewardInfo && (
                      <span className="rounded-lg bg-blue-500/10 px-2 py-1 text-[10px] font-black text-blue-600">
                        🎁 {site.rewardInfo}
                      </span>
                    )}
                  </div>

                  {/* Coins Supported */}
                  <div className="pt-2 border-t border-border/60">
                    <div className="text-[10px] font-bold uppercase tracking-wider text-ink-muted mb-1.5">
                      Moedas Suportadas ({getSiteCoins(site.coins).length}):
                    </div>
                    <div className="flex items-center gap-1 flex-wrap">
                      {getSiteCoins(site.coins).map((coinStr) => {
                        const raw = coinStr.toUpperCase();
                        const c = isCoin(raw) ? raw : undefined;
                        return (
                          <span
                            key={coinStr}
                            className="inline-flex items-center gap-1 rounded-md border border-border/80 bg-surface px-1.5 py-0.5 text-[10px] font-mono font-bold text-ink"
                            title={raw}
                          >
                            {c ? (
                              <img src={coinLogo(c)} alt={c} className="h-3 w-3 rounded-full object-contain" />
                            ) : (
                              <i className="bi bi-coin text-bitcoin" />
                            )}
                            <span>{raw}</span>
                          </span>
                        );
                      })}
                    </div>
                  </div>
                </div>

                {/* Card Action Footer */}
                <div className="pt-5 mt-4 border-t border-border/50 flex items-center justify-between gap-3">
                  <div className="text-[10px] text-ink-muted font-bold flex items-center gap-1">
                    <i className="bi bi-hand-thumbs-up-fill text-blue-500" />
                    <span>{site.rating || 98}% Confiabilidade</span>
                  </div>

                  {isInternal ? (
                    <Link
                      to={site.url}
                      className="inline-flex items-center gap-1.5 rounded-xl bg-bitcoin px-4 py-2 text-xs font-black text-white hover:bg-bitcoin-dark transition-all shadow-xs active:scale-95"
                    >
                      <span>Reivindicar Agora</span>
                      <i className="bi bi-arrow-right" />
                    </Link>
                  ) : (
                    <a
                      href={site.url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="inline-flex items-center gap-1.5 rounded-xl bg-blue-600 px-4 py-2 text-xs font-black text-white hover:bg-blue-700 transition-all shadow-xs active:scale-95"
                    >
                      <span>Visitar Faucet</span>
                      <i className="bi bi-box-arrow-up-right text-[10px]" />
                    </a>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* WEBMASTER & MERCHANT SUBMISSION CTA */}
      <div className="rounded-3xl border border-emerald-500/30 bg-gradient-to-br from-emerald-500/10 via-paper to-paper p-6 sm:p-8 shadow-xs">
        <div className="flex flex-col md:flex-row items-center justify-between gap-6">
          <div className="space-y-2 text-center md:text-left">
            <div className="inline-flex items-center gap-2 rounded-full border border-emerald-500/30 bg-emerald-500/10 px-3 py-1 text-xs font-black text-emerald-700">
              <i className="bi bi-shop" /> Para Criadores de Faucets & Webmasters
            </div>
            <h3 className="text-xl sm:text-2xl font-black text-ink">
              Quer listar sua Torneira ou Site no SatsPay?
            </h3>
            <p className="text-xs sm:text-sm text-ink-muted max-w-xl">
              Integre nossa API de micropagamentos instantâneos para pagar seus usuários sem tarifas de rede e receba milhares de acessos diários diretamente pelo nosso diretório público.
            </p>
          </div>

          <div className="flex flex-col sm:flex-row items-center gap-3 shrink-0 w-full md:w-auto">
            <Link
              to="/merchant/sites"
              className="w-full sm:w-auto text-center rounded-xl bg-emerald-600 px-5 py-3 text-xs font-black text-white hover:bg-emerald-700 transition-all shadow-sm active:scale-95"
            >
              <i className="bi bi-plus-circle-fill mr-1.5" /> {t('merchantSites.submitMine')}
            </Link>
            <Link
              to="/docs"
              className="w-full sm:w-auto text-center rounded-xl border border-border bg-paper px-4 py-3 text-xs font-bold text-ink hover:bg-surface transition-all shadow-2xs"
            >
              <i className="bi bi-code-slash mr-1.5" /> {t('faucetlist.docsCta')}
            </Link>
          </div>
        </div>
      </div>
    </div>
  );
}

