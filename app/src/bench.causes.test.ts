// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, expect, it } from "vitest";
import { watchCauses } from "./bench";

/**
 * **Every `data-cause` the window ever drew** (#1291's follow-up): what `gone-kept.layout.e2e.ts`
 * reads to say a kept line was never drawn, not for one frame. An assertion after the window
 * settled cannot tell "never drawn" from "drawn, then taken away"; a record kept from before
 * React's first render can.
 */

const watching: MutationObserver[] = [];

afterEach(() => {
  for (const one of watching.splice(0)) one.disconnect();
  document.body.replaceChildren();
});

/** The observer's records are delivered as a microtask, after the change. */
const delivered = () => new Promise((done) => setTimeout(done, 0));

it("writes down a cause inserted, one inside what was inserted, and one set later", async () => {
  const seen: string[] = [];
  watching.push(watchCauses(document, seen));

  const line = document.createElement("p");
  line.setAttribute("data-cause", "project-gone:/a");
  const holder = document.createElement("div");
  holder.innerHTML = '<span><b data-cause="project-gone:/b"></b></span>';
  document.body.append(line, holder);
  await delivered();
  line.setAttribute("data-cause", "project-gone:/c");
  await delivered();

  expect(seen).toEqual(["project-gone:/a", "project-gone:/b", "project-gone:/c"]);
});

it("keeps a cause drawn and taken away before anything read the page, once", async () => {
  const seen: string[] = [];
  watching.push(watchCauses(document, seen));

  const line = document.createElement("p");
  line.setAttribute("data-cause", "project-gone:/kept");
  document.body.append(line);
  line.remove();
  document.body.append(line);
  line.remove();
  await delivered();

  expect(document.querySelector("[data-cause]")).toBeNull();
  expect(seen).toEqual(["project-gone:/kept"]);
});
