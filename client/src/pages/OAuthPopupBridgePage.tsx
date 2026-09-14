import { useEffect, useMemo, useState } from 'react';
import { useSearchParams } from 'react-router-dom';

/**
 * COOP-safe popup completion page.
 *
 * Authorize navigates here when the flow ran inside a popup. We:
 * 1) postMessage the OAuth result to window.opener (if COOP left it intact)
 * 2) always fall back to the merchant redirect_url so their callback/bridge runs
 */
export function OAuthPopupBridgePage() {
  const [params] = useSearchParams();
  const [status, setStatus] = useState('Concluindo autenticação…');

  const payload = useMemo(() => {
    const redirectUrl = params.get('redirect_url') || '';
    const error = params.get('error') || '';
    let code: string | null = params.get('code');
    let state: string | null = params.get('state');
    if (redirectUrl && !code) {
      try {
        const u = new URL(redirectUrl);
        code = u.searchParams.get('code');
        state = state || u.searchParams.get('state');
      } catch {
        // ignore
      }
    }
    return { redirectUrl, error, code, state };
  }, [params]);

  useEffect(() => {
    let navigated = false;

    const goMerchant = () => {
      if (navigated) return;
      navigated = true;
      if (payload.redirectUrl) {
        window.location.replace(payload.redirectUrl);
      } else {
        setStatus('Autenticação concluída. Você pode fechar esta janela.');
        window.setTimeout(() => window.close(), 400);
      }
    };

    try {
      if (window.opener && !window.opener.closed) {
        // Prefer merchant redirect origin; never broadcast OAuth codes to '*'.
        let targetOrigin = '';
        try {
          if (payload.redirectUrl) targetOrigin = new URL(payload.redirectUrl).origin;
        } catch {
          targetOrigin = '';
        }

        if (targetOrigin) {
          if (payload.error) {
            window.opener.postMessage(
              { type: 'SATSPAY_AUTH_ERROR', error: payload.error },
              targetOrigin,
            );
          } else {
            window.opener.postMessage(
              {
                type: 'SATSPAY_AUTH_SUCCESS',
                code: payload.code,
                state: payload.state,
                redirect_url: payload.redirectUrl,
              },
              targetOrigin,
            );
          }
          setStatus('Login enviado ao site de origem…');
          window.setTimeout(() => {
            try {
              window.close();
            } catch {
              // ignore
            }
            // If COOP / browser blocks close, still send user to merchant callback.
            window.setTimeout(goMerchant, 250);
          }, 120);
          return;
        }
      }
    } catch {
      // opener blocked by COOP
    }

    setStatus('Redirecionando de volta ao aplicativo…');
    goMerchant();
  }, [payload]);

  return (
    <div className="flex min-h-screen items-center justify-center bg-canvas p-6 text-ink">
      <div className="max-w-sm text-center space-y-3">
        <img src="/sdk/satspay-logo.png" alt="" className="mx-auto h-10 w-10 object-contain" />
        <div className="h-6 w-6 mx-auto animate-spin rounded-full border-2 border-bitcoin border-t-transparent" />
        <p className="text-sm text-ink-muted">{status}</p>
      </div>
    </div>
  );
}
