-- Zero (ZER). ADD VALUE cannot be used in the same transaction as the
-- new enum label, so wallet/HD backfill lives in 0032.
ALTER TYPE coin ADD VALUE 'ZER';
