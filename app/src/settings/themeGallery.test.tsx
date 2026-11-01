// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { THEME_FOLDERS_CHANGED } from "../Extensions";
import {
  SPECIMEN_BUSY,
  type GalleryCard,
  type ThemeAt,
  type ThemeGallery,
  type WindowOption,
  type YourTheme,
} from "../bindings";
import { putBack, showing } from "../theme/hoverPreview";
import { DEFAULT_THEME, drawIn, inForce, property } from "../theme/theme";
import type { LiveSetting } from "./groups";
import { ReadOnlySaid } from "../ReadOnlyLine";
import { THEME_CHAT, type ThemeChatAsk } from "./links";
import { forgetGalleries, themeRows } from "./themeGallery";
import {
  BUSY_FOR_MS,
  BUSY_WAIT_MS,
  forgetSpecimens,
  keptSpecimens,
  MOST_KEPT,
} from "./themeSpecimens";
import { tipOf } from "../ui/tooltip/tooltip.testkit";
import { PURLIS_LIGHT } from "../theme/handed.testkit";

/**
 * **Settings › Theme's gallery** (#2331): a card per theme the core lists, the theme drawn live
 * while a card is pointed at or focused and put back when it is left, a press that writes the
 * person's choice where the level keeps it, and the chosen theme's appearances, options, check
 * findings, Duplicate as my theme and Open folder. The core is the mock below; the engine's
 * own answers are held by `purlis_core::theme::gallery`'s tests.
 */

const PLANE = "/home/dev/plane";

const LIGHT = PURLIS_LIGHT;

function card(over: Partial<GalleryCard> & Pick<GalleryCard, "id" | "name">): GalleryCard {
  const from = over.from ?? "machine";
  const level = ["machine", "project", "workspace"].includes(from) ? from : null;
  return {
    key: level === null ? over.id : `${level}:${over.id}`,
    level,
    from: "machine",
    preview: false,
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
  chosen: "purlis-classic",
  options: [],
  theirs: null,
};

/** The person's own choice of `theme`, of the folder level `from`. */
function mine(theme: string, appearance: string | null, from: string | null = null): YourTheme {
  return { theme, appearance, from, options: {} };
}

/** The gallery with `over`, its chosen card the core's: the person's choice here, else what is
 *  drawn. */
function shown(over: Partial<ThemeGallery>): ThemeGallery {
  const gallery = { ...GALLERY, ...over };
  const yours = gallery.yours;
  const theirs =
    yours === null
      ? undefined
      : gallery.cards.find(
          (one) => one.id === yours.theme && (yours.from === null || one.level === yours.from),
        )?.key;
  return { ...gallery, chosen: over.chosen ?? theirs ?? gallery.drawn };
}

/** The core: the gallery, what each card draws, and every write, recorded. */
function core(
  gallery: ThemeGallery = GALLERY,
  {
    held = false,
    painter = () => undefined as string | undefined,
    pictures,
  }: {
    /** Each card's answer waits for `release`. */
    held?: boolean;
    /** The painter's refusal of the `n`th specimen asked, or none. */
    painter?: (n: number) => string | undefined;
    /** Each picture waits for `paint`. */
    pictures?: "held";
  } = {},
) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  const waiting: (() => void)[] = [];
  const easels: (() => void)[] = [];
  let painting = 0;
  let most = 0;
  let painted = 0;
  mockIPC(
    async (cmd, args) => {
      asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
      switch (cmd) {
        case "theme_gallery":
          // A new answer each time, as the app's is.
          return { ...gallery };
        case "theme_card_drawn":
          if (held) await new Promise<void>((done) => waiting.push(done));
          return { theme: LIGHT, complaints: [], follows_system: false, floors: null, options: [] };
        case "theme_specimen": {
          const refused = painter(++painted);
          if (refused !== undefined) throw refused;
          painting++;
          most = Math.max(most, painting);
          await new Promise<void>((done) =>
            pictures === "held" ? easels.push(done) : setTimeout(done, 1),
          );
          painting--;
          return { png: "AA==", width: 1280, height: 800, millis: 1, appearance: "dark" };
        }
        case "theme_choose":
        case "theme_choose_machine":
        case "theme_open_folder":
          return null;
        case "theme_choose_option":
          return ["Accented has no option gone any more, so your value for it was dropped."];
        case "theme_duplicate":
          return "dusk-mine";
        case "theme_in_force":
          return { theme: DEFAULT_THEME, complaints: [], follows_system: false, options: [] };
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );
  return {
    asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args),
    most: () => most,
    /** Answers every card asked for so far. */
    release: () => waiting.splice(0).forEach((done) => done()),
    /** Finishes every picture being painted. */
    paint: () => easels.splice(0).forEach((done) => done()),
  };
}

/** One row of the group as Settings draws it: its label and its control. */
function Row({ setting }: { setting: LiveSetting }) {
  const { control, undo, error, reset } = setting.useControl();
  return (
    <section aria-label={setting.label}>
      <span id={`${setting.id}-label`}>{setting.label}</span>
      {control({ id: setting.id, labelledBy: `${setting.id}-label` })}
      {reset && (
        <button type="button" disabled={reset.disabled} onClick={reset.onReset}>
          {reset.label}
        </button>
      )}
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
  vi.unstubAllGlobals();
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
    // A card is named by its theme's name, and where it comes from is its description (#2911).
    expect(screen.getByRole("option", { name: "Dusk" })).toHaveAccessibleDescription(
      "Yours, on this machine · 1 warning",
    );
  });

  it("says a built-in theme that ships as a preview is one (#2336)", async () => {
    const purlis = card({
      id: "purlis",
      name: "Purlis",
      from: "built-in",
      preview: true,
      folder: false,
    });
    core({ ...GALLERY, cards: [CLASSIC, purlis, DUSK, TEAM] });
    render(<Group at={null} />);
    await cards();
    expect(named("Purlis Classic")).not.toHaveTextContent("Preview");
    expect(named("Dusk")).not.toHaveTextContent("Preview");
    const shown = screen
      .getAllByRole("option")
      .find((one) => one.querySelector(".text-text-primary")?.textContent === "Purlis");
    expect(shown).toHaveTextContent("Built in · Preview");
  });

  it("is one listbox and one Tab stop, and Tab lands on the chosen card", async () => {
    core(shown({ yours: mine("dusk", null), drawn: "machine:dusk" }));
    render(<Group at={null} />);
    await cards();
    const list = screen.getByRole("listbox");
    expect(list).toHaveAttribute("aria-labelledby", "you.theme.gallery-label");
    expect(named("Dusk")).toHaveAttribute("aria-selected", "true");
    expect(named("Purlis Classic")).toHaveAttribute("aria-selected", "false");
    expect(document.activeElement).toBe(document.body);
    act(() => list.focus());
    expect(named("Dusk")).toHaveFocus();
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
      {
        at: null,
        theme: "dusk",
        from: "machine",
        appearance: "system",
        light: false,
        moreContrast: false,
        person: null,
      },
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
      expect(asked("theme_choose_machine")).toEqual([
        { theme: "dusk", appearance: null, from: "machine" },
      ]),
    );
  });

  it("writes a project's choice where the theme levels read it, and Undo puts the last back", async () => {
    const { asked } = core(shown({ yours: mine("purlis-classic", "light") }));
    render(<Group at={{ plane: PLANE, workspace: null }} />);
    await cards();
    fireEvent.click(named("Team"));
    await waitFor(() =>
      expect(asked("theme_choose")).toEqual([
        { plane: PLANE, workspace: null, theme: "team", appearance: null, from: "project" },
      ]),
    );
    fireEvent.click(await screen.findByRole("button", { name: "Undo" }));
    await waitFor(() =>
      expect(asked("theme_choose")[1]).toEqual({
        plane: PLANE,
        workspace: null,
        theme: "purlis-classic",
        appearance: "light",
        from: null,
      }),
    );
    expect(asked("theme_choose_machine")).toEqual([]);
  });

  // Before the pictures test on purpose (#2698): nothing this one starts may reach that one.
  it("is read again when a theme folder changes on disk (#2324)", async () => {
    const mock = core();
    render(<Group at={{ plane: PLANE, workspace: "alpha" }} />);
    await cards();
    const before = mock.asked("theme_gallery").length;
    await act(async () => {
      await emit(THEME_FOLDERS_CHANGED, { survey: false });
    });
    await waitFor(() => expect(mock.asked("theme_gallery").length).toBe(before + 1));
    expect(mock.asked("theme_gallery").at(-1)).toEqual({
      at: { plane: PLANE, workspace: "alpha" },
    });
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
      from: null,
    });
  });
});

describe("under the chosen card (#2331)", () => {
  it("offers its appearances and says what following the system does with one tag", async () => {
    const { asked } = core(shown({ yours: mine("dusk", null), drawn: "machine:dusk" }));
    render(<Group at={null} />);
    const appearance = await screen.findByRole("radiogroup");
    expect(within(appearance).getAllByRole("radio")).toHaveLength(3);
    expect(appearance).toHaveTextContent(
      "Dusk has only a dark appearance, so following the system stays dark",
    );
    fireEvent.click(within(appearance).getByRole("radio", { name: "Follow the system" }));
    await waitFor(() =>
      expect(asked("theme_choose_machine")).toEqual([
        { theme: "dusk", appearance: "system", from: "machine" },
      ]),
    );
  });

  it("says the theme check's findings in purlis theme check's words", async () => {
    core(shown({ yours: mine("dusk", null), drawn: "machine:dusk" }));
    render(<Group at={null} />);
    const check = await screen.findByRole("region", { name: "Theme check" });
    await waitFor(() =>
      expect(check).toHaveTextContent("text.primary on surface.base: 1.02, under 4.5"),
    );
  });

  it("duplicates it as a theme of your own and opens its folder", async () => {
    const { asked } = core(shown({ yours: mine("dusk", null), drawn: "machine:dusk" }));
    render(<Group at={null} />);
    await cards();
    fireEvent.click(screen.getByRole("button", { name: "Duplicate as my theme" }));
    expect(
      await screen.findByText("Made dusk-mine, a theme of your own that extends Dusk."),
    ).toBeTruthy();
    expect(asked("theme_duplicate")).toEqual([{ at: null, theme: "dusk", from: "machine" }]);
    fireEvent.click(screen.getByRole("button", { name: "Open folder" }));
    await waitFor(() =>
      expect(asked("theme_open_folder")).toEqual([{ at: null, theme: "dusk", from: "machine" }]),
    );
  });

  it("asks the project's window for a theme chat: New theme here, Edit with a chat on the chosen card (#2333)", async () => {
    const { asked } = core(shown({ yours: mine("team", null, "project"), drawn: "project:team" }));
    const heard: ThemeChatAsk[] = [];
    const hear = (event: Event) => heard.push((event as CustomEvent<ThemeChatAsk>).detail);
    window.addEventListener(THEME_CHAT, hear);
    try {
      render(<Group at={{ plane: PLANE, workspace: "alpha" }} />);
      await cards();
      fireEvent.click(screen.getByRole("button", { name: "Edit with a chat" }));
      fireEvent.click(screen.getByRole("button", { name: "New theme" }));
      expect(heard).toEqual([
        { plane: PLANE, workspace: "alpha", theme: "team", from: "project" },
        { plane: PLANE, workspace: "alpha", theme: null, from: null },
      ]);
      // The window asks; the project's window opens the chat. Nothing is written from here.
      expect(asked("theme_chat")).toEqual([]);
    } finally {
      window.removeEventListener(THEME_CHAT, hear);
    }
  });

  it("opens no theme chat in a read-only project, and says why", async () => {
    core(shown({ yours: mine("team", null, "project"), drawn: "project:team" }));
    render(
      <ReadOnlySaid.Provider value="This project is read-only.">
        <Group at={{ plane: PLANE, workspace: null }} />
      </ReadOnlySaid.Provider>,
    );
    await cards();
    for (const name of ["Edit with a chat", "New theme"]) {
      const button = screen.getByRole("button", { name });
      expect(button).toHaveAttribute("aria-disabled", "true");
      expect(tipOf(button)).toBe("This project is read-only. A theme made here could not be kept.");
    }
  });

  it("opens no theme chat at the machine's level, where no project is, and says why", async () => {
    core(shown({ yours: mine("dusk", null), drawn: "machine:dusk" }));
    render(<Group at={null} />);
    await cards();
    for (const name of ["Edit with a chat", "New theme"]) {
      const button = screen.getByRole("button", { name });
      expect(button).toHaveAttribute("aria-disabled", "true");
      expect(tipOf(button)).toContain("at the Project or Workspace level");
    }
  });

  it("offers neither on a theme a folder of your own cannot extend, nor a folder it has not", async () => {
    core(
      shown({
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
        yours: mine("sol/Dark", null),
        drawn: "sol/Dark",
      }),
    );
    render(<Group at={{ plane: PLANE, workspace: null }} />);
    await cards();
    expect(screen.getByRole("button", { name: "Duplicate as my theme" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    expect(screen.getByRole("button", { name: "Open folder" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
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

describe("a preview never outlives its card (#2331 review)", () => {
  it("draws nothing for an answer that arrives after the tab went", async () => {
    const { release } = core(GALLERY, { held: true });
    const { unmount } = render(<Group at={null} />);
    await cards();
    const before = inForce();
    fireEvent.pointerEnter(named("Dusk"));
    act(() => unmount());
    await act(async () => release());
    expect(inForce()).toBe(before);
    expect(showing()).toBe(false);
  });

  it("draws nothing for an answer that arrives after the card was pressed", async () => {
    const { asked, release } = core(GALLERY, { held: true });
    render(<Group at={null} />);
    await cards();
    const before = inForce();
    fireEvent.pointerEnter(named("Dusk"));
    fireEvent.click(named("Dusk"));
    await waitFor(() => expect(asked("theme_choose_machine")).toHaveLength(1));
    await act(async () => release());
    expect(inForce()).toBe(before);
    expect(showing()).toBe(false);
  });

  it("puts yours back once the cards are out of the document without unmounting", async () => {
    core();
    const { container } = render(<Group at={null} />);
    await cards();
    const before = inForce();
    fireEvent.pointerEnter(named("Dusk"));
    await waitFor(() => expect(inForce()).toBe(LIGHT));
    // A project switch takes its view out of the document and keeps it mounted.
    const parent = container.parentElement;
    container.remove();
    await waitFor(() => expect(inForce()).toBe(before));
    parent?.append(container);
  });

  it.each([
    [
      "the window is hidden",
      () => {
        Object.defineProperty(document, "visibilityState", { value: "hidden", configurable: true });
        document.dispatchEvent(new Event("visibilitychange"));
      },
    ],
    ["the window loses the focus", () => window.dispatchEvent(new Event("blur"))],
    [
      "Escape is pressed",
      () => document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true })),
    ],
  ])("puts yours back when %s", async (_, happens) => {
    core();
    render(<Group at={null} />);
    await cards();
    const before = inForce();
    fireEvent.pointerEnter(named("Dusk"));
    await waitFor(() => expect(inForce()).toBe(LIGHT));
    act(happens);
    expect(inForce()).toBe(before);
    Object.defineProperty(document, "visibilityState", { value: "visible", configurable: true });
  });

  it("puts yours back when the pointer leaves the window with no leave on the card (#3095)", async () => {
    core();
    render(<Group at={null} />);
    await cards();
    const before = inForce();
    fireEvent.pointerEnter(named("Dusk"));
    await waitFor(() => expect(inForce()).toBe(LIGHT));
    fireEvent.pointerOut(document, { relatedTarget: null });
    await waitFor(() => expect(inForce()).toBe(before));
  });

  it("keeps the card drawn while the keyboard is on it when the pointer leaves the window (#3095)", async () => {
    core();
    render(<Group at={null} />);
    await cards();
    act(() => named("Dusk").focus());
    await waitFor(() => expect(inForce()).toBe(LIGHT));
    fireEvent.pointerEnter(named("Dusk"));
    fireEvent.pointerOut(document, { relatedTarget: null });
    await new Promise((done) => setTimeout(done, 5));
    expect(inForce()).toBe(LIGHT);
  });

  it("keeps the card drawn while the keyboard is still on it after the pointer leaves", async () => {
    core();
    render(<Group at={null} />);
    await cards();
    act(() => named("Dusk").focus());
    await waitFor(() => expect(inForce()).toBe(LIGHT));
    fireEvent.pointerEnter(named("Dusk"));
    fireEvent.pointerLeave(screen.getByRole("listbox"));
    await new Promise((done) => setTimeout(done, 5));
    expect(inForce()).toBe(LIGHT);
  });
});

describe("choosing nothing, and a refused card (#2331 review)", () => {
  it("takes the choice here away with Use the default, and Undo brings it back", async () => {
    const { asked } = core(shown({ yours: mine("dusk", "dark"), drawn: "machine:dusk" }));
    render(<Group at={null} />);
    await cards();
    fireEvent.click(screen.getByRole("button", { name: "Use the default" }));
    await waitFor(() =>
      expect(asked("theme_choose_machine")).toEqual([
        { theme: null, appearance: null, from: null },
      ]),
    );
    fireEvent.click(await screen.findByRole("button", { name: "Undo" }));
    await waitFor(() =>
      expect(asked("theme_choose_machine")[1]).toEqual({
        theme: "dusk",
        appearance: "dark",
        from: null,
      }),
    );
  });

  it("offers no Use the default, and no appearance to pick, while nothing is chosen here", async () => {
    const { asked } = core();
    render(<Group at={null} />);
    await cards();
    expect(screen.getByRole("button", { name: "Use the default" })).toBeDisabled();
    const appearance = await screen.findByRole("radiogroup");
    for (const radio of within(appearance).getAllByRole("radio")) expect(radio).toBeDisabled();
    expect(appearance).toHaveAccessibleDescription(
      "Nothing is chosen here: choose a theme above to pick its appearance.",
    );
    expect(asked("theme_choose_machine")).toEqual([]);
  });

  it("chooses nothing on a refused card, and says why it is refused", async () => {
    const BROKEN = card({
      id: "broken",
      name: "Broken",
      refused: true,
      said: ["surface.base is not a colour"],
      appearances: [],
    });
    const { asked } = core(shown({ cards: [CLASSIC, BROKEN] }));
    render(<Group at={null} />);
    await cards();
    const broken = named("Broken");
    // It says it cannot be chosen, keeps the keyboard so why is read, and nothing chooses it.
    expect(broken).toHaveAttribute("aria-disabled", "true");
    act(() => broken.focus());
    expect(broken).toHaveFocus();
    expect(broken).toHaveAccessibleDescription("surface.base is not a colour");
    fireEvent.click(broken);
    fireEvent.keyDown(broken, { key: "Enter" });
    await new Promise((done) => setTimeout(done, 5));
    expect(asked("theme_choose_machine")).toEqual([]);
  });
});

describe("a card's pictures when the painter is busy (#2331 review)", () => {
  it("asks again after the wait on the app's own busy answer, and keeps no failure", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    // Busy for the first ask, then every picture is drawn.
    const { asked } = core(
      { ...GALLERY, cards: [DUSK] },
      { painter: (n) => (n === 1 ? SPECIMEN_BUSY : undefined) },
    );
    render(<Group at={null} />);
    await cards();
    await waitFor(() => expect(asked("theme_specimen")).toHaveLength(1));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(BUSY_WAIT_MS);
    });
    await waitFor(() => expect(asked("theme_specimen")).toHaveLength(2));
    await waitFor(() => expect(document.querySelector("img")).not.toBeNull());
  });

  it("asks a busy painter again every second, past a chat's whole turn, until it draws (#2751)", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    // Busy for as long as a chat's lookup and two pictures take, then drawn.
    const { asked } = core(
      { ...GALLERY, cards: [DUSK] },
      { painter: (n) => (n <= 16 ? SPECIMEN_BUSY : undefined) },
    );
    render(<Group at={null} />);
    await cards();
    await waitFor(() => expect(asked("theme_specimen")).toHaveLength(1));
    expect(BUSY_WAIT_MS).toBeLessThanOrEqual(1_000);
    for (let n = 2; n <= 17; n++) {
      await act(async () => {
        await vi.advanceTimersByTimeAsync(BUSY_WAIT_MS);
      });
      await waitFor(() => expect(asked("theme_specimen")).toHaveLength(n));
    }
    await waitFor(() => expect(document.querySelector("img")).not.toBeNull());
  });

  it("stops asking a busy painter for a card that left the screen", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const { asked } = core({ ...GALLERY, cards: [DUSK] }, { painter: () => SPECIMEN_BUSY });
    const shown = render(<Group at={null} />);
    await cards();
    await waitFor(() => expect(asked("theme_specimen")).toHaveLength(1));
    shown.unmount();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(BUSY_FOR_MS);
    });
    // Its card is gone: the painter is not asked again.
    expect(asked("theme_specimen")).toHaveLength(1);
  });

  it("asks again for a picture that could not be drawn the next time its card is shown", async () => {
    const { asked } = core(
      { ...GALLERY, cards: [DUSK] },
      { painter: (n) => (n === 1 ? "no webview here" : undefined) },
    );
    const first = render(<Group at={null} />);
    await cards();
    await waitFor(() => expect(asked("theme_specimen")).toHaveLength(1));
    first.unmount();
    render(<Group at={null} />);
    await cards();
    await waitFor(() => expect(asked("theme_specimen")).toHaveLength(2));
  });
});

describe("a choice of a theme the level no longer has (#2331 review)", () => {
  it("says so on the appearance, rather than that nothing is chosen", async () => {
    core(shown({ yours: mine("gone", null) }));
    render(<Group at={null} />);
    const appearance = await screen.findByRole("radiogroup");
    expect(appearance).toHaveAccessibleDescription(
      "Your choice here, gone, is no theme this level has, so the levels underneath decide: choose a theme above to pick its appearance.",
    );
  });
});

describe("two folders of one name (#2516)", () => {
  const THEIRS = card({ id: "brand", name: "Brand, the team's", from: "project" });
  const OURS = card({ id: "brand", name: "Brand, mine", from: "machine" });

  it("are two cards, each saying its level, and a press names the level it picked", async () => {
    const { asked } = core(shown({ cards: [CLASSIC, THEIRS, OURS] }));
    render(<Group at={{ plane: PLANE, workspace: null }} />);
    await cards();
    expect(named("Brand, the team's")).toHaveTextContent("This project's");
    expect(named("Brand, mine")).toHaveTextContent("Yours, on this machine");
    fireEvent.click(named("Brand, mine"));
    await waitFor(() =>
      expect(asked("theme_choose")).toEqual([
        { plane: PLANE, workspace: null, theme: "brand", appearance: null, from: "machine" },
      ]),
    );
  });

  it("marks the card of the level the choice names, and no other of its name", async () => {
    core(
      shown({
        cards: [CLASSIC, THEIRS, OURS],
        yours: mine("brand", null, "machine"),
        drawn: "machine:brand",
      }),
    );
    render(<Group at={{ plane: PLANE, workspace: null }} />);
    await cards();
    expect(named("Brand, mine")).toHaveAttribute("aria-selected", "true");
    expect(named("Brand, the team's")).toHaveAttribute("aria-selected", "false");
  });
});

describe("the chosen theme's options (#2527)", () => {
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
    level: "project-choice",
  };
  const LOUD: WindowOption = {
    id: "loud",
    label: "Loud",
    description: "",
    kind: "toggle",
    values: [
      { id: "off", label: "Off" },
      { id: "on", label: "On" },
    ],
    value: "off",
    default: "off",
    chosen: false,
    level: null,
  };

  it("shows each value with the level that chose it, and keeps a value picked at the level", async () => {
    const { asked } = core(
      shown({
        yours: mine("dusk", null, "machine"),
        drawn: "machine:dusk",
        options: [ACCENT, LOUD],
      }),
    );
    render(<Group at={{ plane: PLANE, workspace: "alpha" }} />);
    const options = await screen.findByRole("region", { name: "Options" });
    expect(
      within(options)
        .getByText(/^Accent/)
        .closest("p"),
    ).toHaveTextContent("Project");
    expect(within(options).getByText(/^Loud/).closest("p")).toHaveTextContent("Theme default");
    const loud = within(options).getByRole("radiogroup", { name: "Loud" });
    fireEvent.click(within(loud).getByRole("radio", { name: "On" }));
    await waitFor(() =>
      expect(asked("theme_choose_option")).toEqual([
        { at: { plane: PLANE, workspace: "alpha" }, option: "loud", value: "on" },
      ]),
    );
    // What the core said it dropped is said, and the gallery is asked again.
    expect(
      await within(options).findByText(
        "Accented has no option gone any more, so your value for it was dropped.",
      ),
    ).toBeTruthy();
    await waitFor(() => expect(asked("theme_gallery").length).toBeGreaterThan(1));
  });

  it("names each option's values by the option alone, described by its line", async () => {
    core(
      shown({
        yours: mine("dusk", null, "machine"),
        drawn: "machine:dusk",
        options: [{ ...ACCENT, description: "The colour of what you act on." }],
      }),
    );
    render(<Group at={null} />);
    const options = await screen.findByRole("region", { name: "Options" });
    const accent = within(options).getByRole("radiogroup", { name: "Accent" });
    expect(accent).toHaveAccessibleDescription("The colour of what you act on.");
  });

  it("goes back to the theme's default with Theme default, which sends no value", async () => {
    const { asked } = core(
      shown({
        yours: mine("dusk", null, "machine"),
        drawn: "machine:dusk",
        options: [ACCENT, LOUD],
      }),
    );
    render(<Group at={null} />);
    const options = await screen.findByRole("region", { name: "Options" });
    const accent = within(options).getByRole("radiogroup", { name: "Accent" });
    const back = within(accent).getByRole("radio", { name: "Theme default (Lavender)" });
    expect(back).not.toBeChecked();
    // A value nothing chose stands on Theme default, and pressing it again keeps nothing.
    const loud = within(options).getByRole("radiogroup", { name: "Loud" });
    expect(within(loud).getByRole("radio", { name: "Theme default (Off)" })).toBeChecked();
    fireEvent.click(back);
    await waitFor(() =>
      expect(asked("theme_choose_option")).toEqual([{ at: null, option: "accent", value: null }]),
    );
  });

  it("says a theme with none has none", async () => {
    core(shown({ yours: mine("dusk", null), drawn: "machine:dusk" }));
    render(<Group at={null} />);
    const options = await screen.findByRole("region", { name: "Options" });
    await waitFor(() => expect(options).toHaveTextContent("The chosen theme has no options."));
  });
});

describe("only the cards on screen are pictured (#2614)", () => {
  /** The screen, as `IntersectionObserver` tells it: `show(card, on)` brings a card's pictures
   *  on screen or takes them off it. */
  function screenOf() {
    const told: ((entries: { target: Element; isIntersecting: boolean }[]) => void)[] = [];
    const watched = new Set<Element>();
    vi.stubGlobal(
      "IntersectionObserver",
      class {
        constructor(callback: (entries: { target: Element; isIntersecting: boolean }[]) => void) {
          told.push(callback);
        }
        observe(target: Element) {
          watched.add(target);
        }
        unobserve(target: Element) {
          watched.delete(target);
        }
        disconnect() {
          watched.clear();
        }
      },
    );
    return {
      show: (name: string, on = true) => {
        const pictures = named(name).firstElementChild;
        if (pictures === null || !watched.has(pictures)) throw new Error(`${name} is not watched`);
        act(() => told.forEach((tell) => tell([{ target: pictures, isIntersecting: on }])));
      },
    };
  }

  const many = (n: number) =>
    Array.from({ length: n }, (_, i) => card({ id: `theme-${i}`, name: `Theme ${i}` }));

  it("asks for no picture of a card off screen, and for each as it scrolls in", async () => {
    const shown = screenOf();
    const { asked } = core({ ...GALLERY, cards: many(6) });
    render(<Group at={null} />);
    await cards();
    shown.show("Theme 0");
    shown.show("Theme 1");
    await waitFor(() => expect(asked("theme_specimen")).toHaveLength(2));
    await new Promise((done) => setTimeout(done, 10));
    expect(asked("theme_specimen").map((one) => one.theme)).toEqual(["theme-0", "theme-1"]);
    shown.show("Theme 4");
    await waitFor(() => expect(asked("theme_specimen")).toHaveLength(3));
    expect(asked("theme_specimen")[2]).toMatchObject({ theme: "theme-4" });
  });

  it("does not ask for a picture still waiting its turn once its card leaves the screen", async () => {
    const shown = screenOf();
    const { asked, paint } = core({ ...GALLERY, cards: many(3) }, { pictures: "held" });
    render(<Group at={null} />);
    await cards();
    shown.show("Theme 0");
    shown.show("Theme 1");
    shown.show("Theme 2");
    await waitFor(() => expect(asked("theme_specimen")).toHaveLength(1));
    // Scrolled past before its turn.
    shown.show("Theme 1", false);
    await act(async () => paint());
    await waitFor(() => expect(asked("theme_specimen")).toHaveLength(2));
    await act(async () => paint());
    await new Promise((done) => setTimeout(done, 10));
    expect(asked("theme_specimen").map((one) => one.theme)).toEqual(["theme-0", "theme-2"]);
  });

  it(`keeps at most ${MOST_KEPT} pictures in all, and asks again for one let go`, async () => {
    const shown = screenOf();
    const { asked } = core({ ...GALLERY, cards: many(MOST_KEPT + 4) });
    render(<Group at={null} />);
    await cards();
    // Scrolled through every card, one at a time, each pictured before it leaves.
    for (let i = 0; i < MOST_KEPT + 4; i++) {
      shown.show(`Theme ${i}`);
      await waitFor(() => expect(asked("theme_specimen")).toHaveLength(i + 1));
      await waitFor(() => expect(named(`Theme ${i}`).querySelector("img")).not.toBeNull());
      expect(keptSpecimens()).toBeLessThanOrEqual(MOST_KEPT);
      shown.show(`Theme ${i}`, false);
    }
    expect(keptSpecimens()).toBe(MOST_KEPT);
    // The first, shown least recently, was let go: back on screen, it is asked for again.
    shown.show("Theme 0");
    await waitFor(() => expect(asked("theme_specimen")).toHaveLength(MOST_KEPT + 5));
    expect(asked("theme_specimen").at(-1)).toMatchObject({ theme: "theme-0" });
    // The last, shown most recently, is kept.
    shown.show(`Theme ${MOST_KEPT + 3}`);
    await new Promise((done) => setTimeout(done, 10));
    expect(asked("theme_specimen")).toHaveLength(MOST_KEPT + 5);
  });

  it("watches a refused card's pictures once it becomes a theme while the tab is open", async () => {
    const shown = screenOf();
    const fixed = card({ id: "broken", name: "Broken" });
    const gallery: ThemeGallery = {
      ...GALLERY,
      cards: [{ ...fixed, refused: true, said: ["surface.base is not a colour"], appearances: [] }],
    };
    const { asked } = core(gallery);
    render(<Group at={{ plane: PLANE, workspace: null }} />);
    await cards();
    expect(named("Broken")).toHaveAttribute("aria-disabled", "true");
    // Its folder is put right on disk: the same card, now a theme, with a new stamp.
    gallery.cards = [{ ...fixed, stamp: "1" }];
    await act(async () => {
      await emit(THEME_FOLDERS_CHANGED, { survey: false });
    });
    await waitFor(() => expect(named("Broken")).not.toHaveAttribute("aria-disabled"));
    shown.show("Broken");
    await waitFor(() => expect(asked("theme_specimen")).toHaveLength(1));
    expect(asked("theme_specimen")[0]).toMatchObject({ theme: "broken" });
  });

  it("never lets go of a picture a card on screen shows", async () => {
    const shown = screenOf();
    const { asked } = core({ ...GALLERY, cards: many(MOST_KEPT + 2) });
    render(<Group at={null} />);
    await cards();
    for (let i = 0; i < MOST_KEPT + 2; i++) shown.show(`Theme ${i}`);
    await waitFor(() => expect(asked("theme_specimen")).toHaveLength(MOST_KEPT + 2));
    await waitFor(() => expect(document.querySelectorAll("img")).toHaveLength(MOST_KEPT + 2));
    expect(keptSpecimens()).toBe(MOST_KEPT + 2);
    // Off screen, the ones past the bound are let go.
    for (let i = 0; i < MOST_KEPT + 2; i++) shown.show(`Theme ${i}`, false);
    expect(keptSpecimens()).toBe(MOST_KEPT);
  });
});

describe("nothing a gallery test starts outlives it (#2698)", () => {
  it("drops a picture asked before the pictures were forgotten, and starts nothing with it", async () => {
    const before = core({ ...GALLERY, cards: [DUSK] }, { pictures: "held" });
    render(<Group at={null} />);
    await cards();
    await waitFor(() => expect(before.asked("theme_specimen")).toHaveLength(1));
    // What the end of a test does, with the picture still being painted.
    cleanup();
    forgetGalleries();
    forgetSpecimens();
    const after = core(GALLERY, { pictures: "held" });
    render(<Group at={null} />);
    await cards();
    await waitFor(() => expect(after.asked("theme_specimen")).toHaveLength(1));
    // The first test's picture is done: it starts no second one beside the one painting.
    await act(async () => before.paint());
    await new Promise((done) => setTimeout(done, 10));
    expect(after.asked("theme_specimen")).toHaveLength(1);
    await act(async () => after.paint());
    await waitFor(() => expect(after.asked("theme_specimen")).toHaveLength(2));
    expect(after.most()).toBe(1);
  });

  it("ends the wait for a busy painter when the pictures are forgotten", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const { asked } = core({ ...GALLERY, cards: [DUSK] }, { painter: () => SPECIMEN_BUSY });
    render(<Group at={null} />);
    await cards();
    await waitFor(() => expect(asked("theme_specimen")).toHaveLength(1));
    await waitFor(() => expect(vi.getTimerCount()).toBeGreaterThan(0));
    cleanup();
    forgetSpecimens();
    expect(vi.getTimerCount()).toBe(0);
  });
});

describe("your own theme.json, drawn (#2615)", () => {
  /** What the levels draw when it is the person's own `theme.json`: no card is drawn. */
  const theirs = (over: Partial<ThemeGallery>) =>
    shown({ drawn: null, chosen: null, level: "machine-default", ...over });

  it("says it is drawn and follows the system, and marks no card", async () => {
    core(
      theirs({
        from: "your own theme.json on this machine",
        theirs: { follows_system: true, note: null },
      }),
    );
    render(<Group at={null} />);
    await cards();
    expect(
      screen.getByText(/^Drawn here: your own theme.json, following the system/),
    ).toHaveTextContent("This machine");
    for (const one of screen.getAllByRole("option"))
      expect(one).toHaveAttribute("aria-selected", "false");
    expect(screen.getByRole("region", { name: "Appearance" })).toHaveTextContent(
      "Your own theme.json is drawn here, and it is no card",
    );
    expect(screen.getByRole("region", { name: "Theme check" })).not.toHaveTextContent(
      "Reading the themes",
    );
  });

  it("says a theme.json of one appearance stays in it when the old system id is chosen", async () => {
    core(
      theirs({
        yours: mine("system", null),
        level: "machine-default",
        theirs: {
          follows_system: false,
          note: "Mine has only a dark appearance, so following the system stays dark",
        },
      }),
    );
    render(<Group at={null} />);
    await cards();
    expect(
      screen.getByText(
        /^Chosen here: your own theme.json \(Mine has only a dark appearance, so following the system stays dark\)/,
      ),
    ).toBeTruthy();
    // The old id names the person's own theme, not Purlis Classic.
    expect(named("Purlis Classic")).toHaveAttribute("aria-selected", "false");
  });

  it("marks Purlis Classic for the old system id with no theme of your own, following the system", async () => {
    core(shown({ yours: mine("system", null), chosen: "purlis-classic" }));
    render(<Group at={null} />);
    await cards();
    expect(named("Purlis Classic")).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText(/^Chosen here: Purlis Classic/)).toBeTruthy();
  });
});
