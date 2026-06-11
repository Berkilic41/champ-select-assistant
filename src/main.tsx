import React from "react";
import ReactDOM from "react-dom/client";
// Self-hosted fonts (bundled by Vite → served from 'self', CSP-safe, offline-ok)
import "@fontsource/inter/400.css";
import "@fontsource/inter/500.css";
import "@fontsource/inter/600.css";
import "@fontsource/sora/600.css";
import "@fontsource/sora/700.css";
import "@fontsource/sora/800.css";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/500.css";
import "./styles/variables.css";
import "./styles/animations.css";
import "./index.css";
import "./i18n";
import { MotionConfig } from "framer-motion";
import App from "./App";
import { ErrorBoundary } from "./components/shared/ErrorBoundary";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      {/* Honour the OS "reduce motion" preference for all framer-motion animations. */}
      <MotionConfig reducedMotion="user">
        <App />
      </MotionConfig>
    </ErrorBoundary>
  </React.StrictMode>,
);
