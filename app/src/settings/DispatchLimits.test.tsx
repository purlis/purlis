// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, onTestFinished } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { DispatchLimits, DispatchLimitRow, SettingsRefusal } from "../bindings";
import { DISPATCH, DispatchLimitsTable, dispatchGroup, workspaceDispatchGroup } from "./dispatch";
import type { LiveSetting } from "./groups";
import { SETTINGS_ACTION, type SettingsActionAsk } from "./links";
import { ReadOnlySaid } from "../ReadOnlyLine";
import { drawnWith } from "../cascade.testkit";
import { offeredIn, pickIn } from "../primitives/select.testkit";
import { hoverTipOf, tipOf } from "../ui/tooltip/tooltip.testkit";

/**
 * **Settings › Project › Dispatch** (#1439, #1440): the limits table (the project's row, a row
 * per workspace or persona override, Add an override), yours on this machine, the policy's
 * ceilings drawn locked, and the slot the dispatch grants fill. The same limits are edited on a
 * workspace's settings and on the persona view. Every rule is the core's: the window writes one
 * key of a settings file through `save_project_settings` and draws what the core answers.
 */

const PLANE = "/home/dev/plane";
const SHARED = "schema = 1\n";

const LIMITS: DispatchLimits["limits"] = [
  ["running-per-chat", "Running per chat", 6, false, 10000],
  ["live-per-lineage", "Live per lineage", 16, false, 10000],
  ["depth", "Depth", 3, false, 8],
  ["messages-per-minute", "Messages per minute", 10, false, 10000],
  ["may-dispatch", "May dispatch", null, true, 10000],
  ["may-run-at-once", "May run at once", null, true, 10000],
  // The two that are off until set (#1512), as the core lists them.
  ["tokens-per-session", "Tokens per session", null, false, 1000000000],
  ["minutes-per-task", "Minutes per task", null, false, 10000],
].map(([word, label, standard, persona_only, most]) => ({
  word: word as string,
  label: label as string,
  help: `What ${String(label).toLowerCase()} limits.`,
  default: standard as number | null,
  persona_only: persona_only as boolean,
  most: most as number,
  ceiling: null,
}));

const DEFAULTS = [6, 16, 3, 10, null, null, null, null];

/** `values` as long as the core's limits: the two off until set left unwritten. */
const eight = (values: (number | null)[]) => [...values, ...Array(8 - values.length).fill(null)];

function row(
  scope: string,
  name: string,
  values: (number | null)[] = [],
  beneath: (number | null)[] = DEFAULTS,
): DispatchLimitRow {
  return { scope, name, values: eight(values), beneath: eight(beneath), ignored: [] };
}

function page(over: Partial<DispatchLimits> = {}): DispatchLimits {
  return {
    file: "purlis.toml",
    local_file: "purlis.local.toml",
    base: SHARED,
    local_base: null,
    limits: LIMITS,
    rows: [row("project", "")],
    mine: [row("project", "")],
    locked_by: null,
    refused: [],
    local_left_out: null,
    policy_refused: false,
    ...over,
  };
}

type Saved = { which: string; base: string | null; change: unknown };

/** The core: answers the page, the names an override may be for, and each save. */
function core(
  first: DispatchLimits,
  save: (asked: Saved) => { now?: DispatchLimits; refused?: string[] } = () => ({}),
) {
  let now = first;
  const saves: Saved[] = [];
  let reads = 0;
  mockIPC((cmd, args) => {
    if (cmd === "dispatch_limits") {
      reads += 1;
      return now;
    }
    if (cmd === "start_options")
      return { profiles: [], refused: [], personas: ["devops", "steward"] };
    if (cmd === "plane_sidebar") return { workspaces: [{ name: "alpha" }, { name: "beta" }] };
    if (cmd === "save_project_settings") {
      const asked = args as unknown as Saved;
      saves.push(asked);
      const said = save(asked);
      if (said.refused !== undefined) return { kind: "refused", reasons: said.refused };
      if (said.now !== undefined) now = said.now;
      return { kind: "saved", file: {} };
    }
    return undefined;
  });
  return { saves, reads: () => reads };
}

afterEach(() => {
  cleanup();
  clearMocks();
});

/** One setting of the Dispatch page, drawn as its row draws it. */
function Setting({ id }: { id: string }) {
  const setting = dispatchGroup(PLANE).settings.find((one) => one.id === id) as LiveSetting;
  const { control, undo } = setting.useControl();
  return (
    <>
      {control({ id: "d", labelledBy: "d-label" })}
      {/* The settings row draws the last change's Undo; here it is drawn bare. */}
      {undo && (
        <button type="button" onClick={undo}>
          Undo
        </button>
      )}
    </>
  );
}

const Page = () => <Setting id={`${DISPATCH}.limits`} />;

/** Types `text` into `box` and leaves it, which is when a limit is written. */
function typeInto(box: HTMLElement, text: string) {
  fireEvent.change(box, { target: { value: text } });
  fireEvent.blur(box);
}

const edit = (path: string[], value: unknown) => ({
  kind: "edits",
  edits: [{ path: path.map((key) => ({ key })), value }],
});

describe("Settings › Project › Dispatch", () => {
  it("is a page of the Project level, with the limits, the grants' slot and the locks", () => {
    const group = dispatchGroup(PLANE);
    expect(group.id).toBe("project.dispatch");
    expect(group.label).toBe("Dispatch");
    expect(group.settings.map((one) => one.id)).toEqual([
      "project.dispatch.limits",
      "project.dispatch.grants",
      "project.dispatch.locks",
    ]);
  });

  it("says where the limits are kept once, for the group, and not under the table (DS-10)", async () => {
    expect(dispatchGroup(PLANE, "purlis.toml").kept).toBe(
      "Kept in purlis.toml, which your team sees.",
    );
    core(page());
    render(<Page />);

    await screen.findByRole("table", { name: "Dispatch limits" });
    expect(screen.queryByText(/Kept in purlis\.toml/)).toBeNull();
    expect(screen.getByText(/The most specific row wins/)).toBeInTheDocument();
  });

  it("draws the project's row with what is in force where it sets nothing", async () => {
    core(page({ rows: [row("project", "", [4, null, null, null, null, null])] }));
    render(<Page />);

    const table = await screen.findByRole("table", { name: "Dispatch limits" });
    // Each column is named by the limit's whole label, whatever its heading shows (DS-9).
    expect(
      within(table)
        .getAllByRole("columnheader")
        .map((one) => one.getAttribute("aria-label") ?? one.textContent),
    ).toEqual([
      "Level",
      "Running per chat",
      "Live per lineage",
      "Depth",
      "Messages per minute",
      "May dispatch",
      "May run at once",
      "Tokens per session",
      "Minutes per task",
      "",
    ]);
    expect(within(table).getByRole("rowheader", { name: "Project" })).toBeInTheDocument();
    expect(screen.getByLabelText("Running per chat for the project")).toHaveValue("4");
    // Not set here: the box is empty and shows the value beneath it.
    const depth = screen.getByLabelText("Depth for the project");
    expect(depth).toHaveValue("");
    expect(depth).toHaveAttribute("placeholder", "3");
    // The set's Field, as a table's cell (#626): its own name, its help as its tooltip, and a
    // count's keyboard.
    expect(depth.closest(".ui-setting-box")).toHaveAttribute("data-scope", "field");
    expect(tipOf(depth)).toEqual("What depth limits.");
    expect(depth).toHaveAttribute("inputmode", "numeric");
    // A persona's limits are not the project's to set.
    expect(screen.queryByLabelText("May dispatch for the project")).not.toBeInTheDocument();
  });

  it("stacks into a form where its column is narrow: each box under its limit's whole label (#1859)", async () => {
    core(
      page({
        rows: [row("project", ""), row("persona", "devops", [null, null, 1, null, null, 2])],
      }),
    );
    render(<Page />);

    const table = await screen.findByRole("table", { name: "Dispatch limits" });
    // The shared table's stacking form, in the room it measures (`primitives/collections.css`).
    expect(table).toHaveClass("ui-table", "ui-table-stacks");
    expect(table.closest(".ui-table-room")).not.toBeNull();
    // Each box's cell carries its column's whole words, which the stacked form shows above it.
    const cellOf = (name: string) => screen.getByLabelText(name).closest("td") as HTMLElement;
    expect(cellOf("Depth for the project")).toHaveAttribute("data-heading", "Depth");
    expect(cellOf("Running per chat for the project")).toHaveAttribute(
      "data-heading",
      "Running per chat",
    );
    expect(cellOf("May run at once for the persona devops")).toHaveAttribute(
      "data-heading",
      "May run at once",
    );
    // A persona's own limit has no box on the project's row: its cell is empty, and stacked it
    // is not drawn at all.
    const project = within(table).getByRole("rowheader", { name: "Project" }).closest("tr");
    expect(
      [...(project as HTMLElement).querySelectorAll("td[data-heading]")].filter(
        (cell) => cell.childElementCount === 0,
      ),
    ).toHaveLength(2);
  });

  it("says where the token and time limits are set that a harness reporting no tokens is not counted", async () => {
    // #1512: never a guess, said where the limit is set.
    core(page({ rows: [row("project", "", [null, null, null, null, null, null])] }));
    render(<Page />);

    const note = await screen.findByTestId("dispatch-tokens-time-note");
    expect(note.textContent).toContain("off until set");
    expect(note.textContent).toContain("a session starts no new task");
    expect(note.textContent).toContain("never one you are answering or typing in");
    expect(note.textContent).not.toContain("not enforced yet");
    expect(note.textContent).toContain("a harness that reports no tokens is not counted");
    expect(note.textContent).toContain("working time only");
  });

  it("draws a row per workspace and persona override, a persona's with its two own limits", async () => {
    core(
      page({
        rows: [
          row("project", ""),
          row("workspace", "alpha", [12, null, null, null, null, null]),
          row("persona", "devops", [null, null, 1, null, null, 2]),
        ],
      }),
    );
    render(<Page />);

    await screen.findByRole("table", { name: "Dispatch limits" });
    expect(screen.getByRole("rowheader", { name: "Workspace alpha" })).toBeInTheDocument();
    expect(screen.getByRole("rowheader", { name: "Persona devops" })).toBeInTheDocument();
    expect(screen.getByLabelText("Running per chat for the workspace alpha")).toHaveValue("12");
    expect(screen.getByLabelText("Depth for the persona devops")).toHaveValue("1");
    expect(screen.getByLabelText("May run at once for the persona devops")).toHaveValue("2");
    // No cap until one is written.
    expect(screen.getByLabelText("May dispatch for the persona devops")).toHaveAttribute(
      "placeholder",
      "no cap",
    );
  });

  it("writes a changed limit into the project file and draws what the core then answers", async () => {
    const after = page({ rows: [row("project", "", [null, null, 2, null, null, null])] });
    const { saves, reads } = core(page(), () => ({ now: after }));
    render(<Page />);

    typeInto(await screen.findByLabelText("Depth for the project"), "2");

    await waitFor(() => expect(saves).toHaveLength(1));
    expect(saves[0]).toMatchObject({
      plane: PLANE,
      which: "shared",
      base: SHARED,
      change: edit(["dispatch", "depth"], { kind: "integer", value: 2 }),
    });
    // Read again, so what is shown is what is on disk.
    await waitFor(() => expect(reads()).toBe(2));
    expect(screen.getByLabelText("Depth for the project")).toHaveValue("2");
  });

  it("writes on Enter too, and nothing when the box is left as it was", async () => {
    const { saves } = core(page());
    render(<Page />);

    const depth = await screen.findByLabelText("Depth for the project");
    fireEvent.blur(depth);
    expect(saves).toHaveLength(0);
    await userEvent.type(depth, "5{Enter}");
    await waitFor(() => expect(saves).toHaveLength(1));
    expect(saves[0].change).toEqual(edit(["dispatch", "depth"], { kind: "integer", value: 5 }));
  });

  it("takes a limit out of the file when its box is emptied, so the level beneath is in force", async () => {
    const { saves } = core(page({ rows: [row("project", "", [4, null, null, null, null, null])] }));
    render(<Page />);

    typeInto(await screen.findByLabelText("Running per chat for the project"), "");

    await waitFor(() => expect(saves).toHaveLength(1));
    expect(saves[0].change).toEqual(edit(["dispatch", "running-per-chat"], null));
  });

  it("says the core's refusal of a depth of 9 and keeps showing what is on disk", async () => {
    const why =
      "dispatch.depth in purlis.toml is 9, and depth is never above 8, so it is not read and the level beneath it is in force";
    core(page(), () => ({ refused: [why] }));
    render(<Page />);

    typeInto(await screen.findByLabelText("Depth for the project"), "9");

    expect(await screen.findByText(why)).toBeInTheDocument();
    await waitFor(() => expect(screen.getByLabelText("Depth for the project")).toHaveValue(""));
  });

  it("sends what is not a number as it was typed, for the core to refuse", async () => {
    const { saves } = core(page(), () => ({ refused: ["dispatch.depth is not a whole number"] }));
    render(<Page />);

    typeInto(await screen.findByLabelText("Depth for the project"), "deep");

    await waitFor(() => expect(saves).toHaveLength(1));
    expect(saves[0].change).toEqual(edit(["dispatch", "depth"], { kind: "text", value: "deep" }));
    expect(await screen.findByText("dispatch.depth is not a whole number")).toBeInTheDocument();
  });

  it("adds an override for a workspace or a persona that has none, and writes its first limit", async () => {
    const { saves } = core(page({ rows: [row("project", ""), row("workspace", "alpha")] }));
    render(<Page />);

    const pick = await screen.findByRole("combobox", { name: "Override for" });
    // What has a row already is not offered again.
    await waitFor(async () =>
      expect(await offeredIn(pick)).toEqual([
        "Workspace beta",
        "Persona devops",
        "Persona steward",
      ]),
    );
    await pickIn(pick, "Persona devops");
    await userEvent.click(screen.getByRole("button", { name: "Add an override" }));

    // A row to fill in: nothing is written until a limit is.
    expect(screen.getByRole("rowheader", { name: "Persona devops" })).toBeInTheDocument();
    expect(saves).toHaveLength(0);
    typeInto(screen.getByLabelText("May run at once for the persona devops"), "1");
    await waitFor(() => expect(saves).toHaveLength(1));
    expect(saves[0]).toMatchObject({
      which: "shared",
      change: edit(["dispatch", "personas", "devops", "may-run-at-once"], {
        kind: "integer",
        value: 1,
      }),
    });
  });

  it("removes an override, table and all", async () => {
    const { saves } = core(
      page({
        rows: [row("project", ""), row("workspace", "alpha", [12, null, null, null, null, null])],
      }),
      () => ({ now: page() }),
    );
    render(<Page />);

    await userEvent.click(
      await screen.findByRole("button", { name: "Remove the override for the workspace alpha" }),
    );

    await waitFor(() => expect(saves).toHaveLength(1));
    expect(saves[0].change).toEqual(edit(["dispatch", "workspaces", "alpha"], null));
    await waitFor(() =>
      expect(screen.queryByRole("rowheader", { name: "Workspace alpha" })).not.toBeInTheDocument(),
    );
  });

  it("keeps your own limit on this machine, and says a higher one is ignored", async () => {
    const ignored =
      "Your own limit of 5 for depth is above the project's 3, so it is ignored: a limit on this machine can only lower one.";
    const { saves } = core(
      page({
        mine: [{ ...row("project", "", [null, null, 5, null, null, null]), ignored: [ignored] }],
      }),
    );
    render(<Page />);

    expect(
      await screen.findByRole("rowheader", { name: "Me on this machine" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Depth for me on this machine")).toHaveValue("5");
    expect(screen.getByText(ignored)).toBeInTheDocument();

    typeInto(screen.getByLabelText("Depth for me on this machine"), "2");
    await waitFor(() => expect(saves).toHaveLength(1));
    // This machine's file, which was not there: no base.
    expect(saves[0]).toMatchObject({
      which: "local",
      base: null,
      change: edit(["dispatch", "depth"], { kind: "integer", value: 2 }),
    });
  });

  it("still draws and applies your row while git would carry this machine's file, and says so", async () => {
    core(
      page({
        mine: [row("project", "", [null, null, 1, null, null, null])],
        local_left_out: "purlis.local.toml is tracked by git, so it is not read.",
      }),
    );
    render(<Page />);

    await screen.findByRole("table", { name: "Dispatch limits" });
    // A limit of yours can only lower one, so it holds whatever git says of the file.
    expect(screen.getByLabelText("Depth for me on this machine")).toHaveValue("1");
    expect(
      screen.getByText(
        "purlis.local.toml is tracked by git, so it is not read. Your dispatch limits in it still apply, because they can only lower one.",
      ),
    ).toBeInTheDocument();
  });

  it("offers Undo for the last limit changed, which puts back what the box held", async () => {
    const before = page({ rows: [row("project", "", [null, null, 4, null, null, null])] });
    const after = page({ rows: [row("project", "", [null, null, 2, null, null, null])] });
    let now = after;
    const { saves } = core(before, () => ({ now }));
    render(<Page />);

    // Nothing to undo until something is changed here.
    await screen.findByRole("table", { name: "Dispatch limits" });
    expect(screen.queryByRole("button", { name: "Undo" })).not.toBeInTheDocument();
    typeInto(screen.getByLabelText("Depth for the project"), "2");
    await waitFor(() => expect(saves).toHaveLength(1));

    now = before;
    await userEvent.click(await screen.findByRole("button", { name: "Undo" }));
    await waitFor(() => expect(saves).toHaveLength(2));
    expect(saves[1]).toMatchObject({
      which: "shared",
      change: edit(["dispatch", "depth"], { kind: "integer", value: 4 }),
    });
    // One level: the Undo is gone once it is used.
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "Undo" })).not.toBeInTheDocument(),
    );
    expect(screen.getByLabelText("Depth for the project")).toHaveValue("4");
  });

  it("undoes a first value by taking the key out again", async () => {
    const { saves } = core(page(), () => ({
      now: page({ rows: [row("project", "", [null, null, 2, null, null, null])] }),
    }));
    render(<Page />);

    typeInto(await screen.findByLabelText("Depth for the project"), "2");
    await userEvent.click(await screen.findByRole("button", { name: "Undo" }));

    await waitFor(() => expect(saves).toHaveLength(2));
    expect(saves[1].change).toEqual(edit(["dispatch", "depth"], null));
  });

  it("undoes a removed override by writing back each limit it set", async () => {
    const { saves } = core(
      page({
        rows: [row("project", ""), row("persona", "devops", [null, null, 1, null, null, 2])],
      }),
      () => ({ now: page() }),
    );
    render(<Page />);

    await userEvent.click(
      await screen.findByRole("button", { name: "Remove the override for the persona devops" }),
    );
    await userEvent.click(await screen.findByRole("button", { name: "Undo" }));

    await waitFor(() => expect(saves).toHaveLength(2));
    expect(saves[1].change).toEqual({
      kind: "edits",
      edits: [
        {
          path: ["dispatch", "personas", "devops", "depth"].map((key) => ({ key })),
          value: { kind: "integer", value: 1 },
        },
        {
          path: ["dispatch", "personas", "devops", "may-run-at-once"].map((key) => ({ key })),
          value: { kind: "integer", value: 2 },
        },
      ],
    });
  });

  it("offers no Undo for a change the core refused", async () => {
    core(page(), () => ({ refused: ["dispatch.depth is 9, and depth is never above 8"] }));
    render(<Page />);

    typeInto(await screen.findByLabelText("Depth for the project"), "9");

    await screen.findByText("dispatch.depth is 9, and depth is never above 8");
    expect(screen.queryByRole("button", { name: "Undo" })).not.toBeInTheDocument();
  });

  it("draws a policy's ceiling as locked, with who set it, and no control", async () => {
    core(
      page({
        limits: LIMITS.map((one) => (one.word === "depth" ? { ...one, ceiling: 2 } : one)),
        locked_by: "Locked by policy, set by IT in /etc/purlis/policy.json.",
      }),
    );
    render(<Setting id={`${DISPATCH}.locks`} />);

    const locks = await screen.findByRole("list", { name: "Policy locks" });
    expect(
      within(locks)
        .getAllByRole("listitem")
        .map((one) => one.textContent),
    ).toEqual([
      "Depth is at most 2, whatever is set here. Locked by policy, set by IT in /etc/purlis/policy.json.",
    ]);
    expect(within(locks).queryByRole("textbox")).not.toBeInTheDocument();
    expect(within(locks).queryByRole("button")).not.toBeInTheDocument();
  });

  it("says a refused policy file is refused, once, and not that each limit is 0", async () => {
    const refused =
      "Locked by policy: this machine's policy file, /etc/purlis/policy.json, is refused (it is not JSON), so everything it could lock is locked until this machine's administrator fixes it.";
    core(
      page({
        limits: LIMITS.map((one) => ({ ...one, ceiling: 0 })),
        locked_by: refused,
        policy_refused: true,
      }),
    );
    render(<Setting id={`${DISPATCH}.locks`} />);

    const locks = await screen.findByRole("list", { name: "Policy locks" });
    expect(
      within(locks)
        .getAllByRole("listitem")
        .map((one) => one.textContent),
    ).toEqual([`Dispatch is off on this machine. ${refused}`]);
    expect(within(locks).queryByText(/is 0/)).not.toBeInTheDocument();
  });

  it("says a policy's 0 messages a minute stops messages, not dispatch", async () => {
    const by = "Locked by policy, set by IT in /etc/purlis/policy.json.";
    core(
      page({
        limits: LIMITS.map((one) =>
          one.word === "messages-per-minute" || one.word === "depth" ? { ...one, ceiling: 0 } : one,
        ),
        locked_by: by,
      }),
    );
    render(<Setting id={`${DISPATCH}.locks`} />);

    const locks = await screen.findByRole("list", { name: "Policy locks" });
    expect(
      within(locks)
        .getAllByRole("listitem")
        .map((one) => one.textContent),
    ).toEqual([
      `Depth is 0, so dispatch is off on this machine. ${by}`,
      `Messages per minute is 0, so no chat sends another a message on this machine. ${by}`,
    ]);
  });

  it("says no policy limits dispatch where none does", async () => {
    core(page());
    render(<Setting id={`${DISPATCH}.locks`} />);
    expect(
      await screen.findByText("No policy limits dispatch on this machine."),
    ).toBeInTheDocument();
  });

  it("draws the table of dispatch grants, each with its action", async () => {
    core(page());
    const grants = {
      grants: [
        {
          id: "you\u001fsteward\u001fdevops",
          asking: "steward",
          target: "devops",
          level: "you",
          by: null,
          at: null,
          chat: null,
          locked: null,
          waiting: false,
          declined: false,
          workspace: null,
          nowhere: null,
        },
      ],
      all_locked: null,
      locked_pairs: [{ asking: "qa", target: "devops" }],
      locked_by: "Locked by policy, set by IT in /etc/purlis/policy.json.",
    };
    // After `core`, so this is the one the window asks.
    mockIPC((cmd) => (cmd === "dispatch_grants" ? grants : undefined));
    render(<Setting id={`${DISPATCH}.grants`} />);

    const table = await screen.findByRole("table", { name: /Who may dispatch to whom/ });
    const revoke = within(table).getByRole("button", {
      name: "Revoke: my grant for steward to devops",
    });
    expect(revoke.closest("tr")).toHaveTextContent("Me on this machine");
    // What policy locks is the next row's, not said twice.
    expect(screen.queryByRole("list", { name: "Locked by policy" })).not.toBeInTheDocument();
  });

  it("says there are no dispatch grants where there are none", async () => {
    core(page());
    render(<Setting id={`${DISPATCH}.grants`} />);
    expect(
      await screen.findByText("No persona's chats may dispatch to another persona yet."),
    ).toBeInTheDocument();
  });

  it("lists a locked pair and a lock on all dispatch with the policy's ceilings", async () => {
    const limits = page({
      limits: LIMITS.map((one) => (one.word === "depth" ? { ...one, ceiling: 2 } : one)),
      locked_by: "Locked by policy, set by IT in /etc/purlis/policy.json.",
    });
    const all =
      "Policy forbids one chat dispatching to another. Locked by policy, set by IT in /etc/purlis/policy.json.";
    mockIPC((cmd) => {
      if (cmd === "dispatch_limits") return limits;
      if (cmd === "dispatch_grants")
        return {
          grants: [],
          all_locked: all,
          locked_pairs: [{ asking: "steward", target: "devops" }],
          locked_by: "Locked by policy, set by IT in /etc/purlis/policy.json.",
        };
      return undefined;
    });
    render(<Setting id={`${DISPATCH}.locks`} />);

    const locks = await screen.findByRole("list", { name: "Policy locks" });
    expect(
      within(locks)
        .getAllByRole("listitem")
        .map((one) => one.textContent),
    ).toEqual([
      "Depth is at most 2, whatever is set here. Locked by policy, set by IT in /etc/purlis/policy.json.",
      all,
      "steward chats may not dispatch to devops. Locked by policy, set by IT in /etc/purlis/policy.json.",
    ]);
    expect(within(locks).queryByRole("button")).not.toBeInTheDocument();
  });

  it("says what the core could not read, and offers no table", async () => {
    mockIPC((cmd) => {
      if (cmd === "dispatch_limits")
        throw "purlis.toml is not UTF-8 text, so purlis cannot show it";
      return undefined;
    });
    render(<Page />);
    expect(
      await screen.findByText("purlis.toml is not UTF-8 text, so purlis cannot show it"),
    ).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });
});

describe("a workspace's settings", () => {
  function Workspace() {
    const group = workspaceDispatchGroup(PLANE, "alpha");
    expect(group.id).toBe("workspace.dispatch");
    const { control } = (group.settings[0] as LiveSetting).useControl();
    return <>{control({ id: "w", labelledBy: "w-label" })}</>;
  }

  it("say once, for the group, where the limits are kept, and not under the table", async () => {
    expect(workspaceDispatchGroup(PLANE, "alpha").kept).toMatch(/^Kept in the project's/);
    core(page({ rows: [row("project", ""), row("workspace", "alpha", [null, 9])] }));
    render(<Workspace />);

    await screen.findByRole("table", { name: "Dispatch limits" });
    expect(screen.queryByText(/Kept in/)).toBeNull();
    expect(screen.getByText("An empty box takes the project's value, shown in it.")).toBeVisible();
  });

  it("edit this workspace's own limits, over the project's", async () => {
    const { saves } = core(
      page({
        rows: [
          row("project", "", [4, null, null, null, null, null]),
          row("workspace", "alpha", [null, 9, null, null, null, null], [4, 16, 3, 10, null, null]),
          row("workspace", "beta", [1, null, null, null, null, null]),
        ],
      }),
    );
    render(<Workspace />);

    const table = await screen.findByRole("table", { name: "Dispatch limits" });
    // This workspace's row and no other.
    expect(
      within(table)
        .getAllByRole("rowheader")
        .map((one) => one.textContent),
    ).toEqual(["This workspace"]);
    expect(screen.getByLabelText("Live per lineage for this workspace")).toHaveValue("9");
    // The project's 4 is in force until this workspace sets its own.
    expect(screen.getByLabelText("Running per chat for this workspace")).toHaveAttribute(
      "placeholder",
      "4",
    );
    // A persona's limits are not a workspace's.
    expect(screen.queryByLabelText("May dispatch for this workspace")).not.toBeInTheDocument();

    typeInto(screen.getByLabelText("Running per chat for this workspace"), "12");
    await waitFor(() => expect(saves).toHaveLength(1));
    expect(saves[0]).toMatchObject({
      which: "shared",
      change: edit(["dispatch", "workspaces", "alpha", "running-per-chat"], {
        kind: "integer",
        value: 12,
      }),
    });
  });

  it("add the override by typing its first limit, and remove it again", async () => {
    const withRow = page({
      rows: [row("project", ""), row("workspace", "alpha", [null, null, 2, null, null, null])],
    });
    const { saves } = core(
      page({ rows: [row("project", "", [null, null, 5, null, null, null])] }),
      () => ({
        now: withRow,
      }),
    );
    render(<Workspace />);

    // No override yet: the row is there to fill in, on the project's values.
    const depth = await screen.findByLabelText("Depth for this workspace");
    expect(depth).toHaveAttribute("placeholder", "5");
    expect(screen.queryByRole("button", { name: /Remove/ })).not.toBeInTheDocument();
    typeInto(depth, "2");
    await waitFor(() => expect(saves).toHaveLength(1));

    await userEvent.click(
      await screen.findByRole("button", { name: "Remove the override for this workspace" }),
    );
    await waitFor(() => expect(saves).toHaveLength(2));
    expect(saves[1].change).toEqual(edit(["dispatch", "workspaces", "alpha"], null));
  });
});

/**
 * **A persona's row links to the persona's tab** (#1388): a persona stays in its own tab, and
 * Settings links to it where it names one. A workspace's row, the project's, a name that is no
 * persona of the project now, and the persona view's own table do not.
 */
describe("a persona's row", () => {
  it("links to the persona's tab, and only a persona the project has", async () => {
    const heard: SettingsActionAsk[] = [];
    const on = (event: Event) => heard.push((event as CustomEvent<SettingsActionAsk>).detail);
    window.addEventListener(SETTINGS_ACTION, on);
    onTestFinished(() => window.removeEventListener(SETTINGS_ACTION, on));
    core(
      page({
        rows: [
          row("project", ""),
          row("workspace", "alpha", [12]),
          row("persona", "devops", [null, null, 1]),
          row("persona", "gone", [null, null, 1]),
        ],
      }),
    );
    render(<Page />);

    const table = await screen.findByRole("table", { name: "Dispatch limits" });
    const devops = await within(table).findByRole("button", { name: "Show devops" });
    await userEvent.click(devops);

    expect(heard).toEqual([{ plane: PLANE, action: "persona.show:devops" }]);
    expect(within(table).getAllByRole("button", { name: /^Show / })).toHaveLength(1);
    // The row's name is still the row's own.
    expect(screen.getByRole("rowheader", { name: "Persona devops" })).toBeInTheDocument();
  });

  it("does not link on the persona view, which is that persona's tab", async () => {
    core(page({ rows: [row("project", ""), row("persona", "devops", [null, null, 1])] }));
    render(<DispatchLimitsTable plane={PLANE} scope={{ kind: "persona", name: "devops" }} />);

    const table = await screen.findByRole("table", { name: "Dispatch limits" });
    expect(within(table).queryByRole("button", { name: /^Show / })).toBeNull();
  });
});

describe("the persona view", () => {
  it("edits this persona's own limits, its two own among them, in the project file", async () => {
    const { saves } = core(
      page({
        rows: [row("project", ""), row("persona", "devops", [null, null, 1, null, null, 2])],
      }),
    );
    render(<DispatchLimitsTable plane={PLANE} scope={{ kind: "persona", name: "devops" }} />);

    const table = await screen.findByRole("table", { name: "Dispatch limits" });
    expect(
      within(table)
        .getAllByRole("rowheader")
        .map((one) => one.textContent),
    ).toEqual(["This persona"]);
    expect(screen.getByLabelText("May run at once for this persona")).toHaveValue("2");
    // Kept in the project's file, never the persona's own, and it says so.
    expect(
      screen.getByText(
        /Kept in purlis\.toml, which your team sees, and never in the persona's own file/,
      ),
    ).toBeInTheDocument();

    typeInto(screen.getByLabelText("May dispatch for this persona"), "3");
    await waitFor(() => expect(saves).toHaveLength(1));
    expect(saves[0]).toMatchObject({
      which: "shared",
      change: edit(["dispatch", "personas", "devops", "may-dispatch"], {
        kind: "integer",
        value: 3,
      }),
    });
  });

  it("offers Undo under the table for the last limit changed there", async () => {
    const before = page({
      rows: [row("project", ""), row("persona", "devops", [null, null, 1, null, null, 2])],
    });
    const { saves } = core(before, () => ({
      now: page({
        rows: [row("project", ""), row("persona", "devops", [null, null, 1, null, null, 5])],
      }),
    }));
    render(<DispatchLimitsTable plane={PLANE} scope={{ kind: "persona", name: "devops" }} />);

    typeInto(await screen.findByLabelText("May run at once for this persona"), "5");
    await userEvent.click(await screen.findByRole("button", { name: "Undo" }));

    await waitFor(() => expect(saves).toHaveLength(2));
    expect(saves[1].change).toEqual(
      edit(["dispatch", "personas", "devops", "may-run-at-once"], { kind: "integer", value: 2 }),
    );
  });

  it("draws nothing where the core says nothing of the limits", async () => {
    mockIPC(() => undefined);
    const { container } = render(
      <DispatchLimitsTable plane={PLANE} scope={{ kind: "persona", name: "devops" }} />,
    );
    await waitFor(() =>
      expect(screen.queryByText("Reading the dispatch limits…")).not.toBeInTheDocument(),
    );
    expect(container.querySelector("table")).toBeNull();
  });
});

/**
 * **The profiles the project lists for dispatched chats** (#1522, `[dispatch.profiles]`): shown
 * on the Dispatch page, each persona's list beside the persona's own profile, read-only. Edit
 * as TOML is the way to change them, and the core's sentence about a list it cannot read is
 * said with them.
 */
describe("the profiles for dispatched chats", () => {
  const listed = (lists: Record<string, string[]>, refusals: SettingsRefusal[] = []) => ({
    file: "purlis.toml",
    exists: true,
    text: SHARED,
    refusals,
    parsed: true,
    fields: Object.entries(lists).map(([persona, profiles]) => ({
      path: [{ key: "dispatch" }, { key: "profiles" }, { key: persona }],
      value: { kind: "list" as const, value: profiles },
    })),
  });

  function Profiles({ shared }: { shared: ReturnType<typeof listed> }) {
    const setting = dispatchGroup(PLANE, "purlis.toml", shared).settings.find(
      (one) => one.id === `${DISPATCH}.profiles`,
    ) as LiveSetting | undefined;
    if (setting === undefined) return <p>no row</p>;
    return <>{setting.useControl().control({ id: "p", labelledBy: "p-label" })}</>;
  }

  function options(personaProfiles: Record<string, string> = {}) {
    mockIPC((cmd) => {
      if (cmd === "start_options")
        return {
          profiles: [],
          refused: [],
          personas: ["devops", "steward"],
          persona_profiles: personaProfiles,
        };
      return undefined;
    });
  }

  it("is a row of the page, after the grants, where the project's file was read", () => {
    const group = dispatchGroup(PLANE, "purlis.toml", listed({}));
    expect(group.settings.map((one) => one.id)).toEqual([
      "project.dispatch.limits",
      "project.dispatch.grants",
      "project.dispatch.profiles",
      "project.dispatch.locks",
    ]);
    const row = group.settings[2];
    expect(row.label).toBe("Profiles for dispatched chats");
    expect(row.help).toMatch(/Change them under Edit as TOML\./);
  });

  it("lists each persona's profiles beside the persona's own, with nothing to type in", async () => {
    options({ devops: "work" });
    render(<Profiles shared={listed({ devops: ["work", "codex"], steward: ["claude"] })} />);
    expect(
      await screen.findByText(
        "A chat dispatched to devops starts on work or codex, or not at all. Its own profile is work.",
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText("A chat dispatched to steward starts on claude, or not at all."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("says what no list means", async () => {
    options();
    render(<Profiles shared={listed({})} />);
    expect(
      await screen.findByText(
        "No persona has a list, so a chat dispatched to any persona may start on any profile the project offers.",
      ),
    ).toBeInTheDocument();
  });

  it("says the core's sentence about a list it cannot read", async () => {
    options();
    const why =
      "dispatch.profiles.devops in purlis.toml is not a list, so no chat dispatched to devops starts until it is mended";
    render(
      <Profiles
        shared={listed({}, [
          { why, key: ["dispatch", "profiles", "devops"] },
          { why: "something else entirely", key: ["dispatch", "depth"] },
        ])}
      />,
    );
    expect(await screen.findByText(why)).toBeInTheDocument();
    expect(screen.queryByText("something else entirely")).not.toBeInTheDocument();
  });
});

describe("on a read-only project (#1795)", () => {
  const SENTENCE = "purlis opens the project read-only and writes nothing to it.";
  const ReadOnly = () => (
    <ReadOnlySaid.Provider value={SENTENCE}>
      <Page />
    </ReadOnlySaid.Provider>
  );

  it("is a row Settings leaves on: its controls say for themselves", () => {
    for (const group of [dispatchGroup(PLANE), workspaceDispatchGroup(PLANE, "alpha")]) {
      const limits = group.settings.find((one) => one.id.endsWith(".limits")) as LiveSetting;
      expect(limits.writes).toBe("per control");
    }
  });

  it("turns off every box of the committed file, and keeps yours on this machine on", async () => {
    const { saves } = core(
      page({
        rows: [row("project", ""), row("workspace", "alpha", [12, null, null, null, null, null])],
      }),
    );
    render(<ReadOnly />);

    const project = await screen.findByLabelText("Depth for the project");
    expect(project).toBeDisabled();
    expect(await hoverTipOf(project)).toEqual(SENTENCE);
    expect(screen.getByLabelText("Depth for the workspace alpha")).toBeDisabled();
    const remove = screen.getByRole("button", {
      name: "Remove the override for the workspace alpha",
    });
    expect(remove).toHaveAttribute("aria-disabled", "true");
    expect(tipOf(remove)).toEqual(SENTENCE);
    expect(screen.getByRole("button", { name: "Add an override" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );

    const mine = screen.getByLabelText("Depth for me on this machine");
    expect(mine).toBeEnabled();
    typeInto(mine, "2");
    await waitFor(() => expect(saves).toHaveLength(1));
    expect(saves[0]).toMatchObject({ which: "local" });
  });
});

describe("the limits table's look (DS-9, #1841)", () => {
  /** The page with an override, so the table holds every kind of cell: boxes, Show, Remove. */
  async function drawn() {
    core(page({ rows: [row("project", ""), row("persona", "devops", [null, null, 1])] }));
    render(<Page />);
    return screen.findByRole("table", { name: "Dispatch limits" });
  }

  it("is the shared table, its headings each on one line", async () => {
    const table = await drawn();

    expect(table).toHaveClass("ui-table");
    for (const heading of within(table).getAllByRole("columnheader"))
      expect(drawnWith(heading, "white-space")).toBe("nowrap");
  });

  it("shows a long label in a word or two, and names the column by the whole label", async () => {
    const table = await drawn();

    const shown = within(table)
      .getAllByRole("columnheader")
      .map((one) => one.textContent);
    expect(shown).toEqual([
      "Level",
      "Running",
      "Live",
      "Depth",
      "Messages",
      "May dispatch",
      "At once",
      "Tokens",
      "Minutes",
      "",
    ]);
    expect(
      within(table).getByRole("columnheader", { name: "Messages per minute" }),
    ).toHaveTextContent("Messages");
    // A heading no shorter than its label is named by its own words.
    expect(within(table).getByRole("columnheader", { name: "Depth" })).not.toHaveAttribute(
      "aria-label",
    );
  });

  it("says a heading's whole label and what it limits in a tooltip, by keyboard or pointer", async () => {
    const user = userEvent.setup();
    const table = await drawn();
    const messages = within(
      within(table).getByRole("columnheader", { name: "Messages per minute" }),
    ).getByText("Messages");

    const tip = tipOf(messages);
    expect(tip).toContain("Messages per minute");
    expect(tip).toContain("What messages per minute limits.");

    // The pointer comes onto the heading from the table.
    await user.hover(table);
    await user.hover(within(table).getByText("Depth", { selector: "th *" }));
    expect(await screen.findByRole("tooltip")).toHaveTextContent("What depth limits.");
    // The tooltip is the window's one, not the engine's `title` as well.
    for (const heading of within(table).getAllByRole("columnheader"))
      expect(tipOf(heading)).toBeNull();
  });

  it("draws every box and button in it one control tall, in the shared field and button", async () => {
    const table = await drawn();

    const box = screen.getByLabelText("Depth for the persona devops");
    expect(box.parentElement).toHaveAttribute("data-scope", "text-input");
    expect(drawnWith(box.parentElement as Element, "block-size")).toBe("var(--size-control)");
    for (const button of [
      within(table).getByRole("button", { name: "Remove the override for the persona devops" }),
      within(table).getByRole("button", { name: "Show devops" }),
      screen.getByRole("button", { name: "Add an override" }),
    ]) {
      expect(button).toHaveAttribute("data-scope", "button");
      expect(drawnWith(button, "block-size")).toBe("var(--size-control)");
    }
    // Its buttons are a column as narrow as they are, at the row's end.
    const actions = within(table).getByRole("button", {
      name: "Remove the override for the persona devops",
    }).parentElement as HTMLElement;
    expect(actions).toHaveClass("ui-cell-actions");
  });

  it("sizes nothing of its own: its cells take the shared table's padding and alignment", async () => {
    const table = await drawn();
    const cell = screen.getByLabelText("Depth for the persona devops").closest("td") as HTMLElement;

    expect(drawnWith(cell, "padding-inline")).toBe("var(--ui-cell-pad-inline)");
    expect(drawnWith(cell, "vertical-align")).toBe("middle");
    expect(drawnWith(table, "margin")).toBeUndefined();
  });

  it("has Undo under the persona view's table in the shared button", async () => {
    core(page({ rows: [row("project", ""), row("persona", "devops")] }));
    render(<DispatchLimitsTable plane={PLANE} scope={{ kind: "persona", name: "devops" }} />);
    typeInto(await screen.findByLabelText("Depth for this persona"), "2");

    const undo = await screen.findByRole("button", { name: "Undo" });
    expect(undo).toHaveAttribute("data-scope", "button");
  });
});
