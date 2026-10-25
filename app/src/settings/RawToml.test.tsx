// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { SettingsTab } from "./SettingsTab";
import { ReadOnlyMendable, ReadOnlySaid } from "../ReadOnlyLine";
import { forgetThisLaunch } from "../regions";
import { GLOBAL } from "../windowprefs";
import type { SettingsChange, SettingsFile, SettingsWhich } from "../bindings";
import { backspace, textOf, typeIn, typeable } from "../editor/typing.testkit";
import { pickIn } from "../ui/select/select.testkit";

/**
 * **Edit as TOML** (SE-19, #1169; the spec on #558, V89d): one link per file at the foot of the
 * Project level's nav opens that file's whole text in the right column, with an explicit Save
 * and Discard. Whether a text is refused, and in which words, is the core's
 * (`purlis_core::settings`'s tests); the core is the mock here, and what is held is what the
 * person sees and what the window sends. The text is CodeMirror's (DS-10, #1842), typed into
 * through `typing.testkit`, since jsdom hands CodeMirror no typed key.
 */

const PLANE = "/home/dev/plane";

const SHARED_TEXT = `# the team's
[plane]
mode = "push"   # how far a save goes
`;

const LOCAL_TEXT = `[harness]\ndefault = "claude"\n`;

const file = (which: SettingsWhich, text: string, exists = true): SettingsFile => ({
  which,
  file: which === "shared" ? "charter.toml" : "charter.local.toml",
  exists,
  text,
  refusals: [],
  parsed: true,
  fields: [],
});

type Sent = { which: SettingsWhich; base: string | null; change: SettingsChange };

/**
 * The core, as a mock: `refuse` answers every save with these reasons instead of writing. Like
 * the core, a save made against text the file no longer holds is refused. `held`, when given,
 * is awaited before a save answers.
 */
function core({
  refuse = undefined as string[] | undefined,
  local = file("local", LOCAL_TEXT),
  held = undefined as (() => Promise<void>) | undefined,
} = {}) {
  const files: Record<SettingsWhich, SettingsFile> = {
    shared: file("shared", SHARED_TEXT),
    local,
  };
  const sent: Sent[] = [];
  mockIPC(
    (cmd, args) => {
      const given = (args ?? {}) as Record<string, unknown>;
      switch (cmd) {
        case "project_settings":
          return { shared: files.shared, local: files.local };
        case "project_extensions":
          return { extensions: [], local_left_out: null };
        case "project_harness_plugins":
          return [];
        case "extensions_on":
          return [];
        case "save_project_settings": {
          const which = given.which as SettingsWhich;
          const change = given.change as SettingsChange;
          const base = given.base as string | null;
          sent.push({ which, base, change });
          return (held ?? (() => Promise.resolve()))().then(() => {
            if (refuse) return { kind: "refused", reasons: refuse };
            const was = files[which];
            if ((was.exists ? was.text : null) !== base)
              return {
                kind: "refused",
                reasons: [
                  `${was.file} changed on disk since this tab read it, so nothing was saved.`,
                ],
              };
            if (change.kind === "raw")
              files[which] = { ...files[which], exists: true, text: change.text };
            else {
              // A form's change: each key set or gone, and a text that differs.
              let fields = [...files[which].fields];
              for (const edit of change.edits) {
                const at = JSON.stringify(edit.path);
                fields = fields.filter((one) => JSON.stringify(one.path) !== at);
                if (edit.value !== null) fields.push({ path: edit.path, value: edit.value });
              }
              files[which] = { ...files[which], fields, text: `${files[which].text}# form\n` };
            }
            return { kind: "saved", file: files[which] };
          });
        }
      }
      return undefined;
    },
    { shouldMockEvents: true },
  );
  return { sent, files };
}

beforeEach(() => {
  forgetThisLaunch();
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: { path: "/home/dev/.config/charter/layout.json", found: false, document: null },
    theme: { path: "", found: false, document: null, trouble: null },
  };
});
afterEach(() => {
  cleanup();
  clearMocks();
  Reflect.deleteProperty(globalThis, GLOBAL);
});

async function atProject() {
  render(<SettingsTab plane={PLANE} level="project" />);
  await screen.findByRole("button", { name: "General" });
}

/** A file's link, by the name it shows, in the links at the foot of the nav. */
const link = (name: string) =>
  within(screen.getByRole("group", { name: "Edit as TOML" })).getByRole("button", { name });
const editAsToml = (name: string) => userEvent.click(link(name));
/** The file's text, once its editor has arrived (a chunk of its own); in the merge view, the
 *  edit's side. */
const raw = (name: string) => screen.findByRole("textbox", { name: `${name}, as TOML` });
/** What the file's text holds, once it holds `text`. */
const holds = (name: string, text: string) =>
  waitFor(async () => expect(textOf(await raw(name))).toBe(text));

/** Settles the plane-changed event a write outside the tab sends. */
const changedOnDisk = () =>
  act(async () => {
    await emit("plane-changed", { plane: PLANE, changes: [], answers: [{ answer: "settings" }] });
  });

describe("Edit as TOML", () => {
  it("is one link per file at the foot of the nav, not a group of its own", async () => {
    core();
    await atProject();

    const links = screen.getByRole("group", { name: "Edit as TOML" });
    expect(
      within(links)
        .getAllByRole("button")
        .map((one) => one.textContent),
    ).toEqual(["charter.toml", "charter.local.toml"]);
    const nav = screen.getByRole("navigation", { name: "Groups" });
    expect(within(nav).queryByRole("button", { name: /TOML/ })).toBeNull();
  });

  it("opens the file's whole text in the right column, in place of the group", async () => {
    core();
    await atProject();

    await editAsToml("charter.toml");

    await holds("charter.toml", SHARED_TEXT);
    expect(screen.queryByRole("region", { name: "General" })).toBeNull();
    expect(link("charter.toml")).toHaveAttribute("aria-current", "true");
    expect(screen.getByRole("button", { name: "Save charter.toml" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Discard" })).toBeDisabled();
  });

  it("writes nothing as it is typed, and Save writes the whole text against what was read", async () => {
    const { sent } = core();
    await atProject();
    await editAsToml("charter.toml");

    typeIn(await raw("charter.toml"), '[update]\nchannel = "dev"\n');
    expect(sent).toHaveLength(0);
    await userEvent.click(screen.getByRole("button", { name: "Save charter.toml" }));

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      which: "shared",
      base: SHARED_TEXT,
      change: { kind: "raw", text: `${SHARED_TEXT}[update]\nchannel = "dev"\n` },
    });
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Save charter.toml" })).toBeDisabled(),
    );
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("Discard drops the edit and shows the file as it is", async () => {
    const { sent } = core();
    await atProject();
    await editAsToml("charter.local.toml");

    typeIn(await raw("charter.local.toml"), "oops");
    await userEvent.click(screen.getByRole("button", { name: "Discard" }));

    await holds("charter.local.toml", LOCAL_TEXT);
    expect(screen.getByRole("button", { name: "Save charter.local.toml" })).toBeDisabled();
    expect(sent).toHaveLength(0);
  });

  it("says a text the core refuses in its own words, writes nothing and keeps what was typed", async () => {
    const why =
      "charter.toml is not valid TOML: TOML parse error at line 4, column 8 — invalid table header";
    const { files } = core({ refuse: [why] });
    await atProject();
    await editAsToml("charter.toml");

    typeIn(await raw("charter.toml"), "[broken");
    await userEvent.click(screen.getByRole("button", { name: "Save charter.toml" }));

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Nothing was saved:");
    expect(alert).toHaveTextContent(why);
    await holds("charter.toml", `${SHARED_TEXT}[broken`);
    expect(files.shared.text).toBe(SHARED_TEXT);
  });

  it("saves a Local file that is not there yet as new, and says the save creates it", async () => {
    const { sent } = core({ local: file("local", "", false) });
    await atProject();
    await editAsToml("charter.local.toml");

    expect(screen.getByText(/Not created yet: the first save creates it\./)).toBeInTheDocument();
    typeIn(await raw("charter.local.toml"), "[harness]");
    await userEvent.click(screen.getByRole("button", { name: "Save charter.local.toml" }));

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toMatchObject({ which: "local", base: null });
  });

  it("follows a file changed outside the tab while nothing is typed", async () => {
    const { files } = core();
    await atProject();
    await editAsToml("charter.toml");

    files.shared = { ...files.shared, text: `${SHARED_TEXT}# edited by hand\n` };
    await changedOnDisk();

    await holds("charter.toml", `${SHARED_TEXT}# edited by hand\n`);
  });

  it("keeps an edit when the file changes outside the tab, and shows the file beside it", async () => {
    const { files, sent } = core();
    await atProject();
    await editAsToml("charter.toml");
    typeIn(await raw("charter.toml"), "# mine\n");

    files.shared = { ...files.shared, text: `${SHARED_TEXT}# edited by hand\n` };
    await changedOnDisk();

    expect(
      await screen.findByText(/charter\.toml changed on disk since this edit began/),
    ).toBeInTheDocument();
    // The merge view: the file as it now is, read only, beside the edit, still typed into.
    const disk = await screen.findByRole("textbox", { name: "charter.toml, on disk now" });
    expect(textOf(disk)).toBe(`${SHARED_TEXT}# edited by hand\n`);
    expect(typeable(disk)).toBe(false);
    await holds("charter.toml", `${SHARED_TEXT}# mine\n`);
    expect(typeable(await raw("charter.toml"))).toBe(true);
    // Nothing is saved over the file until the person chooses.
    expect(screen.getByRole("button", { name: "Save charter.toml" })).toBeDisabled();
    expect(sent).toHaveLength(0);

    await userEvent.click(screen.getByRole("button", { name: "Discard" }));
    await holds("charter.toml", files.shared.text);
    expect(screen.queryByRole("textbox", { name: "charter.toml, on disk now" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Keep my edit" })).toBeNull();
  });

  it("keeps both texts in the merge view while the edit goes on", async () => {
    const { files } = core();
    await atProject();
    await editAsToml("charter.toml");
    typeIn(await raw("charter.toml"), "# mine\n");
    files.shared = { ...files.shared, text: `${SHARED_TEXT}# edited by hand\n` };
    await changedOnDisk();
    await screen.findByRole("textbox", { name: "charter.toml, on disk now" });

    typeIn(await raw("charter.toml"), "# more\n");

    await holds("charter.toml", `${SHARED_TEXT}# mine\n# more\n`);
    expect(textOf(screen.getByRole("textbox", { name: "charter.toml, on disk now" }))).toBe(
      `${SHARED_TEXT}# edited by hand\n`,
    );
  });

  it("Keep my edit makes the edit one of the file as it now is, and Save writes it over that", async () => {
    const { files, sent } = core();
    await atProject();
    await editAsToml("charter.toml");
    typeIn(await raw("charter.toml"), "# mine\n");
    const outside = `${SHARED_TEXT}# edited by hand\n`;
    files.shared = { ...files.shared, text: outside };
    await changedOnDisk();

    await userEvent.click(await screen.findByRole("button", { name: "Keep my edit" }));

    // Back to the one editor, holding the edit, with the keyboard in it.
    await waitFor(() =>
      expect(screen.queryByRole("textbox", { name: "charter.toml, on disk now" })).toBeNull(),
    );
    await holds("charter.toml", `${SHARED_TEXT}# mine\n`);
    await waitFor(async () => expect(await raw("charter.toml")).toHaveFocus());
    const editor = screen.getByRole("region", { name: "charter.toml" });
    expect(within(editor).getByRole("status")).toBeEmptyDOMElement();
    await userEvent.click(screen.getByRole("button", { name: "Save charter.toml" }));

    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      which: "shared",
      base: outside,
      change: { kind: "raw", text: `${SHARED_TEXT}# mine\n` },
    });
    await waitFor(() => expect(files.shared.text).toBe(`${SHARED_TEXT}# mine\n`));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("keeps what was typed when a group is opened and the file's link is pressed again", async () => {
    core();
    await atProject();
    await editAsToml("charter.toml");
    typeIn(await raw("charter.toml"), "# mine\n");

    await userEvent.click(
      within(screen.getByRole("navigation", { name: "Groups" })).getByRole("button", {
        name: "Saving",
      }),
    );
    expect(screen.getByRole("region", { name: "Saving" })).toBeInTheDocument();
    await editAsToml("charter.toml");

    await holds("charter.toml", `${SHARED_TEXT}# mine\n`);
  });

  it("keeps a save's refusal with the edit when a group is opened and the file's link is pressed again", async () => {
    const why = "charter.toml is not valid TOML: invalid table header";
    core({ refuse: [why] });
    await atProject();
    await editAsToml("charter.toml");
    typeIn(await raw("charter.toml"), "[broken");
    await userEvent.click(screen.getByRole("button", { name: "Save charter.toml" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(why);

    await userEvent.click(
      within(screen.getByRole("navigation", { name: "Groups" })).getByRole("button", {
        name: "Saving",
      }),
    );
    await editAsToml("charter.toml");

    await holds("charter.toml", `${SHARED_TEXT}[broken`);
    expect(screen.getByRole("alert")).toHaveTextContent(`Nothing was saved:${why}`);

    // Discard drops the refusal with the edit.
    await userEvent.click(screen.getByRole("button", { name: "Discard" }));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("keeps the changed-on-disk region mounted, empty until the file moves under an edit", async () => {
    const { files } = core();
    await atProject();
    await editAsToml("charter.toml");

    // There before it has anything to say, so a screen reader hears its first sentence.
    const editor = () => screen.getByRole("region", { name: "charter.toml" });
    const region = within(editor()).getByRole("status");
    expect(region).toBeEmptyDOMElement();
    typeIn(await raw("charter.toml"), "# mine\n");

    files.shared = { ...files.shared, text: `${SHARED_TEXT}# edited by hand\n` };
    await changedOnDisk();

    await waitFor(() =>
      expect(region).toHaveTextContent(/charter\.toml changed on disk since this edit began/),
    );
    expect(within(editor()).getByRole("status")).toBe(region);
  });

  it("is where a file that is not TOML is mended: its link is there while its groups are not", async () => {
    const { files } = core();
    files.shared = {
      ...files.shared,
      text: "[plane\n",
      parsed: false,
      refusals: [{ why: "charter.toml is not valid TOML", key: null }],
    };
    render(<SettingsTab plane={PLANE} level="project" />);
    await screen.findByRole("button", { name: "Harness & profiles" });
    expect(screen.queryByRole("button", { name: "General" })).toBeNull();

    await editAsToml("charter.toml");

    await holds("charter.toml", "[plane\n");
  });

  it("never saves over an outside change, even once the edit was typed back to what it read", async () => {
    const { files, sent } = core();
    await atProject();
    await editAsToml("charter.toml");
    typeIn(await raw("charter.toml"), "#");
    files.shared = { ...files.shared, text: `${SHARED_TEXT}# edited by hand\n` };
    await changedOnDisk();

    // Typed back to exactly the text it read, then typed again.
    backspace(await raw("charter.toml"));
    expect(
      await screen.findByText(/charter\.toml changed on disk since this edit began/),
    ).toBeInTheDocument();
    typeIn(await raw("charter.toml"), "# mine\n");

    expect(screen.getByRole("button", { name: "Save charter.toml" })).toBeDisabled();
    expect(sent).toHaveLength(0);
  });

  it("keeps the outside change and what was typed after an edit was typed back, the file changed, and typing went on", async () => {
    const { files, sent } = core();
    await atProject();
    await editAsToml("charter.toml");
    typeIn(await raw("charter.toml"), "# mine");
    backspace(await raw("charter.toml"), "# mine".length);
    const outside = `${SHARED_TEXT}# edited by hand\n`;
    files.shared = { ...files.shared, text: outside };
    await changedOnDisk();
    await screen.findByText(/charter\.toml changed on disk since this edit began/);

    typeIn(await raw("charter.toml"), "# then\n");

    expect(screen.getByRole("button", { name: "Save charter.toml" })).toBeDisabled();
    expect(sent).toHaveLength(0);
    expect(files.shared.text).toBe(outside);
    // What was typed is kept, and Discard takes up the outside change.
    await holds("charter.toml", `${SHARED_TEXT}# then\n`);
    await userEvent.click(screen.getByRole("button", { name: "Discard" }));
    await holds("charter.toml", outside);
  });

  it("writes an edit typed straight after a save, against what that save wrote", async () => {
    const { files } = core();
    await atProject();
    await editAsToml("charter.toml");
    typeIn(await raw("charter.toml"), "# one\n");
    await userEvent.click(screen.getByRole("button", { name: "Save charter.toml" }));
    await waitFor(() => expect(files.shared.text).toBe(`${SHARED_TEXT}# one\n`));

    typeIn(await raw("charter.toml"), "# two\n");
    await userEvent.click(screen.getByRole("button", { name: "Save charter.toml" }));

    await waitFor(() => expect(files.shared.text).toBe(`${SHARED_TEXT}# one\n# two\n`));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.queryByText(/changed on disk/)).toBeNull();
  });

  it("loses nothing typed while a save is on its way", async () => {
    let letGo = () => {};
    let holding = true;
    const { files } = core({
      held: () =>
        holding
          ? new Promise<void>((done) => {
              holding = false;
              letGo = done;
            })
          : Promise.resolve(),
    });
    await atProject();
    await editAsToml("charter.toml");
    typeIn(await raw("charter.toml"), "# one\n");
    await userEvent.click(screen.getByRole("button", { name: "Save charter.toml" }));
    await screen.findByRole("button", { name: "Saving…" });

    typeIn(await raw("charter.toml"), "# two\n");
    await act(async () => letGo());
    await waitFor(() => expect(files.shared.text).toBe(`${SHARED_TEXT}# one\n`));

    // What was typed during the save is still there, as an edit of what the save wrote.
    await holds("charter.toml", `${SHARED_TEXT}# one\n# two\n`);
    expect(screen.queryByText(/changed on disk/)).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Save charter.toml" }));
    await waitFor(() => expect(files.shared.text).toBe(`${SHARED_TEXT}# one\n# two\n`));
  });

  it("saves the next edit against what the last save wrote", async () => {
    const { sent } = core();
    await atProject();
    await editAsToml("charter.toml");
    typeIn(await raw("charter.toml"), "# one\n");
    await userEvent.click(screen.getByRole("button", { name: "Save charter.toml" }));
    await waitFor(() => expect(sent).toHaveLength(1));

    typeIn(await raw("charter.toml"), "# two\n");
    await userEvent.click(screen.getByRole("button", { name: "Save charter.toml" }));

    await waitFor(() => expect(sent).toHaveLength(2));
    expect(sent[1].base).toBe(`${SHARED_TEXT}# one\n`);
  });

  it("takes away a form change's Undo once the file is saved as TOML", async () => {
    core();
    await atProject();
    await userEvent.click(
      within(screen.getByRole("navigation", { name: "Groups" })).getByRole("button", {
        name: "Saving",
      }),
    );
    await pickIn(screen.getByLabelText("Mode"), "commit");
    expect(await screen.findByRole("button", { name: "Undo" })).toBeInTheDocument();

    await editAsToml("charter.toml");
    typeIn(await raw("charter.toml"), "# mine\n");
    await userEvent.click(screen.getByRole("button", { name: "Save charter.toml" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Save charter.toml" })).toBeDisabled(),
    );
    await userEvent.click(
      within(screen.getByRole("navigation", { name: "Groups" })).getByRole("button", {
        name: "Saving",
      }),
    );

    expect(screen.queryByRole("button", { name: "Undo" })).toBeNull();
  });

  it("gives the focus back to the text after Save, Discard and a refusal", async () => {
    core({ refuse: ["charter.toml is not valid TOML"] });
    await atProject();
    await editAsToml("charter.toml");
    typeIn(await raw("charter.toml"), "[broken");

    await userEvent.click(screen.getByRole("button", { name: "Save charter.toml" }));
    await screen.findByRole("alert");
    expect(await raw("charter.toml")).toHaveFocus();

    await userEvent.click(screen.getByRole("button", { name: "Discard" }));
    expect(await raw("charter.toml")).toHaveFocus();
  });

  it("gives the focus back to the text after a save that is written", async () => {
    core();
    await atProject();
    await editAsToml("charter.toml");
    typeIn(await raw("charter.toml"), "# mine\n");

    await userEvent.click(screen.getByRole("button", { name: "Save charter.toml" }));

    await waitFor(async () => expect(await raw("charter.toml")).toHaveFocus());
  });

  it("says, under what charter refuses, where a file that is not TOML is mended", async () => {
    const { files } = core();
    files.shared = {
      ...files.shared,
      text: "[plane\n",
      parsed: false,
      refusals: [{ why: "charter.toml is not valid TOML", key: null }],
    };
    render(<SettingsTab plane={PLANE} level="project" />);

    expect(
      await screen.findByText("Open charter.toml under Edit as TOML to mend it."),
    ).toBeInTheDocument();
  });
});

describe("Edit as TOML on a read-only project (#826, #1759)", () => {
  const UNREADABLE =
    "charter.toml cannot be read (expected `=`), so purlis cannot tell which format this project is in";

  async function readOnlyAtProject(mendable: boolean) {
    render(
      <ReadOnlySaid.Provider value={UNREADABLE}>
        <ReadOnlyMendable.Provider value={mendable}>
          <SettingsTab plane={PLANE} level="project" />
        </ReadOnlyMendable.Provider>
      </ReadOnlySaid.Provider>,
    );
    await screen.findByRole("button", { name: "General" });
  }

  it("stays on for the shared file whose manifest does not read: it is how the person mends it", async () => {
    core();
    await readOnlyAtProject(true);

    await editAsToml("charter.toml");

    expect(typeable(await raw("charter.toml"))).toBe(true);
    expect((await raw("charter.toml")).closest("fieldset.read-only-off")).toBeNull();
  });

  it("is off, with the project's sentence, for any other read-only reason", async () => {
    core();
    await readOnlyAtProject(false);

    await editAsToml("charter.toml");

    const text = await raw("charter.toml");
    expect(text).toHaveAttribute("aria-readonly", "true");
    expect(typeable(text)).toBe(false);
    // Why it is off: its group\'s description, which its legend\'s tooltip says too (#2908).
    expect(text.closest("fieldset")).toHaveAccessibleDescription(UNREADABLE);
  });

  it("keeps a group's controls off even where the manifest is mendable", async () => {
    core();
    await readOnlyAtProject(true);

    expect(document.querySelector("fieldset.read-only-off")).toBeDisabled();
  });
});
