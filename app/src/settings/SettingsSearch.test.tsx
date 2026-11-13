// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { useState } from "react";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SettingsField, SettingsFile } from "../bindings";
import { SettingsTab } from "./SettingsTab";
import type { Level } from "./groups";
import { forgetGroups } from "./links";
import { forgetThisLaunch } from "../regions";
import { GLOBAL } from "../windowprefs";

/**
 * **One settings search across every level** (#2920, N27; #2814·4): the box in Settings' side
 * column searches every setting of every level the tab offers. While it holds words, the nav's
 * first page is every match, at every level, and the nav under it narrows to this level's pages
 * that hold one (SE-21). A match at this level is drawn with its control, so it is changed where
 * it is found; one at another level is a way to it, which lands on its field there. "Set here"
 * narrows to the values set at this level.
 */

const PLANE = "/home/dev/plane";

const field = (keys: string[], value: string): SettingsField => ({
  path: keys.map((key) => ({ key })),
  value: { kind: "text", value },
});

/** The project's files: the team's sets `[plane] mode`, the local one nothing. */
const SHARED: SettingsFile = {
  which: "shared",
  file: "charter.toml",
  exists: true,
  text: '[plane]\nmode = "push"\n',
  refusals: [],
  parsed: true,
  fields: [field(["plane", "mode"], "push")],
};
const LOCAL: SettingsFile = {
  which: "local",
  file: "charter.local.toml",
  exists: true,
  text: "",
  refusals: [],
  parsed: true,
  fields: [],
};

beforeEach(() => {
  forgetThisLaunch();
  forgetGroups();
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: {
      path: "/home/op/.config/charter/layout.json",
      found: false,
      document: null,
      trouble: null,
    },
    theme: { path: "", found: false, document: null, trouble: null },
  };
  mockIPC((command) => {
    if (command === "project_settings") return { shared: SHARED, local: LOCAL };
    // The Skills groups are on every machine since the studio's flag went (#2786); their
    // budget reads the studio, which this core has none of.
    if (command === "skill_studio") throw "no studio in this test";
    return null;
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
  Reflect.deleteProperty(globalThis, GLOBAL);
});

/** A Settings tab whose level switcher changes its level, as the window's tabs do. */
function Tab({ at = "you", plane }: { at?: Level; plane?: string }) {
  const [level, setLevel] = useState<Level>(at);
  return <SettingsTab plane={plane as never} level={level} onLevelChange={setLevel} />;
}

const box = () => screen.getByRole("searchbox", { name: "Search settings" });
const setHere = () => screen.getByRole("button", { name: "Set here" });
const nav = () => screen.getByRole("navigation", { name: "Groups" });
const inNav = () =>
  within(nav())
    .queryAllByRole("button")
    .filter((one) => !one.hasAttribute("aria-expanded"))
    .map((one) => one.textContent);
const page = (name: string | RegExp) => screen.getByRole("region", { name });
const sections = () =>
  within(page(/settings? found|No setting/))
    .queryAllByRole("heading", { level: 3 })
    .filter((one) => one.closest("[hidden]") === null)
    .map((one) => one.textContent);
const count = () => screen.getByRole("status", { name: "Settings found" });

describe("searching every setting from Settings", () => {
  it("has a labelled search box and a Set here toggle, off, above the nav", () => {
    render(<Tab />);

    expect(box()).toHaveValue("");
    expect(box()).toHaveAttribute("placeholder", "Search all levels");
    expect(setHere()).toHaveAttribute("aria-pressed", "false");
    expect(box().compareDocumentPosition(nav()) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("lists what it finds at every level on one page, the first of the nav", async () => {
    render(<Tab plane={PLANE} />);

    await userEvent.type(box(), "theme");

    // Every level's matches, under the level and the page each is on.
    await waitFor(() =>
      expect(sections()).toEqual(
        expect.arrayContaining(["You › Theme", "Project › Theme", "Project › Appearance"]),
      ),
    );
    expect(inNav()[0]).toMatch(/^All results/);
    expect(within(nav()).getByRole("button", { name: /^All results/ })).toHaveAttribute(
      "aria-current",
      "page",
    );
    // This level's pages that hold a match are the nav under it, as the filter's were.
    expect(inNav().slice(1)).toContain("Theme");
    expect(inNav().slice(1)).not.toContain("Editor");
  });

  it("draws this level's matches with their controls, to change where they are found", async () => {
    render(<Tab plane={PLANE} />);

    await userEvent.type(box(), "terminal text");

    expect(screen.getByRole("slider", { name: /terminal text size/i })).toBeInTheDocument();
    expect(screen.queryByRole("slider", { name: /window text size/i })).not.toBeInTheDocument();
  });

  it("offers another level's match as a way there, which lands on its field", async () => {
    render(<Tab plane={PLANE} />);

    await userEvent.type(box(), "target branch");
    await waitFor(() => expect(sections()).toEqual(["Project › Saving"]));

    await userEvent.click(screen.getByRole("button", { name: "Go to Saving: Target branch" }));

    // The Project level, at Saving, with the keyboard in the field and the search done.
    await waitFor(() => expect(page("Saving")).toBeInTheDocument());
    await waitFor(() =>
      expect(document.activeElement?.closest("[data-setting]")).toHaveAttribute(
        "data-setting",
        "project.saving.plane.branch",
      ),
    );
    expect(box()).toHaveValue("");
  });

  it("draws a setting of a page that holds a list with its control, not as a way there", async () => {
    render(<Tab at="project" plane={PLANE} />);
    await waitFor(() => expect(page("General")).toBeInTheDocument());

    // Harness & profiles holds the list of profiles; its default harness is a setting like any,
    // found by its key.
    // In one change, as a paste is: no page a part of the key matches is drawn on the way.
    await userEvent.click(box());
    await userEvent.paste("harness.default");

    await waitFor(() => expect(sections()).toEqual(["Project › Harness & profiles"]));
    expect(
      screen.queryByRole("button", { name: /^Go to Harness & profiles: Default harness/ }),
    ).not.toBeInTheDocument();
    expect(
      document.querySelector(
        '[data-setting="project.harness.harness.default"] .ui-setting-control [data-scope]',
      ),
    ).not.toBeNull();
  });

  it("says how many it found, at every level, and when nothing matches", async () => {
    render(<Tab plane={PLANE} />);

    await userEvent.type(box(), "text size");
    await waitFor(() => expect(count()).toHaveTextContent("2 settings match"));
    expect(page("2 settings found")).toBeInTheDocument();

    await userEvent.type(box(), "zzz");
    await waitFor(() => expect(count()).toHaveTextContent("No setting matches “text sizezzz”."));
  });

  it("shows a page of the nav narrowed, and All results again from the nav", async () => {
    render(<Tab />);

    await userEvent.type(box(), "your editor");
    await userEvent.click(within(nav()).getByRole("button", { name: "Editor" }));

    expect(page("Editor")).toBeInTheDocument();
    expect(screen.getByRole("radiogroup", { name: "Your editor" })).toBeInTheDocument();

    await userEvent.click(within(nav()).getByRole("button", { name: /^All results/ }));
    expect(page(/settings? found/)).toBeInTheDocument();
  });

  it("puts the level back as it was when the box is cleared", async () => {
    render(<Tab />);

    await userEvent.type(box(), "zzz{Escape}");

    expect(box()).toHaveValue("");
    expect(inNav()[0]).toBe("Text");
    expect(page("Text")).toBeInTheDocument();
  });
});

describe("Set here", () => {
  it("narrows to the values set at this level, and leaves other levels out", async () => {
    render(<Tab at="project" plane={PLANE} />);
    await waitFor(() => expect(page("General")).toBeInTheDocument());

    await userEvent.click(setHere());

    expect(setHere()).toHaveAttribute("aria-pressed", "true");
    await waitFor(() => expect(sections()).toEqual(["Project › Saving"]));
    const rows = [...document.querySelectorAll("[data-setting]")].map((one) =>
      one.getAttribute("data-setting"),
    );
    expect(rows).toEqual(["project.saving.plane.mode"]);
    await waitFor(() => expect(count()).toHaveTextContent("1 setting matches"));
  });

  it("says so when nothing is set at this level", async () => {
    render(<Tab />);

    await userEvent.click(setHere());

    await waitFor(() => expect(count()).toHaveTextContent("No setting is set at this level."));
  });

  it("takes the search's words too", async () => {
    render(<Tab at="project" plane={PLANE} />);
    await waitFor(() => expect(page("General")).toBeInTheDocument());

    await userEvent.click(setHere());
    await userEvent.type(box(), "auto-save");

    await waitFor(() =>
      expect(count()).toHaveTextContent("No setting set at this level matches “auto-save”."),
    );
  });
});

describe("the search, by keyboard", () => {
  it("goes from the box to Set here, then to All results in the nav", async () => {
    render(<Tab />);

    await userEvent.type(box(), "editor");
    await userEvent.tab();
    expect(setHere()).toHaveFocus();
    await userEvent.keyboard(" ");
    expect(setHere()).toHaveAttribute("aria-pressed", "true");
    await userEvent.tab();

    expect(within(nav()).getByRole("button", { name: /^All results/ })).toHaveFocus();
  });
});
