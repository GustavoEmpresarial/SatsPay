/**
 * SatsPay — official payment button.
 *
 * Your backend creates the invoice (POST /v1/merchant/deposits) and hands the
 * returned `checkoutUrl` to this button. No API key ever reaches the browser,
 * and the amount cannot be tampered with client-side, because the button only
 * carries a link your server already signed off on.
 *
 *   <script src="https://www.satspay.pro/sdk/satspay-pay.js" async defer></script>
 *
 *   <div class="satspay-pay"
 *        data-checkout_url="https://www.satspay.pro/pay/550e8400-..."
 *        data-theme="bitcoin"        <!-- bitcoin | dark | light | outline -->
 *        data-size="large"           <!-- small | medium | large -->
 *        data-shape="rounded"        <!-- rounded | pill | square -->
 *        data-label="Pagar com cripto"
 *        data-amount="25 USDT"       <!-- optional, shown next to the label -->
 *        data-target="self"          <!-- self | blank -->
 *        data-onclick="beforeCheckout"></div>
 *
 * Buttons render on DOMContentLoaded; call `window.SatsPay.renderButtons()`
 * after injecting markup yourself (SPAs).
 */
(function () {
  'use strict';

  var ORIGIN = (function () {
    var s = document.currentScript && document.currentScript.src;
    try {
      return s ? new URL(s).origin : 'https://www.satspay.pro';
    } catch (e) {
      return 'https://www.satspay.pro';
    }
  })();

  var SELECTOR = '.satspay-pay';
  var RENDERED = 'data-satspay-rendered';

  var THEMES = {
    bitcoin: { bg: '#F7931A', fg: '#FFFFFF', border: '#F7931A', hover: '#E07E0B' },
    dark: { bg: '#0B1220', fg: '#FFFFFF', border: '#0B1220', hover: '#1B2435' },
    light: { bg: '#FFFFFF', fg: '#0B1220', border: '#D8DEE9', hover: '#F3F5F9' },
    outline: { bg: 'transparent', fg: '#0B1220', border: '#0B1220', hover: 'rgba(11,18,32,0.06)' }
  };

  var SIZES = {
    small: { pad: '8px 14px', font: '13px', icon: 16, gap: '8px' },
    medium: { pad: '11px 18px', font: '14px', icon: 18, gap: '9px' },
    large: { pad: '14px 22px', font: '15px', icon: 20, gap: '10px' }
  };

  var RADII = { rounded: '12px', pill: '999px', square: '4px' };

  function attr(el, name, fallback) {
    var v = el.getAttribute('data-' + name);
    return v === null || v === '' ? fallback : v;
  }

  function resolveCallback(name) {
    if (!name) return null;
    var fn = window[name];
    return typeof fn === 'function' ? fn : null;
  }

  /** Only ever navigate to an absolute https URL or a same-origin path. */
  function safeCheckoutUrl(raw) {
    if (!raw) return null;
    try {
      var url = new URL(raw, ORIGIN);
      if (url.protocol !== 'https:' && url.protocol !== 'http:') return null;
      return url.href;
    } catch (e) {
      return null;
    }
  }

  /**
   * The real SatsPay mark, same asset the login button uses. An <img> can
   * fail (offline, blocked, cache miss) and a payment button must never
   * render as a broken image, so it falls back to the inline glyph below.
   */
  function logoNode(size, fill) {
    var img = document.createElement('img');
    img.src = ORIGIN + '/sdk/satspay-logo.png?v=3';
    img.alt = '';
    img.width = size;
    img.height = size;
    img.setAttribute('aria-hidden', 'true');
    img.style.cssText = 'display:block;width:' + size + 'px;height:' + size + 'px;object-fit:contain;border-radius:4px';
    img.addEventListener('error', function () {
      var span = document.createElement('span');
      span.style.cssText = 'display:flex;align-items:center';
      span.innerHTML = logoSvg(size, fill);
      if (img.parentNode) img.parentNode.replaceChild(span, img);
    });
    return img;
  }

  function logoSvg(size, fill) {
    // Fallback mark, inline so it paints with no network at all.
    return (
      '<svg width="' + size + '" height="' + size + '" viewBox="0 0 32 32" aria-hidden="true" focusable="false">' +
      '<circle cx="16" cy="16" r="16" fill="' + fill + '" opacity="0.18"></circle>' +
      '<path fill="' + fill + '" d="M21.6 14.1c.25-1.7-1.04-2.6-2.8-3.2l.57-2.3-1.4-.35-.56 2.24c-.37-.1-.75-.18-1.13-.26l.56-2.25-1.4-.35-.57 2.3c-.3-.07-.6-.14-.89-.21v-.01l-1.93-.48-.37 1.5s1.04.24 1.02.25c.57.14.67.51.65.81l-.65 2.62c.04.01.09.02.15.05l-.15-.04-.91 3.67c-.07.17-.24.43-.63.33.01.02-1.02-.25-1.02-.25l-.7 1.6 1.82.46c.34.08.67.17 1 .25l-.58 2.33 1.4.35.57-2.3c.38.1.75.2 1.11.29l-.57 2.29 1.4.35.58-2.33c2.39.45 4.18.27 4.94-1.89.61-1.74-.03-2.74-1.29-3.4.92-.21 1.61-.81 1.79-2.06zm-3.2 4.5c-.43 1.74-3.37.8-4.32.56l.77-3.08c.95.24 4.01.71 3.55 2.52zm.44-4.53c-.4 1.58-2.84.78-3.63.58l.7-2.8c.79.2 3.34.56 2.93 2.22z"></path>' +
      '</svg>'
    );
  }

  function styleButton(btn, theme, size, radius) {
    var t = THEMES[theme] || THEMES.bitcoin;
    var s = SIZES[size] || SIZES.medium;
    btn.style.cssText = [
      'display:inline-flex',
      'align-items:center',
      'justify-content:center',
      'gap:' + s.gap,
      'padding:' + s.pad,
      'font-size:' + s.font,
      'font-weight:700',
      'font-family:system-ui,-apple-system,"Segoe UI",Roboto,sans-serif',
      'line-height:1',
      'color:' + t.fg,
      'background:' + t.bg,
      'border:1px solid ' + t.border,
      'border-radius:' + (RADII[radius] || RADII.rounded),
      'cursor:pointer',
      'text-decoration:none',
      'transition:background .15s ease,transform .08s ease',
      'box-shadow:0 1px 2px rgba(16,24,40,.06)'
    ].join(';');
    btn.addEventListener('mouseenter', function () { btn.style.background = t.hover; });
    btn.addEventListener('mouseleave', function () { btn.style.background = t.bg; });
    btn.addEventListener('mousedown', function () { btn.style.transform = 'scale(.98)'; });
    btn.addEventListener('mouseup', function () { btn.style.transform = 'scale(1)'; });
    return t;
  }

  function renderOne(el) {
    if (el.getAttribute(RENDERED) === '1') return;

    var checkoutUrl = safeCheckoutUrl(attr(el, 'checkout_url', attr(el, 'checkout-url', '')));
    var theme = attr(el, 'theme', 'bitcoin');
    var size = attr(el, 'size', 'medium');
    var radius = attr(el, 'shape', 'rounded');
    var label = attr(el, 'label', 'Pagar com SatsPay');
    var amount = attr(el, 'amount', '');
    var target = attr(el, 'target', 'self') === 'blank' ? '_blank' : '_self';
    var onclick = resolveCallback(attr(el, 'onclick', ''));

    var btn = document.createElement(checkoutUrl ? 'a' : 'button');
    if (checkoutUrl) {
      btn.href = checkoutUrl;
      btn.target = target;
      if (target === '_blank') btn.rel = 'noopener noreferrer';
    } else {
      btn.type = 'button';
      btn.disabled = true;
      btn.style.opacity = '.55';
      btn.title = 'satspay-pay: data-checkout_url ausente ou inválido';
    }

    var t = styleButton(btn, theme, size, radius);
    var s = SIZES[size] || SIZES.medium;

    var text = amount ? label + ' · ' + amount : label;
    var caption = document.createElement('span');
    caption.textContent = text; // textContent, never innerHTML — label is merchant input
    btn.appendChild(logoNode(s.icon, t.fg));
    btn.appendChild(caption);
    btn.setAttribute('aria-label', text);

    if (onclick) {
      btn.addEventListener('click', function (ev) {
        try {
          if (onclick({ checkoutUrl: checkoutUrl, element: el }) === false) ev.preventDefault();
        } catch (e) {
          /* a merchant callback must never block the payment */
        }
      });
    }

    el.setAttribute(RENDERED, '1');
    el.appendChild(btn);
  }

  function renderButtons(root) {
    var scope = root && root.querySelectorAll ? root : document;
    var nodes = scope.querySelectorAll(SELECTOR);
    for (var i = 0; i < nodes.length; i++) renderOne(nodes[i]);
    return nodes.length;
  }

  window.SatsPay = window.SatsPay || {};
  window.SatsPay.renderButtons = renderButtons;
  window.SatsPay.origin = ORIGIN;

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', function () { renderButtons(); });
  } else {
    renderButtons();
  }
})();
