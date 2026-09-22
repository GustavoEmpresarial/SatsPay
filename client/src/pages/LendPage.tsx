import { motion } from 'framer-motion';

/** Lend — manutenção (sem mercados / supply / borrow). */
export function LendPage() {
  return (
    <div className="mx-auto max-w-3xl space-y-6 pb-12">
      <div className="flex flex-col gap-4 rounded-2xl border border-amber-500/30 bg-gradient-to-r from-amber-500/10 via-amber-500/5 to-transparent p-4 sm:flex-row sm:items-center sm:justify-between sm:p-5">
        <div className="flex items-start gap-3 sm:items-center">
          <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-amber-500/20 text-xl font-bold text-amber-600">
            <i className="bi bi-tools" />
          </div>
          <div>
            <div className="flex flex-wrap items-center gap-2">
              <span className="rounded-md bg-amber-500 px-2 py-0.5 text-[10px] font-black uppercase tracking-wider text-white shadow-xs">
                Manutenção
              </span>
              <span className="text-xs font-bold text-ink">Mercado Aave V3 temporariamente indisponível</span>
            </div>
            <p className="mt-1 text-xs leading-relaxed text-ink-muted">
              Fornecimento, resgate, empréstimo e pagamento estão pausados. Saldo e posições existentes
              permanecem registrados no ledger; nenhuma ação on-chain nesta aba até a reabertura.
            </p>
          </div>
        </div>
        <span className="shrink-0 self-start whitespace-nowrap rounded-xl border border-amber-500/30 bg-surface px-3 py-1.5 text-xs font-bold text-amber-700 sm:self-center">
          Em Manutenção
        </span>
      </div>

      <motion.section
        initial={{ opacity: 0, y: 8 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.35 }}
        className="rounded-2xl border border-border bg-paper p-8 text-center shadow-xs sm:p-12"
      >
        <div className="mx-auto mb-5 flex h-16 w-16 items-center justify-center rounded-2xl bg-amber-500/10 text-3xl text-amber-600">
          <i className="bi bi-bank" />
        </div>
        <h1 className="text-2xl font-black tracking-tight text-ink sm:text-3xl">Empréstimos · Aave V3</h1>
        <p className="mx-auto mt-3 max-w-md text-sm leading-relaxed text-ink-muted">
          O mercado de liquidez Polygon (supply / borrow) está em manutenção. Volte em breve — a API
          também recusa novas operações enquanto esta aba estiver fechada.
        </p>
        <div className="mt-8 inline-flex items-center gap-2 rounded-full border border-amber-500/30 bg-amber-500/10 px-4 py-2 text-xs font-bold uppercase tracking-wider text-amber-700">
          <span className="h-2 w-2 animate-pulse rounded-full bg-amber-500" />
          Manutenção
        </div>
      </motion.section>
    </div>
  );
}
