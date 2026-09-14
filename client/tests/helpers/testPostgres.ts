import { execFile as execFileCb } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import pg from 'pg';
import { assertTestDatabaseUrl } from './assertTestDatabase.js';

const execFile = promisify(execFileCb);

/** Same image as `deploy/docker/docker-compose.yml`. */
const POSTGRES_IMAGE = 'postgres:16-alpine';
/** Official listen port inside the postgres image. */
const POSTGRES_CONTAINER_PORT = 5432;
/** Healthcheck interval/timeout/retries from the same compose file. */
const HEALTHCHECK_INTERVAL = '10s';
const HEALTHCHECK_TIMEOUT = '5s';
const HEALTHCHECK_RETRIES = 8;
const HEALTHCHECK_POLL_MS = 10_000;

const TEST_DB_NAME = 'bitcosats_test';
const TEST_DB_USER = 'bitcosats_test';
const TEST_DB_PASSWORD = 'bitcosats_test';

const MIGRATIONS_DIR = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../../crates/db/migrations',
);

export interface TestPostgres {
  url: string;
  client: pg.Client;
  stop: () => Promise<void>;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

async function docker(args: string[]): Promise<string> {
  const { stdout } = await execFile('docker', args);
  return stdout.trim();
}

async function waitUntilHealthy(containerId: string): Promise<void> {
  const maxPolls = HEALTHCHECK_RETRIES;
  for (let i = 0; i < maxPolls; i += 1) {
    const status = await docker([
      'inspect',
      '-f',
      '{{.State.Health.Status}}',
      containerId,
    ]);
    if (status === 'healthy') return;
    const running = await docker(['inspect', '-f', '{{.State.Running}}', containerId]);
    if (running !== 'true') {
      throw new Error('test postgres container exited before becoming healthy');
    }
    await sleep(HEALTHCHECK_POLL_MS);
  }
  throw new Error('test postgres did not become healthy');
}

async function applyMigrations(client: pg.Client): Promise<void> {
  const existing = await client.query<{ ok: boolean }>(
    "SELECT to_regclass('public.users') IS NOT NULL AS ok",
  );
  if (existing.rows[0]?.ok) return;

  const files = (await readdir(MIGRATIONS_DIR))
    .filter((name) => name.endsWith('.sql'))
    .sort();
  for (const file of files) {
    const sql = await readFile(path.join(MIGRATIONS_DIR, file), 'utf8');
    await client.query(sql);
  }
}

async function startEphemeralPostgres(): Promise<{ url: string; stop: () => Promise<void> }> {
  const name = `bitcosats-vitest-pg-${randomUUID().slice(0, 8)}`;
  const containerId = await docker([
    'run',
    '-d',
    '--rm',
    '--name',
    name,
    '-e',
    `POSTGRES_USER=${TEST_DB_USER}`,
    '-e',
    `POSTGRES_PASSWORD=${TEST_DB_PASSWORD}`,
    '-e',
    `POSTGRES_DB=${TEST_DB_NAME}`,
    '-p',
    `127.0.0.1::${POSTGRES_CONTAINER_PORT}`,
    '--health-cmd',
    `pg_isready -U ${TEST_DB_USER} -d ${TEST_DB_NAME}`,
    '--health-interval',
    HEALTHCHECK_INTERVAL,
    '--health-timeout',
    HEALTHCHECK_TIMEOUT,
    '--health-retries',
    String(HEALTHCHECK_RETRIES),
    POSTGRES_IMAGE,
  ]);

  const stop = async () => {
    await docker(['stop', containerId]).catch(() => undefined);
  };

  try {
    await waitUntilHealthy(containerId);
    const mapping = await docker(['port', containerId, String(POSTGRES_CONTAINER_PORT)]);
    const hostPort = mapping.split(':').pop();
    if (!hostPort) throw new Error('could not resolve published postgres port');
    const url = `postgresql://${TEST_DB_USER}:${TEST_DB_PASSWORD}@127.0.0.1:${hostPort}/${TEST_DB_NAME}`;
    assertTestDatabaseUrl(url);
    return { url, stop };
  } catch (err) {
    await stop();
    throw err;
  }
}

export async function openTestPostgres(): Promise<TestPostgres> {
  const fromEnv = process.env.DATABASE_URL;
  let url: string;
  let stop: () => Promise<void>;

  if (fromEnv) {
    assertTestDatabaseUrl(fromEnv);
    url = fromEnv;
    stop = async () => undefined;
  } else {
    const ephemeral = await startEphemeralPostgres();
    url = ephemeral.url;
    stop = ephemeral.stop;
  }

  const client = new pg.Client({ connectionString: url });
  try {
    await client.connect();
    await applyMigrations(client);
    return { url, client, stop };
  } catch (err) {
    await client.end().catch(() => undefined);
    await stop();
    throw err;
  }
}
