-- New live coins. ADD VALUE cannot be used in the same transaction as the
-- new enum labels, so wallet/HD backfill lives in 0017.
ALTER TYPE coin ADD VALUE 'DGB';
ALTER TYPE coin ADD VALUE 'SOL';
ALTER TYPE coin ADD VALUE 'USDT';
ALTER TYPE coin ADD VALUE 'USDC';
