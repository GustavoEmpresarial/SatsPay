/**
 * Client-side registration validation (mirrors RegisterPage rules).
 * Keep in sync with server-side auth constraints where applicable.
 */

export function passwordIssues(password: string): string[] {
  const issues: string[] = [];
  if (password.length < 10) issues.push('minLength');
  if (password.length > 128) issues.push('maxLength');
  if (!/[A-Z]/.test(password)) issues.push('uppercase');
  if (!/[a-z]/.test(password)) issues.push('lowercase');
  if (!/[0-9]/.test(password)) issues.push('digit');
  return issues;
}

export function usernameIssue(username: string): string | null {
  const u = username.trim();
  if (u.length < 3 || u.length > 24) return 'usernameLength';
  if (!/^[a-zA-Z][a-zA-Z0-9_]*$/.test(u)) return 'usernameFormat';
  return null;
}

export function passwordsMatch(password: string, confirm: string): boolean {
  return password === confirm;
}
