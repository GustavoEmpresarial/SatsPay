import { ApiError } from './api.js';

const ERROR_TRANSLATIONS: Record<string, string> = {
  unauthorized: 'Sessão expirada ou não autenticada. Por favor, atualize a página ou entre novamente.',
  'missing refresh token': 'Sessão expirada. Por favor, faça login novamente.',
  'refresh reuse detected': 'Sessão expirada por segurança. Entre novamente.',
  'invalid token': 'Token inválido. Entre novamente.',
  'captcha verification failed': 'Falha na verificação de segurança anti-bot. Complete a verificação novamente.',
  'action too fast': 'Ação muito rápida. Por favor, tente novamente.',
  'wallet not found': 'Carteira não encontrada para esta moeda.',
  'insufficient balance': 'Saldo insuficiente para realizar esta operação.',
  'platform inventory': 'Inventário da plataforma esgotado para esta moeda. Tente outra ou volte mais tarde.',
  'insufficient for this operation': 'Inventário da plataforma esgotado para esta moeda. Tente outra ou volte mais tarde.',
  'faucet paused': 'Faucet pausado: custo de rede acima da receita de taxas nesta moeda.',
  'network fees exceed': 'Faucet pausado: custo de rede acima da receita de taxas nesta moeda.',
  'unsupported media type': 'Requisição inválida (Content-Type). Atualize a página e tente novamente.',
  'next claim available': 'Aguarde o cooldown de 11 horas do faucet.',
  'aguarde o cooldown do faucet': 'Aguarde o cooldown de 11 horas do faucet.',
};

/**
 * Turn an ApiError into a readable message with Portuguese localization.
 */
export function formatApiError(err: unknown, fallback = 'Erro inesperado. Tente novamente.'): string {
  if (!(err instanceof ApiError)) {
    if (err instanceof Error) {
      const lower = err.message.toLowerCase();
      for (const [k, v] of Object.entries(ERROR_TRANSLATIONS)) {
        if (lower.includes(k.toLowerCase())) return v;
      }
      return err.message;
    }
    return fallback;
  }

  if (err.code === 'VALIDATION_ERROR' && err.details && typeof err.details === 'object') {
    const flat = err.details as { fieldErrors?: Record<string, string[]>; formErrors?: string[] };
    const fieldMsgs = Object.entries(flat.fieldErrors ?? {})
      .flatMap(([field, msgs]) => (msgs ?? []).map((m) => `${field}: ${m}`));
    const all = [...(flat.formErrors ?? []), ...fieldMsgs];
    if (all.length) return all.join(' · ');
  }

  const msgLower = (err.message || '').toLowerCase();
  for (const [k, v] of Object.entries(ERROR_TRANSLATIONS)) {
    if (msgLower === k.toLowerCase() || msgLower.includes(k.toLowerCase())) {
      return v;
    }
  }

  if (err.status === 401) {
    return 'Sessão expirada. Por favor, atualize a página ou entre novamente.';
  }

  if (err.status === 415) {
    return 'Requisição inválida (Content-Type). Atualize a página e tente novamente.';
  }

  return err.message || fallback;
}
