// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { composeStories } from "@storybook/react-vite";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as stories from "./ProtectionRow.stories";

// Whether the focus is in sight needs a layout, which jsdom has none of: the browser run holds
// that. What jsdom can hold is that the focus is somewhere, never on the page.
vi.mock(import("./stories.testkit"), async (actual) => ({
  ...(await actual()),
  expectFocusInSight: () => {
    const at = document.activeElement;
    if (at === null || at === document.body) throw new Error("nothing has the focus");
  },
}));

/**
 * **Setup's Protection row** (T1-24): the stories of First run and New project, drawn and walked
 * here in jsdom as `npm run test:stories` walks them in a browser, so each requirement's test is
 * seen to fail and pass where there is no browser to run it in. What the press writes is the
 * window test's (`SetupProtection.window.test.tsx`) and the core's
 * (`the_setup_choice_is_the_value_settings_reads`).
 */
const composed = composeStories(stories);
type Composed = (typeof composed)[keyof typeof composed];

afterEach(cleanup);

/** Draws `Story` and answers the body the page is drawn in (a dialog is drawn over it). */
async function drawn(Story: Composed) {
  await Story.load();
  render(<Story />);
  return document.body;
}

/** Draws `Story` and walks its `play`, with what a story's run hands it. */
async function walk(Story: Composed) {
  await Story.load();
  const { container } = render(<Story />);
  await Story.play?.({
    canvasElement: container,
    canvas: within(container),
    userEvent: userEvent.setup(),
  } as unknown as Parameters<NonNullable<Composed["play"]>>[0]);
}

const rows = async (inside: HTMLElement) => {
  await waitFor(() => expect(within(inside).getAllByRole("radiogroup").length).toBeGreaterThan(0));
  return within(inside).queryAllByRole("radiogroup", { name: "Protection" });
};

describe("setup's Protection row", () => {
  it("first run shows exactly one Protection row (R-059)", async () => {
    expect(await rows(await drawn(composed.OnFirstRun))).toHaveLength(1);
    await walk(composed.OnFirstRun);
  });

  it("New project shows the same Protection row (R-060)", async () => {
    const [first] = await rows(await drawn(composed.OnFirstRun));
    const names = within(first)
      .getAllByRole("radio")
      .map((one) => one.closest("label")?.textContent);
    cleanup();
    const shown = await rows(await drawn(composed.InNewProject));
    expect(shown).toHaveLength(1);
    expect(
      within(shown[0])
        .getAllByRole("radio")
        .map((one) => one.closest("label")?.textContent),
    ).toEqual(names);
    cleanup();
    await walk(composed.InNewProject);
  });

  it("the Protection row reads Basic sandbox (recommended) · Full sandbox · No sandbox with Basic preselected (R-061)", async () => {
    for (const Story of [composed.OnFirstRun, composed.InNewProject]) {
      const [row] = await rows(await drawn(Story));
      const radios = within(row).getAllByRole("radio");
      // In this order, each named exactly so.
      expect(radios).toEqual(
        ["Basic sandbox (recommended)", "Full sandbox", "No sandbox"].map((name) =>
          within(row).getByRole("radio", { name }),
        ),
      );
      expect(radios.filter((one) => (one as HTMLInputElement).checked)).toEqual([radios[0]]);
      cleanup();
    }
  });

  it("each Protection option has one line of help (R-062)", async () => {
    for (const Story of [composed.OnFirstRun, composed.InNewProject]) {
      const [row] = await rows(await drawn(Story));
      const helps = within(row)
        .getAllByRole("radio")
        .map((one) => one.getAttribute("aria-describedby"))
        .map((ids) =>
          (ids ?? "").split(" ").map((id) => document.getElementById(id)?.textContent ?? ""),
        )
        .map((said) => said.filter((one) => one !== ""));
      expect(helps).toHaveLength(3);
      for (const help of helps) expect(help).toHaveLength(1);
      expect(new Set(helps.map(([one]) => one)).size).toBe(3);
      cleanup();
    }
  });

  it("the Protection row says Change any time in Settings (R-063)", async () => {
    for (const Story of [composed.OnFirstRun, composed.InNewProject]) {
      const [row] = await rows(await drawn(Story));
      const described = (row.getAttribute("aria-describedby") ?? "")
        .split(" ")
        .map((id) => document.getElementById(id)?.textContent);
      expect(described).toContain("Change any time in Settings.");
      cleanup();
    }
  });

  it("Full sandbox help says new hosts are asked about the first time (R-295)", async () => {
    for (const Story of [composed.OnFirstRun, composed.InNewProject]) {
      const [row] = await rows(await drawn(Story));
      const full = within(row).getByRole("radio", { name: /^Full sandbox/ });
      const help = (full.getAttribute("aria-describedby") ?? "")
        .split(" ")
        .map((id) => document.getElementById(id)?.textContent ?? "")
        .join(" ");
      expect(help).toContain("New hosts are asked about the first time a chat reaches them.");
      expect(
        screen.getAllByText(/New hosts are asked about the first time a chat reaches them/),
      ).toHaveLength(1);
      cleanup();
    }
  });
});
