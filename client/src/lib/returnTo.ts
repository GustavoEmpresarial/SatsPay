/**
 * Safe post-login redirect target.
 * Only same-origin relative paths are allowed (blocks open redirects).
 */
export function resolveReturnTo(
  raw: string | null | undefined,
  fallback = '/dashboard',
): string {
  if (!raw) return fallback;
  let decoded = raw;
  try {
    decoded = decodeURIComponent(raw);
  } catch {
    decoded = raw;
  }
  if (!decoded.startsWith('/')) return fallback;
  if (decoded.startsWith('//') || decoded.includes('://')) return fallback;
  if (decoded.includes('\\')) return fallback;
  return decoded;
}
