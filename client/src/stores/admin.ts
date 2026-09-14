import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface AdminSession {
  id: string;
  email: string;
}

interface AdminState {
  admin: AdminSession | null;
  /** Short-lived JWT — memory only (never written to localStorage). */
  accessToken: string | null;
  setSession: (payload: { admin: AdminSession; accessToken: string; refreshToken?: string | null }) => void;
  logout: () => void;
}

type PersistedAdmin = { admin: AdminSession | null };

export const useAdminStore = create<AdminState>()(
  persist(
    (set) => ({
      admin: null,
      accessToken: null,
      setSession: ({ admin, accessToken }) => set({ admin, accessToken }),
      logout: () => set({ admin: null, accessToken: null }),
    }),
    {
      name: 'bitcosats-admin',
      version: 2,
      partialize: (state): PersistedAdmin => ({ admin: state.admin }),
      migrate: (persisted): PersistedAdmin => {
        const raw = persisted as { admin?: AdminSession | null } | null;
        return { admin: raw?.admin ?? null };
      },
    },
  ),
);
