import React from "react";
import ReactDOM from "react-dom/client";
import "./styles/variables.css";
import "./styles/animations.css";
import "./index.css";
import "./i18n";
import App from "./App";
import { ErrorBoundary } from "./components/shared/ErrorBoundary";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>,
);
