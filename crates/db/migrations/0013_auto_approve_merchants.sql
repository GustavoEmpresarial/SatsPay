-- 0013_auto_approve_merchants.sql
-- Acesso de comerciante liberado imediatamente para todos os usuários sem exigência de aprovação

ALTER TABLE users ALTER COLUMN merchant_status SET DEFAULT 'APPROVED'::merchant_status;
UPDATE users SET merchant_status = 'APPROVED'::merchant_status;
