import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { drawTheme } from "./theme";
import "@xterm/xterm/css/xterm.css";
import "./App.css";

// The phone's colours are the desktop's (`app/src/theme`), drawn before anything is rendered,
// following the system's appearance.
drawTheme();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
