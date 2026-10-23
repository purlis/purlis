// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { WindowOption } from "../bindings";
import { DEFAULT_THEME, drawIn } from "../theme/theme";
import { OptionsList, useOptionsInForce } from "./themeOptions";
import { tipOf } from "../ui/tooltip/tooltip.testkit";
import { PURLIS_LIGHT } from "../theme/handed.testkit";

/** The core's answer for a theme drawn with `options`. */
function answering(options: () => WindowOption[]) {
  mockIPC((cmd) => {
    if (cmd === "theme_in_force")
      return { theme: DEFAULT_THEME, complaints: [], follows_system: false, options: options() };
    return null;
  });
}

const BORDERS: WindowOption = {
  id: "borders",
  label: "Borders",
  description: "A rule between panes.",
  kind: "toggle",
  values: [
    { id: "off", label: "Off" },
    { id: "on", label: "On" },
  ],
  value: "on",
  default: "on",
  chosen: false,
  level: null,
};

const ACCENT: WindowOption = {
  id: "accent",
  label: "Accent",
  description: "",
  kind: "choice",
  values: [
    { id: "lavender", label: "Lavender" },
    { id: "teal", label: "Teal" },
  ],
  value: "teal",
  default: "lavender",
  chosen: true,
  level: null,
};

/** The options of the theme drawn, as Settings › Theme's Options row lists them. */
function Row() {
  return <OptionsList options={useOptionsInForce()} ids={{ id: "options" }} />;
}

afterEach(() => {
  cleanup();
  clearMocks();
  drawIn(DEFAULT_THEME);
});

describe("Settings › Theme › Options (#2325, #2331)", () => {
  it("shows each option of the theme drawn, its value and where the value comes from", async () => {
    answering(() => [BORDERS, ACCENT]);
    render(<Row />);
    const borders = await screen.findByText(/^Borders: On/);
    expect(borders.textContent).toContain("Theme default");
    expect(tipOf(borders)).toBe("A rule between panes.");
    const accent = screen.getByText(/^Accent: Teal/);
    expect(accent.textContent).toContain("Chosen");
  });

  it("names the level whose choice made a value (#2527)", async () => {
    answering(() => [{ ...ACCENT, level: "workspace-choice" }]);
    render(<Row />);
    const accent = await screen.findByText(/^Accent: Teal/);
    expect(accent.textContent).toContain("Workspace");
    expect(
      accent.querySelector(".ui-setting-badge")
        ? tipOf(accent.querySelector(".ui-setting-badge"))
        : undefined,
    ).toBe("Accent is drawn as your choice for this workspace on this machine has it.");
  });

  it("names a committed default's value as the team's (#2707)", async () => {
    answering(() => [{ ...ACCENT, level: "project-default" }]);
    render(<Row />);
    const accent = await screen.findByText(/^Accent: Teal/);
    expect(accent.textContent).toContain("Team");
    expect(
      accent.querySelector(".ui-setting-badge")
        ? tipOf(accent.querySelector(".ui-setting-badge"))
        : undefined,
    ).toBe("Accent is drawn as this project's committed default has it.");
  });

  it("says when the theme drawn has no options", async () => {
    answering(() => []);
    render(<Row />);
    expect(await screen.findByText("The theme drawn here has no options.")).toBeTruthy();
  });

  it("asks the core again when the window draws another theme", async () => {
    let options: WindowOption[] = [];
    answering(() => options);
    render(<Row />);
    await screen.findByText("The theme drawn here has no options.");
    options = [BORDERS];
    act(() => drawIn(PURLIS_LIGHT));
    await waitFor(() => expect(screen.getByText(/^Borders: On/)).toBeTruthy());
  });
});
