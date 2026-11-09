// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The window's half of the sandboxed-frame rendering (ADR 0120 §3, first option). The probe's
// Rust side calls `probe.frame(...)` through `eval`; what a page says on its channel comes back
// through the `heard` command, which only this window is granted.
const invoke = (cmd, args) => window.__TAURI_INTERNALS__.invoke(cmd, args ?? {});

window.probe = {
  /** One artifact page in a sandboxed, opaque-origin frame, with one port of its own. */
  frame(channel, url) {
    const frame = document.createElement("iframe");
    frame.setAttribute("sandbox", "allow-scripts");
    frame.style.width = "400px";
    frame.style.height = "120px";
    const { port1, port2 } = new MessageChannel();
    // Only this port is the page's channel: nothing that arrives on the window's own `message`
    // event is ever read as the page speaking.
    port1.onmessage = (event) => invoke("heard", { channel, message: JSON.stringify(event.data) });
    frame.addEventListener(
      "load",
      () => frame.contentWindow.postMessage({ purlis: "port" }, "*", [port2]),
      { once: true },
    );
    frame.src = url;
    document.getElementById("frames").appendChild(frame);
  },
  /** That the window's own script still runs. */
  ping() {
    invoke("pong");
  },
};

// The window has loaded: the probe starts asking.
addEventListener("DOMContentLoaded", () => invoke("pong"));
