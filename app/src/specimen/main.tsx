// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The theme specimen's page** (#2330): drawn in a hidden window of its own (`specimen.rs`),
 * which photographs it once it says it is drawn.
 *
 * It is handed what to draw before any of its code runs, on {@link GIVEN}: the theme, the size
 * to lay itself out at, the workspace colour to tint it with and its fonts' faces. The size is laid on the page itself, so it is drawn whole at that
 * size whatever the hidden window was given. Motion is collapsed, the cursor hidden and nothing
 * blinks, so the same theme always draws the same picture.
 */
import "../styles.css";
import "./specimen.css";
import { flushSync } from "react-dom";
import ReactDOM from "react-dom/client";
import { Specimen } from "./Specimen";
import { drawn, failed, given } from "./page";
import { prepare } from "./prepare";

async function draw(): Promise<void> {
  const handed = given();
  // Every face made before anything is laid out, so each terminal measures its cells in the
  // theme's own terminal font.
  await prepare(handed, document.documentElement);
  const { theme } = handed;
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
