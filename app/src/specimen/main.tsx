// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The theme specimen's page** (#2330): drawn in a hidden window of its own (`specimen.rs`),
 * which photographs it once it says it is drawn.
 *
 * It is handed what to draw before any of its code runs, on {@link GIVEN}: the theme, and the
 * size to lay itself out at. The size is laid on the page itself, so it is drawn whole at that
 * size whatever the hidden window was given. Motion is collapsed, the cursor hidden and nothing
 * blinks, so the same theme always draws the same picture.
 */
import "../styles.css";
import "./specimen.css";
import { flushSync } from "react-dom";
import ReactDOM from "react-dom/client";
import { DEFAULT_TEXT, drawWindowText } from "../textSize";
import { apply, drawIn, type Theme } from "../theme/theme";
import { Specimen } from "./Specimen";
import { drawn, failed, given } from "./page";

async function draw(): Promise<void> {
  const { theme, width, height } = given();
  const root = document.documentElement;
  drawIn(theme);
  // Still: every duration collapsed, as reduced motion collapses it.
  apply(theme as Theme, root, true);
  drawWindowText(DEFAULT_TEXT.window, root);
  root.style.setProperty("--specimen-width", `${width}px`);
  root.style.setProperty("--specimen-height", `${height}px`);
  const writing: Promise<void>[] = [];
  const holder = document.getElementById("specimen");
  if (!holder) throw new Error("the specimen page has no #specimen");
  flushSync(() => {
    ReactDOM.createRoot(holder).render(
      <Specimen theme={theme} drawing={{ wait: (written) => writing.push(written) }} />,
    );
  });
  await Promise.all(writing);
  await document.fonts.ready;
  drawn();
}

draw().catch(failed);
