/** Known production API/VM host — never run tests against it. */
export const PRODUCTION_DB_HOSTS = new Set(['169.58.45.155']);

const LOOPBACK_HOSTS = new Set(['localhost', '127.0.0.1', '::1']);

/**
 * Production compose default (`deploy/docker/docker-compose.yml`) is
 * POSTGRES_DB=bitcosats. A URL with that name on a non-loopback host is prod.
 */
const PRODUCTION_DB_NAME = 'bitcosats';

const TEST_NAME_PATTERN = /test|dev|ci|smoke/;

export function assertTestDatabaseUrl(url: string): URL {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    throw new Error('DATABASE_URL is not a valid URL');
  }

  if (parsed.protocol !== 'postgres:' && parsed.protocol !== 'postgresql:') {
    throw new Error('DATABASE_URL must be a postgres URL');
  }

  const host = parsed.hostname.toLowerCase();
  if (PRODUCTION_DB_HOSTS.has(host)) {
    throw new Error('DATABASE_URL points at the production host');
  }

  const dbName = parsed.pathname.replace(/^\//, '').split('?')[0] ?? '';
  const isLoopback = LOOPBACK_HOSTS.has(host);
  const nameLooksTest = TEST_NAME_PATTERN.test(dbName) || TEST_NAME_PATTERN.test(host);

  if (dbName === PRODUCTION_DB_NAME && !isLoopback) {
    throw new Error('DATABASE_URL uses the production database name on a remote host');
  }

  if (!isLoopback && !nameLooksTest) {
    throw new Error('DATABASE_URL must target a test database');
  }

  return parsed;
}
