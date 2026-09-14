import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type AccountMode = 'user' | 'merchant';

interface PrefsState {
  mode: AccountMode;
  setMode: (m: AccountMode) => void;
}

export const usePrefsStore = create<PrefsState>()(
  persist(
    (set) => ({
      mode: 'user',
      setMode: (m) => set({ mode: m }),
    }),
    { name: 'bitcosats-prefs' },
  ),
);
