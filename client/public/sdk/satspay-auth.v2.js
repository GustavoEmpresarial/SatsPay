/**
 * SatsPay Identity & OAuth 2.0 Web SDK
 * https://www.satspay.pro
 * Version: 2.1.0 (2026-09-08) — PKCE S256 + official SatsPay PNG logo (never Bitcoin SVG)
 *
 * Two flows (both supported):
 * 1) redirect (default) — full-page to SatsPay, then back to redirect_uri?code=…
 * 2) popup — centered window + postMessage to opener (falls back to redirect if blocked)
 *
 * Prefer the versioned URL so CDNs cannot stick on an old immutable copy:
 * <script src="https://www.satspay.pro/sdk/satspay-auth.v2.js" async defer></script>
 * <div class="satspay-signin"
 *      data-client_id="…"
 *      data-redirect_uri="https://seusite.com/auth/callback"
 *      data-mode="redirect"
 *      data-onsuccess="onSatsPaySignIn"></div>
 */

(function (window, document) {
  'use strict';

  var SATSPAY_BASE_URL = window.SATSPAY_AUTH_URL || 'https://www.satspay.pro';
  // Official brand mark (Lightning hex). Cache-busted query for CDN edges.
  var LOGO_URL = SATSPAY_BASE_URL + '/sdk/satspay-logo.png?v=3';

  function randomState() {
    try {
      var bytes = new Uint8Array(16);
      window.crypto.getRandomValues(bytes);
      return Array.prototype.map
        .call(bytes, function (b) {
          return ('0' + b.toString(16)).slice(-2);
        })
        .join('');
    } catch (e) {
      return String(Date.now()) + Math.random().toString(36).slice(2);
    }
  }

  function base64UrlEncode(buf) {
    var str = '';
    var bytes = new Uint8Array(buf);
    for (var i = 0; i < bytes.length; i++) str += String.fromCharCode(bytes[i]);
    return btoa(str).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
  }

  function randomVerifier() {
    var bytes = new Uint8Array(32);
    try {
      window.crypto.getRandomValues(bytes);
    } catch (e) {
      for (var i = 0; i < bytes.length; i++) bytes[i] = (Math.random() * 256) | 0;
    }
    return base64UrlEncode(bytes.buffer);
  }

  function sha256Base64Url(plain) {
    var data = new TextEncoder().encode(plain);
    return window.crypto.subtle.digest('SHA-256', data).then(function (hash) {
      return base64UrlEncode(hash);
    });
  }

  function pkceStorageKey(state) {
    return 'satspay_pkce_' + String(state || '');
  }

  function invokeCallback(cb, payload) {
    if (typeof cb === 'function') {
      cb(payload);
      return;
    }
    if (typeof cb === 'string' && typeof window[cb] === 'function') {
      window[cb](payload);
    }
  }

  var SatsPay = {
    config: {
      clientId: '',
      redirectUri: '',
      scope: 'openid profile email',
      state: '',
      /** 'redirect' (recommended) | 'popup' */
      mode: 'redirect',
    },

    init: function (options) {
      if (typeof options === 'object' && options) {
        for (var key in options) {
          if (Object.prototype.hasOwnProperty.call(options, key)) {
            this.config[key] = options[key];
          }
        }
      }
      return this;
    },

    getAuthUrl: function (customParams) {
      var params = Object.assign({}, this.config, customParams || {});
      var url = (window.SATSPAY_AUTH_URL || SATSPAY_BASE_URL) + '/oauth/authorize?';
      var query = [];

      if (!params.state) params.state = randomState();
      if (params.clientId) query.push('client_id=' + encodeURIComponent(params.clientId));
      if (params.redirectUri) query.push('redirect_uri=' + encodeURIComponent(params.redirectUri));
      if (params.scope) query.push('scope=' + encodeURIComponent(params.scope));
      if (params.state) query.push('state=' + encodeURIComponent(params.state));
      query.push('response_type=code');
      if (params.popup) query.push('popup=1');
      if (params.codeChallenge) {
        query.push('code_challenge=' + encodeURIComponent(params.codeChallenge));
        query.push('code_challenge_method=' + encodeURIComponent(params.codeChallengeMethod || 'S256'));
      }

      return url + query.join('&');
    },

    /** Read+clear PKCE verifier stored before redirect (merchant callback). */
    consumePkceVerifier: function (state) {
      try {
        var key = pkceStorageKey(state);
        var v = sessionStorage.getItem(key);
        sessionStorage.removeItem(key);
        return v;
      } catch (e) {
        return null;
      }
    },

    /**
     * Start OAuth.
     * redirect → navigates current window to SatsPay; merchant callback receives ?code=
     * popup → opens window; onSuccess({ code, state, redirect_url, code_verifier }) via postMessage
     */
    signIn: function (options) {
      options = options || {};
      var self = this;
      var mode = options.mode || this.config.mode || 'redirect';
      var state = options.state || this.config.state || randomState();
      var onSuccess = options.onSuccess;
      var onError = options.onError;

      function startWithPkce(codeChallenge, verifier) {
        try {
          sessionStorage.setItem(pkceStorageKey(state), verifier);
        } catch (e) {}

        var authOpts = Object.assign({}, options, {
          state: state,
          codeChallenge: codeChallenge,
          codeChallengeMethod: 'S256',
        });

        if (mode === 'popup') {
          var authUrl = self.getAuthUrl(Object.assign({}, authOpts, { popup: true }));
          var width = 480;
          var height = 720;
          var left = window.screenX + (window.outerWidth - width) / 2;
          var top = window.screenY + (window.outerHeight - height) / 2;

          try {
            sessionStorage.setItem('satspay_oauth_popup', '1');
          } catch (e) {}

          var popup = window.open(
            authUrl,
            'SatsPaySignInWindow',
            'width=' +
              width +
              ',height=' +
              height +
              ',top=' +
              top +
              ',left=' +
              left +
              ',toolbar=no,menubar=no,location=yes,status=no,resizable=yes,scrollbars=yes',
          );

          if (!popup || popup.closed || typeof popup.closed === 'undefined') {
            console.warn('[SatsPay] Popup bloqueado — usando redirect.');
            window.location.href = self.getAuthUrl(
              Object.assign({}, authOpts, { popup: false }),
            );
            return;
          }

          var finished = false;
          var messageListener = function (event) {
            try {
              var satspayOrigin = new URL(SATSPAY_BASE_URL).origin;
              if (event.origin !== satspayOrigin) return;
            } catch (e) {
              return;
            }
            if (!event.data || typeof event.data.type !== 'string') return;
            if (event.data.type !== 'SATSPAY_AUTH_SUCCESS' && event.data.type !== 'SATSPAY_AUTH_ERROR') {
              return;
            }

            finished = true;
            window.removeEventListener('message', messageListener);
            try {
              if (popup && !popup.closed) popup.close();
            } catch (e) {}

            if (event.data.type === 'SATSPAY_AUTH_SUCCESS') {
              event.data.code_verifier = verifier;
              invokeCallback(onSuccess, event.data);
            } else {
              invokeCallback(onError, event.data);
            }
          };

          window.addEventListener('message', messageListener);

          var poll = window.setInterval(function () {
            if (finished) {
              window.clearInterval(poll);
              return;
            }
            try {
              if (popup.closed) {
                window.clearInterval(poll);
                window.removeEventListener('message', messageListener);
                invokeCallback(onError, { type: 'SATSPAY_AUTH_ERROR', error: 'popup_closed' });
              }
            } catch (e) {
              window.clearInterval(poll);
            }
          }, 500);

          return;
        }

        window.location.href = self.getAuthUrl(
          Object.assign({}, authOpts, { popup: false }),
        );
      }

      var verifier = randomVerifier();
      if (window.crypto && window.crypto.subtle) {
        sha256Base64Url(verifier).then(function (challenge) {
          startWithPkce(challenge, verifier);
        }).catch(function () {
          // Fallback without PKCE if SubtleCrypto fails (rare).
          startWithPkce(null, verifier);
        });
      } else {
        startWithPkce(null, verifier);
      }
    },

    renderButton: function (container, options) {
      if (!container) return;
      options = options || {};

      var theme = options.theme || container.getAttribute('data-theme') || 'light';
      var size = options.size || container.getAttribute('data-size') || 'large';
      var textType = options.text || container.getAttribute('data-text') || 'signin_with';
      var clientId = options.clientId || container.getAttribute('data-client_id') || this.config.clientId;
      var redirectUri =
        options.redirectUri || container.getAttribute('data-redirect_uri') || this.config.redirectUri;
      var mode =
        options.mode || container.getAttribute('data-mode') || this.config.mode || 'redirect';
      var onSuccess = options.onSuccess || container.getAttribute('data-onsuccess');
      var onError = options.onError || container.getAttribute('data-onerror');
      var logoUrl = options.logoUrl || container.getAttribute('data-logo') || LOGO_URL;
      var scope = options.scope || container.getAttribute('data-scope') || this.config.scope;

      var label = 'Entrar com SatsPay';
      if (textType === 'continue_with') label = 'Continuar com SatsPay';
      if (textType === 'signup_with') label = 'Cadastrar com SatsPay';
      if (textType === 'en_signin') label = 'Sign in with SatsPay';
      if (textType === 'en_continue') label = 'Continue with SatsPay';

      var height = size === 'small' ? '38px' : size === 'medium' ? '44px' : '48px';
      var fontSize = size === 'small' ? '12px' : size === 'medium' ? '13px' : '14px';
      var padding = size === 'small' ? '0 14px' : size === 'medium' ? '0 18px' : '0 20px';
      var iconSize = size === 'small' ? '18px' : size === 'medium' ? '22px' : '24px';

      var bg = 'linear-gradient(135deg, #F7931A 0%, #E07D0A 100%)';
      var color = '#FFFFFF';
      var border = '1px solid rgba(255, 255, 255, 0.2)';
      var shadow = '0 4px 14px rgba(247, 147, 26, 0.35)';

      if (theme === 'dark') {
        bg = 'linear-gradient(135deg, #131B2E 0%, #0B0F19 100%)';
        color = '#F8FAFC';
        border = '1px solid rgba(255, 255, 255, 0.12)';
        shadow = '0 4px 12px rgba(0, 0, 0, 0.4)';
      } else if (theme === 'light') {
        bg = '#FFFFFF';
        color = '#0F172A';
        border = '1px solid #E2E8F0';
        shadow = '0 2px 8px rgba(0, 0, 0, 0.06)';
      }

      var btn = document.createElement('button');
      btn.type = 'button';
      btn.style.cssText = [
        'display: inline-flex',
        'align-items: center',
        'justify-content: center',
        'gap: 10px',
        'height: ' + height,
        'padding: ' + padding,
        'font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", sans-serif',
        'font-size: ' + fontSize,
        'font-weight: 700',
        'letter-spacing: -0.01em',
        'color: ' + color,
        'background: ' + bg,
        'border: ' + border,
        'border-radius: 12px',
        'box-shadow: ' + shadow,
        'cursor: pointer',
        'transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1)',
        'text-decoration: none',
        'user-select: none',
        'outline: none',
      ].join(';');

      var img = document.createElement('img');
      img.src = logoUrl;
      img.alt = '';
      img.width = 24;
      img.height = 24;
      img.decoding = 'async';
      img.style.cssText =
        'height: ' + iconSize + '; width: ' + iconSize + '; object-fit: contain; flex-shrink: 0;';
      img.onerror = function () {
        // Never fall back to a different asset — retry the official SDK mark only.
        if (img.src.indexOf('satspay-logo.png') === -1) {
          img.src = LOGO_URL;
        }
      };

      var span = document.createElement('span');
      span.textContent = label;

      btn.appendChild(img);
      btn.appendChild(span);
      btn.setAttribute('aria-label', label);

      btn.onmouseenter = function () {
        btn.style.transform = 'translateY(-1.5px)';
        btn.style.boxShadow =
          theme === 'bitcoin' ? '0 6px 20px rgba(247, 147, 26, 0.45)' : '0 6px 16px rgba(0,0,0,0.3)';
        btn.style.filter = 'brightness(1.05)';
      };
      btn.onmouseleave = function () {
        btn.style.transform = 'translateY(0)';
        btn.style.boxShadow = shadow;
        btn.style.filter = 'none';
      };
      btn.onmousedown = function () {
        btn.style.transform = 'translateY(0.5px) scale(0.98)';
      };

      btn.onclick = function (e) {
        e.preventDefault();
        if (!clientId) {
          console.error('[SatsPay] data-client_id obrigatório');
          return;
        }
        if (!redirectUri) {
          console.error('[SatsPay] data-redirect_uri obrigatório');
          return;
        }
        SatsPay.signIn({
          clientId: clientId,
          redirectUri: redirectUri,
          scope: scope,
          mode: mode,
          onSuccess: onSuccess,
          onError: onError,
        });
      };

      container.innerHTML = '';
      container.appendChild(btn);
    },

    autoRender: function () {
      var elements = document.querySelectorAll('.satspay-signin, [data-satspay-signin]');
      for (var i = 0; i < elements.length; i++) {
        SatsPay.renderButton(elements[i]);
      }
    },
  };

  window.SatsPay = SatsPay;
  window.SatsPayAuth = SatsPay;

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', SatsPay.autoRender);
  } else {
    SatsPay.autoRender();
  }
})(window, document);
