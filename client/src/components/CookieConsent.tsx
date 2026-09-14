import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { motion, AnimatePresence, useReducedMotion } from 'framer-motion';

const STORAGE_KEY = 'bitcosats_cookie_consent';

export function CookieConsent() {
  const [visible, setVisible] = useState(false);
  const prefersReduced = useReducedMotion();

  useEffect(() => {
    try {
      if (!localStorage.getItem(STORAGE_KEY)) setVisible(true);
    } catch {
      setVisible(true);
    }
  }, []);

  function dismiss(value: 'accepted' | 'essential') {
    try {
      localStorage.setItem(STORAGE_KEY, value);
    } catch {
      /* ignore */
    }
    setVisible(false);
  }

  return (
    <AnimatePresence>
      {visible && (
        <div className="pointer-events-none fixed inset-x-0 bottom-16 sm:bottom-4 z-40 flex justify-center px-3 sm:px-4 md:justify-end">
          <motion.div
            role="dialog"
            aria-modal="false"
            initial={prefersReduced ? false : { opacity: 0, y: 20, scale: 0.95 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={prefersReduced ? { opacity: 0 } : { opacity: 0, y: 15, scale: 0.95 }}
            transition={{ type: 'spring', stiffness: 400, damping: 30 }}
            className="pointer-events-auto flex items-center justify-between gap-3 w-full max-w-lg rounded-2xl border border-border/90 bg-paper/95 p-3 sm:p-3.5 shadow-2xl backdrop-blur-md"
          >
            <div className="flex items-center gap-2.5 min-w-0">
              <span className="text-base sm:text-lg shrink-0">🍪</span>
              <p className="text-[11px] sm:text-xs text-ink-muted leading-tight truncate sm:whitespace-normal">
                Usamos cookies essenciais para segurança.{' '}
                <Link
                  to="/cookies"
                  className="font-medium text-bitcoin-dark underline hover:text-bitcoin inline"
                >
                  Saiba mais
                </Link>
                .
              </p>
            </div>

            <div className="flex items-center gap-1.5 shrink-0">
              <button
                type="button"
                onClick={() => dismiss('accepted')}
                className="rounded-xl bg-bitcoin hover:bg-bitcoin-dark text-white font-bold text-[11px] sm:text-xs px-3 py-1.5 shadow-xs active:scale-95 transition-all"
              >
                Aceitar
              </button>
              <button
                type="button"
                onClick={() => dismiss('essential')}
                className="rounded-lg p-1 text-ink-muted hover:text-ink text-sm sm:hidden"
                aria-label="Fechar"
              >
                <i className="bi bi-x" />
              </button>
            </div>
          </motion.div>
        </div>
      )}
    </AnimatePresence>
  );
}
