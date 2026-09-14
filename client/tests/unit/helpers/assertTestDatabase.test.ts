import { describe, expect, it } from 'vitest';
import { assertTestDatabaseUrl, PRODUCTION_DB_HOSTS } from '../../helpers/assertTestDatabase.js';

describe('assertTestDatabaseUrl', () => {
  it('rejects the production VM host', () => {
    const [prodHost] = PRODUCTION_DB_HOSTS;
    expect(prodHost).toBeDefined();
    expect(() =>
      assertTestDatabaseUrl(`postgresql://bitcosats:secret@${prodHost}:5432/bitcosats`),
    ).toThrow(/production host/);
  });

  it('rejects the production database name on a remote host', () => {
    expect(() =>
      assertTestDatabaseUrl('postgresql://bitcosats:secret@db.example.com:5432/bitcosats'),
    ).toThrow(/production database name/);
  });

  it('rejects a remote URL that does not look like a test database', () => {
    expect(() =>
      assertTestDatabaseUrl('postgresql://app:secret@10.0.0.8:5432/wallet'),
    ).toThrow(/test database/);
  });

  it('accepts loopback test URLs', () => {
    const parsed = assertTestDatabaseUrl(
      'postgresql://bitcosats_test:bitcosats_test@127.0.0.1:5432/bitcosats_test',
    );
    expect(parsed.hostname).toBe('127.0.0.1');
    expect(parsed.pathname).toBe('/bitcosats_test');
  });

  it('rejects a non-postgres URL', () => {
    expect(() => assertTestDatabaseUrl('mysql://root@localhost/test')).toThrow(/postgres URL/);
  });
});
