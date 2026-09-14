import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type ThemeMode = 'light' | 'dark' | 'black';

interface ThemeState {
  theme: ThemeMode;
  setTheme: (theme: ThemeMode) => void;
}

function isThemeMode(value: unknown): value is ThemeMode {
  return value === 'light' || value === 'dark' || value === 'black';
}

export function applyTheme(theme: ThemeMode): void {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;
  root.classList.remove('dark', 'theme-black');
  if (theme === 'dark') {
    root.classList.add('dark');
  } else if (theme === 'black') {
    root.classList.add('dark', 'theme-black');
  }
}

export const useThemeStore = create<ThemeState>()(
  persist(
    (set) => ({
      theme: 'light',
      setTheme: (theme) => {
        applyTheme(theme);
        set({ theme });
      },
    }),
    {
      name: 'satspay-theme',
      onRehydrateStorage: () => (state) => {
        applyTheme(state?.theme && isThemeMode(state.theme) ? state.theme : 'light');
      },
    },
  ),
);

if (typeof window !== 'undefined') {
  try {
    const raw = localStorage.getItem('satspay-theme');
    if (raw) {
      const parsed = JSON.parse(raw) as { state?: { theme?: unknown } };
      const theme = isThemeMode(parsed.state?.theme) ? parsed.state.theme : 'light';
      applyTheme(theme);
    } else {
      applyTheme('light');
    }
  } catch {
    applyTheme('light');
  }
}
