/**
 * @vitest-environment jsdom
 * The payment button ships as a plain script merchants embed on their own
 * pages, so it is exercised here exactly as a browser would: evaluate the
 * file, let it render, inspect the DOM.
 */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../');
const sdk = readFileSync(path.join(root, 'public/sdk/satspay-pay.js'), 'utf8');

function load(markup: string) {
  document.body.innerHTML = markup;
  // eslint-disable-next-line no-new-func
  new Function(sdk)();
  return document.body;
}

beforeEach(() => {
  document.body.innerHTML = '';
  delete (window as unknown as Record<string, unknown>).SatsPay;
});

describe('satspay-pay.js', () => {
  it('renders an anchor pointing at the checkout URL', () => {
    const body = load(
      '<div class="satspay-pay" data-checkout_url="https://www.satspay.pro/pay/abc-123"></div>',
    );
    const link = body.querySelector('a') as HTMLAnchorElement;
    expect(link).toBeTruthy();
    expect(link.href).toBe('https://www.satspay.pro/pay/abc-123');
    expect(link.textContent).toContain('Pagar com SatsPay');
  });

  it('carries the real SatsPay logo, not a drawn stand-in', () => {
    const body = load('<div class="satspay-pay" data-checkout_url="https://www.satspay.pro/pay/x"></div>');
    const img = body.querySelector('img') as HTMLImageElement;
    expect(img, 'button must show the brand mark').toBeTruthy();
    expect(img.src).toContain('/sdk/satspay-logo.png');
    expect(img.alt).toBe('');
    expect(img.getAttribute('aria-hidden')).toBe('true');
  });

  it('falls back to an inline mark if the logo fails to load', () => {
    // A broken image on a payment button is worse than a plain glyph.
    const body = load('<div class="satspay-pay" data-checkout_url="https://www.satspay.pro/pay/x"></div>');
    const img = body.querySelector('img') as HTMLImageElement;
    img.dispatchEvent(new Event('error'));
    expect(body.querySelector('img')).toBeNull();
    expect(body.querySelector('svg'), 'fallback mark must render').toBeTruthy();
  });

  it('offers a white theme', () => {
    // "botão personalizado branco com a nossa logo"
    const body = load(
      '<div class="satspay-pay" data-checkout_url="https://www.satspay.pro/pay/x" data-theme="light"></div>',
    );
    const link = body.querySelector('a') as HTMLAnchorElement;
    expect(link.style.background.replace(/\s/g, '')).toMatch(/#FFFFFF|rgb\(255,255,255\)/i);
    expect(link.querySelector('img')).toBeTruthy();
  });

  it('never hands the browser a javascript: URL', () => {
    // data-checkout_url is merchant-controlled markup; a hostile or mistaken
    // value must not become a navigable link.
    const body = load(
      '<div class="satspay-pay" data-checkout_url="javascript:alert(1)"></div>',
    );
    expect(body.querySelector('a')).toBeNull();
    const btn = body.querySelector('button') as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
  });

  it('treats the label as text, not markup', () => {
    const body = load(
      '<div class="satspay-pay" data-checkout_url="https://www.satspay.pro/pay/x" data-label="&lt;img src=x onerror=alert(1)&gt;"></div>',
    );
    const link = body.querySelector('a') as HTMLAnchorElement;

    // The button legitimately contains one <img>: the brand logo. The label
    // must not be able to add another, nor bring an onerror handler with it.
    const imgs = Array.from(link.querySelectorAll('img'));
    expect(imgs).toHaveLength(1);
    expect(imgs[0].src).toContain('/sdk/satspay-logo.png');
    expect(imgs[0].getAttribute('onerror')).toBeNull();
    expect(link.querySelector('span')!.textContent).toBe('<img src=x onerror=alert(1)>');
  });

  it('disables itself when no checkout URL is given', () => {
    const body = load('<div class="satspay-pay"></div>');
    const btn = body.querySelector('button') as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
    expect(btn.title).toMatch(/checkout_url/);
  });

  it('applies theme, size and shape', () => {
    const body = load(
      '<div class="satspay-pay" data-checkout_url="https://www.satspay.pro/pay/x" data-theme="dark" data-size="large" data-shape="pill"></div>',
    );
    const link = body.querySelector('a') as HTMLAnchorElement;
    expect(link.style.borderRadius).toBe('999px');
    expect(link.style.fontSize).toBe('15px');
    expect(link.style.background.replace(/\s/g, '')).toMatch(/#0B1220|rgb\(11,18,32\)/i);
  });

  it('appends the amount to the label when given', () => {
    const body = load(
      '<div class="satspay-pay" data-checkout_url="https://www.satspay.pro/pay/x" data-label="Pagar" data-amount="25 USDT"></div>',
    );
    expect(body.querySelector('a')!.textContent).toContain('Pagar · 25 USDT');
  });

  it('opens in a new tab safely when asked', () => {
    const body = load(
      '<div class="satspay-pay" data-checkout_url="https://www.satspay.pro/pay/x" data-target="blank"></div>',
    );
    const link = body.querySelector('a') as HTMLAnchorElement;
    expect(link.target).toBe('_blank');
    expect(link.rel).toContain('noopener');
  });

  it('calls the merchant hook and lets it cancel navigation', () => {
    const spy = vi.fn(() => false);
    (window as unknown as Record<string, unknown>).beforeCheckout = spy;
    const body = load(
      '<div class="satspay-pay" data-checkout_url="https://www.satspay.pro/pay/x" data-onclick="beforeCheckout"></div>',
    );
    const link = body.querySelector('a') as HTMLAnchorElement;
    const ev = new MouseEvent('click', { cancelable: true, bubbles: true });
    link.dispatchEvent(ev);
    expect(spy).toHaveBeenCalledOnce();
    expect(ev.defaultPrevented).toBe(true);
  });

  it('survives a throwing merchant hook', () => {
    (window as unknown as Record<string, unknown>).boom = () => {
      throw new Error('merchant bug');
    };
    const body = load(
      '<div class="satspay-pay" data-checkout_url="https://www.satspay.pro/pay/x" data-onclick="boom"></div>',
    );
    const link = body.querySelector('a') as HTMLAnchorElement;
    expect(() => link.dispatchEvent(new MouseEvent('click', { cancelable: true }))).not.toThrow();
  });

  it('renders SPA-injected buttons on demand and never twice', () => {
    const body = load('<div class="satspay-pay" data-checkout_url="https://www.satspay.pro/pay/x"></div>');
    expect(body.querySelectorAll('a')).toHaveLength(1);

    const api = (window as unknown as { SatsPay: { renderButtons: (r?: ParentNode) => number } }).SatsPay;
    api.renderButtons();
    expect(body.querySelectorAll('a'), 're-render must not duplicate the button').toHaveLength(1);

    const extra = document.createElement('div');
    extra.className = 'satspay-pay';
    extra.setAttribute('data-checkout_url', 'https://www.satspay.pro/pay/y');
    body.appendChild(extra);
    api.renderButtons();
    expect(body.querySelectorAll('a')).toHaveLength(2);
  });
});
