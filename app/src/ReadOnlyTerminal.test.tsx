// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The terminal's read-only mode** (#3676): a screen a reader can read line by line, never a
 * picture of its text, one Tab stop that says it is read-only, and find and copy from the
 * keyboard. That it is drawn as a terminal is, and is heard in a real browser, is held by its
 * stories (`ReadOnlyTerminal.stories.tsx`).
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { COPY_REFUSED, ReadOnlyTerminal, READ_ONLY_SAID } from "./ReadOnlyTerminal";

afterEach(cleanup);

// jsdom has no `matchMedia`, and xterm asks it for the display's pixel ratio as it opens.
vi.stubGlobal("matchMedia", (query: string) => ({
  matches: false,
  media: query,
  addEventListener: () => {},
  removeEventListener: () => {},
  addListener: () => {},
  removeListener: () => {},
}));

const CHAT =
  "> deploy this to staging\r\n\r\n\u001b[1m● Skill(deploy)\u001b[0m\r\n  ⎿ Loaded deploy";

/** Whether this is a Mac, as the window reads it. */
const mac = () => navigator.platform.startsWith("Mac");
/** The chord of `key`: ⌘ on a Mac, Ctrl elsewhere. */
const chord = (key: string) => (mac() ? { key, metaKey: true } : { key, ctrlKey: true });

async function drawn(props: Partial<Parameters<typeof ReadOnlyTerminal>[0]> = {}) {
  render(
    <ReadOnlyTerminal
      screen={CHAT}
      label="A grade's chat of deploy"
      note="It takes no input and can't be resumed."
      cols={60}
      rows={8}
      fontSize={13}
      {...props}
    />,
  );
  const region = screen.getByRole("region", { name: "A grade's chat of deploy" });
  await waitFor(() => expect(within(region).getAllByRole("listitem").length).toBeGreaterThan(0));
  return region;
}

describe("a read-only terminal", () => {
  it("is read line by line, and never stands for its text as a picture", async () => {
    const region = await drawn();
    const lines = within(region)
      .getAllByRole("listitem")
      .map((one) => one.textContent);
    expect(lines).toEqual(["> deploy this to staging", "● Skill(deploy)", "  ⎿ Loaded deploy"]);
    expect(document.querySelector('[role="img"]')).toBeNull();
    // The grid is what is seen; its text is the list.
    expect(region.querySelector(".xterm")?.closest('[aria-hidden="true"]')).not.toBeNull();
  });

  it("says it is read-only, in words outside the grid that describe it", async () => {
    const region = await drawn();
    expect(screen.getByText("It takes no input and can't be resumed.")).toBeVisible();
    expect(region).toHaveAccessibleDescription("It takes no input and can't be resumed.");
  });

  it("says so by default too", async () => {
    const region = await drawn({ note: undefined });
    expect(region).toHaveAccessibleDescription(READ_ONLY_SAID);
  });

  it("is one Tab stop: xterm's own text box is not one", async () => {
    const region = await drawn();
    expect(region).toHaveAttribute("tabindex", "0");
    for (const box of region.querySelectorAll("textarea"))
      expect(box).toHaveAttribute("tabindex", "-1");
  });

  it("joins a row xterm wrapped to the row it continues", async () => {
    const region = await drawn({ screen: "x".repeat(25), cols: 10 });
    expect(
      within(region)
        .getAllByRole("listitem")
        .map((one) => one.textContent),
    ).toEqual(["x".repeat(25)]);
  });

  it("keeps the space a row wrapped on between the words either side of it", async () => {
    const region = await drawn({ screen: "deploy123 failed", cols: 10 });
    expect(
      within(region)
        .getAllByRole("listitem")
        .map((one) => one.textContent),
    ).toEqual(["deploy123 failed"]);
  });

  it("says a copy the clipboard refused", async () => {
    const writeText = vi.fn<(text: string) => Promise<void>>(async () => {
      throw new Error("refused");
    });
    const region = await drawn({ clipboard: { writeText } });
    act(() => {
      fireEvent.keyDown(region, chord("a"));
    });
    act(() => {
      fireEvent.keyDown(region, chord("c"));
    });
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent(COPY_REFUSED));
  });

  it("opens the find bar on its find key, and Esc there brings the keyboard back", async () => {
    const region = await drawn();
    act(() => region.focus());
    act(() => {
      fireEvent.keyDown(region, chord("f"));
    });
    const find = screen.getByRole("search", { name: "Find in A grade's chat of deploy" });
    const box = within(find).getByRole("searchbox", { name: "Find" });
    expect(box).toHaveFocus();
    act(() => {
      fireEvent.keyDown(box, { key: "Escape" });
    });
    expect(screen.queryByRole("search")).toBeNull();
    expect(region).toHaveFocus();
  });

  it("selects the whole screen and copies it from the keyboard", async () => {
    const writeText = vi.fn<(text: string) => Promise<void>>(async () => {});
    const region = await drawn({ clipboard: { writeText } });
    act(() => region.focus());
    act(() => {
      fireEvent.keyDown(region, chord("a"));
    });
    act(() => {
      fireEvent.keyDown(region, chord("c"));
    });
    expect(writeText).toHaveBeenCalledOnce();
    const copied = writeText.mock.calls[0]?.[0] ?? "";
    expect(copied).toContain("> deploy this to staging");
    expect(copied).toContain("⎿ Loaded deploy");
  });

  it("copies nothing with nothing selected", async () => {
    const writeText = vi.fn<(text: string) => Promise<void>>(async () => {});
    const region = await drawn({ clipboard: { writeText } });
    act(() => {
      fireEvent.keyDown(region, chord("c"));
    });
    expect(writeText).not.toHaveBeenCalled();
  });
});
