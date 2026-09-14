import { useEffect, type ReactNode } from 'react';
import { createPortal } from 'react-dom';

export interface ModalProps {
  open: boolean;
  onClose?: () => void;
  children: ReactNode;
  /** Extra classes on the dimmed full-screen backdrop */
  backdropClassName?: string;
  /** Extra classes on the centered panel wrapper */
  panelClassName?: string;
  /** Close when clicking the dimmed area (default true) */
  closeOnBackdrop?: boolean;
  /** Accessible label for the dialog */
  'aria-label'?: string;
}

/**
 * Viewport-level modal. Always portals to `document.body` so overlays are not
 * trapped by layout ancestors (`overflow-x-hidden` on `<main>`, stacked under
 * the mobile header/nav). That trapping was showing a clear stripe on top of
 * the blurred backdrop across the app.
 */
export function Modal({
  open,
  onClose,
  children,
  backdropClassName = 'bg-black/65 backdrop-blur-sm',
  panelClassName = '',
  closeOnBackdrop = true,
  'aria-label': ariaLabel = 'Dialog',
}: ModalProps) {
  useEffect(() => {
    if (!open) return;
    const prev = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose?.();
    };
    window.addEventListener('keydown', onKey);
    return () => {
      document.body.style.overflow = prev;
      window.removeEventListener('keydown', onKey);
    };
  }, [open, onClose]);

  if (!open || typeof document === 'undefined') return null;

  return createPortal(
    <div
      className="fixed inset-0 z-[100] flex items-center justify-center p-4"
      role="dialog"
      aria-modal="true"
      aria-label={ariaLabel}
    >
      <div
        className={`absolute inset-0 ${backdropClassName}`}
        onClick={closeOnBackdrop ? onClose : undefined}
        aria-hidden="true"
      />
      <div className={`relative z-[101] w-full max-h-[min(90vh,900px)] overflow-y-auto ${panelClassName}`}>
        {children}
      </div>
    </div>,
    document.body,
  );
}
