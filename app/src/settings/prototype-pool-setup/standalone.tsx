// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** PROTOTYPE, throwaway (spec #4469): the one-page build, drawn in the real built-in themes. */
import "../../../.storybook/handed";
import "../../styles.css";
import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { defaultCell, themes } from "virtual:purlis-handed";
import { drawIn, fetchFacesWith, type Theme } from "../../theme/theme";
import { PrototypePoolSetup, type Look } from "./PrototypePoolSetup";

fetchFacesWith(async (asset) => {
  const { default: faces } = await import("virtual:purlis-faces");
  const text = faces[asset];
  if (text === undefined) return undefined;
  return Uint8Array.from(atob(text), (char) => char.charCodeAt(0)).buffer;
});

function firstLook(): Look {
  const dark =
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-color-scheme: dark)").matches;
  return { appearance: dark ? "dark" : "light", theme: defaultCell.theme };
}

function draw(look: Look) {
  const theme = themes[look.theme]?.[look.appearance] as unknown as Theme;
  drawIn(theme);
}

function Page() {
  const [look, setLook] = useState(() => {
    const first = firstLook();
    draw(first);
    return first;
  });
  useEffect(() => draw(look), [look]);
  return <PrototypePoolSetup look={look} onLook={setLook} />;
}

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <Page />
    </StrictMode>,
  );
}
