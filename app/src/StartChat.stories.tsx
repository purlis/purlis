// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Start a chat** (#2918, prototype B of #2798): the window's own dialog, two panes at a fixed
 * height — the persona list under its filter beside what the chat starts with — drawn with the
 * prototype's 28 personas and three harnesses. Held to axe, walked by the keyboard and compared
 * with its baseline in Chromium and WebKit (`docs/component-stories.md`). The picture is of the
 * dialog as it opens, before the play answers it (`pictureNow`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor, within } from "storybook/test";
import type { ProfileRow, StartOptions } from "./bindings";
import { StartChat, type RunsOnInPicker } from "./StartChat";
import {
  SHORT_WINDOW,
  expectAnchored,
  expectFocusInSight,
  inAWindow,
  pictureNow,
  scrollIntoItsPane,
  untilFocused,
  untilInSight,
} from "./stories.testkit";

const PERSONAS = [
  "steward",
  "devops",
  "domain-authority-master",
  "elastic-logs-master",
  "email-investigation-master",
  "email-verification-master",
  "entri-master",
  "esp-integrations-master",
  "grafana-master",
  "inbox-placement-master",
  "integrations-master",
  "jira-git-flow-master",
  "keycloak-master",
  "limits-master",
  "marketplace-integrations-master",
  "messaging-integrations-master",
  "nestjs-developer",
  "organizations-master",
  "pending-actions-master",
  "performance-investigator",
  "platform-and-authz-master",
  "psa-integrations-master",
  "public-api-master",
  "reporting-master",
  "security-reviewer",
  "sending-domains-master",
  "support-triage",
  "ui-designer",
];

/** One of the built-in profiles, as the core answers it. */
function builtIn(
  name: string,
  kind: string,
  glyph: string,
  monogram: string,
  billed: string,
): ProfileRow {
  return {
    name,
    kind,
    shown: name,
    source: "built-in",
    is_default: kind === "claude",
    ready_to_type: true,
    harness: null,
    sandbox: null,
    approval: null,
    model: "the harness's default",
    billed_to: billed,
    icon: { glyph, own: null, monogram },
  };
}

const OPTIONS: StartOptions = {
  profiles: [
    builtIn("claude", "claude", "spark", "C", "whatever Claude Code is signed in with · claude"),
    builtIn("opencode", "opencode", "braces", "OC", "whatever opencode is signed in with"),
    builtIn("codex", "codex", "terminal", "CX", "whatever Codex is signed in with"),
  ],
  own_icons: null,
  refused: [],
  personas: PERSONAS,
  persona: "steward",
  persona_profiles: {},
  persona_descriptions: { steward: "Control Plane Steward" },
  ignore_fix: null,
  ignore_fix_id: null,
  declares_none: true,
};

/** Where a start's Runs on choice sends the chat, as the story's status says it (#4536). */
const whereSaid = (runsOn?: { pool: string | null } | null) =>
  runsOn == null ? "" : runsOn.pool === null ? " on this machine" : ` on pool ${runsOn.pool}`;

/** The dialog as the window opens it, and what was answered once it is gone. */
function Window({
  options = OPTIONS,
  runsOn,
  trouble,
  poolUnavailable,
  pending = false,
}: {
  options?: StartOptions;
  /** Where a new chat would run, and why: the Runs on row (#4536), with how each pool stands
   *  (#4484). */
  runsOn?: RunsOnInPicker;
  trouble?: string;
  poolUnavailable?: string;
  pending?: boolean;
}) {
  const [open, setOpen] = useState(true);
  const [said, setSaid] = useState("Nothing started");
  return (
    <>
      <p role="status" style={{ margin: 0 }}>
        {said}
      </p>
      {open && (
        <StartChat
          options={options}
          pending={pending}
          place="smart-ide"
          runsOn={runsOn}
          trouble={trouble}
          poolUnavailable={poolUnavailable}
          onPoolSetup={(pool) => {
            setSaid(pool === undefined ? "Pool setup" : `Pool setup on ${pool}`);
            setOpen(false);
          }}
          onStart={(profile, persona, footer, label, _branch, _sandbox, where) => {
            setSaid(
              `Started ${label ?? persona ?? "a chat"} as ${persona ?? "no persona"} on ${profile}` +
                (footer ? " with purlis's footer" : "") +
                whereSaid(where),
            );
            setOpen(false);
          }}
          onApprove={() => setSaid("Approved")}
          onCancel={() => {
            setSaid("Cancelled");
            setOpen(false);
          }}
        />
      )}
    </>
  );
}

const meta = {
  component: Window,
  parameters: { screenshot: "page" },
} satisfies Meta<typeof Window>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The dialog's box: where it is and how big, which no pick changes. */
const boxOf = (dialog: HTMLElement) => {
  const { top, width, height } = dialog.getBoundingClientRect();
  return [top, width, height];
};

/**
 * **Prototype B**: the wide size at a fixed height, the list beside the details. Filtering,
 * picking a persona and a harness move and resize nothing, the footer and Start stay in view,
 * and ⏎ in the filter starts the chat on what is picked.
 */
export const PrototypeB: Story = {
  play: async ({ userEvent }) => {
    // A dialog is drawn over the page, outside the story's page.
    const page = within(document.body);
    const dialog = await page.findByRole("dialog", { name: "Start a chat" });
    await expect(dialog).toHaveAttribute("data-size", "wide");
    await expect(dialog).toHaveAccessibleDescription("In smart-ide · purlis's built-in profiles");
    // The keyboard starts on Cancel: a Return pressed by habit starts nothing.
    await untilFocused(
      () => page.getByRole("button", { name: "Cancel" }),
      "Start a chat: the keyboard starts on Cancel",
    );
    await pictureNow();
    // At its fixed top, its bottom clear of the window's (R2).
    expectAnchored(dialog);
    const box = boxOf(dialog);

    const filter = page.getByRole("searchbox", { name: "Filter personas" });
    await expect(filter).toHaveAttribute("placeholder", "Filter 28 personas");
    await userEvent.click(filter);
    await userEvent.type(filter, "master");
    // The pick the filter left out went to the first match (R13); the caret stayed (R168).
    await expect(filter).toHaveFocus();
    await expect(page.getByRole("option", { name: "domain-authority-master" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await userEvent.keyboard("{ArrowDown}");
    await expect(page.getByRole("option", { name: "elastic-logs-master" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await expect(boxOf(dialog)).toEqual(box);

    // A harness card picked: the dialog keeps its box and the summary follows (R33).
    await userEvent.click(page.getByRole("radio", { name: "opencode" }));
    await expect(page.getByTestId("start-chat-summary")).toHaveTextContent(
      "elastic-logs-master on",
    );
    await expect(page.getByTestId("start-chat-summary")).toHaveTextContent("opencode");
    await expect(boxOf(dialog)).toEqual(box);

    // The footer and its answers are in view whatever the panes hold.
    const start = page.getByRole("button", { name: "Start chat" });
    const shown = start.getBoundingClientRect();
    await expect(shown.bottom).toBeLessThanOrEqual(window.innerHeight);

    await userEvent.type(filter, "zz");
    await waitFor(() => expect(page.getByText("No persona is called “masterzz”.")).toBeVisible());
    await userEvent.clear(filter);
    await userEvent.type(filter, "keycloak");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(page.getByRole("status", { hidden: true })).toHaveTextContent(
        "Started keycloak-master as keycloak-master on opencode",
      ),
    );
  },
};

/**
 * **In a short window** (#3389, #3510): at about 500 px the dialog is shorter than its 620 px,
 * keeps its fixed top and the 24 px under it, and the footer, with Start, is inside the window,
 * where the operator could not reach it before. No picture: the window is resized for this
 * story alone.
 */
export const InAShortWindow: Story = {
  parameters: { screenshot: false },
  play: async () => {
    const page = within(document.body);
    const dialog = await page.findByRole("dialog", { name: "Start a chat" });
    await untilFocused(
      () => page.getByRole("button", { name: "Cancel" }),
      "Start a chat: the keyboard starts on Cancel",
    );
    if (await inAWindow(SHORT_WINDOW.width, SHORT_WINDOW.height))
      await expect(dialog.getBoundingClientRect().height).toBeLessThan(620);
    expectAnchored(dialog);
    const box = dialog.getBoundingClientRect();
    for (const name of ["Cancel", "Start chat"]) {
      const answer = page.getByRole("button", { name }).getBoundingClientRect();
      // In the window, and inside the dialog, which clips what runs past its edge.
      await expect(answer.top).toBeGreaterThanOrEqual(0);
      await expect(answer.bottom).toBeLessThanOrEqual(window.innerHeight);
      await expect(answer.top).toBeGreaterThanOrEqual(box.top - 0.5);
      await expect(answer.bottom).toBeLessThanOrEqual(box.bottom + 0.5);
    }
  },
};

/** The prototype's harnesses in a project that runs every chat sandboxed, one asking approval. */
const GUARDED: StartOptions = {
  ...OPTIONS,
  profiles: [
    {
      ...OPTIONS.profiles[0],
      sandbox: {
        state: "sandboxed",
        said: "claude reaches only the hosts this project allows.",
        install: null,
        locked: null,
      },
    },
    { ...OPTIONS.profiles[1], shown: "opencode --port 0", approval: "new" },
    OPTIONS.profiles[2],
  ],
};

/**
 * **A sandboxed project with a profile to approve**: ticking the opt-out and picking the profile
 * that asks for approval change the main answer's words and look, and move and widen nothing —
 * not the dialog, not the answer (R163).
 */
export const SandboxAndApproval: Story = {
  args: { options: GUARDED },
  play: async ({ userEvent }) => {
    const page = within(document.body);
    const dialog = await page.findByRole("dialog", { name: "Start a chat" });
    await untilFocused(
      () => page.getByRole("button", { name: "Cancel" }),
      "Start a chat: the keyboard starts on Cancel",
    );
    // Said in the details' Sandbox section; the dialog fades in as it opens, so it is waited for.
    await waitFor(() =>
      expect(page.getByText("claude reaches only the hosts this project allows.")).toBeVisible(),
    );
    await pictureNow();
    const box = boxOf(dialog);
    const answer = () => page.getByRole("button", { name: /^(Start|Approve)/ });
    const room = () => {
      const { top, width, height } = answer().getBoundingClientRect();
      return [top, width, height];
    };
    const before = room();

    await userEvent.click(page.getByRole("checkbox", { name: "Start without the sandbox" }));
    await expect(answer()).toHaveAccessibleName("Start without the sandbox");
    await expect(
      page.getByRole("textbox", { name: "Why, if you want it recorded" }),
    ).toBeInTheDocument();
    await expect(boxOf(dialog)).toEqual(box);
    await expect(room()).toEqual(before);

    await userEvent.click(page.getByRole("radio", { name: "opencode" }));
    await expect(answer()).toHaveAccessibleName("Approve and start");
    await expect(page.getByRole("alert")).toHaveTextContent("opencode --port 0");
    await expect(boxOf(dialog)).toEqual(box);
    await expect(room()).toEqual(before);
    expectFocusInSight();
  },
};

/** Every pool this machine keeps, in the Runs on stories (#4536). */
const POOLS = ["builds", "gpu"];

/** The dialog open and the keyboard on Cancel, as each Runs on story starts. */
async function opened() {
  const page = within(document.body);
  await page.findByRole("dialog", { name: "Start a chat" });
  await untilFocused(
    () => page.getByRole("button", { name: "Cancel" }),
    "Start a chat: the keyboard starts on Cancel",
  );
  await waitFor(() => expect(page.getByRole("heading", { name: "Runs on" })).toBeVisible());
  return page;
}

/**
 * **The Runs on row in the picture** (#5016): the row sits below the harness cards, under the
 * details pane's fold as the dialog opens, so it is scrolled to the middle of the pane, and only
 * the pane, before `pictureNow`. What a person reads there is held wholly in sight: its heading,
 * the row's control (the select, or Pool setup…) and the line under it that says why.
 */
async function runsOnInSight(page: ReturnType<typeof within>) {
  const section = page.getByRole("region", { name: "Runs on" });
  scrollIntoItsPane(section);
  const inside = within(section);
  await untilInSight(
    [
      inside.getByRole("heading", { name: "Runs on" }),
      ...inside.queryAllByRole("button"),
      ...section.querySelectorAll("p"),
    ],
    "Runs on: the row in the picture",
  );
}

/**
 * **A workspace's first chat on its project's pool** (#4591): before anything is sent there, the
 * row says plainly that the workspace's code is copied to that pool's runner and the chat's work
 * comes back here.
 */
export const RunsOnFirstOnThePool: Story = {
  args: { runsOn: { pool: "builds", level: "project", at_root: false, pools: POOLS, ran_on: [] } },
  play: async () => {
    const page = await opened();
    const runsOn = page.getByRole("button", { name: /Runs on$/ });
    await expect(runsOn).toHaveAccessibleDescription(
      "Pool builds, as this project's chats run. This is this workspace's first chat on " +
        "pool builds. Its code is copied to that pool's runner. The chat's work comes back " +
        "here when it ends.",
    );
    await runsOnInSight(page);
    await pictureNow();
  },
};

/**
 * **Runs on, following the project's pool** (#4536): the row says the place and the level that
 * put it there. Picked from the keyboard, a pool is for this start only and says so, and Start
 * carries it.
 */
export const RunsOnFollowsTheProject: Story = {
  args: {
    runsOn: { pool: "builds", level: "project", at_root: false, pools: POOLS, ran_on: POOLS },
  },
  play: async ({ userEvent }) => {
    const page = await opened();
    const runsOn = page.getByRole("button", { name: /Runs on$/ });
    await expect(runsOn).toHaveTextContent("As set (pool builds)");
    await expect(runsOn).toHaveAccessibleDescription("Pool builds, as this project's chats run.");
    await runsOnInSight(page);
    await pictureNow();

    runsOn.focus();
    await userEvent.keyboard("{Enter}");
    await untilFocused(
      () => page.getByRole("option", { name: "As set (pool builds)" }),
      "Runs on: its list opens on what it holds",
    );
    expectFocusInSight();
    await userEvent.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}");
    await untilFocused(
      () => page.getByRole("option", { name: "Pool gpu" }),
      "Runs on: the arrows move to pool gpu",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await untilFocused(() => runsOn, "Runs on: a pick hands the keyboard back to the select");
    expectFocusInSight();
    await expect(runsOn).toHaveTextContent("Pool gpu");
    await expect(runsOn).toHaveAccessibleDescription(
      "Pool gpu for this start only. No setting changes.",
    );

    await userEvent.click(page.getByRole("button", { name: "Start chat" }));
    await waitFor(() =>
      expect(page.getByRole("status", { hidden: true })).toHaveTextContent(
        "Started steward as steward on claude on pool gpu",
      ),
    );
  },
};

/** **A pool whose setup isn't finished** (#4484): marked in the list, said under the row, with
 *  Finish Pool setup… on that pool, reached by the keyboard. */
export const RunsOnSetupNotFinished: Story = {
  args: {
    runsOn: {
      pool: "builds",
      level: "project",
      at_root: false,
      pools: POOLS,
      ran_on: POOLS,
      notFinished: {
        builds:
          "Setup not finished: chats start there, but gh and git push have no account to act as.",
      },
    },
  },
  play: async ({ userEvent }) => {
    const page = await opened();
    const runsOn = page.getByRole("button", { name: /Runs on$/ });
    await expect(runsOn).toHaveAccessibleDescription(
      "Pool builds, as this project's chats run. Setup not finished: chats start there, but gh and git push have no account to act as.",
    );
    const finish = page.getByRole("button", { name: "Finish Pool setup…" });
    runsOn.focus();
    await userEvent.tab();
    await untilFocused(() => finish, "Finish Pool setup…: the next stop after Runs on");
    expectFocusInSight();
    await pictureNow();
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(page.getByRole("status", { hidden: true })).toHaveTextContent("Pool setup on builds"),
    );
  },
};

/** **Runs on, where the workspace runs its chats on this machine** (#4536). */
export const RunsOnThisMachineForTheWorkspace: Story = {
  args: { runsOn: { pool: null, level: "workspace", at_root: false, pools: POOLS, ran_on: POOLS } },
  play: async () => {
    const page = await opened();
    const runsOn = page.getByRole("button", { name: /Runs on$/ });
    await expect(runsOn).toHaveTextContent("As set (this machine)");
    await expect(runsOn).toHaveAccessibleDescription(
      "This machine, as this workspace's chats run.",
    );
    await runsOnInSight(page);
    await pictureNow();
  },
};

/** **Runs on, at the project's root** (#4536): a chat there runs here, and no pool is offered. */
export const RunsOnAtTheRoot: Story = {
  args: {
    runsOn: { pool: null, level: "this_machine", at_root: true, pools: POOLS, ran_on: POOLS },
  },
  play: async () => {
    const page = await opened();
    await expect(
      page.getByText("This machine. A chat at the project's root always runs here."),
    ).toBeVisible();
    await expect(page.queryByRole("button", { name: /Runs on$/ })).toBeNull();
    await runsOnInSight(page);
    await pictureNow();
  },
};

/** **Runs on, with no pool on this machine** (#4536): the place, and Pool setup… to add one,
 *  rather than a choice of one. */
export const RunsOnWithNoPool: Story = {
  args: { runsOn: { pool: null, level: "this_machine", at_root: false, pools: [], ran_on: [] } },
  play: async ({ userEvent }) => {
    const page = await opened();
    await expect(
      page.getByText("This machine. No pool is set up on this machine yet."),
    ).toBeVisible();
    await runsOnInSight(page);
    await pictureNow();
    const setup = page.getByRole("button", { name: "Pool setup…" });
    setup.focus();
    await untilFocused(() => setup, "Runs on: Pool setup… takes the keyboard");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(page.getByRole("status", { hidden: true })).toHaveTextContent("Pool setup"),
    );
  },
};

/**
 * **A pool that could not be reached** (#4536): nothing started, and the picker offers Try
 * again, Pool setup… and Run on this machine this once, under the refusal. The last runs this
 * one chat here and changes no setting.
 */
export const PoolUnavailable: Story = {
  args: {
    runsOn: { pool: "builds", level: "project", at_root: false, pools: POOLS, ran_on: POOLS },
    trouble: "Pool builds is stopped, so the chat did not start.",
    poolUnavailable: "builds",
  },
  play: async ({ userEvent }) => {
    const page = await opened();
    await expect(page.getByRole("alert")).toHaveTextContent(
      "Pool builds is stopped, so the chat did not start.",
    );
    for (const name of ["Try again", "Pool setup…", "Run on this machine this once"])
      await expect(page.getByRole("button", { name })).toBeVisible();
    await pictureNow();
    const here = page.getByRole("button", { name: "Run on this machine this once" });
    here.focus();
    await untilFocused(() => here, "Pool unavailable: Run on this machine this once");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(page.getByRole("status", { hidden: true })).toHaveTextContent(
        "Started steward as steward on claude on this machine",
      ),
    );
  },
};

/**
 * **The dialog before each row's details arrive** (#4736): it opens on the personas and the
 * profiles, and every card says its billing and approval are being checked. The keyboard path is
 * the dialog's own, the same as with the details in.
 */
export const DetailsPending: Story = {
  args: { pending: true },
  play: async ({ userEvent }) => {
    const page = within(document.body);
    await page.findByRole("dialog", { name: "Start a chat" });
    await untilFocused(
      () => page.getByRole("button", { name: "Cancel" }),
      "Start a chat: the keyboard starts on Cancel",
    );
    await waitFor(() =>
      expect(page.getAllByText(/Checking billing and approval…/)[0]).toBeVisible(),
    );
    // Held while the dialog is open: once Escape closes it, the story has no opener for the
    // focus to go back to.
    expectFocusInSight();
    await pictureNow();
    await userEvent.keyboard("{Escape}");
    await expect(page.getByRole("status")).toHaveTextContent("Cancelled");
  },
};
