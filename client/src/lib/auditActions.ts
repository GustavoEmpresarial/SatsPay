export interface AuditActionStyle {
  label: string;
  icon: string;
  badge: string;
}

const DEFAULT_STYLE: AuditActionStyle = {
  label: '',
  icon: 'bi-info-circle',
  badge: 'bg-slate-100 text-slate-700 dark:bg-slate-800 dark:text-slate-300',
};

const AUDIT_ACTION_STYLES: Record<string, AuditActionStyle> = {
  AUTH_LOGIN_SUCCESS: {
    label: 'Login realizado com sucesso',
    icon: 'bi-check-circle-fill text-emerald-600',
    badge: 'bg-emerald-100 text-emerald-700 dark:bg-emerald-950/40 dark:text-emerald-400',
  },
  AUTH_LOGIN_FAILED: {
    label: 'Tentativa de login recusada',
    icon: 'bi-exclamation-triangle-fill text-rose-600',
    badge: 'bg-rose-100 text-rose-700 dark:bg-rose-950/40 dark:text-rose-400',
  },
  AUTH_REGISTER: {
    label: 'Conta criada com sucesso',
    icon: 'bi-person-plus-fill text-blue-600',
    badge: 'bg-blue-100 text-blue-700 dark:bg-blue-950/40 dark:text-blue-400',
  },
  AUTH_REGISTER_FAILED: {
    label: 'Cadastro recusado',
    icon: 'bi-exclamation-triangle-fill text-rose-600',
    badge: 'bg-rose-100 text-rose-700 dark:bg-rose-950/40 dark:text-rose-400',
  },
  AUTH_LOGOUT: {
    label: 'Sessão encerrada',
    icon: 'bi-box-arrow-right text-amber-600',
    badge: 'bg-amber-100 text-amber-700 dark:bg-amber-950/40 dark:text-amber-400',
  },
  AUTH_UPDATE_USERNAME: {
    label: 'Nome de usuário atualizado',
    icon: 'bi-pencil-square text-purple-600',
    badge: 'bg-purple-100 text-purple-700 dark:bg-purple-950/40 dark:text-purple-400',
  },
  AUTH_2FA_CODE_SENT: {
    label: 'Código 2FA enviado',
    icon: 'bi-shield-lock-fill text-indigo-600',
    badge: 'bg-indigo-100 text-indigo-700 dark:bg-indigo-950/40 dark:text-indigo-400',
  },
  AUTH_ADMIN_LOGIN_SUCCESS: {
    label: 'Login administrativo',
    icon: 'bi-shield-check text-rose-600',
    badge: 'bg-rose-100 text-rose-700 dark:bg-rose-950/40 dark:text-rose-400',
  },
  WITHDRAWAL_REQUESTED: {
    label: 'Saque solicitado',
    icon: 'bi-arrow-up-right-circle-fill text-indigo-600',
    badge: 'bg-indigo-100 text-indigo-700 dark:bg-indigo-950/40 dark:text-indigo-400',
  },
  WITHDRAWAL_REQUEST_FAILED: {
    label: 'Saque recusado pelo sistema',
    icon: 'bi-x-circle-fill text-rose-600',
    badge: 'bg-rose-100 text-rose-700 dark:bg-rose-950/40 dark:text-rose-400',
  },
  WITHDRAWAL_FAILED: {
    label: 'Saque falhou on-chain (estornado)',
    icon: 'bi-x-octagon-fill text-rose-600',
    badge: 'bg-rose-100 text-rose-700 dark:bg-rose-950/40 dark:text-rose-400',
  },
  WITHDRAWAL_APPROVE: {
    label: 'Saque aprovado',
    icon: 'bi-check2-circle text-emerald-600',
    badge: 'bg-emerald-100 text-emerald-700 dark:bg-emerald-950/40 dark:text-emerald-400',
  },
  WITHDRAWAL_REJECT: {
    label: 'Saque rejeitado',
    icon: 'bi-slash-circle-fill text-amber-600',
    badge: 'bg-amber-100 text-amber-700 dark:bg-amber-950/40 dark:text-amber-400',
  },
  WALLET_INTERNAL_TRANSFER: {
    label: 'Transferência entre saldos',
    icon: 'bi-arrow-left-right text-cyan-600',
    badge: 'bg-cyan-100 text-cyan-700 dark:bg-cyan-950/40 dark:text-cyan-400',
  },
  FAUCET_CLAIM: {
    label: 'Faucet reivindicado',
    icon: 'bi-droplet-fill text-sky-600',
    badge: 'bg-sky-100 text-sky-700 dark:bg-sky-950/40 dark:text-sky-400',
  },
  FAUCET_CLAIM_FAILED: {
    label: 'Faucet recusado',
    icon: 'bi-droplet text-rose-600',
    badge: 'bg-rose-100 text-rose-700 dark:bg-rose-950/40 dark:text-rose-400',
  },
  SWAP_EXECUTE: {
    label: 'Câmbio executado',
    icon: 'bi-arrow-left-right text-emerald-600',
    badge: 'bg-emerald-100 text-emerald-700 dark:bg-emerald-950/40 dark:text-emerald-400',
  },
  SWAP_EXECUTE_FAILED: {
    label: 'Câmbio recusado',
    icon: 'bi-arrow-left-right text-rose-600',
    badge: 'bg-rose-100 text-rose-700 dark:bg-rose-950/40 dark:text-rose-400',
  },
};

export function describeAuditAction(action: string): AuditActionStyle {
  const known = AUDIT_ACTION_STYLES[action];
  if (known) return known;
  return { ...DEFAULT_STYLE, label: action };
}
