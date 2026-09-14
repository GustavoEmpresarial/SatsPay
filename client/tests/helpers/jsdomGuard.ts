/** jsdom can lose <body> after many portal mounts / navigations. */
export function ensureJsdomDocument(): void {
  if (typeof document === 'undefined') return;
  if (!document.documentElement) {
    const html = document.createElement('html');
    document.appendChild(html);
  }
  if (!document.body) {
    const body = document.createElement('body');
    document.documentElement.appendChild(body);
  }
}
