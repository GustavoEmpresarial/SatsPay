/** @vitest-environment jsdom */
import { beforeEach, describe, expect, it } from 'vitest';
import { useAuthStore } from '../../../src/stores/auth.js';
import { usePrefsStore } from '../../../src/stores/prefs.js';
import { applyTheme, useThemeStore } from '../../../src/stores/theme.js';
import { useAdminStore } from '../../../src/stores/admin.js';

describe('stores', () => {
  beforeEach(() => {
    useAuthStore.setState({ user: null, accessToken: null });
    usePrefsStore.setState({ mode: 'user' });
    useThemeStore.setState({ theme: 'light' });
  });

  it('auth setSession / updateUser / logout', () => {
    useAuthStore.getState().setSession({
      user: {
        id: '1',
        email: 'a@b.co',
        username: 'alice',
        twoFactorEnabled: false,
        merchantStatus: 'NONE',
        createdAt: 'x',
      },
      accessToken: 'tok',
    });
    expect(useAuthStore.getState().accessToken).toBe('tok');
    useAuthStore.getState().updateUser({ username: 'alice2' });
    expect(useAuthStore.getState().user?.username).toBe('alice2');
    useAuthStore.getState().logout();
    expect(useAuthStore.getState().user).toBeNull();
    expect(useAuthStore.getState().accessToken).toBeNull();
  });

  it('prefs mode toggles', () => {
    usePrefsStore.getState().setMode('merchant');
    expect(usePrefsStore.getState().mode).toBe('merchant');
  });

  it('theme applyTheme mutates documentElement', () => {
    applyTheme('dark');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    applyTheme('black');
    expect(document.documentElement.classList.contains('theme-black')).toBe(true);
    applyTheme('light');
    expect(document.documentElement.classList.contains('dark')).toBe(false);
    useThemeStore.getState().setTheme('dark');
    expect(useThemeStore.getState().theme).toBe('dark');
  });

  it('admin store setSession / logout', () => {
    useAdminStore.getState().setSession({
      admin: { id: '1', email: 'admin@test.com' },
      accessToken: 'adm-tok',
    });
    expect(useAdminStore.getState().admin?.email).toBe('admin@test.com');
    expect(useAdminStore.getState().accessToken).toBe('adm-tok');
    useAdminStore.getState().logout();
    expect(useAdminStore.getState().admin).toBeNull();
  });
});
