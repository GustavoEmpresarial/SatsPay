import { useState } from 'react';
import { clsx } from 'clsx';

/**
 * Code presentation shared by the public API docs and the merchant panel.
 *
 * These lived privately inside `ApiDocsPage`; the merchant dashboard needed
 * the same look, and copying them would have guaranteed the two screens
 * drifted apart. Moved verbatim — no visual change to /docs.
 */
export type CodeLang = 'curl' | 'js' | 'python' | 'php' | 'go' | 'rust';

export function CodeBlock({ code, label }: { code: string; label?: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="relative overflow-hidden rounded-2xl border border-border/80 bg-[#090D16] text-white shadow-xl">
      {label && (
        <div className="flex items-center justify-between border-b border-white/10 bg-white/[0.04] px-4 py-2.5">
          <div className="flex items-center gap-2">
            <span className="flex h-2 w-2 rounded-full bg-emerald-500" />
            <span className="font-mono text-[11px] font-bold uppercase tracking-wider text-white/70">{label}</span>
          </div>
          <button
            type="button"
            onClick={() => {
              navigator.clipboard.writeText(code);
              setCopied(true);
              setTimeout(() => setCopied(false), 1500);
            }}
            className="flex items-center gap-1.5 rounded-lg bg-white/5 px-2.5 py-1 text-[11px] font-semibold text-white/80 hover:bg-white/15 hover:text-white transition-all active:scale-95"
          >
            <i className={`bi ${copied ? 'bi-check2 text-emerald-400 font-bold' : 'bi-clipboard'}`} />
            <span>{copied ? 'Copiado!' : 'Copiar'}</span>
          </button>
        </div>
      )}
      <pre className="overflow-x-auto p-4 sm:p-5 font-mono text-[12px] leading-relaxed text-emerald-400/95 [scrollbar-width:none]">
        {code}
      </pre>
    </div>
  );
}

export function MultiLangCodeBlock({ snippets }: { snippets: Partial<Record<CodeLang, string>> }) {
  const [lang, setLang] = useState<CodeLang>('curl');

  const allLanguages: { id: CodeLang; label: string; icon: string }[] = [
    { id: 'curl', label: 'cURL', icon: 'bi-terminal-fill' },
    { id: 'js', label: 'Node.js', icon: 'bi-filetype-js' },
    { id: 'python', label: 'Python', icon: 'bi-filetype-py' },
    { id: 'php', label: 'PHP', icon: 'bi-filetype-php' },
    { id: 'go', label: 'Go', icon: 'bi-box-seam' },
    { id: 'rust', label: 'Rust', icon: 'bi-gear-fill' },
  ];
  const languages = allLanguages.filter((l) => Boolean(snippets[l.id]));

  const currentCode = snippets[lang] || snippets.curl || Object.values(snippets)[0] || '';

  return (
    <div className="space-y-2.5">
      <div className="flex items-center gap-1.5 overflow-x-auto pb-1 [scrollbar-width:none]">
        {languages.map((l) => (
          <button
            key={l.id}
            type="button"
            onClick={() => setLang(l.id)}
            className={clsx(
              'flex items-center gap-1.5 rounded-xl px-3.5 py-1.5 text-xs font-bold transition-all',
              lang === l.id
                ? 'bg-bitcoin text-white shadow-md shadow-bitcoin/25 scale-102'
                : 'bg-surface text-ink-muted hover:bg-paper hover:text-ink border border-border',
            )}
          >
            <i className={`bi ${l.icon}`} />
            <span>{l.label}</span>
          </button>
        ))}
      </div>
      <CodeBlock label={languages.find((l) => l.id === lang)?.label.toUpperCase()} code={currentCode} />
    </div>
  );
}

export function MethodBadge({ method }: { method: 'GET' | 'POST' | 'DELETE' }) {
  const color =
    method === 'GET'
      ? 'bg-blue-500/15 text-blue-600 border-blue-500/30'
      : method === 'POST'
      ? 'bg-emerald-500/15 text-emerald-600 border-emerald-500/30'
      : 'bg-rose-500/15 text-rose-600 border-rose-500/30';
  return (
    <span className={clsx('rounded-lg border px-2.5 py-0.5 font-mono text-[11px] font-black tracking-wider', color)}>
      {method}
    </span>
  );
}

export function EndpointHeader({ method, path, title, badge }: { method: 'GET' | 'POST' | 'DELETE'; path: string; title: string; badge?: string }) {
  return (
    <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 border-b border-border pb-4">
      <div className="space-y-1">
        <div className="flex items-center gap-2.5 flex-wrap">
          <MethodBadge method={method} />
          <code className="rounded-xl border border-border bg-surface px-3 py-1 font-mono text-xs sm:text-sm font-bold text-ink">
            {path}
          </code>
          {badge && (
            <span className="rounded-full bg-bitcoin/10 px-2.5 py-0.5 text-[10px] font-extrabold text-bitcoin-dark">
              {badge}
            </span>
          )}
        </div>
        <h3 className="text-lg sm:text-xl font-black text-ink">{title}</h3>
      </div>
    </div>
  );
}

