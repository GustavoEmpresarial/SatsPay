import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { useThemeStore, type ThemeMode } from '../stores/theme.js';

const MODES: {
  code: ThemeMode;
  icon: string;
  short: string;
  labelKey: 'themeLight' | 'themeDark' | 'themeBlack';
}[] = [
  { code: 'light', icon: 'bi-sun-fill', short: 'Light', labelKey: 'themeLight' },
  { code: 'dark', icon: 'bi-moon-stars-fill', short: 'Dark', labelKey: 'themeDark' },
  { code: 'black', icon: 'bi-circle-fill', short: 'Black', labelKey: 'themeBlack' },
];

export function ThemeToggle() {
  const { t } = useTranslation();
  const theme = useThemeStore((s) => s.theme);
  const setTheme = useThemeStore((s) => s.setTheme);

  return (
    <div
      role="group"
      aria-label={t('common.theme')}
      className="inline-flex items-center rounded-full border border-border/80 bg-surface/80 p-0.5 shadow-2xs"
    >
      {MODES.map((mode) => {
        const active = theme === mode.code;
        return (
          <button
            key={mode.code}
            type="button"
            onClick={() => {
              if (!active) setTheme(mode.code);
            }}
            aria-pressed={active}
            title={t(`common.${mode.labelKey}`)}
            className={clsx(
              'inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-bold tracking-wider transition-all',
              active ? 'bg-paper text-ink shadow-sm' : 'text-ink-muted hover:text-ink',
            )}
          >
            <i
              className={clsx(
                'bi text-sm leading-none',
                mode.icon,
                mode.code === 'black' && !active && 'opacity-70',
              )}
              aria-hidden
            />
            <span className="hidden uppercase sm:inline">{mode.short}</span>
          </button>
        );
      })}
    </div>
  );
}
