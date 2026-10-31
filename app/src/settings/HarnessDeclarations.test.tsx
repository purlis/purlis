// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SettingsTab } from "./SettingsTab";
import { ReadOnlySaid } from "../ReadOnlyLine";
import { forgetThisLaunch } from "../regions";
import { GLOBAL } from "../windowprefs";
import { forgetGroups } from "./links";
import { forgetYourEditor, setYourEditor } from "../yourEditor";
import type { ProjectHarnesses, SettingsFile } from "../bindings";

/**
 * **Settings shows the project's harness declarations read-only, each with Open file** (#1389,
 * spec #1221's collection shapes): under Harness & profiles, each `harnesses/<name>.toml` the
 * project declares, and each purlis would not read with its reason. Nothing here writes one.
 */

const PLANE = "/home/dev/plane";

const SHARED: SettingsFile = {
  which: "shared",
  file: "charter.toml",
  exists: true,
  text: "schema = 1\n",
  refusals: [],
  parsed: true,
  fields: [{ path: [{ key: "schema" }], value: { kind: "integer", value: 1 } }],
};

const LOCAL: SettingsFile = {
  which: "local",
  file: "charter.local.toml",
  exists: false,
  text: "",
  refusals: [],
  parsed: true,
  fields: [],
};

const DECLARED: ProjectHarnesses = {
  declared: [
    {
      name: "gemini",
      title: "Gemini CLI",
      program: "gemini",
      file: "harnesses/gemini.toml",
      acp: true,
    },
  ],
  refused: [{ file: "harnesses/broken.toml", reason: "it is not valid TOML." }],
};

function core() {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    switch (cmd) {
      case "project_settings":
        return { shared: SHARED, local: LOCAL };
      case "project_extensions":
        return { extensions: [], local_left_out: null };
      case "project_harness_declarations":
        return DECLARED;
      case "open_harness_declaration":
        return null;
      default:
        return null;
    }
  });
  return { calls: (cmd: string) => asked.filter((one) => one.cmd === cmd) };
}

beforeEach(() => {
  forgetThisLaunch();
  forgetGroups();
  forgetYourEditor();
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: { path: "/home/dev/layout.json", found: false, document: null, trouble: null },
    theme: { path: "", found: false, document: null, trouble: null },
  };
});
afterEach(() => {
  cleanup();
  clearMocks();
  forgetYourEditor();
  Reflect.deleteProperty(globalThis, GLOBAL);
});

async function atHarness() {
  render(<SettingsTab plane={PLANE} level="project" />);
  const nav = await screen.findByRole("navigation", { name: "Groups" });
  await userEvent.click(await within(nav).findByRole("button", { name: "Harness & profiles" }));
}

describe("the project's harness declarations in Settings (#1389)", () => {
  it("lists each declaration read-only, and each refused one with why", async () => {
    core();
    await atHarness();

    const gemini = await screen.findByRole("group", { name: "Gemini CLI (gemini)" });
    expect(gemini).toHaveAccessibleDescription(
      "A harness this project declares in harnesses/gemini.toml: it starts gemini, and can run as an ACP agent. Read-only here: edit the file to change it.",
    );
    expect(within(gemini).getByRole("button", { name: "Open file" })).toBeInTheDocument();
    expect(within(gemini).queryByRole("textbox")).toBeNull();
    const broken = screen.getByRole("group", { name: "harnesses/broken.toml" });
    expect(broken).toHaveAccessibleDescription(
      "purlis did not read this harness declaration: it is not valid TOML.",
    );
  });

  it("opens the declaration in your editor, by the file the listing named", async () => {
    const { calls } = core();
    setYourEditor("zed");
    await atHarness();

    const gemini = await screen.findByRole("group", { name: "Gemini CLI (gemini)" });
    await userEvent.click(within(gemini).getByRole("button", { name: "Open file" }));

    expect(calls("open_harness_declaration").map((one) => one.args)).toEqual([
      { plane: PLANE, file: "harnesses/gemini.toml", editor: "zed" },
    ]);
  });

  it("says to choose an editor first, and opens nothing", async () => {
    const { calls } = core();
    await atHarness();

    const gemini = await screen.findByRole("group", { name: "Gemini CLI (gemini)" });
    await userEvent.click(within(gemini).getByRole("button", { name: "Open file" }));

    expect(await screen.findByText("Choose your editor in Settings first.")).toBeInTheDocument();
    expect(calls("open_harness_declaration")).toEqual([]);
  });
});

describe("the harness declarations on a read-only project (#1795)", () => {
  it("keep Open file on: purlis writes nothing there, your editor opens the file", async () => {
    const { calls } = core();
    setYourEditor("zed");
    render(
      <ReadOnlySaid.Provider value="purlis opens the project read-only and writes nothing to it.">
        <SettingsTab plane={PLANE} level="project" />
      </ReadOnlySaid.Provider>,
    );
    const nav = await screen.findByRole("navigation", { name: "Groups" });
    await userEvent.click(await within(nav).findByRole("button", { name: "Harness & profiles" }));

    const gemini = await screen.findByRole("group", { name: "Gemini CLI (gemini)" });
    const open = within(gemini).getByRole("button", { name: "Open file" });
    expect(open).toBeEnabled();
    await userEvent.click(open);
    expect(calls("open_harness_declaration")).toHaveLength(1);
  });
});
