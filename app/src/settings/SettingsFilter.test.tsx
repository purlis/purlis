// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SettingsTab } from "./SettingsTab";
import { forgetThisLaunch } from "../regions";
import { GLOBAL } from "../windowprefs";

/**
 * **Filter settings by name** (SE-21, #1171; the spec on #558, V89c), as the search across every
 * level keeps it (#2920): the box above the nav narrows the nav to this level's groups that match
 * and each of them to the settings that match, by their labels and help text, case-insensitively,
 * and says so when nothing does. The nav's first page while it holds words is every match
 * (`SettingsSearch.test.tsx`); these are its pages under it. Driven at the You level by typing in
 * the box the way a person does and reading what is on screen.
 */

beforeEach(() => {
  forgetThisLaunch();
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: {
      path: "/home/op/.config/charter/layout.json",
      found: false,
      document: null,
      trouble: null,
    },
    theme: { path: "", found: false, document: null, trouble: null },
  };
  mockIPC(() => null);
});
afterEach(() => {
  cleanup();
  clearMocks();
  Reflect.deleteProperty(globalThis, GLOBAL);
});

const box = () => screen.getByRole("searchbox", { name: "Search settings" });
const nav = () => screen.getByRole("navigation", { name: "Groups" });
const inNav = () =>
  within(nav())
    .queryAllByRole("button")
    // A group's toggle is no page (R42), and every match is the search's own page (#2920).
    .filter((one) => !one.hasAttribute("aria-expanded"))
    .map((one) => one.textContent)
    .filter((one) => !one?.startsWith("All results"));
const allResults = () => within(nav()).getByRole("button", { name: /^All results/ });
const group = (name: string) => within(nav()).getByRole("button", { name });
const shown = () => screen.getByRole("region", { name: /./ });
const sliders = () =>
  screen.queryAllByRole("slider").map((one) => one.getAttribute("aria-label") ?? one.id);
const count = () => screen.getByRole("status", { name: "Settings found" });

describe("filtering the Settings tab by name", () => {
  it("has a labelled search box, empty, with the whole nav under it", () => {
    render(<SettingsTab />);

    expect(box()).toHaveValue("");
    expect(inNav()).toEqual([
      "Text",
      "Theme",
      "Density and fonts",
      "Motion and contrast",
      "Your colours",
      "Editor",
      "Chats list",
      "Attention",
      "Reports",
      "This machine",
      "Browsers",
      "Pools",
      "Accounts",
      "Profiles",
      "Previews",
    ]);
    expect(box().compareDocumentPosition(nav()) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("narrows the nav and the settings to a setting's label, whatever its case", async () => {
    render(<SettingsTab />);

    await userEvent.type(box(), "TERMINAL TEXT");

    expect(inNav()).toEqual(["Text"]);
    await userEvent.click(group("Text"));
    expect(screen.getByRole("slider", { name: /terminal text size/i })).toBeInTheDocument();
    expect(screen.queryByRole("slider", { name: /window text size/i })).not.toBeInTheDocument();
  });

  it("matches a setting's help text, and shows the group that holds it", async () => {
    render(<SettingsTab />);

    await userEvent.type(box(), "the line you are reading");

    expect(inNav()).toEqual(["Editor"]);
    await userEvent.click(group("Editor"));
    expect(group("Editor")).toHaveAttribute("aria-current", "page");
    expect(shown()).toHaveAccessibleName("Editor");
    expect(screen.getByRole("radiogroup", { name: "Your editor" })).toBeInTheDocument();
    expect(sliders()).toEqual([]);
  });

  it("shows every setting of a group whose own label or help matches", async () => {
    render(<SettingsTab />);

    await userEvent.type(box(), "how big");

    expect(inNav()).toEqual(["Text"]);
    await userEvent.click(group("Text"));
    expect(screen.getByRole("slider", { name: /window text size/i })).toBeInTheDocument();
    expect(screen.getByRole("slider", { name: /terminal text size/i })).toBeInTheDocument();
  });

  it("finds Reports by each of its other words, which it never draws (#5688)", async () => {
    render(<SettingsTab />);

    for (const word of ["reports", "crash", "telemetry", "error reporting"]) {
      await userEvent.clear(box());
      await userEvent.type(box(), word);

      expect(inNav(), word).toContain("Reports");
      await userEvent.click(group("Reports"));
      expect(shown()).toHaveAccessibleName("Reports");
      expect(screen.getByRole("radiogroup", { name: "Crash reports" })).toBeInTheDocument();
      expect(screen.getByRole("radiogroup", { name: "Agent drafts" })).toBeInTheDocument();
    }
    expect(screen.queryByText(/telemetry/i)).not.toBeInTheDocument();
  });

  it("says in a sentence when nothing matches, and the nav holds no page of the level", async () => {
    render(<SettingsTab />);

    await userEvent.type(box(), "colour of the moon");

    expect(inNav()).toEqual([]);
    // Said on the page of every match, where the group would be; the live region says it once
    // more, to a screen reader only.
    expect(screen.getByRole("region", { name: "No setting found" })).toBeInTheDocument();
    expect(
      screen.getByText("No setting matches “colour of the moon”.", {
        ignore: "[role=status]",
      }),
    ).toBeVisible();
  });

  it("says how many settings match, politely, and nothing while the box is empty", async () => {
    render(<SettingsTab />);
    expect(count()).toHaveAttribute("aria-live", "polite");
    expect(count()).toHaveTextContent(/^$/);

    await userEvent.type(box(), "text size");
    expect(count()).toHaveTextContent("2 settings match");

    await userEvent.clear(box());
    await userEvent.type(box(), "terminal text");
    expect(count()).toHaveTextContent("1 setting matches");

    await userEvent.type(box(), "zzz");
    expect(count()).toHaveTextContent("No setting matches “terminal textzzz”.");
  });

  it("puts the whole nav back when the box is cleared, at the group picked while it searched", async () => {
    render(<SettingsTab />);

    await userEvent.type(box(), "your editor");
    expect(inNav()).toEqual(["Editor"]);
    await userEvent.click(group("Editor"));
    await userEvent.clear(box());

    expect(inNav()).toEqual([
      "Text",
      "Theme",
      "Density and fonts",
      "Motion and contrast",
      "Your colours",
      "Editor",
      "Chats list",
      "Attention",
      "Reports",
      "This machine",
      "Browsers",
      "Pools",
      "Accounts",
      "Profiles",
      "Previews",
    ]);
    expect(group("Editor")).toHaveAttribute("aria-current", "page");
    expect(count()).toHaveTextContent(/^$/);
  });

  it("puts back the group shown before, when nothing was picked while it searched", async () => {
    render(<SettingsTab />);
    await userEvent.click(group("Editor"));

    await userEvent.type(box(), "text size");
    await userEvent.clear(box());

    expect(group("Editor")).toHaveAttribute("aria-current", "page");
  });

  it("clears the box at Escape", async () => {
    render(<SettingsTab />);

    await userEvent.type(box(), "zzz{Escape}");

    expect(box()).toHaveValue("");
    expect(inNav()).toEqual([
      "Text",
      "Theme",
      "Density and fonts",
      "Motion and contrast",
      "Your colours",
      "Editor",
      "Chats list",
      "Attention",
      "Reports",
      "This machine",
      "Browsers",
      "Pools",
      "Accounts",
      "Profiles",
      "Previews",
    ]);
  });
});

describe("the filtered nav, by keyboard", () => {
  it("is reached by Tab from the box, past Set here, at every match, its first page", async () => {
    render(<SettingsTab />);

    await userEvent.type(box(), "your editor");
    await userEvent.tab();
    await userEvent.tab();

    expect(allResults()).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}");
    expect(group("Editor")).toHaveFocus();
  });

  it("still moves between the groups left in it with the arrow keys", async () => {
    render(<SettingsTab />);

    // "wher" is in the window size's help ("anywhere") and Your editor's ("Where"), and in
    // neither group's own label or help: both groups stay, each narrowed to that one setting.
    // This machine's own help has it ("where updates come from"), so it stays whole, and the
    // Chats list keeps Where you answer (#4565), and Reports its Crash reports ("where it
    // crashed", #5688).
    await userEvent.type(box(), "wher");
    expect(inNav()).toEqual(["Text", "Editor", "Chats list", "Reports", "This machine"]);
    expect(screen.getByRole("slider", { name: /window text size/i })).toBeInTheDocument();
    expect(screen.queryByRole("slider", { name: /terminal text size/i })).not.toBeInTheDocument();
    group("Text").focus();
    await userEvent.keyboard("{ArrowDown}");
    expect(group("Editor")).toHaveFocus();
    await userEvent.keyboard("{Enter}");

    expect(shown()).toHaveAccessibleName("Editor");
  });
});
