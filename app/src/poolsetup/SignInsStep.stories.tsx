// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Pool setup's Sign-ins step** (#4482): every state a person meets it in, drawn, held to axe,
 * walked by the keyboard and pictured in every pictured theme cell. The core is faked at its
 * command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { PoolProfiles, PoolSetupSizes, PoolSetupStep } from "../bindings";
import { expectFocusInSight } from "../stories.testkit";
import { SignInsStep } from "./SignInsStep";

const STEP: PoolSetupStep = {
  id: "signins",
  name: "Sign-ins",
  state: "current",
  line: null,
  findings: [],
  next: { id: "choose-profiles", label: "Choose profiles", does: "", command: null, url: null },
  skippable: true,
  also: null,
  parts: [],
  admin: null,
  undo: null,
  see: {
    note: "This is the sign-in the button runs.",
    text: "purlis profile sign-in personal --pool dev-box",
  },
  checks: [],
};

const row = (name: string, harness: string, standing: string) => ({
  name,
  kind: harness === "Codex" ? "codex" : "claude",
  harness,
  standing,
  not_kept: null,
  limit: null,
  plans: [],
});

const PROFILES: PoolProfiles = {
  kept: [row("work", "Claude Code", "signed-in")],
  addable: [
    { name: "personal", harness: "Claude Code" },
    { name: "oss", harness: "Codex" },
  ],
  fresh_claim: false,
};

const size = (name: string, mib: number, in_use: boolean) => ({
  name,
  mib,
  files: 900,
  unread: null,
  over: [],
  in_use,
});

const SIZES: PoolSetupSizes = {
  most_mib: 128,
  most_files: 8192,
  profiles: [size("work", 42, true), size("personal", 18, true), size("oss", 9, false)],
};

const OVER: PoolSetupSizes = {
  ...SIZES,
  profiles: [
    {
      ...size("work", 212, true),
      over: [
        {
          key: "profileSize",
          said: "it needs 212 MiB, over this pool's limit of 128 MiB",
          raise_to: 256,
          older_runner: null,
        },
      ],
    },
    size("personal", 18, true),
    size("oss", 9, false),
  ],
};

/** What the core spells for the ticked profiles (#4718), as `poolsetup::signins::by_hand` does. */
function byHand(args: unknown) {
  const { pool, adding, takingOff, signingIn } = args as {
    pool: string;
    adding: string[];
    takingOff: string[];
    signingIn: string[];
  };
  const lines = [
    ...(adding.length > 0 ? [`purlis pool add-profile ${pool} ${adding.join(" ")}`] : []),
    ...takingOff.map((name) => `purlis pool remove-profile ${pool} ${name}`),
    ...signingIn.map((name) => `purlis profile sign-in ${name} --pool ${pool}`),
  ];
  if (lines.length === 0) return null;
  return {
    note: "These record the ticked profiles on the pool, then sign in each one that needs it, as the button does. Run them yourself in your project's folder if you prefer, and purlis picks it up.",
    text: lines.join("\n"),
  };
}

function faked(profiles: PoolProfiles, sizes: PoolSetupSizes) {
  return () => {
    mockIPC((cmd, args) => {
      if (cmd === "pool_setup_sign_ins_by_hand") return byHand(args);
      if (cmd === "pool_profiles") return profiles;
      if (cmd === "pool_setup_sizes") return sizes;
      if (cmd === "pool_limits") return { groups: [], advanced: [], waiting: [] };
      if (cmd === "pool_sign_in_start") return 1;
      if (cmd === "pool_sign_in_next") return new Promise(() => undefined);
      return null;
    });
  };
}

const meta = {
  component: SignInsStep,
  args: { step: STEP, pool: "dev-box", plane: "p", changed: () => undefined },
  decorators: [
    (Story) => (
      <div style={{ maxWidth: 720 }} className="pool-setup-page">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof SignInsStep>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The ones in use ticked, each one's size against the limit. */
export const InUseTicked: Story = {
  beforeEach: faked(PROFILES, SIZES),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const work = await canvas.findByRole("checkbox", { name: /^work/ });
    work.focus();
    await expect(work).toHaveFocus();
    await expectFocusInSight();
    await userEvent.tab();
    await expect(canvas.getByRole("checkbox", { name: /^personal/ })).toHaveFocus();
    await expectFocusInSight();
  },
};

/** A profile over what the pool lets a profile send, with the limit raised in place. */
export const OverItsLimit: Story = {
  beforeEach: faked(PROFILES, OVER),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const raise = await canvas.findByRole("button", { name: "Raise the limit to 256 MiB" });
    raise.focus();
    await expect(raise).toHaveFocus();
    await expectFocusInSight();
  },
};

/** A profile only a runner on the current image would take (#5175): no limit to raise. */
export const OverWhatAnOlderRunnerTakes: Story = {
  beforeEach: faked(PROFILES, {
    ...OVER,
    profiles: [
      {
        ...size("work", 212, true),
        over: [
          {
            key: "profileSize",
            said: "it needs 212 MiB, and this pool's runner runs an older purlis, which takes at most 120 MiB (Most a profile may send: size): a runner on the current image takes more, or send less",
            raise_to: null,
            older_runner: "move",
          },
        ],
      },
      size("personal", 18, true),
      size("oss", 9, false),
    ],
  }),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const replace = await canvas.findByRole("button", { name: "Replace the runner…" });
    replace.focus();
    await expect(replace).toHaveFocus();
    await expectFocusInSight();
  },
};

/** Signing a profile in inside the step, through the harness's own flow. */
export const SigningIn: Story = {
  beforeEach: () => {
    let added = false;
    mockIPC((cmd, args) => {
      if (cmd === "pool_setup_sign_ins_by_hand") return byHand(args);
      if (cmd === "pool_profiles")
        return added
          ? {
              ...PROFILES,
              kept: [...PROFILES.kept, row("personal", "Claude Code", "signed-out")],
              addable: [PROFILES.addable[1]],
            }
          : PROFILES;
      if (cmd === "pool_profile_add") {
        added = true;
        return null;
      }
      if (cmd === "pool_setup_sizes") return SIZES;
      if (cmd === "pool_limits") return { groups: [], advanced: [], waiting: [] };
      if (cmd === "pool_sign_in_start") return 1;
      if (cmd === "pool_sign_in_next") return new Promise(() => undefined);
      return null;
    });
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Sign in 1 profile" }));
    await canvas.findByRole("heading", { name: "Sign in personal" });
  },
};

/** Skipped, with a way back. */
export const Skipped: Story = {
  beforeEach: faked(PROFILES, SIZES),
  args: { step: { ...STEP, state: "skipped", next: null, see: null } },
};

/** A pool whose runner keeps nothing past its claim: where a sign-in is kept, said. */
export const OnAClusterPool: Story = {
  beforeEach: faked({ ...PROFILES, fresh_claim: true }, SIZES),
};

/** The runner is asleep: what purlis found, and Skip for now. */
export const TheRunnerIsAsleep: Story = {
  beforeEach: faked(PROFILES, SIZES),
  args: {
    step: {
      ...STEP,
      findings: ["The runner of pool dev-box isn't running, so purlis can't read its sign-ins."],
      next: {
        id: "wake-runner",
        label: "Wake the runner",
        does: "Starts the runner's container, and it goes back to sleep by itself.",
        command: null,
        url: null,
      },
      see: null,
    },
  },
};
