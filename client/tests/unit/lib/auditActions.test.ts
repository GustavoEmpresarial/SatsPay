import { describe, expect, it } from 'vitest';
import { describeAuditAction } from '../../../src/lib/auditActions.js';

describe('describeAuditAction', () => {
  it('labels known security and money-movement events', () => {
    expect(describeAuditAction('AUTH_LOGIN_FAILED').label).toBe('Tentativa de login recusada');
    expect(describeAuditAction('FAUCET_CLAIM').label).toBe('Faucet reivindicado');
    expect(describeAuditAction('SWAP_EXECUTE').label).toBe('Câmbio executado');
    expect(describeAuditAction('WITHDRAWAL_FAILED').label).toBe('Saque falhou on-chain (estornado)');
  });

  it('passes unknown actions through', () => {
    expect(describeAuditAction('NEW_EVENT').label).toBe('NEW_EVENT');
  });
});
