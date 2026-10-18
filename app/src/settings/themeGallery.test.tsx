// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { GalleryCard, ThemeAt, ThemeGallery } from "../bindings";
import { putBack } from "../theme/hoverPreview";
import { BUILT_IN, DEFAULT_THEME, drawIn, inForce, property } from "../theme/theme";
import type { LiveSetting } from "./groups";
import { forgetGalleries, themeRows } from "./themeGallery";
import { forgetSpecimens } from "./themeSpecimens";

/**
 * **Settings › Theme's gallery** (#2331): a card per theme the core lists, the theme drawn live
 * while a card is pointed at or focused and put back when it is left, a press that writes the
 * person's choice where the level keeps it, and the chosen theme's appearances, options, check
 * findings, Duplicate as my theme and Open folder. The core is the mock below; the engine's
 * own answers are held by `purlis_core::theme::gallery`'s tests.
 */

const PLANE = "/home/dev/plane";

const LIGHT = BUILT_IN["purlis-light"];

function card(over: Partial<GalleryCard> & Pick<GalleryCard, "id" | "name">): GalleryCard {
  return {
    from: "machine",
    extension: null,
    appearances: [{ id: "dark", appearance: "dark", high_contrast: false }],
    options: [],
    refused: false,
    said: [],
    warnings: [],
    floors: [],
    follows: `${over.name} has only a dark appearance, so following the system stays dark`,
    duplicable: true,
    folder: true,
    stamp: "0",
    ...over,
  };
}

const CLASSIC = card({
  id: "purlis-classic",
  name: "Purlis Classic",
  from: "built-in",
  appearances: [
    { id: "dark", appearance: "dark", high_contrast: false },
    { id: "light", appearance: "light", high_contrast: false },
  ],
  follows: null,
  folder: false,
});

const DUSK = card({
  id: "dusk",
  name: "Dusk",
  warnings: ["text.primary on surface.base: 1.02, under 4.5"],
});

const TEAM = card({ id: "team", name: "Team", from: "project", duplicable: false });

const GALLERY: ThemeGallery = {
  cards: [CLASSIC, DUSK, TEAM],
  yours: null,
  drawn: "purlis-classic",
  level: "purlis",
  from: "nothing chose one",
};

/** The core: the gallery, what each card draws, and every write, recorded. */
function core(gallery: ThemeGallery = GALLERY) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  let painting = 0;
  let most = 0;
  mockIPC(async (cmd, args) => {
    asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    switch (cmd) {
      case "theme_gallery":
        return gallery;
      case "theme_card_drawn":
        return { theme: LIGHT, complaints: [], follows_system: false, floors: null, options: [] };
      case "theme_specimen": {
        painting++;
        most = Math.max(most, painting);
        await new Promise((done) => setTimeout(done, 1));
        painting--;
        return { png: "AA==", width: 1280, height: 800, millis: 1, appearance: "dark" };
      }
      case "theme_choose":
      case "theme_choose_machine":
      case "theme_open_folder":
        return null;
      case "theme_duplicate":
        return "dusk-mine";
      case "theme_in_force":
        return { theme: DEFAULT_THEME, complaints: [], follows_system: false, options: [] };
      default:
        return null;
    }
  });
  return {
    asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args),
    most: () => most,
  };
}

/** One row of the group as Settings draws it: its label and its control. */
function Row({ setting }: { setting: LiveSetting }) {
  const { control, undo, error } = setting.useControl();
  return (
    <section aria-label={setting.label}>
      <span id={`${setting.id}-label`}>{setting.label}</span>
      {control({ id: setting.id, labelledBy: `${setting.id}-label` })}
      {undo && (
        <button type="button" onClick={undo}>
          Undo
        </button>
      )}
      {error?.map((why) => (
        <p key={why} role="alert">
          {why}
        </p>
      ))}
    </section>
  );
}

function Group({ at }: { at: ThemeAt | null }) {
  const rows = themeRows(at === null ? "you.theme" : "project.theme", at);
  return (
    <>
      {rows.map((one) => (
        <Row key={one.id} setting={one} />
      ))}
    </>
  );
}

const surface = () => document.documentElement.style.getPropertyValue(property("surface.base"));

async function cards() {
  return screen.findAllByRole("option");
}

function named(name: string): HTMLElement {
  const found = screen.getAllByRole("option").find((one) => one.textContent?.includes(name));
  if (found === undefined) throw new Error(`no card names ${name}`);
  return found;
}

afterEach(() => {
  putBack();
  cleanup();
  clearMocks();
  forgetGalleries();
  forgetSpecimens();
  vi.useRealTimers();
  drawIn(DEFAULT_THEME);
});

describe("Settings › Theme's cards (#2331)", () => {
  it("shows a card per theme the core lists, with where each comes from and what it found", async () => {
    core();
    render(<Group at={null} />);
    const shown = await cards();
    expect(shown.map((one) => one.querySelector(".text-text-primary")?.textContent)).toEqual([
      "Purlis Classic",
      "Dusk",
      "Team",
    ]);
    expect(named("Purlis Classic")).toHaveTextContent("Built in · Drawn here");
    expect(named("Dusk")).toHaveTextContent("Yours, on this machine · 1 warning");
    expect(named("Team")).toHaveTextContent("This project's");
    expect(screen.getByText(/^Drawn here: Purlis Classic/)).toHaveTextContent("Purlis");
  });

  it("is one listbox and one Tab stop, the chosen card the stop, and takes no focus itself", async () => {
    core({ ...GALLERY, yours: { theme: "dusk", appearance: null }, drawn: "dusk" });
    render(<Group at={null} />);
    await cards();
    const list = screen.getByRole("listbox");
    expect(list).toHaveAttribute("aria-labelledby", "you.theme.gallery-label");
    const stops = within(list)
      .getAllByRole("option")
      .filter((one) => one.tabIndex === 0);
    expect(stops).toEqual([named("Dusk")]);
    expect(named("Dusk")).toHaveAttribute("aria-selected", "true");
    expect(named("Purlis Classic")).toHaveAttribute("aria-selected", "false");
    expect(document.activeElement).toBe(document.body);
  });

  it("draws a card's theme while it is pointed at, and puts yours back when it is left", async () => {
    const { asked } = core();
    render(<Group at={null} />);
    await cards();
    const before = inForce();
    const mine = surface();
    fireEvent.pointerEnter(named("Dusk"));
    await waitFor(() => expect(inForce()).toBe(LIGHT));
    expect(surface()).toBe(LIGHT.values["surface.base"]);
    expect(asked("theme_card_drawn")).toEqual([
      { at: null, theme: "dusk", appearance: "system", light: false, moreContrast: false },
    ]);
    fireEvent.pointerLeave(named("Dusk"));
    await waitFor(() => expect(inForce()).toBe(before));
    expect(surface()).toBe(mine);
    // Nothing was written.
    expect(asked("theme_choose_machine")).toEqual([]);
  });

  it("moves between cards with the arrow keys, drawing each, and chooses on Enter", async () => {
    const { asked } = core();
    const user = userEvent.setup();
    render(<Group at={null} />);
    await cards();
    await user.tab();
    expect(document.activeElement).toBe(named("Purlis Classic"));
    await user.keyboard("{ArrowRight}");
    expect(document.activeElement).toBe(named("Dusk"));
    await waitFor(() => expect(inForce()).toBe(LIGHT));
    expect(asked("theme_choose_machine")).toEqual([]);
    await user.keyboard("{Enter}");
    await waitFor(() =>
      expect(asked("theme_choose_machine")).toEqual([{ theme: "dusk", appearance: null }]),
    );
  });

  it("writes a project's choice where the theme levels read it, and Undo puts the last back", async () => {
    const { asked } = core({ ...GALLERY, yours: { theme: "purlis-classic", appearance: "light" } });
    render(<Group at={{ plane: PLANE, workspace: null }} />);
    await cards();
    fireEvent.click(named("Team"));
    await waitFor(() =>
      expect(asked("theme_choose")).toEqual([
        { plane: PLANE, workspace: null, theme: "team", appearance: null },
      ]),
    );
    fireEvent.click(await screen.findByRole("button", { name: "Undo" }));
    await waitFor(() =>
      expect(asked("theme_choose")[1]).toEqual({
        plane: PLANE,
        workspace: null,
        theme: "purlis-classic",
        appearance: "light",
      }),
    );
    expect(asked("theme_choose_machine")).toEqual([]);
  });

  it("asks for each card's pictures one at a time", async () => {
    const { asked, most } = core();
    render(<Group at={null} />);
    await cards();
    await waitFor(() => expect(asked("theme_specimen")).toHaveLength(4));
    expect(most()).toBe(1);
    expect(asked("theme_specimen")[1]).toEqual({
      theme: "purlis-classic",
      appearance: "light",
      at: null,
    });
  });
});

describe("under the chosen card (#2331)", () => {
  it("offers its appearances and says what following the system does with one tag", async () => {
    const { asked } = core({
      ...GALLERY,
      yours: { theme: "dusk", appearance: null },
      drawn: "dusk",
    });
    render(<Group at={null} />);
    const appearance = await screen.findByRole("radiogroup");
    expect(within(appearance).getAllByRole("radio")).toHaveLength(3);
    expect(appearance).toHaveTextContent(
      "Dusk has only a dark appearance, so following the system stays dark",
    );
    fireEvent.click(within(appearance).getByRole("radio", { name: "Follow the system" }));
    await waitFor(() =>
      expect(asked("theme_choose_machine")).toEqual([{ theme: "dusk", appearance: "system" }]),
    );
  });

  it("says the theme check's findings in purlis theme check's words", async () => {
    core({ ...GALLERY, yours: { theme: "dusk", appearance: null }, drawn: "dusk" });
    render(<Group at={null} />);
    const check = await screen.findByRole("region", { name: "Theme check" });
    await waitFor(() =>
      expect(check).toHaveTextContent("text.primary on surface.base: 1.02, under 4.5"),
    );
  });

  it("duplicates it as a theme of your own and opens its folder", async () => {
    const { asked } = core({
      ...GALLERY,
      yours: { theme: "dusk", appearance: null },
      drawn: "dusk",
    });
    render(<Group at={null} />);
    await cards();
    fireEvent.click(screen.getByRole("button", { name: "Duplicate as my theme" }));
    expect(
      await screen.findByText("Made dusk-mine, a theme of your own that extends Dusk."),
    ).toBeTruthy();
    expect(asked("theme_duplicate")).toEqual([{ at: null, theme: "dusk" }]);
    fireEvent.click(screen.getByRole("button", { name: "Open folder" }));
    await waitFor(() => expect(asked("theme_open_folder")).toEqual([{ at: null, theme: "dusk" }]));
  });

  it("offers neither on a theme a folder of your own cannot extend, nor a folder it has not", async () => {
    core({
      ...GALLERY,
      cards: [
        CLASSIC,
        TEAM,
        card({
          id: "sol/Dark",
          name: "Dark",
          from: "extension",
          extension: "sol",
          duplicable: false,
          folder: false,
        }),
      ],
      yours: { theme: "sol/Dark", appearance: null },
      drawn: "sol/Dark",
    });
    render(<Group at={{ plane: PLANE, workspace: null }} />);
    await cards();
    expect(screen.getByRole("button", { name: "Duplicate as my theme" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Open folder" })).toBeDisabled();
  });
});

describe("a card left for good", () => {
  it("puts your theme back when the tab goes while a card is drawn", async () => {
    core();
    const { unmount } = render(<Group at={null} />);
    await cards();
    const before = inForce();
    fireEvent.pointerEnter(named("Dusk"));
    await waitFor(() => expect(inForce()).toBe(LIGHT));
    act(() => unmount());
    expect(inForce()).toBe(before);
  });
});
