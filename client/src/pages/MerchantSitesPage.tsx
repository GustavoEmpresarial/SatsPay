import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { api } from '../lib/api.js';
import { formatApiError } from '../lib/formatError.js';
import { COINS, type Coin } from '@/shared';
import { clsx } from 'clsx';

interface FaucetSite {
  id: string;
  name: string;
  url: string;
  description: string;
  coins: Coin[];
  rewardInfo: string | null;
  status: 'PENDING' | 'APPROVED' | 'REJECTED' | 'SUSPENDED';
  rejectionReason: string | null;
  clicks: number;
}

export function MerchantSitesPage() {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const [name, setName] = useState('');
  const [url, setUrl] = useState('');
  const [description, setDescription] = useState('');
  const [rewardInfo, setRewardInfo] = useState('');
  const [coins, setCoins] = useState<Coin[]>(['USDT', 'BTC', 'SOL']);
  const [error, setError] = useState<string | null>(null);

  const sitesQ = useQuery({
    queryKey: ['my-faucet-sites'],
    queryFn: () => api<{ sites: FaucetSite[] }>('/faucetlist/mine'),
  });

  const create = useMutation({
    mutationFn: () =>
      api('/faucetlist', {
        method: 'POST',
        json: { name, url, description, coins, ...(rewardInfo ? { rewardInfo } : {}) },
      }),
    onSuccess: () => {
      setError(null);
      setName('');
      setUrl('');
      setDescription('');
      setRewardInfo('');
      setCoins(['USDT', 'BTC', 'SOL']);
      qc.invalidateQueries({ queryKey: ['my-faucet-sites'] });
    },
    onError: (e) => setError(formatApiError(e)),
  });

  const remove = useMutation({
    mutationFn: (id: string) => api(`/faucetlist/${id}`, { method: 'DELETE' }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['my-faucet-sites'] }),
  });

  function toggleCoin(c: Coin) {
    setCoins((prev) => (prev.includes(c) ? prev.filter((x) => x !== c) : [...prev, c]));
  }

  const sites = Array.isArray(sitesQ.data?.sites) ? sitesQ.data.sites : [];

  return (
    <div className="space-y-6 max-w-5xl mx-auto pb-12">
      {/* HEADER */}
      <div className="border-b border-border/80 pb-4">
        <div className="mb-1 flex items-center gap-2 text-xs font-bold uppercase tracking-wider text-bitcoin-dark">
          <i className="bi bi-shop-window" /> {t('merchantSites.badge')}
        </div>
        <h1 className="text-2xl sm:text-3xl font-black tracking-tight text-ink">
          {t('merchantSites.title')}
        </h1>
        <p className="text-xs sm:text-sm text-ink-muted mt-1">
          {t('merchantSites.subtitle')}
        </p>
      </div>

      {/* CADASTRO DE NOVO SITE */}
      <section className="rounded-3xl border border-border bg-paper p-5 sm:p-6 shadow-xs space-y-4">
        <div className="flex items-center justify-between border-b border-border/80 pb-3">
          <h2 className="flex items-center gap-2 text-xs sm:text-sm font-bold uppercase tracking-wider text-ink">
            <i className="bi bi-plus-circle-fill text-bitcoin" /> {t('merchantSites.submitNew')}
          </h2>
          <span className="text-[10px] font-bold uppercase text-emerald-600 bg-emerald-500/10 px-2.5 py-0.5 rounded-full">
            Publicação Imediata
          </span>
        </div>

        {error && (
          <div className="rounded-2xl border border-rose-500/20 bg-rose-500/10 p-3.5 text-xs font-bold text-rose-700">
            {error}
          </div>
        )}

        <div className="grid gap-4 md:grid-cols-2">
          <div>
            <label className="block text-xs font-bold text-ink mb-1">{t('merchantSites.siteName')}</label>
            <input
              className="input w-full text-xs"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t('merchantSites.siteNamePlaceholder')}
            />
          </div>
          <div>
            <label className="block text-xs font-bold text-ink mb-1">{t('merchantSites.url')}</label>
            <input
              className="input w-full font-mono text-xs"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder={t('merchantSites.urlPlaceholder')}
            />
          </div>
          <div className="md:col-span-2">
            <label className="block text-xs font-bold text-ink mb-1">{t('merchantSites.description')}</label>
            <textarea
              className="input w-full min-h-[70px] text-xs"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder={t('merchantSites.descPlaceholder')}
            />
          </div>
          <div>
            <label className="block text-xs font-bold text-ink mb-1">
              {t('merchantSites.rewardInfo')}
            </label>
            <input
              className="input w-full text-xs"
              value={rewardInfo}
              onChange={(e) => setRewardInfo(e.target.value)}
              placeholder={t('merchantSites.rewardPlaceholder')}
            />
          </div>
          <div>
            <label className="block text-xs font-bold text-ink mb-1">{t('merchantSites.coins')}</label>
            <div className="flex flex-wrap gap-1.5">
              {COINS.map((c) => {
                const on = coins.includes(c);
                return (
                  <button
                    key={c}
                    type="button"
                    onClick={() => toggleCoin(c)}
                    className={clsx(
                      'rounded-xl px-2.5 py-1 text-xs font-bold transition-all border',
                      on
                        ? 'bg-bitcoin text-white border-bitcoin shadow-xs'
                        : 'bg-surface text-ink-muted border-border hover:text-ink hover:bg-paper',
                    )}
                  >
                    {c}
                  </button>
                );
              })}
            </div>
          </div>
        </div>

        <div className="flex justify-end pt-2 border-t border-border/60">
          <button
            className="btn-primary text-xs font-bold px-6 py-2.5 rounded-xl flex items-center gap-2"
            disabled={
              create.isPending ||
              coins.length === 0 ||
              name.length < 2 ||
              url.length < 4 ||
              description.length < 10
            }
            onClick={() => create.mutate()}
          >
            {create.isPending ? (
              <>
                <span className="h-3.5 w-3.5 animate-spin rounded-full border-2 border-white border-t-transparent" />
                <span>{t('merchantSites.submitting')}</span>
              </>
            ) : (
              <>
                <i className="bi bi-check2-circle text-base" />
                <span>{t('merchantSites.submitCta')}</span>
              </>
            )}
          </button>
        </div>
      </section>

      {/* MEUS SITES CADASTRADOS */}
      <section className="rounded-3xl border border-border bg-paper overflow-hidden shadow-xs">
        <div className="border-b border-border bg-surface/40 p-4 sm:p-5 flex items-center justify-between">
          <h2 className="text-xs sm:text-sm font-bold text-ink flex items-center gap-2">
            <i className="bi bi-list-stars text-bitcoin" />
            <span>{t('merchantSites.mySubmissions')}</span>
          </h2>
          <span className="text-xs font-bold text-ink-muted">
            {sites.length}
          </span>
        </div>

        {sitesQ.isLoading && (
          <div className="p-4 space-y-2">
            <div className="h-16 animate-pulse rounded-2xl bg-surface" />
          </div>
        )}

        {!sitesQ.isLoading && sites.length === 0 && (
          <div className="p-10 text-center space-y-2">
            <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-2xl bg-surface text-ink-muted text-2xl">
              <i className="bi bi-shop" />
            </div>
            <p className="text-xs font-bold text-ink">{t('merchantSites.empty')}</p>
          </div>
        )}

        {sites.length > 0 && (
          <ul className="divide-y divide-border">
            {sites.map((s) => (
              <li
                key={s.id}
                className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 p-4 sm:p-5 hover:bg-surface/50 transition-colors"
              >
                <div className="space-y-1 min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-bold text-sm text-ink">{s.name}</span>
                    <span className="rounded-full bg-emerald-500/10 px-2.5 py-0.5 text-[10px] font-bold text-emerald-700 flex items-center gap-1">
                      <span className="h-1.5 w-1.5 rounded-full bg-emerald-500" />
                      {t('merchantSites.liveBadge')}
                    </span>
                    <span className="text-[11px] text-ink-muted font-medium">
                      · {s.clicks} {t('merchantSites.visits')}
                    </span>
                  </div>
                  <div className="truncate font-mono text-xs text-bitcoin-dark">{s.url}</div>
                  <p className="text-xs text-ink-muted line-clamp-1">{s.description}</p>
                </div>

                <div className="flex items-center gap-2 shrink-0 self-end sm:self-center">
                  <a
                    href={s.url}
                    target="_blank"
                    rel="noreferrer"
                    className="rounded-xl border border-border bg-paper hover:bg-surface px-3 py-1.5 text-xs font-bold text-ink transition-all flex items-center gap-1"
                  >
                    <i className="bi bi-box-arrow-up-right text-[11px]" />
                    <span>{t('merchantSites.visit')}</span>
                  </a>
                  <button
                    onClick={() => remove.mutate(s.id)}
                    disabled={remove.isPending}
                    className="rounded-xl border border-rose-500/20 bg-rose-500/5 hover:bg-rose-500/10 px-3 py-1.5 text-xs font-bold text-rose-700 transition-all flex items-center gap-1"
                  >
                    <i className="bi bi-trash text-xs" />
                    <span>{t('merchantSites.delete')}</span>
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
