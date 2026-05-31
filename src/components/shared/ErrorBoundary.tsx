import React from "react";
import { AlertTriangle } from "lucide-react";
import i18n from "../../i18n";
import "./ErrorBoundary.css";

interface Props {
  children: React.ReactNode;
}

interface State {
  hasError: boolean;
  message?: string;
}

/**
 * App-root error boundary: a render-time crash shows a friendly, localized
 * fallback (title + body + retry) instead of a white screen. Critical for
 * beta stability — see CB-1 §D (stabilite) scenarios.
 *
 * Must be a class component: React only supports error boundaries via
 * `getDerivedStateFromError` / `componentDidCatch`. i18n is read from the
 * shared instance directly since class components can't use hooks.
 */
export class ErrorBoundary extends React.Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(error: unknown): State {
    return {
      hasError: true,
      message: error instanceof Error ? error.message : undefined,
    };
  }

  componentDidCatch(error: unknown, info: unknown) {
    // Log for local diagnostics; no telemetry is sent (see PRIVACY.md).
    console.error("ErrorBoundary caught:", error, info);
  }

  handleRetry = () => {
    this.setState({ hasError: false, message: undefined });
  };

  render() {
    if (!this.state.hasError) {
      return this.props.children;
    }

    return (
      <div className="error-boundary" role="alert">
        <div className="error-boundary__card">
          <div className="error-boundary__icon"><AlertTriangle size={28} /></div>
          <h2 className="error-boundary__title">{i18n.t("app.errorTitle")}</h2>
          <p className="error-boundary__body">{i18n.t("app.errorBody")}</p>
          {this.state.message && (
            <p className="error-boundary__detail">{this.state.message}</p>
          )}
          <button className="error-boundary__retry" onClick={this.handleRetry}>
            {i18n.t("app.errorRetry")}
          </button>
        </div>
      </div>
    );
  }
}
