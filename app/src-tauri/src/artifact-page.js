// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **purlis's own script in every artifact page** (`artifacts.rs`, ADR 0120 §3): the platform
// runs it at the start of each load, before the page's own markup, whatever the page's CSP
// says. It asks for nothing but its own origin, which the CSP already admits.
//
// - **Esc and F6 (Shift+F6 back) are purlis's keys**, not the page's (#5952): each is told by
//   asking `/.purlis/key/<key>` as an image, and purlis hands the keyboard back to the window.
// - **purlis's theme comes first** (#5955): `/.purlis/theme.css` is linked before anything of
//   the page's, so the page's own rules win. purlis calls `__purlisTheme()` when the window's
//   theme changes, and the sheet is asked for again.
// - **The scroll is kept across a reload** (#5955): where the page was is put in this load's
//   session storage as it goes, and scrolled back to once the next load has laid out.
(() => {
  const SCROLL = "purlis:scroll";
  const THEME = "/.purlis/theme.css";

  addEventListener(
    "keydown",
    (e) => {
      const key =
        e.key === "Escape" ? "escape" : e.key === "F6" ? (e.shiftKey ? "f6-back" : "f6") : null;
      if (key === null) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      new Image().src = "/.purlis/key/" + key;
    },
    true,
  );

  const sheet = document.createElement("link");
  sheet.rel = "stylesheet";
  sheet.href = THEME;
  // First of all, so anything of the page's comes after it and wins. At the start of a load
  // the document has its root and nothing in it yet.
  const link = () => {
    const root = document.documentElement;
    if (root === null || sheet.isConnected) return;
    root.insertBefore(sheet, root.firstChild);
  };
  link();
  if (!sheet.isConnected) document.addEventListener("readystatechange", link, { once: true });
  Object.defineProperty(window, "__purlisTheme", {
    value: () => {
      sheet.href = THEME + "?" + Date.now();
    },
  });

  const keep = () => {
    try {
      sessionStorage.setItem(SCROLL, JSON.stringify([scrollX, scrollY]));
    } catch {
      // No storage for this load: the next one starts at the top.
    }
  };
  addEventListener("pagehide", keep);
  addEventListener("beforeunload", keep);
  addEventListener("load", () => {
    let kept = null;
    try {
      kept = JSON.parse(sessionStorage.getItem(SCROLL) ?? "null");
      sessionStorage.removeItem(SCROLL);
    } catch {
      kept = null;
    }
    if (
      Array.isArray(kept) &&
      kept.length === 2 &&
      kept.every((n) => typeof n === "number" && Number.isFinite(n))
    )
      scrollTo(kept[0], kept[1]);
  });
})();
