import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { PublicUser } from '@/shared';

interface AuthSession {
  user?: PublicUser | null;
  accessToken?: string | null;
  /** @deprecated Ignored — refresh lives only in HttpOnly cookie. */
  refreshToken?: string | null;
  tokens?: { accessToken?: string | null; refreshToken?: string | null };
}

export interface AuthState {
  user: PublicUser | null;
  /** Short-lived JWT — memory only (never written to localStorage). */
  accessToken: string | null;
  setSession: (session: AuthSession) => void;
  updateUser: (user: Partial<PublicUser>) => void;
  logout: () => void;
}

type PersistedAuth = { user: PublicUser | null };

export const useAuthStore = create<AuthState>()(
  persist(
    (set) => ({
      user: null,
      accessToken: null,
      setSession: (session: AuthSession) =>
        set((state) => {
          const user = session.user ? { ...state.user, ...session.user } : state.user;
          const accessToken =
            session.accessToken || session.tokens?.accessToken || state.accessToken || null;
          return { user, accessToken };
        }),
      updateUser: (partial) =>
        set((state) => ({
          user: state.user ? { ...state.user, ...partial } : null,
        })),
      logout: () => set({ user: null, accessToken: null }),
    }),
    {
      name: 'bitcosats-auth',
      version: 2,
      // Only non-secret identity survives reloads. Tokens must come from cookie refresh.
      partialize: (state): PersistedAuth => ({ user: state.user }),
      migrate: (persisted): PersistedAuth => {
        const raw = persisted as { user?: PublicUser | null; accessToken?: unknown; refreshToken?: unknown } | null;
        return { user: raw?.user ?? null };
      },
    },
  ),
);
