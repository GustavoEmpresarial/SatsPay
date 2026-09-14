import { useEffect, useRef, useImperativeHandle, forwardRef } from 'react';

declare global {
  interface Window {
    turnstile?: {
      render: (
        container: HTMLElement | string,
        params: {
          sitekey: string;
          callback?: (token: string) => void;
          'error-callback'?: () => void;
          'expired-callback'?: () => void;
          theme?: 'light' | 'dark' | 'auto';
          size?: 'normal' | 'flexible' | 'compact';
          action?: string;
        },
      ) => string;
      reset: (widgetId?: string) => void;
      remove: (widgetId: string) => void;
    };
    onloadTurnstileCallback?: () => void;
  }
}

export const TURNSTILE_SITE_KEY = '0x4AAAAAAEswvRfYx6WLwWlB';

export interface TurnstileRef {
  reset: () => void;
}

export interface TurnstileProps {
  siteKey?: string;
  onVerify: (token: string) => void;
  onReset?: () => void;
  action?: string;
  theme?: 'light' | 'dark' | 'auto';
  size?: 'normal' | 'flexible' | 'compact';
}

export const Turnstile = forwardRef<TurnstileRef, TurnstileProps>(function Turnstile(
  {
    siteKey = TURNSTILE_SITE_KEY,
    onVerify,
    onReset,
    action,
    theme = 'auto',
    size = 'normal',
  },
  ref,
) {
  const containerRef = useRef<HTMLDivElement>(null);
  const widgetIdRef = useRef<string | null>(null);

  // Keep references to avoid re-triggering effect on every parent render
  const onVerifyRef = useRef(onVerify);
  onVerifyRef.current = onVerify;

  const onResetRef = useRef(onReset);
  onResetRef.current = onReset;

  useImperativeHandle(ref, () => ({
    reset: () => {
      if (widgetIdRef.current && window.turnstile) {
        try {
          window.turnstile.reset(widgetIdRef.current);
        } catch {
          // Ignored
        }
      }
    },
  }));

  useEffect(() => {
    let isMounted = true;

    function renderWidget() {
      if (!isMounted || !containerRef.current || !window.turnstile) return;
      if (widgetIdRef.current) {
        return; // Already rendered and active
      }

      try {
        containerRef.current.innerHTML = '';
        const id = window.turnstile.render(containerRef.current, {
          sitekey: siteKey,
          callback: (token: string) => {
            if (isMounted) {
              onVerifyRef.current(token);
            }
          },
          'error-callback': () => {
            if (isMounted && onResetRef.current) {
              onResetRef.current();
            }
          },
          'expired-callback': () => {
            if (isMounted && onResetRef.current) {
              onResetRef.current();
            }
          },
          theme,
          size,
          action,
        });
        widgetIdRef.current = id;
      } catch (err) {
        console.warn('Turnstile render warning:', err);
      }
    }

    function cleanup() {
      isMounted = false;
      if (widgetIdRef.current && window.turnstile) {
        try {
          window.turnstile.remove(widgetIdRef.current);
        } catch {
          // Ignored
        }
        widgetIdRef.current = null;
      }
    }

    if (window.turnstile) {
      renderWidget();
      return cleanup;
    }

    const scriptId = 'cf-turnstile-script';
    let script = document.getElementById(scriptId) as HTMLScriptElement | null;
    if (!script) {
      script = document.createElement('script');
      script.id = scriptId;
      script.src = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit';
      script.async = true;
      script.defer = true;
      document.head.appendChild(script);
    }

    const checkInterval = setInterval(() => {
      if (window.turnstile) {
        clearInterval(checkInterval);
        renderWidget();
      }
    }, 50);

    script.addEventListener('load', renderWidget);

    return () => {
      clearInterval(checkInterval);
      script?.removeEventListener('load', renderWidget);
      cleanup();
    };
  }, [siteKey, action, theme, size]);

  return (
    <div className="flex justify-center my-3 min-h-[65px] w-full overflow-hidden">
      <div ref={containerRef} className="w-full flex justify-center" />
    </div>
  );
});
