// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Pool setup's Where and Pool steps** (#4480, #4481): every state a person meets them in, drawn, held to
 * axe, walked by the keyboard and pictured in every pictured theme cell. The core is faked at its
 * command boundary. These stories are the design's record (the person's ruling of 2026-11-01).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { PoolSetupShown, PoolSetupStep } from "./bindings";
import { PoolSetupTab } from "./PoolSetupTab";
import { forgetPoolSetupStarts, startANewPoolSetup } from "./pools";
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
    see: null,
    checks: [],
    also: null,
    parts: [],
    undo: null,
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
  noticed: false,
  recommended: "docker",
  others: [],
  kind: "docker",
  contexts: {
    names: [],
    recommended: null,
    chosen: null,
    pools: [],
    joining: false,
    pinned: null,
    named_now: null,
  },
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
  noticed: false,
  recommended: "docker",
  others: [],
  kind: "docker",
  contexts: {
    names: [],
    recommended: null,
    chosen: null,
    pools: [],
    joining: false,
    pinned: null,
    named_now: null,
  },
  steps: [
    DONE_WHERE,
    step("pool", "Pool", "current", {
      findings: ["A pool is a named place your chats run. There isn't one yet."],
      next: CREATE,
    }),
    ...LATER,
  ],
};

/** purlis's image of a release, by the whole of its digest, as a person would copy it. */
const DIGEST =
  "ghcr.io/purlis/purlis-runner@sha256:6a06111c3b0a8600feadb270aec771dc239bf407b25625f93f44ab00e11f4d3c";

/** What the last Copy put on the clipboard. */
let pasted: string | undefined;

function faked(shown: PoolSetupShown, image: unknown = { state: "ours", image: "img@sha256:a" }) {
  return () => {
    mockIPC((cmd) => {
      if (cmd === "pool_setup") return shown;
      if (cmd === "runner_image_of_this_version") return image;
      if (cmd === "docker_pools")
        return {
          pools: [],
          defaults: { cpus: "2", memory_mib: 4096, processes: 512, tmp_mib: 256 },
          suspend_after: "24h",
          frames: 10,
          width: 1600,
        };
      if (cmd === "pool_setup_create") throw "a pool named dev-box is on this machine";
      if (cmd === "pool_setup_command") return "purlis pool add dev-box --image img@sha256:a";
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

/** A second pool: the name box empty, and a name no pool has recommended (#6256). */
export const NameASecondPool: Story = {
  beforeEach: faked({ ...POOL_FORM, others: ["docker"], recommended: "docker-2" }),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const name = await canvas.findByRole("textbox", { name: /Name/ });
    await expect(name).toHaveValue("");
    await expect(name).toHaveAttribute("placeholder", "docker-2");
    await canvas.findByText(/Recommended: docker-2\./);
  },
};

/** Add a pool… went on with a new pool's unfinished setup: it says so, with Start over (#6256). */
export const ContinuingThePoolYouStarted: Story = {
  beforeEach: async () => {
    mockIPC((cmd) => {
      if (cmd === "pool_setup") return { ...POOL_FORM, name: "builds", others: ["docker"] };
      if (cmd === "pool_setup_add") return true;
      if (cmd === "runner_image_of_this_version") return { state: "ours", image: "img@sha256:a" };
      if (cmd === "pool_setup_command") return "purlis pool add builds --image img@sha256:a";
      return null;
    });
    await startANewPoolSetup();
    return forgetPoolSetupStarts;
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText("Continuing the pool you started");
    const over = await canvas.findByRole("button", { name: "Start over" });
    over.focus();
    await expect(over).toHaveFocus();
    await expectFocusInSight();
    await userEvent.tab();
    await expect(canvas.getByRole("textbox", { name: /Name/ })).toHaveFocus();
    await expectFocusInSight();
  },
};

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

const fold = async (canvasElement: HTMLElement, name: string) => {
  const canvas = within(canvasElement);
  await userEvent.click(await canvas.findByRole("button", { name }));
};

/** Options open: the image, and every limit with what purlis recommends. */
export const OptionsOpen: Story = {
  beforeEach: faked(POOL_FORM),
  play: async ({ canvasElement }) => {
    await fold(canvasElement, "Options");
    const canvas = within(canvasElement);
    const ours = await canvas.findByRole("radio", { name: /purlis's image/ });
    ours.focus();
    await expect(ours).toHaveFocus();
    await expectFocusInSight();
    await userEvent.keyboard("{ArrowDown}");
    await expect(canvas.getByRole("radio", { name: /My team's own image/ })).toBeChecked();
    await expect(canvas.getByRole("textbox", { name: /Image, by digest/ })).toBeTruthy();
  },
};

/** **The image's digest, to copy** (#6206): Options open, the keyboard reaches Copy digest
 *  beside purlis's image's digest, and Return puts the whole digest on the clipboard. */
export const TheImagesDigestToCopy: Story = {
  beforeEach: () => {
    faked(POOL_FORM, { state: "ours", image: DIGEST })();
    // The page's own clipboard asks the browser for leave a story cannot give, so it is held.
    const clipboard = Object.getOwnPropertyDescriptor(navigator, "clipboard");
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: (text: string) => {
          pasted = text;
          return Promise.resolve();
        },
      },
    });
    return () => {
      if (clipboard) Object.defineProperty(navigator, "clipboard", clipboard);
      else Reflect.deleteProperty(navigator, "clipboard");
    };
  },
  play: async ({ canvasElement }) => {
    pasted = undefined;
    await fold(canvasElement, "Options");
    const canvas = within(canvasElement);
    const ours = await canvas.findByRole("radio", { name: /purlis's image/ });
    ours.focus();
    await userEvent.tab();
    const copy = canvas.getByRole("button", { name: /^Copy digest/ });
    await expect(copy).toHaveFocus();
    await expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(pasted).toBe(DIGEST));
    await canvas.findAllByText("Copied");
  },
};

/** See what this does open: the command the button runs, to copy. */
export const SeeWhatThisDoesOpen: Story = {
  beforeEach: faked({ ...POOL_FORM, name: "dev-box" }),
  play: async ({ canvasElement }) => {
    await fold(canvasElement, "See what this does");
    const canvas = within(canvasElement);
    await canvas.findByText("purlis pool add dev-box --image img@sha256:a");
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: /^Copy/ })).toHaveFocus();
    await expectFocusInSight();
  },
};

/** A team's image that breaks the contract: what's wrong, and nothing made. */
export const ATeamsImageBreaksTheContract: Story = {
  beforeEach: () => {
    mockIPC((cmd) => {
      if (cmd === "pool_setup") return { ...POOL_FORM, name: "dev-box" };
      if (cmd === "runner_image_of_this_version") return { state: "ours", image: "img@sha256:a" };
      if (cmd === "pool_setup_command") return "purlis pool add dev-box --image r@sha256:b";
      if (cmd === "docker_pools")
        return {
          pools: [],
          defaults: { cpus: "2", memory_mib: 4096, processes: 512, tmp_mib: 256 },
          suspend_after: "24h",
          frames: 10,
          width: 1600,
        };
      if (cmd === "pool_setup_create")
        return {
          breaks: [
            "Its user has to be a number other than 0, such as USER 10001, so it never runs as root.",
            "there is no purlis plugin at /usr/local/lib/purlis/plugin, beside /usr/local/bin/purlis",
          ],
        };
      return null;
    });
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await fold(canvasElement, "Options");
    await userEvent.click(await canvas.findByRole("radio", { name: /My team's own image/ }));
    await userEvent.type(canvas.getByRole("textbox", { name: /Image, by digest/ }), "r@sha256:b");
    await userEvent.click(canvas.getByRole("button", { name: "Create the pool" }));
    await canvas.findByText("This image is missing 2 things a runner needs");
    await expect(canvas.getByRole("button", { name: "Use purlis's image instead" })).toBeVisible();
  },
};

/** The name is taken, with a press to carry on with that pool. */
export const ThatNameIsTaken: Story = {
  beforeEach: faked({ ...POOL_FORM, name: "dev-box", others: ["dev-box", "builds"] }),
};

/** Made by hand from See what this does: purlis noticed, and setup follows it. */
export const MadeByHand: Story = {
  beforeEach: faked({
    pool: "dev-box",
    name: "dev-box",
    at: "signins",
    noticed: true,
    recommended: "docker",
    others: [],
    kind: "docker",
    contexts: {
      names: [],
      recommended: null,
      chosen: null,
      pools: [],
      joining: false,
      pinned: null,
      named_now: null,
    },
    steps: [
      DONE_WHERE,
      step("pool", "Pool", "done", { line: "dev-box" }),
      step("signins", "Sign-ins", "current"),
      step("identity", "Agent identity", "later"),
      step("try", "Try it", "later"),
    ],
  }),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: /^Pool/ }));
    await canvas.findByText("Found pool dev-box");
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
    noticed: false,
    recommended: "docker",
    others: [],
    kind: "docker",
    contexts: {
      names: [],
      recommended: null,
      chosen: null,
      pools: [],
      joining: false,
      pinned: null,
      named_now: null,
    },
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
    noticed: false,
    recommended: "docker",
    others: [],
    kind: "docker",
    contexts: {
      names: [],
      recommended: null,
      chosen: null,
      pools: [],
      joining: false,
      pinned: null,
      named_now: null,
    },
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
    noticed: false,
    recommended: "docker",
    others: [],
    kind: "docker",
    contexts: {
      names: [],
      recommended: null,
      chosen: null,
      pools: [],
      joining: false,
      pinned: null,
      named_now: null,
    },
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
