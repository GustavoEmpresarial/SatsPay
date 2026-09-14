import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';

const LANGS = [
  { code: 'pt' as const, flag: '🇧🇷', label: 'PT' },
  { code: 'en' as const, flag: '🇺🇸', label: 'EN' },
];

export function LanguageSwitch() {
  const { i18n } = useTranslation();
  const current = i18n.resolvedLanguage?.startsWith('pt') ? 'pt' : 'en';

  return (
    <div
      role="group"
      aria-label="Language / Idioma"
      className="inline-flex items-center rounded-full border border-border/80 bg-surface/80 p-0.5 shadow-2xs"
    >
      {LANGS.map((lang) => {
        const active = current === lang.code;
        return (
          <button
            key={lang.code}
            type="button"
            onClick={() => {
              if (!active) void i18n.changeLanguage(lang.code);
            }}
            aria-pressed={active}
            title={lang.code === 'pt' ? 'Português' : 'English'}
            className={clsx(
              'inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-bold tracking-wider transition-all',
              active ? 'bg-paper text-ink shadow-sm' : 'text-ink-muted hover:text-ink',
            )}
          >
            <span className="text-sm leading-none" aria-hidden>
              {lang.flag}
            </span>
            <span className="uppercase">{lang.label}</span>
          </button>
        );
      })}
    </div>
  );
}
