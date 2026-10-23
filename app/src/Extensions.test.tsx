// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import {
  drawThemeFor,
  Extensions,
  forgetExtensionThemes,
  THEME_FOLDERS_CHANGED,
  themeTroubleSaid,
  troubleSaid,
} from "./Extensions";
import type { PlaneId } from "./bindings";
import {
  BUILT_IN,
  CROSSFADE,
  DEFAULT_THEME,
  drawIn,
  inForce,
  onDrawn,
  THEME_ALIASES,
  type Theme,
} from "./theme/theme";
import { forgetProjectThemes } from "./projectTheme";
import { forgetYourEditor, setYourEditor } from "./yourEditor";
import { aboutThisMachine, sayAboutThisMachine } from "./windowprefs";
import { PURLIS_LIGHT } from "./theme/handed.testkit";

/**
 * A colour for a theme an extension contributes, taken out of charter's own light theme
 * rather than written here.
 *
 * `literals.test.ts` refuses a hex literal anywhere but `src/theme/`, and it is right to: the
 * rule is what keeps a second palette from growing back. A fixture is not an exception to it —
 * any value works, so the one that already exists is the one to use. It differs from the dark
 * theme's, which is what these tests need it to do.
 */
const OTHER = PURLIS_LIGHT.values["surface.base"];

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  cleanup();
  clearMocks();
  drawIn(DEFAULT_THEME);
  forgetProjectThemes();
});

/** The question charter asks, as the core builds it. The two sentences are the core's own. */
const RUNS_AS_YOU =
  "purlis does not confine an extension. It runs as you do, with your files, your network " +
  "and your ability to start programs — nothing purlis has stops one reading your vaults, " +
  "writing purlis's own settings, or changing what your next launch runs. What is listed " +
  "above is what this extension DECLARES, not what it is LIMITED to.";
const FINGERPRINTED =
  "purlis has read every file in this extension's directory — not only the ones it " +
  "declares — and will ask again if any of them changes, or if one is added or taken away. " +
  "That catches an extension that changed under you. It is not a defence against one written " +
  "to deceive you, and it is not a boundary.";
/** `purlis_core::extension::state_note("cache")`, for an extension that declares one. */
const STATE_NOTE =
  "purlis does not read 'cache/'. That is this extension's state directory: the one place " +
  "it may write without charter asking again. purlis refuses to load the extension if that " +
  "directory holds a link or a program, so what is in there is data — but purlis cannot stop " +
  "a program it has already read from treating its own data as code.";

const ASK = {
  id: "solarized",
  name: "Solarized",
  path: "/home/dev/ext/solarized",
  declares: ["a theme, “Solarized Dark”"],
  fingerprint: "a".repeat(64),
  first: true,
  runs_as_you: RUNS_AS_YOU,
  fingerprint_note: FINGERPRINTED,
  state_note: null as string | null,
};

type Row = {
  id: string;
  name: string;
  path: string;
  standing: string;
  source: string;
  on: boolean;
  themes_in_force: string[];
  theme_trouble: { name: string; complaints: string[]; refused: boolean }[];
  declares: string[];
  refused: string | null;
  ask: typeof ASK | null;
};

/** What the core's theme engine answers `theme_in_force` with. */
type InForce = {
  theme: Theme | null;
  complaints: string[];
  follows_system: boolean;
  floors?: string | null;
};
const KEEPS: InForce = { theme: null, complaints: [], follows_system: false };

/** A theme as the core resolves one: complete, `over` laid on the dark built-in. */
function resolved(name: string, over: Partial<Theme["values"]> = {}): Theme {
  return { ...DEFAULT_THEME, name, values: { ...DEFAULT_THEME.values, ...over } };
}

function core({
  rows = [] as Row[],
  inForce: answer = (() => KEEPS) as (asked: Record<string, unknown>) => InForce,
  unreadable = null as string | null,
  dropped = [] as string[],
  picks = null as string | null,
  iconThemes = [] as { extension: string; name: string; text: string }[],
}) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    if (cmd === "installed_extensions")
      return {
        built_in_themes: ["purlis-dark", "purlis-light"],
        extensions: rows,
        unreadable,
        dropped,
      };
    if (cmd === "theme_in_force") return answer((args ?? {}) as Record<string, unknown>);
    if (cmd === "extension_icon_themes") return iconThemes;
    if (cmd === "pick_extension") return picks;
    if (cmd === "install_extension") return ASK;
    if (cmd === "approve_extension") return null;
    if (cmd === "forget_extension") return null;
    if (cmd === "set_extension_on") return null;
    throw new Error(`the window asked for ${cmd}, which this test did not expect`);
  });
  return asked;
}

const newRow: Row = {
  id: "solarized",
  name: "Solarized",
  path: "/home/dev/ext/solarized",
  standing: "new",
  source: "installed",
  on: true,
  themes_in_force: [],
  theme_trouble: [],
  declares: ASK.declares,
  refused: null,
  ask: ASK,
};

/** Persona statistics as the app ships it (charter-app#339). */
const builtInRow: Row = {
  id: "persona-statistics",
  name: "Persona statistics",
  path: "/Applications/charter.app/Contents/Resources/extensions/persona-statistics",
  standing: "approved",
  source: "app",
  on: true,
  themes_in_force: [],
  theme_trouble: [],
  declares: ["a view, “Statistics”"],
  refused: null,
  ask: null,
};

describe("what a contributed icon theme got wrong (#1145)", () => {
  const approved = { ...newRow, standing: "approved", ask: null };
  /** An icon theme whose `file` names no symbol and whose one symbol is coloured with a literal. */
  const WRONG = JSON.stringify({
    symbols: {
      leaf: { viewBox: "0 0 16 16", paths: [{ d: "M0 0h16v16H0z", tone: "red" }] },
    },
    file: "nothing",
  });

  it("says each complaint on the row of the extension that contributes it", async () => {
    core({
      rows: [approved, builtInRow],
      iconThemes: [{ extension: "solarized", name: "Leaves", text: WRONG }],
    });
    render(<Extensions onClose={() => undefined} />);

    const row = (await screen.findByText("Solarized")).closest("li") as HTMLElement;
    const said = await within(row).findByText(/^Icon theme “Leaves”/);
    expect(said).toHaveTextContent(
      /drawn as purlis’s own where it is wrong: a path of leaf is coloured "red", which is not one of icon\./,
    );
    expect(said).toHaveTextContent(/file names "nothing", which is no symbol\.$/);
    const other = screen.getByText("Persona statistics").closest("li") as HTMLElement;
    expect(within(other).queryByText(/^Icon theme/)).toBeNull();
  });

  it("says nothing of an icon theme that is right", async () => {
    core({
      rows: [approved],
      iconThemes: [{ extension: "solarized", name: "Plain", text: "{}" }],
    });
    render(<Extensions onClose={() => undefined} />);

    await screen.findByText("Solarized");
    await waitFor(() => expect(screen.queryByText(/^Icon theme/)).toBeNull());
  });

  it("says one that is not JSON at all", async () => {
    core({
      rows: [approved],
      iconThemes: [{ extension: "solarized", name: "Broken", text: "{" }],
    });
    render(<Extensions onClose={() => undefined} />);

    expect(
      await screen.findByText(
        "Icon theme “Broken”, drawn as purlis’s own where it is wrong: an icon theme is JSON, and this is not.",
      ),
    ).toBeInTheDocument();
  });

  it("bounds what it says: three complaints, each cut short, and how many more", () => {
    const long = "x".repeat(400);
    const said = troubleSaid({ name: "Many", complaints: [long, "b", "c", "d", "e"] });
    expect(said).toContain(`${"x".repeat(159)}…; b; c; and 2 more.`);
    expect(said).not.toContain("d;");
    expect(said.length).toBeLessThan(300);
  });

  it("says the extension's name and complaints as plain text, with no bidi or control mark", () => {
    const said = troubleSaid({
      name: `Seti\u202e${"n".repeat(200)}`,
      complaints: ['folder names "x\u0007\u2066y", which is no symbol'],
    });
    for (const mark of ["\u0007", "\u202e", "\u2066"]) expect(said).not.toContain(mark);
    expect(said).toContain(`Icon theme “Seti${"n".repeat(59)}…”`);
    expect(said).toContain(`folder names "xy", which is no symbol`);
  });
});

describe("what an extension's colour theme got wrong (#2321)", () => {
  const approved = { ...newRow, standing: "approved", ask: null, themes_in_force: ["Dim"] };

  it("says on the extension's row what the theme engine left out", async () => {
    core({
      rows: [
        {
          ...approved,
          theme_trouble: [
            {
              name: "Dim",
              complaints: ["text.primry is not a purlis token, so it is left out"],
              refused: false,
            },
          ],
        },
        builtInRow,
      ],
    });
    render(<Extensions onClose={() => undefined} />);

    const row = (await screen.findByText("Solarized")).closest("li") as HTMLElement;
    expect(
      within(row).getByText(
        "Theme “Dim” is drawn without what purlis left out: text.primry is not a purlis token, so it is left out.",
      ),
    ).toHaveAttribute("role", "status");
    const other = screen.getByText("Persona statistics").closest("li") as HTMLElement;
    expect(within(other).queryByText(/^Theme “/)).toBeNull();
  });

  it("says a refused theme is not drawn, and why", async () => {
    core({
      rows: [
        {
          ...approved,
          theme_trouble: [
            { name: "Dim", complaints: ["a theme is JSON: expected value"], refused: true },
          ],
        },
      ],
    });
    render(<Extensions onClose={() => undefined} />);

    expect(
      await screen.findByText(
        "Theme “Dim” is not drawn, and Purlis is: a theme is JSON: expected value.",
      ),
    ).toBeInTheDocument();
  });

  it("bounds what it says, as an icon theme's", () => {
    const said = themeTroubleSaid({
      name: `Dim\u202e`,
      complaints: ["x".repeat(400), "b", "c", "d"],
      refused: false,
    });
    expect(said).toContain(`${"x".repeat(159)}…; b; c; and 1 more.`);
    expect(said).not.toContain("\u202e");
  });
});

describe("the extension registry", () => {
  it("says it is reading while the list is asked for (#630)", async () => {
    mockIPC(() => new Promise(() => undefined));
    render(<Extensions onClose={() => undefined} />);
    expect(await screen.findByText("Reading the installed extensions…")).toBeInTheDocument();
  });

  it("says why the list could not be read when the ask itself fails (#630)", async () => {
    mockIPC((cmd) => {
      if (cmd === "installed_extensions") throw new Error("the core is gone");
      return [];
    });
    render(<Extensions onClose={() => undefined} />);
    expect(
      await screen.findByText(/purlis could not read the installed extensions: .*the core is gone/),
    ).toHaveClass("trouble");
  });

  it("says none is installed as a claim, with the way to add one (#630)", async () => {
    core({ rows: [] });
    render(<Extensions onClose={() => undefined} />);
    expect(await screen.findByText("No extensions installed yet")).toBeInTheDocument();
    expect(
      screen.getByText("Add an extension… points purlis at the directory that holds one."),
    ).toBeInTheDocument();
  });

  it("says a refused remove as trouble (#630)", async () => {
    mockIPC((cmd) => {
      if (cmd === "installed_extensions")
        return {
          built_in_themes: [],
          extensions: [{ ...newRow, standing: "approved", ask: null }],
          unreadable: null,
          dropped: [],
        };
      if (cmd === "forget_extension")
        throw "purlis could not forget Solarized: the record is read-only";
      return [];
    });
    render(<Extensions onClose={() => undefined} />);
    await userEvent.click(await screen.findByRole("button", { name: "Remove" }));
    expect(await screen.findByText(/could not forget Solarized/)).toHaveClass("trouble");
  });

  it("marks a built-in extension built-in, with no Remove and no question to ask", async () => {
    core({ rows: [builtInRow, { ...newRow, standing: "approved", ask: null }] });
    render(<Extensions onClose={() => undefined} />);

    const row = (await screen.findByText("Persona statistics")).closest("li") as HTMLElement;
    expect(within(row).getByText("built-in")).toBeInTheDocument();
    expect(within(row).queryByRole("button", { name: "Remove" })).not.toBeInTheDocument();
    expect(within(row).queryByRole("button", { name: "Review" })).not.toBeInTheDocument();
    // An installed one is still removed, as before.
    const installed = screen.getByText("Solarized").closest("li") as HTMLElement;
    expect(within(installed).getByRole("button", { name: "Remove" })).toBeInTheDocument();
    expect(within(installed).queryByText("built-in")).not.toBeInTheDocument();
  });

  it("turns a built-in off on this machine rather than removing it", async () => {
    const asked = core({ rows: [builtInRow] });
    render(<Extensions onClose={() => undefined} />);

    // A switch, named by the extension and the words beside it (#1860).
    const on = await screen.findByRole("switch", { name: /statistics on this machine/i });
    expect(on).toBeChecked();
    await userEvent.click(on);

    await vi.waitFor(() =>
      expect(asked).toContainEqual({
        cmd: "set_extension_on",
        args: { id: "persona-statistics", on: false },
      }),
    );
  });

  it("says a built-in turned off on this machine contributes nothing", async () => {
    core({ rows: [{ ...builtInRow, on: false }] });
    render(<Extensions onClose={() => undefined} />);

    expect(await screen.findByRole("switch", { name: /on this machine/i })).not.toBeChecked();
    expect(screen.getByText(/off on this machine — contributing nothing/)).toBeInTheDocument();
  });

  it("names charter's own themes as well as the installed ones", async () => {
    core({ rows: [newRow] });
    render(<Extensions onClose={() => undefined} />);

    expect(await screen.findByText("purlis-dark")).toBeInTheDocument();
    expect(screen.getByText("purlis-light")).toBeInTheDocument();
    expect(screen.getByText("Solarized")).toBeInTheDocument();
  });

  it("says that an unapproved extension is contributing nothing", async () => {
    core({ rows: [newRow] });
    render(<Extensions onClose={() => undefined} />);

    expect(await screen.findByText(/not approved — contributing nothing/)).toBeInTheDocument();
  });

  it("says that a changed extension is contributing nothing, rather than going quiet", async () => {
    core({ rows: [{ ...newRow, standing: "changed" }] });
    render(<Extensions onClose={() => undefined} />);

    expect(
      await screen.findByText(/changed since you approved it — contributing nothing/),
    ).toBeInTheDocument();
  });

  it("says why an extension purlis could not read is contributing nothing", async () => {
    core({
      rows: [
        {
          ...newRow,
          ask: null,
          refused: "'/home/dev/ext/solarized/purlis-extension.json' is not there",
        },
      ],
    });
    render(<Extensions onClose={() => undefined} />);

    expect(await screen.findByText(/is not there/)).toBeInTheDocument();
  });

  it("says that an unreadable record puts nothing in force", async () => {
    core({ rows: [], unreadable: "'/home/o/.config/charter/extensions.json' is not JSON" });
    render(<Extensions onClose={() => undefined} />);

    expect(
      await screen.findByText(/nothing an extension declares is in force/),
    ).toBeInTheDocument();
  });

  it("puts the record's two acts in a row under the sentence, not inside it (#1719)", async () => {
    core({ rows: [], unreadable: "'/home/o/.config/charter/extensions.json' is not JSON" });
    render(<Extensions onClose={() => undefined} />);

    const again = await screen.findByRole("button", { name: "Read again" });
    expect(again.closest("p")).toBeNull();
    expect(again.closest(".ui-setting-actions")).not.toBeNull();
    expect(
      within(again.closest(".ui-setting-actions") as HTMLElement).getByRole("button", {
        name: "Open in your editor",
      }),
    ).toBeInTheDocument();
  });

  it("reads an unreadable record again on a press, once it was mended (NO-8, #1233)", async () => {
    let unreadable: string | null = "'/home/o/.config/charter/extensions.json' is not JSON";
    mockIPC((cmd) => {
      if (cmd === "installed_extensions")
        return { built_in_themes: [], extensions: [newRow], unreadable, dropped: [] };
      if (cmd === "theme_in_force") return { theme: null, complaints: [], follows_system: false };
      throw new Error(`the window asked for ${cmd}, which this test did not expect`);
    });
    render(<Extensions onClose={() => undefined} />);
    await screen.findByText(/nothing an extension declares is in force/);

    unreadable = null;
    await userEvent.click(screen.getByRole("button", { name: "Read again" }));

    await waitFor(() =>
      expect(screen.queryByText(/nothing an extension declares is in force/)).toBeNull(),
    );
    expect(screen.getByText("Solarized")).toBeInTheDocument();
  });

  it("opens the unreadable record in your editor, naming the editor and no file (#1296)", async () => {
    const asked: { cmd: string; args: unknown }[] = [];
    mockIPC((cmd, args) => {
      asked.push({ cmd, args });
      if (cmd === "installed_extensions")
        return {
          built_in_themes: [],
          extensions: [],
          unreadable: "'/home/o/.config/charter/extensions.json' is not JSON",
          dropped: [],
        };
      if (cmd === "theme_in_force") return { theme: null, complaints: [], follows_system: false };
      if (cmd === "open_extension_record") return null;
      throw new Error(`the window asked for ${cmd}, which this test did not expect`);
    });
    render(<Extensions onClose={() => undefined} />);
    await screen.findByText(/nothing an extension declares is in force/);

    forgetYourEditor();
    await userEvent.click(screen.getByRole("button", { name: "Open in your editor" }));
    expect(await screen.findByText(/Choose your editor in Settings › You › Editor/)).toBeVisible();
    expect(asked.some(({ cmd }) => cmd === "open_extension_record")).toBe(false);

    setYourEditor("zed");
    await userEvent.click(screen.getByRole("button", { name: "Open in your editor" }));
    await waitFor(() =>
      expect(asked.filter(({ cmd }) => cmd === "open_extension_record")).toEqual([
        { cmd: "open_extension_record", args: { editor: "zed" } },
      ]),
    );
    forgetYourEditor();
  });
});

describe("the consent surface", () => {
  it("says that an extension runs with the operator's own access", async () => {
    // The whole reason the dialog exists. The operator ruled to ship with no sandbox, so the
    // list of contributions is charter's conduct and not a cage — and a prompt that implied a
    // cage would be worse than no prompt.
    core({ rows: [newRow] });
    render(<Extensions onClose={() => undefined} />);
    await userEvent.click(await screen.findByRole("button", { name: "Review" }));

    const said = await screen.findByText(/does not confine an extension/);
    expect(said).toHaveTextContent("runs as you do");
    expect(said).toHaveTextContent("reading your vaults");
    expect(said).toHaveTextContent("DECLARES");
    expect(said).toHaveTextContent("not what it is LIMITED to");
  });

  it("says that the fingerprint is not a boundary", async () => {
    core({ rows: [newRow] });
    render(<Extensions onClose={() => undefined} />);
    await userEvent.click(await screen.findByRole("button", { name: "Review" }));

    expect(await screen.findByText(/it is not a boundary/)).toBeInTheDocument();
  });

  it("renders the core's sentences as given rather than words of its own", async () => {
    // A dialog that composed its own reassurance would drift kinder than the truth one edit at
    // a time. These two strings come out of `purlis_core::extension` and are pinned there.
    core({ rows: [newRow] });
    render(<Extensions onClose={() => undefined} />);
    await userEvent.click(await screen.findByRole("button", { name: "Review" }));

    expect(await screen.findByText(RUNS_AS_YOU)).toBeInTheDocument();
    expect(screen.getByText(FINGERPRINTED)).toBeInTheDocument();
  });

  it("names the one directory purlis does not read, when there is one", async () => {
    // charter-app#152. The fingerprint note says charter read every file in the directory; an
    // extension with a state directory has one exception to that, and the exception belongs on
    // the screen where the operator says yes.
    core({ rows: [{ ...newRow, ask: { ...ASK, state_note: STATE_NOTE } }] });
    render(<Extensions onClose={() => undefined} />);
    await userEvent.click(await screen.findByRole("button", { name: "Review" }));

    expect(await screen.findByText(STATE_NOTE)).toBeInTheDocument();
  });

  it("says nothing about a state directory for an extension that declares none", async () => {
    // The ordinary case. A dialog that mentioned an exception every extension does not have
    // would teach the operator to skip the paragraph that matters when one does.
    core({ rows: [newRow] });
    render(<Extensions onClose={() => undefined} />);
    await userEvent.click(await screen.findByRole("button", { name: "Review" }));

    await screen.findByText(FINGERPRINTED);
    expect(screen.queryByText(/does not read/)).not.toBeInTheDocument();
  });

  it("names every contribution it was given", async () => {
    core({
      rows: [
        {
          ...newRow,
          declares: [
            "a theme, “Midnight”",
            "a program, bin/x — purlis starts it only when you open one of this extension's views",
          ],
          ask: {
            ...ASK,
            declares: [
              "a theme, “Midnight”",
              "a program, bin/x — purlis starts it only when you open one of this extension's views",
            ],
          },
        },
      ],
    });
    render(<Extensions onClose={() => undefined} />);
    await userEvent.click(await screen.findByRole("button", { name: "Review" }));

    expect(await screen.findByText(/Midnight/)).toBeInTheDocument();
    expect(screen.getByText(/starts it only when you open/)).toBeInTheDocument();
  });

  it("names each capability the extension asks for, as the core words it", async () => {
    // ADR 0053: every capability is visible where the operator says yes. The line is the
    // core's (`Capability::asks`), listed first, and the dialog draws it as given.
    const capability = "the capability “probe” — purlis's test capability, which grants nothing";
    const declares = [capability, "a theme, “Midnight”"];
    core({ rows: [{ ...newRow, declares, ask: { ...ASK, declares } }] });
    render(<Extensions onClose={() => undefined} />);
    await userEvent.click(await screen.findByRole("button", { name: "Review" }));

    const listed = within(await screen.findByRole("dialog")).getAllByRole("listitem");
    expect(listed.map((item) => item.textContent)).toEqual(declares);
  });

  it("carries back the fingerprint that was shown, and not one fetched again", async () => {
    // charter-app#123's shape: what was drawn and what is recorded are one read.
    const asked = core({ rows: [newRow] });
    render(<Extensions onClose={() => undefined} />);
    await userEvent.click(await screen.findByRole("button", { name: "Review" }));
    await userEvent.click(await screen.findByRole("button", { name: "Trust it" }));

    const approved = asked.find((call) => call.cmd === "approve_extension");
    expect(approved?.args).toMatchObject({
      id: "solarized",
      path: "/home/dev/ext/solarized",
      fingerprint: ASK.fingerprint,
    });
  });

  it("approves nothing when it is cancelled", async () => {
    const asked = core({ rows: [newRow] });
    render(<Extensions onClose={() => undefined} />);
    await userEvent.click(await screen.findByRole("button", { name: "Review" }));
    await userEvent.click(await screen.findByRole("button", { name: "Cancel" }));

    expect(asked.some((call) => call.cmd === "approve_extension")).toBe(false);
  });

  it("lists an extension added and left unapproved, without the dialog opened again", async () => {
    // The core writes it into the record unapproved as it asks; a Cancel approves nothing, and
    // the list says it is there, contributing nothing.
    const rows: Row[] = [];
    mockIPC((cmd) => {
      if (cmd === "installed_extensions")
        return {
          built_in_themes: ["purlis-dark"],
          extensions: [...rows],
          unreadable: null,
          dropped: [],
        };
      if (cmd === "extension_icon_themes") return [];
      if (cmd === "theme_in_force") return { theme: null, complaints: [], follows_system: false };
      if (cmd === "pick_extension") return "/home/dev/ext/solarized";
      if (cmd === "install_extension") {
        rows.push(newRow);
        return ASK;
      }
      throw new Error(`the window asked for ${cmd}, which this test did not expect`);
    });
    render(<Extensions onClose={() => undefined} />);
    await userEvent.click(await screen.findByRole("button", { name: "Add an extension…" }));
    await userEvent.click(await screen.findByRole("button", { name: "Cancel" }));

    expect(await screen.findByText("Solarized")).toBeInTheDocument();
  });

  it("asks nothing about an extension that contributes only themes (ADR 0098)", async () => {
    const rows: Row[] = [];
    const asked: string[] = [];
    mockIPC((cmd) => {
      asked.push(cmd);
      if (cmd === "installed_extensions")
        return {
          built_in_themes: ["purlis-dark"],
          extensions: [...rows],
          unreadable: null,
          dropped: [],
        };
      if (cmd === "extension_icon_themes") return [];
      if (cmd === "theme_in_force") return { theme: null, complaints: [], follows_system: false };
      if (cmd === "pick_extension") return "/home/dev/ext/solarized";
      if (cmd === "install_extension") {
        rows.push({
          ...newRow,
          standing: "themes",
          themes_in_force: ["Solarized Dark"],
          ask: null,
        });
        return null;
      }
      throw new Error(`the window asked for ${cmd}, which this test did not expect`);
    });
    render(<Extensions onClose={() => undefined} />);
    await userEvent.click(await screen.findByRole("button", { name: "Add an extension…" }));

    expect(await screen.findByText("themes only — needs no approval")).toBeInTheDocument();
    expect(screen.queryByText(RUNS_AS_YOU)).toBeNull();
    expect(screen.queryByRole("button", { name: "Review" })).toBeNull();
    expect(asked).not.toContain("approve_extension");
  });

  it("asks before it approves, and never the other way round", async () => {
    // Installing is not consenting. The picker and the yes are two clicks because they are two
    // questions, and collapsing them approves something before it is drawn.
    const asked = core({ rows: [], picks: "/home/dev/ext/solarized" });
    render(<Extensions onClose={() => undefined} />);
    await userEvent.click(await screen.findByRole("button", { name: "Add an extension…" }));

    expect(await screen.findByText(RUNS_AS_YOU)).toBeInTheDocument();
    expect(asked.some((call) => call.cmd === "approve_extension")).toBe(false);
  });
});

describe("the theme in force, as the core's theme engine resolves it (spec #2313)", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  /** The operating system's appearance, as `matchMedia` answers it, and a way to change it. */
  function system(light: boolean) {
    const heard = new Set<(event: { matches: boolean }) => void>();
    const query = {
      get matches() {
        return light;
      },
      addEventListener: (_: string, listen: (event: { matches: boolean }) => void) =>
        heard.add(listen),
      removeEventListener: (_: string, listen: (event: { matches: boolean }) => void) =>
        heard.delete(listen),
    };
    vi.stubGlobal("matchMedia", (media: string) =>
      media === "(prefers-color-scheme: light)" ? query : { ...query, matches: false },
    );
    return (to: boolean) => {
      light = to;
      for (const listen of heard) listen({ matches: to });
    };
  }

  it("asks the core with the project's pick, what it has on, and the system's appearance", async () => {
    const asked = core({});
    system(true);
    await drawThemeFor(new Set(["solarized"]), "solarized/Solarized Dark");
    await drawThemeFor("every");
    expect(asked.filter((call) => call.cmd === "theme_in_force").map((call) => call.args)).toEqual([
      {
        pick: "solarized/Solarized Dark",
        on: ["solarized"],
        light: true,
        moreContrast: false,
        reread: false,
        person: null,
        at: null,
        kept: null,
      },
      {
        pick: null,
        on: null,
        light: true,
        moreContrast: false,
        reread: false,
        person: null,
        at: null,
        kept: null,
      },
    ]);
  });

  it("asks the core for the project and workspace in front, whose theme levels it reads (#2323)", async () => {
    const asked = core({});
    system(false);
    await drawThemeFor(new Set(), "brand", { plane: "p-1" as PlaneId, workspace: "alpha" });
    await drawThemeFor(new Set(), "brand", { plane: "p-1" as PlaneId });
    expect(
      asked.filter((call) => call.cmd === "theme_in_force").map((call) => call.args.at),
    ).toEqual([
      { plane: "p-1", workspace: "alpha" },
      { plane: "p-1", workspace: null },
    ]);
    // Today's in-plane picks are mapped into the person's choices once for each place.
    await drawThemeFor(new Set(), "brand", { plane: "p-1" as PlaneId, workspace: "alpha" });
    expect(
      asked
        .filter((call) => call.cmd === "theme_levels_adopt")
        .map((call) => [call.args.plane, call.args.workspace]),
    ).toEqual([
      ["p-1", "alpha"],
      ["p-1", null],
    ]);
  });

  it("crossfades a switch to another workspace's theme, and draws the first at once (#2323)", async () => {
    // `purlis-light` is not handed: the core answers it (#3034).
    core({ inForce: () => ({ ...KEEPS, theme: PURLIS_LIGHT }) });
    const root = document.documentElement;
    await drawThemeFor(new Set(), "purlis-dark", { plane: "p-1" as PlaneId, workspace: "alpha" });
    expect(root.classList.contains(CROSSFADE)).toBe(false);
    await drawThemeFor(new Set(), "purlis-light", { plane: "p-1" as PlaneId, workspace: "beta" });
    await expect.poll(() => inForce()).toEqual(PURLIS_LIGHT);
    expect(root.classList.contains(CROSSFADE)).toBe(true);
    await vi.waitFor(() => expect(root.classList.contains(CROSSFADE)).toBe(false));
  });

  it("asks with more contrast while the system asks for it, and again when it stops", async () => {
    // #2328: `prefers-contrast: more` is the engine's input, and its change draws again.
    forgetExtensionThemes();
    let more = true;
    const heard = new Set<() => void>();
    const contrast = "(prefers-contrast: more)";
    vi.stubGlobal("matchMedia", (media: string) => ({
      get matches() {
        return media === contrast && more;
      },
      addEventListener: (_: string, listen: () => void) => {
        if (media === contrast) heard.add(listen);
      },
      removeEventListener: (_: string, listen: () => void) => heard.delete(listen),
    }));
    const asked = core({
      inForce: (args) => ({
        ...KEEPS,
        theme: resolved(args.moreContrast === true ? "Sharp" : "Plain"),
      }),
    });
    await drawThemeFor(new Set(["acme"]), "acme/Sharp");
    expect(inForce().name).toBe("Sharp");

    more = false;
    for (const listen of heard) listen();
    await expect.poll(() => inForce().name).toBe("Plain");
    expect(
      asked.filter((call) => call.cmd === "theme_in_force").map((call) => call.args.moreContrast),
    ).toEqual([true, false]);
  });

  it("draws the native window in the theme drawn, once per change, and the system's while following", async () => {
    // #2328: the traffic lights, menus and dialogs take the drawn appearance, and the window's
    // own colour is its `window.background`.
    forgetExtensionThemes();
    const turn = system(false);
    const asked = core({
      inForce: (args) => ({
        // `purlis-light` is not handed: the core answers it (#3034).
        theme:
          args.pick === "purlis-light" || args.light === true
            ? PURLIS_LIGHT
            : BUILT_IN["purlis-dark"],
        complaints: [],
        follows_system: args.pick === "system",
      }),
    });
    const native = () =>
      asked.filter((call) => call.cmd === "draw_native_window").map((call) => call.args);
    const light = PURLIS_LIGHT;
    const dark = BUILT_IN["purlis-dark"];

    await drawThemeFor(new Set(), "purlis-light");
    await drawThemeFor(new Set(), "purlis-light");
    expect(native()).toEqual([
      {
        appearance: "light",
        background: light.values["window.background"],
        followsSystem: false,
      },
    ]);

    await drawThemeFor(new Set(), "system");
    expect(native().at(-1)).toEqual({
      appearance: "dark",
      background: dark.values["window.background"],
      followsSystem: true,
    });
    turn(true);
    await expect.poll(() => native().at(-1)?.appearance).toBe("light");
    expect(native().at(-1)?.followsSystem).toBe(true);
  });

  it("draws a built-in pick at once, from what the window was handed, asking the core nothing", () => {
    const asked = core({});
    // The default's dark, by its id or the one it had; `purlis-light` asks the core (#3034).
    for (const [pick, drawn] of [
      ["purlis-dark", "purlis-dark"],
      ...Object.entries(THEME_ALIASES).filter(([, now]) => now === "purlis-dark"),
      ["purlis-dark", "purlis-dark"],
    ]) {
      // No `await`: the switch has drawn by the time it returns, so no frame shows the last one.
      void drawThemeFor(new Set(["solarized"]), pick);
      expect(inForce(), pick).toBe(BUILT_IN[drawn]);
    }
    expect(asked.some((call) => call.cmd === "theme_in_force")).toBe(false);
  });

  it("draws what the core answered, and keeps the window when it answers no theme", async () => {
    let answer: InForce = {
      ...KEEPS,
      theme: resolved("Solarized Dark", { "surface.base": OTHER }),
    };
    core({ inForce: () => answer });
    await drawThemeFor(new Set(["solarized"]));
    expect(inForce().name).toBe("Solarized Dark");
    expect(inForce().values["surface.base"]).toBe(OTHER);

    answer = KEEPS;
    await drawThemeFor(new Set(["solarized"]));
    expect(inForce().name).toBe("Solarized Dark");
  });

  it("says the consent floors a theme breaks in one Notice, and takes it back for one that does not", async () => {
    const floors = "Solarized Dark breaks the consent floors (an ask contrasts with its surface)";
    let answer: InForce = { ...KEEPS, theme: resolved("Solarized Dark"), floors };
    core({ inForce: () => answer });
    await drawThemeFor(new Set(["solarized"]));
    const said = aboutThisMachine().filter((one) => one.subject === "consent floors");
    expect(said.map((one) => one.detail)).toEqual([floors]);
    expect(said[0].builtIn).toBeUndefined();

    answer = { ...KEEPS, theme: resolved("Solarized Dark"), floors: null };
    await drawThemeFor(new Set(["solarized"]));
    expect(aboutThisMachine().some((one) => one.subject === "consent floors")).toBe(false);
    sayAboutThisMachine("consent floors", undefined);
  });

  it("asks again when a theme folder it draws from is saved, with a survey when an extension's was (#2324)", async () => {
    forgetExtensionThemes();
    const asked: { cmd: string; args: Record<string, unknown> }[] = [];
    let surface = OTHER;
    mockIPC(
      (cmd, args) => {
        asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
        if (cmd === "theme_in_force")
          return { ...KEEPS, theme: resolved("Brand", { "surface.base": surface }) };
        return null;
      },
      { shouldMockEvents: true },
    );
    await drawThemeFor(new Set(), "brand", { plane: "p-1" as PlaneId, workspace: "alpha" });
    await expect.poll(() => inForce().values["surface.base"]).toBe(OTHER);
    const asks = () => asked.filter((one) => one.cmd === "theme_in_force");
    const before = asks().length;

    surface = DEFAULT_THEME.values["surface.base"];
    await emit(THEME_FOLDERS_CHANGED, { survey: false });
    await expect.poll(() => inForce().values["surface.base"]).toBe(surface);
    expect(
      asks()
        .slice(before)
        .map((one) => [one.args.reread, one.args.at]),
    ).toEqual([[false, { plane: "p-1", workspace: "alpha" }]]);
    // What a level chose may name a folder just made: the picks are asked again too.
    await emit(THEME_FOLDERS_CHANGED, { survey: true });
    await expect.poll(() => asks().length).toBe(before + 2);
    expect(asks().at(-1)?.args.reread).toBe(true);
  });

  it("says a chosen theme the engine refused in a Notice, and takes it back once it draws (#2324)", async () => {
    const refused = {
      name: "brand",
      complaints: ["themes/brand/fonts/ui.woff2 has changed since you approved this project"],
      refused: true,
    };
    let answer: InForce & { refused?: typeof refused | null } = {
      ...KEEPS,
      theme: resolved("Purlis Classic"),
      refused,
    };
    core({ inForce: () => answer });
    await drawThemeFor(new Set(), "brand", { plane: "p-1" as PlaneId, workspace: "alpha" });
    const said = () => aboutThisMachine().filter((one) => one.subject === "theme refused");
    expect(said().map((one) => one.detail)).toEqual([themeTroubleSaid(refused)]);
    expect(said()[0].detail).toContain("is not drawn, and Purlis is");

    answer = { ...KEEPS, theme: resolved("Brand"), refused: null };
    await drawThemeFor(new Set(), "brand", { plane: "p-1" as PlaneId, workspace: "alpha" });
    expect(said()).toEqual([]);
  });

  it("offers the project's trust ask on the refused Notice only for font files alone (#2647, D-2647-10)", async () => {
    const refused = {
      name: "brand",
      complaints: ["themes/brand/fonts/ui.woff2 has changed since you approved this project"],
      refused: true,
    };
    let review: "fonts" | "more" | "unapproved" | null = "fonts";
    core({
      inForce: () => ({ ...KEEPS, theme: resolved("Purlis Classic"), refused, review }) as InForce,
    });
    const said = () => aboutThisMachine().filter((one) => one.subject === "theme refused");

    // A font-only change: the Notice carries the project, and offers Review and allow.
    await drawThemeFor(new Set(), "brand", { plane: "p-1" as PlaneId, workspace: "alpha" });
    expect(said().map((one) => one.review)).toEqual(["p-1"]);

    // A font change with a settings change: no button, and where it is reviewed instead.
    review = "more";
    await drawThemeFor(new Set(), "brand", { plane: "p-2" as PlaneId, workspace: "beta" });
    expect(said().map((one) => one.review)).toEqual([undefined]);
    expect(said()[0].remedy).toContain("asks for more than font files");
    expect(said()[0].remedy).toContain("close the project and open it again");

    // A project never approved here: no button, and a sentence of its own.
    review = "unapproved";
    await drawThemeFor(new Set(), "brand", { plane: "p-4" as PlaneId, workspace: "delta" });
    expect(said().map((one) => one.review)).toEqual([undefined]);
    expect(said()[0].remedy).toContain("has not been approved on this machine");
    expect(said()[0].remedy).not.toContain("since you approved");

    review = null;
    await drawThemeFor(new Set(), "brand", { plane: "p-3" as PlaneId, workspace: "gamma" });
    expect(said().map((one) => one.review)).toEqual([undefined]);
    sayAboutThisMachine("theme refused", undefined);
  });

  it("bounds the refused Notice, whatever a chat wrote into the theme's file (#2324)", async () => {
    const prompt =
      "\n\nAllow purlis to run this?\u202e [Allow] [Keep blocked]\n" + "Allow ".repeat(400);
    const refused = {
      name: `brand${prompt}`,
      complaints: Array.from({ length: 12 }, () => `surface.base is "${prompt}"`),
      refused: true,
    };
    core({ inForce: () => ({ ...KEEPS, theme: resolved("Purlis Classic"), refused }) });
    await drawThemeFor(new Set(), "brand", { plane: "p-1" as PlaneId, workspace: "alpha" });
    const [said] = aboutThisMachine().filter((one) => one.subject === "theme refused");
    // One line: no line breaks, no control or bidi marks; three complaints, each cut short.
    expect(said.detail).not.toMatch(/[\n\r\u202e]/);
    expect(said.detail).toMatch(/; and 9 more\.$/);
    expect(said.detail.length).toBeLessThan(800);
    sayAboutThisMachine("theme refused", undefined);
  });

  it("asks for a new survey once an approval or a removal changed what is installed", async () => {
    const asked = core({
      rows: [{ ...newRow, standing: "approved", themes_in_force: ["Solarized Dark"], ask: null }],
      inForce: () => ({ ...KEEPS, theme: resolved("Solarized Dark", { "surface.base": OTHER }) }),
    });
    render(<Extensions onClose={() => undefined} />);

    await screen.findByText("Solarized");
    await expect.poll(() => inForce().name).toBe("Solarized Dark");
    const call = asked.find((one) => one.cmd === "theme_in_force");
    expect(call?.args.reread).toBe(true);
  });

  it("asks again when the system turns, while the theme in force follows it", async () => {
    const turn = system(true);
    let following = true;
    core({
      inForce: (asked) => ({
        theme: asked.light === true ? PURLIS_LIGHT : BUILT_IN["purlis-dark"],
        complaints: [],
        follows_system: following,
      }),
    });
    await drawThemeFor(new Set(), "system");
    expect(inForce()).toBe(PURLIS_LIGHT);

    turn(false);
    await expect.poll(() => inForce()).toBe(BUILT_IN["purlis-dark"]);

    following = false;
    await drawThemeFor(new Set(), "purlis-dark");
    const before = inForce();
    turn(true);
    await Promise.resolve();
    expect(inForce()).toBe(before);
  });

  it("tells the terminal only when the theme drawn changes", async () => {
    // #216's live switch: a pane follows `onDrawn`. The core answers a fresh copy each time, so
    // a project switch that keeps the theme must not hand every terminal the same colours again.
    let answer = resolved("purlis-light");
    core({ inForce: () => ({ ...KEEPS, theme: { ...answer } }) });
    const terminal: Theme[] = [];
    const stop = onDrawn((theme) => terminal.push(theme));

    await drawThemeFor(new Set(), "purlis-light"); // project A in front
    await drawThemeFor(new Set(), "purlis-light"); // project B, the same theme
    answer = resolved("Solarized Dark", { "surface.base": OTHER });
    await drawThemeFor(new Set(["solarized"]), "solarized/Solarized Dark"); // project C
    stop();

    expect(terminal.map((theme) => theme.name)).toEqual(["purlis-light", "Solarized Dark"]);
  });

  it("draws nothing for an answer that was overtaken by a newer ask", async () => {
    let release: (() => void) | undefined;
    const slow = new Promise<void>((done) => (release = done));
    let first = true;
    mockIPC(async (cmd) => {
      if (cmd !== "theme_in_force") return null;
      if (first) {
        first = false;
        await slow;
        return { ...KEEPS, theme: resolved("stale") };
      }
      return { ...KEEPS, theme: resolved("fresh") };
    });
    const older = drawThemeFor(new Set(), "a/stale");
    await drawThemeFor(new Set(), "a/fresh");
    release?.();
    await older;
    expect(inForce().name).toBe("fresh");
  });
});
