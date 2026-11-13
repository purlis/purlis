// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **You › This machine › Pools, and Add a pool** (#2679; the person's ruling of 2026-10-30): the
 * group with no pool yet, saying what a pool is, with Add a pool…; the group with a pool; a pool
 * whose runner was removed outside purlis (#4149); one whose older runners are kept unchecked; and the
 * Add a pool dialog with its Image box showing the runner image it is offered.
 * Each is drawn, held to axe, walked by the keyboard, and pictured in every pictured theme cell
 * (`docs/component-stories.md`). The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { AddDockerPoolHost } from "../AddDockerPool";
import type { PoolLink, PoolProfileRow, PoolRow, PoolsShown, PoolVersion } from "../bindings";
import { askProfiles, startAddingAPool, stopAddingAPool } from "../pools";
import { expectFocusInSight, pictureNow, untilFocused } from "../stories.testkit";
import { SettingGroup, SettingRow } from "./components";
import type { LiveSetting } from "./groups";
import { poolsGroup } from "./pools";

const IMAGE = `ghcr.io/purlis/purlis-runner@sha256:${"4f".repeat(32)}`;
const DEFAULTS = { cpus: "2", memory_mib: 4096, processes: 1024, tmp_mib: 1024 };

const BUILDS: PoolRow = {
  name: "builds",
  image: IMAGE,
  limits: { ...DEFAULTS },
  suspend_after: "24h",
};

/** What a pool's runner says of its version where a story says nothing: this machine's, with no
 *  older runner. */
const LEVEL: PoolVersion = {
  runner: "0.4.2",
  why: null,
  desktop: "0.4.2",
  follows: true,
  draining: 0,
  held: 0,
  named: true,
  waiting: 0,
  owed: [],
  owed_why: null,
  left: [],
  not_checked: [],
};

/** The profiles a pool's runner keeps where a story says nothing: one, signed in. */
const SIGNED_IN: PoolProfileRow[] = [
  {
    name: "work",
    kind: "claude",
    harness: "Claude Code",
    standing: "signed-in",
    not_kept: null,
    limit: null,
    plans: [],
  },
];

/** The core, faked: this machine keeps `pools`, whose runner answers `link`, says `version` and
 *  keeps `profiles`, and its release names {@link IMAGE}. */
function theCore(
  pools: PoolRow[],
  link: PoolLink = { state: "linked" },
  version: PoolVersion = LEVEL,
  profiles: PoolProfileRow[] = SIGNED_IN,
) {
  mockIPC(
    (cmd) => {
      if (cmd === "docker_pools")
        return {
          pools,
          defaults: { ...DEFAULTS },
          suspend_after: "24h",
          frames: 15,
          width: 1280,
          clusters: [],
        } satisfies PoolsShown;
      if (cmd === "docker_pool_link") return link;
      if (cmd === "docker_pool_version") return version;
      if (cmd === "runner_image_of_this_version") return { state: "ours", image: IMAGE };
      if (cmd === "pool_profiles") return { kept: profiles, addable: [] };
      return [];
    },
    { shouldMockEvents: true },
  );
}

/** The group's one row, as the Settings tab draws a live setting. */
function Row({ setting }: { setting: LiveSetting }) {
  const { control, grouped, error } = setting.useControl();
  return (
    <SettingRow
      setting={setting.id}
      label={setting.label}
      help={setting.help}
      grouped={grouped}
      error={error}
      control={control}
    />
  );
}

/** The Pools group as Settings › You draws it, with the dialog its Add a pool… opens. */
function Pools() {
  const group = poolsGroup();
  return (
    <>
      <SettingGroup label={group.label} help={group.help}>
        {group.settings.map((one) => (
          <Row key={one.id} setting={one as LiveSetting} />
        ))}
      </SettingGroup>
      <AddDockerPoolHost />
    </>
  );
}

const meta = {
  component: Pools,
  // The pools the faked core keeps: a story's `parameters.pools`, none by default.
  beforeEach: async ({ parameters }) => {
    theCore(
      (parameters.pools as PoolRow[] | undefined) ?? [],
      parameters.link as PoolLink | undefined,
      parameters.version as PoolVersion | undefined,
      parameters.profiles as PoolProfileRow[] | undefined,
    );
    // The window's pools are held once for every story drawn in this page: a pool of the same
    // name another story's faked core answered for is asked again here, so its profiles' line is
    // this core's.
    await askProfiles("builds");
    return () => {
      stopAddingAPool();
      clearMocks();
    };
  },
} satisfies Meta<typeof Pools>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **No pool yet**: the group says what a pool is, and Tab reaches its Add a pool…. */
export const NoPoolYet: Story = {
  play: async ({ canvas, userEvent }) => {
    const group = await canvas.findByRole("region", { name: "Pools" });
    await within(group).findByText(/No pool yet\. A pool is a runner on your Docker/);
    const add = within(group).getByRole("button", { name: "Add a pool…" });
    await userEvent.tab();
    await untilFocused(() => add, "Tab reaches Add a pool…");
    expectFocusInSight();
  },
};

/**
 * **A pool kept** (#6301): a table of controls, the pool's heading row with its Remove at its
 * end, then the row about it with its profiles, rows of the list family; Add a pool… after the
 * table. Tab goes from Remove into the profiles, one Tab stop, and → reaches a profile's own
 * buttons.
 */
export const WithAPool: Story = {
  parameters: { pools: [BUILDS] },
  play: async ({ canvas, userEvent }) => {
    const group = await canvas.findByRole("region", { name: "Pools" });
    await within(group).findByRole("table", { name: "Pools on this machine" });
    const remove = await within(group).findByRole("button", { name: "Remove pool builds" });
    await within(group).findByText("linked");
    await expect(within(group).queryByText(/No pool yet/)).toBeNull();
    await expect(within(group).getByRole("button", { name: "Add a pool…" })).toBeVisible();
    const profiles = await within(group).findByRole("grid", { name: "Profiles on pool builds" });
    remove.focus();
    await untilFocused(() => remove, "the pool's Remove");
    await userEvent.tab();
    const work = within(profiles).getByRole("row", { name: "work" });
    await untilFocused(() => work, "Tab goes on to the pool's profiles");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(
      () => within(work).getByRole("button", { name: "Sign out: work on pool builds" }),
      "→ reaches the profile's Sign out",
    );
    expectFocusInSight();
  },
};

/**
 * **Profiles signed out** (#6319): each offers its Sign in, named by the words it shows and then
 * whose and where (`Sign in: work on pool builds`), so a person who says what they see is heard
 * (WCAG 2.5.3, Label in Name, which axe holds here). An opencode profile offers one per plan it
 * signs in here. → goes from a profile's row to its Sign in.
 */
export const ProfilesSignedOut: Story = {
  parameters: {
    pools: [BUILDS],
    profiles: [
      { ...SIGNED_IN[0], standing: "signed-out" },
      {
        name: "open",
        kind: "opencode",
        harness: "opencode",
        standing: "signed-out",
        not_kept: null,
        limit: null,
        plans: [
          { id: "openai", title: "ChatGPT" },
          { id: "github-copilot", title: "GitHub Copilot" },
        ],
      },
    ] satisfies PoolProfileRow[],
  },
  play: async ({ canvas, userEvent }) => {
    const group = await canvas.findByRole("region", { name: "Pools" });
    const profiles = await within(group).findByRole("grid", { name: "Profiles on pool builds" });
    const work = within(profiles).getByRole("row", { name: "work" });
    await expect(
      within(profiles).getByRole("button", {
        name: "Sign in to GitHub Copilot: open on pool builds",
      }),
    ).toBeVisible();
    await expect(
      within(profiles).getByRole("button", { name: "Sign in to ChatGPT: open on pool builds" }),
    ).toBeVisible();
    work.focus();
    await untilFocused(() => work, "the profile's row");
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(
      () => within(work).getByRole("button", { name: "Sign in: work on pool builds" }),
      "→ reaches the profile's Sign in",
    );
    expectFocusInSight();
  },
};

/** **An older runner waits on a closed project** (#5461): the row names the project, when it was
 *  last opened, and what lets the runner go, beside the row's Remove. Words only: no new control,
 *  so no keyboard path of its own. */
export const OlderRunnerWaitsOnAProject: Story = {
  parameters: {
    pools: [BUILDS],
    version: {
      ...LEVEL,
      draining: 1,
      owed: [
        {
          path: "/Users/you/projects/alpha",
          name: "alpha",
          opened: Math.floor(Date.now() / 1000) - 12 * 86_400 - 60,
          open_now: false,
        },
      ],
    } satisfies PoolVersion,
  },
  play: async ({ canvas }) => {
    const group = await canvas.findByRole("region", { name: "Pools" });
    const said = await within(group).findByText(
      /An older runner stays until project alpha brings back the work its chats saved on that runner\./,
    );
    await expect(said).toHaveTextContent(
      "alpha is closed, and was last opened 12 days ago. Open alpha to bring its work back.",
    );
    await expect(said).toHaveTextContent(/anything not brought back is deleted with it\./);
    // The way out it names is the row's own Remove, there beside it.
    await expect(within(group).getByRole("button", { name: "Remove pool builds" })).toBeVisible();
    await waitFor(() => expect(within(group).queryByText(/Asking the runner about/)).toBeNull(), {
      timeout: 5000,
    });
  },
};

/** **Two older runners kept unchecked** (the person's ruling of 2026-11-08): each has a line of
 *  its own naming it, what is left of it (a stopped container, or only its saved home), what purlis
 *  does and what lets it go. Words only: no new control, so no keyboard path of its own. */
export const OlderRunnersNotChecked: Story = {
  parameters: {
    pools: [BUILDS],
    version: {
      ...LEVEL,
      draining: 2,
      not_checked: [
        { runner: "builds-0a1b2c3d", left: "stopped" },
        { runner: "builds-4e5f6a7b", left: "home_only" },
      ],
    } satisfies PoolVersion,
  },
  play: async ({ canvas }) => {
    const group = await canvas.findByRole("region", { name: "Pools" });
    await expect(
      await within(group).findByText(/^Older runner builds-0a1b2c3d stays/),
    ).toHaveTextContent(/Start it there and purlis checks it/);
    await expect(within(group).getByText(/^Older runner builds-4e5f6a7b stays/)).toHaveTextContent(
      /only its saved home is left/,
    );
    await waitFor(() => expect(within(group).queryByText(/Asking the runner about/)).toBeNull(), {
      timeout: 5000,
    });
  },
};

/** **A pool whose runner was removed outside purlis** (#4149): its row says so, and its Notice
 *  says its home went too, with Make the runner again. */
export const RunnerRemoved: Story = {
  parameters: { pools: [BUILDS], link: { state: "removed", home_kept: false } },
  play: async ({ canvas }) => {
    const group = await canvas.findByRole("region", { name: "Pools" });
    await within(group).findByText("removed outside purlis");
    await within(group).findByText(/sign-ins kept there/);
    await expect(
      within(group).getByRole("button", { name: "Make the runner again" }),
    ).toBeVisible();
    // Its profiles' line settles before the picture: no longer asking the runner.
    await waitFor(() => expect(within(group).queryByText(/Asking the runner about/)).toBeNull(), {
      timeout: 5000,
    });
  },
};

/** **A pool whose runner stopped on a full disk** (#6205): its row says Docker's disk is full,
 *  with the steps that make room, in the core's words (`stopped_on_a_full_disk_said`). */
export const RunnerStoppedOnAFullDisk: Story = {
  parameters: {
    pools: [BUILDS],
    link: {
      state: "down",
      why:
        "Docker's disk is full, so runner builds stopped. Docker keeps its images, containers " +
        "and volumes on a disk of its own, apart from this computer's. To make room, run docker " +
        "system df to see what takes it, then docker system prune to remove what nothing uses " +
        "(a suspended pool's runner it removes can be made again from its pool, with its " +
        "sign-ins), or give Docker a bigger disk: in Docker Desktop, under Settings, then " +
        "Resources. Then start a chat on the pool, and purlis starts its runner again. The " +
        "runner's last words were: \"purlis: no git hooks at " +
        "/home/purlis/.local/share/dev.purlis.app/git-hooks (No space left on device (os error " +
        '28))"',
    },
  },
  play: async ({ canvas }) => {
    const group = await canvas.findByRole("region", { name: "Pools" });
    await within(group).findByText("not linked");
    await within(group).findByText(/Docker's disk is full/);
    await waitFor(() => expect(within(group).queryByText(/Asking the runner about/)).toBeNull(), {
      timeout: 5000,
    });
  },
};

/** **A pool whose runner was made on another engine** (#4166): it is not said to be removed, and
 *  nothing offers to make it again. */
export const RunnerOnAnotherEngine: Story = {
  parameters: { pools: [BUILDS], link: { state: "on_another_engine" } },
  play: async ({ canvas }) => {
    const group = await canvas.findByRole("region", { name: "Pools" });
    await within(group).findByText("on another Docker engine");
    await within(group).findByText(/Point Docker back at that engine/);
    await expect(within(group).queryByRole("button", { name: "Make the runner again" })).toBeNull();
    await waitFor(() => expect(within(group).queryByText(/Asking the runner about/)).toBeNull(), {
      timeout: 5000,
    });
  },
};

/** **Add a pool, its Image filled**: Kind first, and the Image box showing the runner image
 *  this version's release names, the default when it is left empty. Pictured open; Esc closes
 *  it. */
export const AddAPool: Story = {
  parameters: { screenshot: "page" },
  play: async ({ userEvent }) => {
    startAddingAPool();
    const page = within(document.body);
    const dialog = await page.findByRole("dialog", { name: "Add a pool" });
    const image = within(dialog).getByRole("textbox", { name: "Image" });
    await waitFor(() => expect(image).toHaveAttribute("placeholder", IMAGE));
    await expect(image).toHaveAccessibleDescription(/Empty is the default/);
    await expect(within(dialog).getByRole("radio", { name: /Docker/ })).toBeChecked();
    const name = within(dialog).getByRole("textbox", { name: "Name" });
    await untilFocused(() => name, "Add a pool: the keyboard starts in Name");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => image, "Tab goes on to Image");
    expectFocusInSight();
    await pictureNow();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(page.queryByRole("dialog", { name: "Add a pool" })).toBeNull());
  },
};
