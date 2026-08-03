import { Component, type ErrorInfo, type ReactNode } from 'react';
import { AlertTriangle } from 'lucide-react';

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
}

/**
 * Catches render errors in its subtree so a single bad component (e.g. a
 * dashboard page rendering unexpected event data) can't blank the entire app.
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('Unhandled error in component tree:', error, info.componentStack);
  }

  private reset = () => this.setState({ error: null });

  render() {
    const { error } = this.state;
    if (!error) {
      return this.props.children;
    }

    return (
      <div className="flex flex-col items-center justify-center gap-3 rounded-xl border border-red-800/50 bg-red-950/20 p-10 text-center">
        <AlertTriangle className="h-8 w-8 text-red-400" />
        <p className="text-sm font-medium text-red-300">Something went wrong rendering this page.</p>
        <p className="max-w-md text-xs text-gray-500">{error.message}</p>
        <button
          type="button"
          onClick={this.reset}
          className="mt-1 rounded-lg bg-red-700 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-red-600"
        >
          Try again
        </button>
      </div>
    );
  }
}
