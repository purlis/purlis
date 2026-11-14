// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Setup's Protection row** (T1-24; Q115, Q159): First run's page and the New project dialog,
 * each with the one row, Basic sandbox preselected, drawn with the core faked at its command
 * boundary, held to axe and walked by the keyboard: the arrows move the choice, and the page's
 * own act still finishes setup in one press.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { NoSandboxAtSetup, SetupProtection } from "./bindings";
import { FirstRun } from "./FirstRun";
import { NewProject } from "./NewProject";
import { expectFocusInSight, untilFocused, untilSeenInDialog } from "./stories.testkit";

/** What each story hands the page: its acts, each held to be asked about. */
type Acts = {
  onOpenRepo: ReturnType<typeof fn>;
  onMake?: ReturnType<typeof fn>;
  onCreate?: ReturnType<typeof fn>;
};

/** The core faked at its command boundary, `setup` being what it says of this machine's bound. */
function machine(setup: SetupProtection | null, noSandbox: NoSandboxAtSetup | null = null) {
  return () => {
    mockIPC((cmd) => {
      if (cmd === "first_run_found")
        return {
          harnesses: [{ name: "claude", title: "Claude Code", installed: true, signed_in: true }],
          forges: [],
          templates: [],
        };
      if (cmd === "new_project_folder") return "/home/dev/Projects/billing-api";
      if (cmd === "setup_protection") return setup;
      if (cmd === "no_sandbox_at_setup") return noSandbox;
      return null;
    });
    return () => clearMocks();
  };
}

/** Why the machine's policy holds a new project at Full sandbox, as the core says it. */
const POLICY_WHY = "Your team's policy keeps every project at Full sandbox.";

/** A machine whose policy allows nothing below Full sandbox, and starts the row there. */
const BOUND: SetupProtection = { lowest: "on", default: "on", why: POLICY_WHY, yours: null };

const meta = {
  beforeEach: machine(null),
} satisfies Meta<Acts>;

export default meta;
type Story = StoryObj<Meta<Acts>>;

/** The row's radio group, found by its name. */
const rowIn = (inside: HTMLElement) =>
  within(inside).getByRole("radiogroup", { name: "Protection" });

/** From Basic sandbox, the arrow key moves the choice to Full sandbox, the focus with it. */
async function arrowToFull(row: HTMLElement) {
  const basic = within(row).getByRole("radio", { name: /^Basic sandbox \(recommended\)/ });
  basic.focus();
  await untilFocused(() => basic, "the row starts on Basic sandbox");
  expectFocusInSight();
  await userEvent.keyboard("{ArrowDown}");
  const full = within(row).getByRole("radio", { name: /^Full sandbox/ });
  await untilFocused(() => full, "the arrow moves to Full sandbox");
  await waitFor(() => expect(full).toBeChecked());
  expectFocusInSight();
}

/** First run's page: one row for both of its acts. */
export const OnFirstRun: Story = {
  args: { onOpenRepo: fn(), onMake: fn() },
  render: (args) => (
    <FirstRun
      onOpenRepo={args.onOpenRepo as never}
      onMake={args.onMake as never}
      onBack={() => undefined}
      opening={false}
    />
  ),
  play: async ({ canvasElement, args }) => {
    const row = await waitFor(() => rowIn(canvasElement));
    await arrowToFull(row);
    const canvas = within(canvasElement);
    await userEvent.type(canvas.getByLabelText("Or type the repo's path"), "/home/dev/widget");
    await userEvent.click(canvas.getByRole("button", { name: "Open repo" }));
    await expect(args.onOpenRepo).toHaveBeenCalledTimes(1);
    await expect(args.onOpenRepo).toHaveBeenCalledWith("/home/dev/widget", { kind: "fits" }, "on");
  },
};

/** The New project dialog: the same row, above its acts. */
export const InNewProject: Story = {
  args: { onOpenRepo: fn(), onCreate: fn() },
  parameters: { screenshot: "page" },
  render: (args) => (
    <NewProject
      making={false}
      opening={false}
      onCreate={args.onCreate as never}
      onOpenRepo={args.onOpenRepo as never}
      onCancel={() => undefined}
    />
  ),
  play: async ({ canvasElement, args }) => {
    const dialog = await waitFor(() =>
      within(canvasElement.ownerDocument.body).getByRole("dialog", { name: "New project" }),
    );
    await arrowToFull(rowIn(dialog));
    await userEvent.type(within(dialog).getByLabelText("Repo"), "/home/dev/widget");
    await userEvent.click(within(dialog).getByRole("button", { name: "Open repo" }));
    await expect(args.onOpenRepo).toHaveBeenCalledTimes(1);
    await expect(args.onOpenRepo).toHaveBeenCalledWith("/home/dev/widget", "on");
  },
};

/**
 * Below the bound, out of reach: Basic sandbox and No sandbox are disabled, the row says the
 * policy's reason, and the arrow keys pass over them, so the choice stays on Full sandbox.
 */
async function expectHeldAtFull(row: HTMLElement) {
  const full = within(row).getByRole("radio", { name: /^Full sandbox/ });
  const below = [/^Basic sandbox/, /^No sandbox/].map((name) =>
    within(row).getByRole("radio", { name }),
  );
  await waitFor(() => expect(full).toBeChecked());
  for (const one of below) await expect(one).toBeDisabled();
  const described = (row.getAttribute("aria-describedby") ?? "")
    .split(" ")
    .map((id) => row.ownerDocument.getElementById(id)?.textContent ?? "")
    .join(" ");
  await expect(described).toContain(POLICY_WHY);
  // New project's dialog fades in as it opens, so the reason is read once it is seen (#6360).
  await untilSeenInDialog(within(row.ownerDocument.body).getByText(POLICY_WHY, { exact: false }));
  full.focus();
  await untilFocused(() => full, "the row starts on Full sandbox");
  expectFocusInSight();
  for (const key of ["{ArrowDown}", "{ArrowUp}", "{ArrowRight}", "{ArrowLeft}"]) {
    await userEvent.keyboard(key);
    await untilFocused(() => full, `${key} passes over the options out of reach`);
    await expect(full).toBeChecked();
    for (const one of below) await expect(one).not.toBeChecked();
    expectFocusInSight();
  }
}

/** New project on a machine whose policy allows nothing below Full sandbox (Q241). */
export const BoundByAPolicy: Story = {
  args: { onOpenRepo: fn(), onCreate: fn() },
  beforeEach: machine(BOUND),
  render: (args) => (
    <NewProject
      making={false}
      opening={false}
      onCreate={args.onCreate as never}
      onOpenRepo={args.onOpenRepo as never}
      onCancel={() => undefined}
    />
  ),
  play: async ({ canvasElement, args }) => {
    const dialog = await waitFor(() =>
      within(canvasElement.ownerDocument.body).getByRole("dialog", { name: "New project" }),
    );
    await expectHeldAtFull(rowIn(dialog));
    await userEvent.type(within(dialog).getByLabelText("Repo"), "/home/dev/widget");
    await userEvent.click(within(dialog).getByRole("button", { name: "Open repo" }));
    await expect(args.onOpenRepo).toHaveBeenCalledWith("/home/dev/widget", "on");
  },
};

/**
 * First run where this machine's local project is there already: its repo section says the
 * level that project keeps, read-only, and the row above, for a project setup makes, is still
 * held by the policy.
 */
export const YourLevelReadOnly: Story = {
  args: { onOpenRepo: fn(), onMake: fn() },
  beforeEach: machine({ ...BOUND, yours: "on" }),
  render: (args) => (
    <FirstRun
      onOpenRepo={args.onOpenRepo as never}
      onMake={args.onMake as never}
      onBack={() => undefined}
      opening={false}
    />
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findByText("Your level: Full sandbox · change it in Settings"),
    ).toBeVisible();
    await expectHeldAtFull(await waitFor(() => rowIn(canvasElement)));
  },
};

/** What the core says No sandbox would move: this machine's two plain-file vaults (T1-25). */
const TWO_VAULTS: NoSandboxAtSetup = {
  moves: ["personal", "stripe-test"],
  warning: "setup.no-sandbox.runs-as-you",
  refused: null,
};

/** The core's refusal where the team's committed file registers a plain-file vault (R-298). */
const TEAM_REFUSAL =
  "No sandbox is refused here: the vault ci, which Dana Okafor registered in the project's " +
  "committed vaults.json, is a plain file a chat with no sandbox could read. Choose Basic " +
  "sandbox, or ask Dana Okafor to move it into the keychain.";

/** A project whose team registers the plain-file vault `ci`, which Dana Okafor added. */
const TEAM_VAULT: NoSandboxAtSetup = {
  moves: ["personal"],
  warning: "setup.no-sandbox.runs-as-you",
  refused: { vault: "ci", owner: "Dana Okafor", said: TEAM_REFUSAL },
};

/** From Basic sandbox, two arrows down reach No sandbox, the focus with them. */
async function arrowToNone(row: HTMLElement) {
  const basic = within(row).getByRole("radio", { name: /^Basic sandbox \(recommended\)/ });
  basic.focus();
  await untilFocused(() => basic, "the row starts on Basic sandbox");
  await userEvent.keyboard("{ArrowDown}{ArrowDown}");
  const none = within(row).getByRole("radio", { name: /^No sandbox/ });
  await untilFocused(() => none, "two arrows reach No sandbox");
  await waitFor(() => expect(none).toBeChecked());
  expectFocusInSight();
}

/** The New project dialog drawn for a story of No sandbox. */
const newProject = (args: Acts) => (
  <NewProject
    making={false}
    opening={false}
    onCreate={args.onCreate as never}
    onOpenRepo={args.onOpenRepo as never}
    onCancel={() => undefined}
  />
);

/** The dialog, once drawn. */
const dialogIn = (canvasElement: HTMLElement) =>
  waitFor(() =>
    within(canvasElement.ownerDocument.body).getByRole("dialog", { name: "New project" }),
  );

/**
 * **No sandbox at setup** (T1-25; R-066, R-293): choosing it shows the warning and the vault
 * move in the row, in no second dialog, and the main button says what its one press does.
 */
export const NoSandboxMovesTwoVaults: Story = {
  args: { onOpenRepo: fn(), onCreate: fn() },
  beforeEach: machine(null, TWO_VAULTS),
  parameters: { screenshot: "page" },
  render: newProject,
  play: async ({ canvasElement, args }) => {
    const dialog = await dialogIn(canvasElement);
    await arrowToNone(rowIn(dialog));
    const body = within(canvasElement.ownerDocument.body);
    await expect(await body.findByText(/A chat runs as you/)).toBeVisible();
    await expect(
      // Asked once the folder settles (useNoSandbox), so it comes a moment after the choice.
      await body.findByText(
        /moves 2 plain-file vaults, personal and stripe-test, into the system keychain/,
      ),
    ).toBeVisible();
    await expect(body.getAllByRole("dialog")).toHaveLength(1);
    await userEvent.type(within(dialog).getByLabelText("Repo"), "/home/dev/widget");
    const main = within(dialog).getByRole("button", {
      name: "Open repo, and move 2 vaults to the keychain",
    });
    await userEvent.click(main);
    await expect(args.onOpenRepo).toHaveBeenCalledTimes(1);
    await expect(args.onOpenRepo).toHaveBeenCalledWith("/home/dev/widget", "off");
  },
};

/**
 * **No sandbox refused over a team's vault** (T1-25, R-298): the row says the core's refusal,
 * naming the vault's owner, the main act waits, and Use Basic sandbox puts the choice back.
 */
export const NoSandboxRefusedByATeamVault: Story = {
  args: { onOpenRepo: fn(), onCreate: fn() },
  beforeEach: machine(null, TEAM_VAULT),
  parameters: { screenshot: "page" },
  render: newProject,
  play: async ({ canvasElement, args }) => {
    const dialog = await dialogIn(canvasElement);
    const row = rowIn(dialog);
    await arrowToNone(row);
    const body = within(canvasElement.ownerDocument.body);
    await expect(await body.findByText(TEAM_REFUSAL)).toBeVisible();
    await expect(body.queryByText(/into the system keychain/)).toBeNull();
    await userEvent.type(within(dialog).getByLabelText("Repo"), "/home/dev/widget");
    await expect(within(dialog).getByRole("button", { name: "Open repo" })).toBeDisabled();
    const basic = within(dialog).getByRole("button", { name: "Use Basic sandbox" });
    basic.focus();
    await untilFocused(() => basic, "Use Basic sandbox takes the focus");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(within(row).getByRole("radio", { name: /^Basic sandbox/ })).toBeChecked(),
    );
    await expect(body.queryByText(TEAM_REFUSAL)).toBeNull();
    await userEvent.click(within(dialog).getByRole("button", { name: "Open repo" }));
    await expect(args.onOpenRepo).toHaveBeenCalledWith("/home/dev/widget", "floor");
  },
};
