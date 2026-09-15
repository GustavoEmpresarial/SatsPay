import { COINS, type Coin } from '@/shared';

/**
 * Coin icons are served from this origin (`client/public/sdk/coins/`, also
 * exposed by `GET /v1/public/coins` as `logoUrl`) instead of a third-party
 * CDN: the hosted checkout should not depend on someone else's uptime to
 * render, and merchants need an icon URL they are allowed to hotlink.
 *
 * `/sdk/` is served with `Access-Control-Allow-Origin: *` (see
 * `client/nginx.conf`), so these are safe to embed cross-origin.
 */
const BASE = '/sdk/coins';

export function coinLogo(coin: Coin | string): string {
  const sym = String(coin || '').toUpperCase();
  // Only symbols we actually ship an icon for; anything else would 404 and
  // render as a broken image.
  const known = (COINS as readonly string[]).includes(sym);
  return `${BASE}/${known ? sym.toLowerCase() : 'generic'}.svg`;
}

/** Absolute variant, for anything rendered outside this origin (emails, SDK). */
export function coinLogoAbsolute(coin: Coin | string, origin: string): string {
  return `${origin.replace(/\/$/, '')}${coinLogo(coin)}`;
}
