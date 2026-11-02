// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Pool setup's Where and Pool steps** (#4480): every state a person meets them in, drawn, held to
 * axe, walked by the keyboard and pictured in every pictured theme cell. The core is faked at its
 * command boundary. These stories are the design's record (the person's ruling of 2026-11-01).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { PoolSetupShown, PoolSetupStep } from "./bindings";
import { PoolSetupTab } from "./PoolSetupTab";
import { expectFocusInSight } from "./stories.testkit";

const step = (id: string, name: string, state: string, more: Partial<PoolSetupStep> = {}) =>
  ({
    id,
    name,
    state,
    line: null,
    findings: [],
    next: null,
    skippable: id === "signins" || id === "identity",
    ...more,
  }) as PoolSetupStep;

const LATER = [
  step("signins", "Sign-ins", "later"),
  step("identity", "Agent identity", "later"),
  step("try", "Try it", "later"),
];

const WHERE = (where: Partial<PoolSetupStep>, state: string): PoolSetupShown => ({
  pool: null,
  name: null,
  at: "where",
  steps: [step("where", "Where chats run", state, where), step("pool", "Pool", "later"), ...LATER],
});

const DONE_WHERE = step("where", "Where chats run", "done", { line: "Docker on this machine" });

const CREATE = {
  id: "create-pool",
  label: "Create the pool",
  does: "Makes one container, its runner, in Docker on this machine.",
  command: null,
  url: null,
};

const POOL_FORM: PoolSetupShown = {
  pool: null,
  name: null,
  at: "pool",
  steps: [
    DONE_WHERE,
    step("pool", "Pool", "current", {
      findings: ["A pool is a named place your chats run. There isn't one yet."],
      next: CREATE,
    }),
    ...LATER,
  ],
};

function faked(shown: PoolSetupShown, image: unknown = { state: "ours", image: "img@sha256:a" }) {
  return () => {
    mockIPC((cmd) => {
      if (cmd === "pool_setup") return shown;
      if (cmd === "runner_image_of_this_version") return image;
      if (cmd === "add_docker_pool") throw "a pool named dev-box is on this machine";
      return null;
    });
  };
}

const meta = {
  component: PoolSetupTab,
  args: { plane: "p" },
  decorators: [
    (Story) => (
      <div style={{ height: 720, display: "flex" }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof PoolSetupTab>;

export default meta;
type Story = StoryObj<typeof meta>;

export const DockerIsReady: Story = {
  beforeEach: faked(
    WHERE(
      {
        findings: ["Docker is ready, so purlis recommends running chats there."],
        next: {
          id: "use-docker",
          label: "Continue with Docker",
          does: "Saves your choice. Nothing on this machine changes yet.",
          command: null,
          url: null,
        },
      },
      "current",
    ),
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const main = await canvas.findByRole("button", { name: "Continue with Docker" });
    main.focus();
    await expect(main).toHaveFocus();
    await expectFocusInSight();
    await userEvent.tab({ shift: true });
    await expectFocusInSight();
  },
};

export const DockerIsMissing: Story = {
  beforeEach: faked(
    WHERE(
      {
        findings: [
          "You need Docker first. It runs your chats in a container on this machine, apart from your files.",
        ],
        next: {
          id: "get-docker",
          label: "Open Docker's install page",
          does: "Opens Docker's own page in your browser. purlis doesn't install anything, and checks again by itself.",
          command: null,
          url: "https://docs.docker.com/",
        },
      },
      "attention",
    ),
  ),
};

export const DockerIsStopped: Story = {
  beforeEach: faked(
    WHERE(
      {
        findings: ["Docker needs to be running. purlis found it, and it isn't answering."],
        next: {
          id: "start-docker",
          label: "Open the Docker app",
          does: "Opens Docker where purlis can. Otherwise, start it the way you usually do. purlis checks again by itself.",
          command: null,
          url: null,
        },
      },
      "attention",
    ),
  ),
};

export const NameThePool: Story = { beforeEach: faked(POOL_FORM) };

export const ThePoolWasRefused: Story = {
  beforeEach: faked(POOL_FORM),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const name = await canvas.findByRole("textbox", { name: /Name/ });
    await userEvent.type(name, "dev-box");
    await userEvent.click(await canvas.findByRole("button", { name: "Create the pool" }));
    await canvas.findByText("a pool named dev-box is on this machine");
  },
};

export const NoRunnerImageNamed: Story = {
  beforeEach: faked(POOL_FORM, { state: "none_named" }),
};

export const ThePoolIsMade: Story = {
  beforeEach: faked({
    pool: "dev-box",
    name: "dev-box",
    at: "signins",
    steps: [
      DONE_WHERE,
      step("pool", "Pool", "done", { line: "dev-box" }),
      step("signins", "Sign-ins", "current", {
        findings: ["A profile holds your Claude, Codex or other account. This pool has none yet."],
        next: {
          id: "choose-profiles",
          label: "Choose profiles",
          does: "Picks the accounts a chat on this pool can use. Settings › Pools has the list.",
          command: null,
          url: null,
        },
      }),
      step("identity", "Agent identity", "later"),
      step("try", "Try it", "later"),
    ],
  }),
};

export const TheRunnerWasRemoved: Story = {
  beforeEach: faked({
    pool: "dev-box",
    name: "dev-box",
    at: "pool",
    steps: [
      DONE_WHERE,
      step("pool", "Pool", "attention", {
        line: "dev-box",
        findings: ["The runner of pool dev-box was removed outside purlis."],
        next: {
          id: "make-runner-again",
          label: "Make the runner again",
          does: "Makes a new container for this pool in Docker, with the pool's image and limits.",
          command: null,
          url: null,
        },
      }),
      ...LATER,
    ],
  }),
};

export const DockerIsElsewhere: Story = {
  beforeEach: faked(
    WHERE(
      {
        findings: [
          "Docker here points at an engine purlis can't tell is on this machine, so purlis makes no runner there and asks it nothing.",
        ],
        next: {
          id: "use-local-docker",
          label: "Use this machine's Docker",
          does: "Switch your Docker context or DOCKER_HOST to this machine. purlis checks again by itself.",
          command: null,
          url: null,
        },
      },
      "attention",
    ),
  ),
};

export const GoingBackToWhere: Story = {
  beforeEach: faked(POOL_FORM),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const back = await canvas.findByRole("button", { name: /^Where chats run/ });
    back.focus();
    await userEvent.keyboard("{Enter}");
    await canvas.findByRole("heading", { name: "Where should your chats run?" });
    await expect(back).toHaveFocus();
    await expectFocusInSight();
  },
};

export const ReadingFailed: Story = {
  beforeEach: () => {
    mockIPC((cmd) => {
      if (cmd === "pool_setup") throw "your Pool setup choices could not be read safely";
      return null;
    });
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const again = await canvas.findByRole("button", { name: "Check again" });
    again.focus();
    await expectFocusInSight();
  },
};

export const ARunnerBeingMade: Story = {
  beforeEach: faked({
    pool: "dev-box",
    name: "dev-box",
    at: "pool",
    steps: [
      DONE_WHERE,
      step("pool", "Pool", "attention", {
        findings: [
          "Pool dev-box has no runner yet. If it is being made, this clears by itself. If not, remove the pool in Settings › Pools and add it again.",
        ],
        next: {
          id: "check-again",
          label: "Check again",
          does: "Asks Docker again. It changes nothing.",
          command: null,
          url: null,
        },
      }),
      ...LATER,
    ],
  }),
};
