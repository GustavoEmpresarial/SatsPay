// Polyfill para compatibilidade com navegadores mais antigos (ex: Chrome < 92)
if (!Array.prototype.at) {
  Array.prototype.at = function (n: number) {
    n = Math.trunc(n) || 0;
    if (n < 0) n += this.length;
    if (n < 0 || n >= this.length) return undefined;
    return this[n];
  };
}

if (!String.prototype.at) {
  String.prototype.at = function (n: number) {
    n = Math.trunc(n) || 0;
    if (n < 0) n += this.length;
    if (n < 0 || n >= this.length) return '';
    return this.charAt(n);
  };
}

import React from 'react';
import ReactDOM from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider, QueryCache, MutationCache } from '@tanstack/react-query';
import { App } from './App.js';
import { AppErrorBoundary } from './components/AppErrorBoundary.js';
import { reportClientError, addBreadcrumb, installErrorCollectors } from './lib/reportError.js';
import './i18n/index.js';
import './stores/theme.js';
import 'bootstrap-icons/font/bootstrap-icons.css';
import './styles/index.css';

installErrorCollectors();

function errorStatus(error: unknown): number | undefined {
  if (error && typeof error === 'object' && 'status' in error) {
    const s = (error as { status: unknown }).status;
    return typeof s === 'number' ? s : undefined;
  }
  return undefined;
}

const queryClient = new QueryClient({
  queryCache: new QueryCache({
    onError: (error, query) => {
      const message = error instanceof Error ? error.message : String(error);
      addBreadcrumb('http', `Query failed: ${String(query.queryKey[0])}`, { error: message });
      reportClientError({
        kind: 'query',
        message: `Query ${JSON.stringify(query.queryKey).slice(0, 120)}: ${message}`,
        stack: error instanceof Error ? error.stack : undefined,
        statusCode: errorStatus(error),
        context: { queryKey: query.queryKey },
      });
    },
  }),
  mutationCache: new MutationCache({
    onError: (error, _variables, _context, mutation) => {
      const message = error instanceof Error ? error.message : String(error);
      const key = mutation.options.mutationKey ? String(mutation.options.mutationKey) : 'unknown';
      addBreadcrumb('http', `Mutation failed: ${key}`, { error: message });
      reportClientError({
        kind: 'query',
        message: `Mutation ${key}: ${message}`,
        stack: error instanceof Error ? error.stack : undefined,
        statusCode: errorStatus(error),
        context: { mutationKey: key },
      });
    },
  }),
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60,
      gcTime: 1000 * 60 * 10,
      refetchOnWindowFocus: false,
      refetchOnReconnect: true,
      retry: 1,
    },
  },
});

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <AppErrorBoundary>
          <App />
        </AppErrorBoundary>
      </BrowserRouter>
    </QueryClientProvider>
  </React.StrictMode>,
);
