// M-455: Error boundary to prevent component crashes from crashing the entire app
// React error boundaries must be class components (hooks can't catch render errors)

import { Component, ErrorInfo, ReactNode } from 'react';
import { colors } from '../styles/tokens';

interface Props {
  children: ReactNode;
  // Optional name for identifying which boundary caught the error
  name?: string;
  // Optional fallback UI - defaults to a styled error message
  fallback?: ReactNode;
  // Optional callback when error is caught
  onError?: (error: Error, errorInfo: ErrorInfo) => void;
}

interface State {
  hasError: boolean;
  error: Error | null;
  errorInfo: ErrorInfo | null;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null, errorInfo: null };
  }

  static getDerivedStateFromError(error: Error): Partial<State> {
    // Update state so next render shows fallback UI
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo): void {
    // Log error to console with boundary name for debugging
    const boundaryName = this.props.name || 'Unknown';
    console.error(`[ErrorBoundary:${boundaryName}] Caught error:`, error);
    console.error(`[ErrorBoundary:${boundaryName}] Component stack:`, errorInfo.componentStack);

    this.setState({ errorInfo });

    // Call optional error callback
    if (this.props.onError) {
      this.props.onError(error, errorInfo);
    }
  }

  handleRetry = (): void => {
    this.setState({ hasError: false, error: null, errorInfo: null });
  };

  render(): ReactNode {
    if (this.state.hasError) {
      // Custom fallback if provided
      if (this.props.fallback) {
        return this.props.fallback;
      }

      // Default fallback UI
      const boundaryName = this.props.name || 'Component';
      return (
        <div
          style={{
            padding: '16px',
            backgroundColor: colors.statusBg.errorSolid,
            border: `1px solid ${colors.statusBg.errorBorder}`,
            borderRadius: '8px',
            color: colors.accent.lightRed,
            height: '100%',
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            minHeight: '100px',
          }}
        >
          <div style={{ fontSize: '14px', fontWeight: 600, marginBottom: '8px' }}>
            {boundaryName} Error
          </div>
          <div style={{ fontSize: '12px', color: colors.accent.mediumRed, marginBottom: '12px', textAlign: 'center' }}>
            {this.state.error?.message || 'An unexpected error occurred'}
          </div>
          <button
            type="button"
            onClick={this.handleRetry}
            style={{
              padding: '6px 12px',
              fontSize: '12px',
              backgroundColor: colors.statusBg.errorSolidStrong,
              border: `1px solid ${colors.statusBg.errorBorder}`,
              borderRadius: '4px',
              color: colors.accent.lightRed,
              cursor: 'pointer',
            }}
          >
            Retry
          </button>
          {process.env.NODE_ENV === 'development' && this.state.errorInfo && (
            <details style={{ marginTop: '12px', fontSize: '10px', maxWidth: '100%', overflow: 'auto' }}>
              <summary style={{ cursor: 'pointer', color: colors.accent.mediumRed }}>Stack trace</summary>
              <pre style={{ whiteSpace: 'pre-wrap', color: colors.accent.lightRed, marginTop: '8px' }}>
                {this.state.errorInfo.componentStack}
              </pre>
            </details>
          )}
        </div>
      );
    }

    return this.props.children;
  }
}

export default ErrorBoundary;
