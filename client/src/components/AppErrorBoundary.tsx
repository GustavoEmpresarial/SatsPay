import { Component, type ErrorInfo, type ReactNode } from 'react';
import { reportClientError } from '../lib/reportError.js';

interface Props {
  children: ReactNode;
}

interface State {
  message: string | null;
}

export class AppErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { message: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { message: error.message || 'Erro inesperado' };
  }

  override componentDidCatch(error: Error, info: ErrorInfo): void {
    reportClientError({
      kind: 'react',
      message: error.message || 'React render error',
      stack: `${error.stack ?? ''}\n${info.componentStack ?? ''}`,
    });
  }

  override render(): ReactNode {
    if (!this.state.message) return this.props.children;
    return (
      <div className="mx-auto max-w-lg px-4 py-16 text-center space-y-3">
        <h1 className="text-lg font-black text-ink">Algo quebrou nesta tela</h1>
        <p className="text-xs text-ink-muted">{this.state.message}</p>
        <button
          type="button"
          className="rounded-xl border border-border bg-paper px-3 py-1.5 text-xs font-bold text-ink"
          onClick={() => this.setState({ message: null })}
        >
          Tentar de novo
        </button>
      </div>
    );
  }
}
