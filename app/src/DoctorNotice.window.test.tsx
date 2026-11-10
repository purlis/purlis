// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { waitOutTheUndo } from "./test-inbox";
import type { DoctorReport, DoctorRow } from "./bindings";
import { forgetThisLaunch } from "./regions";
import { findTheWorkspaces } from "./test-strips";

/**
 * **A Notice that carries a fix with a form draws that form** (#1250), against the whole
 * window.
 *
 * The doctor's `git identity` finding stands as a Notice in the Inbox, carrying its fix id,
 * `git-identity`. That fix takes a name and an email (`FIXES_WITH_A_FORM`), so pressing the
 * Notice's Fix opens the same form the Doctor row's Fix opens: the keys already set shown
 * locked, a refusal under its field, and, once the fix is applied, the doctor checks again and
 * the Notice goes because its finding has.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

const PLANE = "/home/dev/plane";

function row(name: string, status: DoctorRow["status"], over: Partial<DoctorRow> = {}): DoctorRow {
  return {
    name,
    status,
    detail: `${name} said`,
    hint: `${name} hint`,
    checked: true,
    settings: null,
    fix: null,
    ...over,
  };
}

const unset = row("git identity", "fail", {
  detail: "not set: user.email",
  fix: "git-identity",
});
const set = row("git identity", "ok", { detail: "Bea Terminal <bea@example.invalid>" });

const report = (rows: DoctorRow[]): DoctorReport => ({
  rows,
  app_rows: [],
  full: false,
  path: "/usr/bin:/bin",
});

type Asked = { cmd: string; args: unknown };

/** The core: one project with one chat, and a doctor whose identity is `identity()`. */
function core(answers: {
  identity: () => DoctorRow;
  fixIdentity?: (args: Record<string, unknown>) => unknown;
  /** The `personas` row, where a test is about it. */
  personas?: () => DoctorRow;
  fix?: (args: Record<string, unknown>) => unknown;
}): Asked[] {
  const asked: Asked[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    const given = (args ?? {}) as Record<string, unknown>;
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        workspaces: [
          {
            name: "alpha",
            path: `${PLANE}/workspaces/alpha`,
            vision: "",
            todos: [],
            chats: [],
          },
        ],
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
      };
    if (cmd === "plane_doctor")
      return report([
        row("git", "ok"),
        answers.identity(),
        ...(answers.personas ? [answers.personas()] : []),
      ]);
    if (cmd === "plane_doctor_fix") return answers.fix?.(given) ?? null;
    if (cmd === "plane_doctor_identity") return { name: "Bea Terminal", email: "" };
    if (cmd === "plane_doctor_fix_identity") return answers.fixIdentity?.(given) ?? null;
    if (cmd === "opened_chats" || cmd === "chat_states" || cmd === "running_sessions") return [];
    if (cmd === "chats_that_would_not_start") return [];
    return null;
  });
  return asked;
}

beforeEach(() => forgetThisLaunch());

afterEach(() => {
  vi.useRealTimers();
  cleanup();
  clearMocks();
});

/** Shows the Notices view (#5645), in the Panel's right half, as a person presses its icon. */
async function showNotices(): Promise<HTMLElement> {
  const tab = within(await screen.findByRole("tablist", { name: "Panel, right half" })).getByRole(
    "tab",
    { name: /^Notices\b/ },
  );
  if (tab.getAttribute("aria-selected") !== "true") await userEvent.click(tab);
  return screen.findByRole("tabpanel", { name: "Notices" });
}

/** Lets the window finish opening: where the keyboard starts is placed once it has. */
const settle = () => new Promise((done) => setTimeout(done, 100));

/**
 * **The doctor's one line in Notices, opened to its findings** (#5645): the line, and the row of
 * the finding named `name` in its list. Opened only where it is not, since Show findings toggles.
 */
async function finding(name: string): Promise<{ line: HTMLElement; row: HTMLElement }> {
  const notices = await showNotices();
  const line = (await within(notices).findByText(/^Doctor: \d+ findings?$/)).closest(
    "[data-cause]",
  ) as HTMLElement;
  if (within(notices).queryByRole("list", { name: "Doctor findings" }) === null) {
    await settle();
    await userEvent.click(within(line).getByRole("button", { name: "Show findings" }));
  }
  const list = await within(notices).findByRole("list", { name: "Doctor findings" });
  const row = await waitFor(() => {
    const found = list.querySelector<HTMLElement>(`[data-row="${name}"]`);
    if (found === null) throw new Error(`no finding ${name}`);
    return found;
  });
  return { line, row };
}

/** What the window last asked the core to keep dismissed for this project. */
const kept = (asked: Asked[]) =>
  (asked.filter((one) => one.cmd === "set_dismissed").at(-1)?.args as { causes?: string[] })
    ?.causes;

/**
 * Types into a field the form has put the keyboard in. Not by a click: jsdom lays nothing out,
 * so every point is on the resize handles, and a click anywhere moves the focus to one.
 */
async function type(field: HTMLElement, text: string) {
  await waitFor(() => expect(field).toHaveFocus());
  await userEvent.type(field, text, { skipClick: true });
}

async function openForm() {
  const { line: notice, row } = await finding("git identity");
  await settle();
  const fix = within(row).getByRole("button", { name: "Fix git identity" });
  await userEvent.click(fix);
  const form = await screen.findByRole("form", { name: "Git identity" });
  return { notice, fix, form };
}

describe("the doctor's git identity finding, in the doctor's Notice line (#1250, #5645)", () => {
  it("stands in Notices' doctor line, and its Fix opens the form, set keys locked", async () => {
    const asked = core({ identity: () => unset });
    render(<App />);
    expect((await finding("git identity")).line).toHaveAttribute("data-cause", "doctor-finding:*");

    const { notice: line, fix, form } = await openForm();
    // A fix that takes input is not applied by the press alone.
    expect(asked.map((one) => one.cmd)).not.toContain("plane_doctor_fix");
    expect(asked.map((one) => one.cmd)).not.toContain("plane_doctor_fix_identity");
    expect(fix).toHaveAttribute("aria-expanded", "true");
    expect(fix).toHaveAttribute("aria-controls", form.id);
    // Beside the line, never inside its live region: typing is not read out as the Notice.
    expect(line).toHaveAttribute("role", "status");
    expect(line.contains(form)).toBe(false);
    expect(form.closest('[role="status"], [aria-live]')).toBeNull();

    const name = within(form).getByLabelText("Name");
    await waitFor(() => expect(name).toHaveValue("Bea Terminal"));
    expect(name).toBeDisabled();
    expect(name).toHaveAccessibleDescription(/Already set/);
    await waitFor(() => expect(within(form).getByLabelText("Email")).toHaveFocus());
  });

  it("shows the core's refusal under the field it is about, and keeps the Notice", async () => {
    core({
      identity: () => unset,
      fixIdentity: () => ({
        kind: "invalid",
        name: [],
        email: ["That does not look like an email."],
      }),
    });
    render(<App />);
    const { notice, form } = await openForm();

    const email = within(form).getByLabelText("Email");
    await type(email, "nope");
    await userEvent.click(within(form).getByRole("button", { name: "Set identity" }));

    await waitFor(() => expect(email).toHaveAccessibleDescription(/does not look like an email/));
    expect(email).toHaveValue("nope");
    expect(notice).toBeInTheDocument();
  });

  it("says a refusal of both fields once, as its own form's summary, not an alert a row (#3291)", async () => {
    core({
      identity: () => unset,
      fixIdentity: () => ({
        kind: "invalid",
        name: ["A name is needed."],
        email: ["That does not look like an email."],
      }),
    });
    render(<App />);
    const { form } = await openForm();
    const email = within(form).getByLabelText("Email");
    await type(email, "nope");
    await userEvent.click(within(form).getByRole("button", { name: "Set identity" }));

    await waitFor(() => expect(email).toHaveAccessibleDescription(/does not look like an email/));
    const alerts = within(form).getAllByRole("alert");
    expect(alerts).toHaveLength(1);
    expect(alerts[0]).toHaveTextContent("2 fields to fix: Name, Email.");
  });

  it("goes once the fix is applied and the doctor checks again", async () => {
    let fixed = false;
    const asked = core({
      identity: () => (fixed ? set : unset),
      fixIdentity: () => {
        fixed = true;
        return {
          kind: "fixed",
          fixed: {
            fix: "git-identity",
            refused: null,
            said: ["✓ set user.email = bea@example.invalid (global git config)"],
            complete: true,
          },
        };
      },
    });
    render(<App />);
    const { form } = await openForm();

    await type(within(form).getByLabelText("Email"), "bea@example.invalid");
    await userEvent.click(within(form).getByRole("button", { name: "Set identity" }));

    expect(asked).toContainEqual({
      cmd: "plane_doctor_fix_identity",
      args: { plane: PLANE, name: "", email: "bea@example.invalid" },
    });
    await waitFor(() => expect(screen.queryByText("not set: user.email")).toBeNull());
    await waitFor(() => expect(screen.queryByText(/^Doctor: /)).toBeNull());
    // Gone because the doctor answered again without the finding, not because it was hidden.
    expect(asked.filter((one) => one.cmd === "plane_doctor").length).toBeGreaterThan(1);
  });

  it("says a fix refused as a whole on the Notice, which stays", async () => {
    core({
      identity: () => unset,
      fixIdentity: () => ({
        kind: "fixed",
        fixed: {
          fix: "git-identity",
          refused: "your global git config picks identities by folder (includeIf)",
          said: [],
          complete: false,
        },
      }),
    });
    render(<App />);
    const { notice, form } = await openForm();
    await type(within(form).getByLabelText("Email"), "bea@example.invalid");
    await userEvent.click(within(form).getByRole("button", { name: "Set identity" }));

    expect(await within(notice).findByText(/picks identities by folder/)).toBeVisible();
    expect(screen.queryByRole("form", { name: "Git identity" })).toBeNull();
  });

  it("stays dismissed until the finding is mended, and shows again when it comes back (V91j)", async () => {
    // A Dismiss is kept once its Undo has stood (#5650): the test waits it out.
    vi.useFakeTimers({ shouldAdvanceTime: true, toFake: ["setTimeout", "clearTimeout"] });
    let identity = unset;
    const asked = core({ identity: () => identity });
    render(<App />);
    const { line: notice } = await finding("git identity");
    await settle();
    await userEvent.click(within(notice).getByRole("button", { name: "Dismiss" }));
    waitOutTheUndo();

    expect(screen.queryByText(/^Doctor: /)).toBeNull();
    await waitFor(() => expect(kept(asked)).toEqual(["doctor-finding:git-identity"]));

    /** Asks the doctor again, as the operator does by opening it. */
    const checkAgain = async () => {
      const before = asked.filter((one) => one.cmd === "plane_doctor").length;
      await userEvent.click(screen.getByTestId("status-doctor"));
      const dialog = await screen.findByRole("dialog");
      await waitFor(() =>
        expect(asked.filter((one) => one.cmd === "plane_doctor").length).toBeGreaterThan(before),
      );
      await settle();
      await userEvent.click(within(dialog).getByRole("button", { name: "Close" }));
    };

    // Still found: still dismissed.
    await checkAgain();
    expect(screen.queryByText(/^Doctor: /)).toBeNull();
    expect(kept(asked)).toEqual(["doctor-finding:git-identity"]);

    // Mended (set from a terminal): the dismissal is let go.
    identity = set;
    await checkAgain();
    await waitFor(() => expect(kept(asked)).toEqual([]));

    // And it comes back: the Notice shows again.
    identity = unset;
    await checkAgain();
    expect((await finding("git identity")).row).toBeInTheDocument();
  });

  it("is not drawn while the identity is set", async () => {
    core({ identity: () => set });
    render(<App />);
    await findTheWorkspaces();
    await new Promise((done) => setTimeout(done, 50));

    expect(document.querySelector('[data-cause^="doctor-finding:"]')).toBeNull();
  });
});

describe("every doctor finding, as one Notice line (#5645)", () => {
  const plugin = row("plugin", "warn", {
    detail: "the plugin is not installed for chats started outside the app",
    fix: "plugin-install",
  });
  const memory = row("memory indexes", "warn", {
    detail: "1 memory is not in its MEMORY.md",
    fix: "memory-optimize",
  });
  const leftover = row("personas", "warn", {
    detail: "1 generated sub-agent file remains in .claude/agents/",
    fix: "persona-agents",
  });

  it("stands as one line, Doctor: N findings, that opens to its list, and none is an update", async () => {
    core({ identity: () => unset, personas: () => plugin });
    render(<App />);
    const notices = await showNotices();

    const line = await within(notices).findByText("Doctor: 2 findings");
    const notice = line.closest("[data-cause]") as HTMLElement;
    expect(notice).toHaveAttribute("data-cause", "doctor-finding:*");
    expect(notices.querySelectorAll('[data-cause^="doctor-finding"]')).toHaveLength(1);
    // Closed, it lists nothing; opened, every finding, each with what the doctor said.
    expect(within(notices).queryByText("not set: user.email")).toBeNull();
    await settle();
    await userEvent.click(within(notice).getByRole("button", { name: "Show findings" }));
    const list = await within(notices).findByRole("list", { name: "Doctor findings" });
    expect(
      within(list)
        .getAllByRole("listitem")
        .map((one) => one.getAttribute("data-row")),
    ).toEqual(["git identity", "plugin"]);
    expect(within(list).getByText("not set: user.email")).toBeInTheDocument();

    const updates = within(await screen.findByRole("tablist", { name: "Attention" })).getByRole(
      "tab",
      { name: /^Updates\b/ },
    );
    await userEvent.click(updates);
    const view = await screen.findByRole("tabpanel", { name: "Updates" });
    expect(within(view).queryByText(/git identity/)).toBeNull();
    expect(within(view).queryByText(/plugin/)).toBeNull();
  });

  it("says one finding as one", async () => {
    core({ identity: () => unset });
    render(<App />);
    const notices = await showNotices();

    expect(await within(notices).findByText("Doctor: 1 finding")).toBeInTheDocument();
  });

  it("offers the doctor's fix: Fix all runs each fix one press may run, then checks again", async () => {
    const asked = core({
      identity: () => set,
      personas: () => memory,
      fix: (args) => ({ fix: args.fix, refused: null, said: [], complete: true }),
    });
    render(<App />);
    const notices = await showNotices();
    const notice = (await within(notices).findByText("Doctor: 1 finding")).closest(
      "[data-cause]",
    ) as HTMLElement;
    await settle();
    const before = asked.filter((one) => one.cmd === "plane_doctor").length;

    await userEvent.click(within(notice).getByRole("button", { name: "Fix all" }));

    await waitFor(() =>
      expect(asked).toContainEqual({
        cmd: "plane_doctor_fix",
        args: { plane: PLANE, fix: "memory-optimize" },
      }),
    );
    await waitFor(() =>
      expect(asked.filter((one) => one.cmd === "plane_doctor").length).toBeGreaterThan(before),
    );
  });

  it("runs no fix that only its name may run, and offers no Fix all for it alone", async () => {
    const asked = core({ identity: () => set, personas: () => plugin });
    render(<App />);
    const notices = await showNotices();
    const notice = (await within(notices).findByText("Doctor: 1 finding")).closest(
      "[data-cause]",
    ) as HTMLElement;
    await settle();

    expect(within(notice).queryByRole("button", { name: "Fix all" })).toBeNull();
    await userEvent.click(within(notice).getByRole("button", { name: "Show findings" }));
    const list = await within(notices).findByRole("list", { name: "Doctor findings" });
    expect(within(list).queryByRole("button", { name: /^Fix/ })).toBeNull();
    expect(asked.map((one) => one.cmd)).not.toContain("plane_doctor_fix");
  });

  it("dismisses every finding it lists, and stands again for a new one", async () => {
    // A Dismiss is kept once its Undo has stood (#5650): the test waits it out.
    vi.useFakeTimers({ shouldAdvanceTime: true, toFake: ["setTimeout", "clearTimeout"] });
    let second: DoctorRow = row("personas", "ok", { detail: "all clean" });
    const asked = core({ identity: () => unset, personas: () => second });
    render(<App />);
    const notices = await showNotices();
    const notice = (await within(notices).findByText("Doctor: 1 finding")).closest(
      "[data-cause]",
    ) as HTMLElement;
    await settle();

    await userEvent.click(within(notice).getByRole("button", { name: "Dismiss" }));
    waitOutTheUndo();

    await waitFor(() => expect(within(notices).queryByText(/^Doctor: /)).toBeNull());
    await waitFor(() => expect(kept(asked)).toEqual(["doctor-finding:git-identity"]));

    second = leftover;
    const before = asked.filter((one) => one.cmd === "plane_doctor").length;
    await userEvent.click(screen.getByTestId("status-doctor"));
    const dialog = await screen.findByRole("dialog");
    await waitFor(() =>
      expect(asked.filter((one) => one.cmd === "plane_doctor").length).toBeGreaterThan(before),
    );
    await settle();
    await userEvent.click(within(dialog).getByRole("button", { name: "Close" }));

    // The new finding alone: the one dismissed stays dismissed.
    expect(await within(notices).findByText("Doctor: 1 finding")).toBeInTheDocument();
    const again = within(notices).getByText("Doctor: 1 finding").closest("[data-cause]");
    await userEvent.click(
      within(again as HTMLElement).getByRole("button", { name: "Show findings" }),
    );
    const list = await within(notices).findByRole("list", { name: "Doctor findings" });
    expect(
      within(list)
        .getAllByRole("listitem")
        .map((one) => one.getAttribute("data-row")),
    ).toEqual(["personas"]);
  });
});

describe("the persona sub-agent files purlis generated, as a Notice (#1990, Q67)", () => {
  const leftover = row("personas", "warn", {
    detail: "1 generated sub-agent file remains in .claude/agents/",
    fix: "persona-agents",
  });
  const gone = row("personas", "ok", { detail: "1 persona, all clean" });

  it("stands once in the doctor's line, its Fix runs the doctor's fix, and it goes with the files", async () => {
    let removed = false;
    const asked = core({
      identity: () => set,
      personas: () => (removed ? gone : leftover),
      fix: () => {
        removed = true;
        return {
          fix: "persona-agents",
          refused: null,
          said: ["- .claude/agents/ops.md (removed)"],
          complete: true,
        };
      },
    });
    render(<App />);

    const { row } = await finding("personas");
    expect(
      await screen.findAllByText("1 generated sub-agent file remains in .claude/agents/"),
    ).toHaveLength(1);
    await settle();

    await userEvent.click(within(row).getByRole("button", { name: "Fix personas" }));

    expect(asked).toContainEqual({
      cmd: "plane_doctor_fix",
      args: { plane: PLANE, fix: "persona-agents" },
    });
    await waitFor(() => expect(screen.queryByText(/generated sub-agent file/)).toBeNull());
    expect(asked.filter((one) => one.cmd === "plane_doctor").length).toBeGreaterThan(1);
  });
});
