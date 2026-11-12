// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Pool setup's cluster path, Where and Check the cluster** (#4492): every state a person meets
 * them in, drawn, held to axe, walked by the keyboard and pictured in every pictured theme cell.
 * The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { PoolSetupAction, PoolSetupShown, PoolSetupStep } from "../bindings";
import { PoolSetupTab } from "../PoolSetupTab";
import { expectFocusInSight } from "../stories.testkit";

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
    ...more,
  }) as PoolSetupStep;

const act = (
  id: string,
  label: string,
  does: string,
  url: string | null = null,
): PoolSetupAction => ({
  id,
  label,
  does,
  command: null,
  url,
});

const AFTER_CHECK = (from: number) =>
  [
    step("check", "Check the cluster", "later"),
    step("install", "Install", "later"),
    step("pool", "Pool", "later"),
    step("signins", "Sign-ins", "later"),
    step("identity", "Agent identity", "later"),
    step("try", "Try it", "later"),
  ].slice(from);

const USE_DOCKER = act(
  "use-docker",
  "Use Docker on this machine instead",
  "Saves your choice. Nothing on this machine changes yet.",
);

const CONTEXTS = { names: ["kind-team", "prod-eu"], recommended: "kind-team", chosen: null };

const shown = (
  at: string,
  steps: PoolSetupStep[],
  contexts: PoolSetupShown["contexts"] = CONTEXTS,
): PoolSetupShown => ({
  pool: null,
  name: null,
  at,
  noticed: false,
  others: [],
  kind: "cluster",
  contexts,
  steps,
});

const DOCTOR_SEE = {
  note: "Doctor asks these, with your own sign-in. Each only reads.",
  text: "kubectl --context kind-team version\nkubectl --context kind-team auth can-i create namespaces",
};

const DONE_WHERE = step("where", "Where chats run", "done", {
  line: "Your team's cluster, through context kind-team",
});

function faked(answer: PoolSetupShown) {
  return () => {
    mockIPC((cmd) => (cmd === "pool_setup" ? answer : null));
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

export const AClusterIsRecommended: Story = {
  beforeEach: faked(
    shown("where", [
      step("where", "Where chats run", "current", {
        findings: [
          "Your kubeconfig reaches a cluster through context kind-team, so purlis recommends running chats there. purlis uses your own sign-in and keeps none of it.",
        ],
        next: act(
          "use-cluster",
          "Continue with this cluster",
          "Saves your choice of context. Nothing on the cluster changes yet.",
        ),
        also: USE_DOCKER,
        see: {
          note: "This is how purlis lists your kubeconfig's contexts.",
          text: "kubectl config get-contexts",
        },
      }),
      ...AFTER_CHECK(0),
    ]),
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const main = await canvas.findByRole("button", { name: "Continue with this cluster" });
    main.focus();
    await userEvent.tab();
    await expect(
      canvas.getByRole("button", { name: "Use Docker on this machine instead" }),
    ).toHaveFocus();
    await expectFocusInSight();
  },
};

export const ContextsUnderOptions: Story = {
  beforeEach: AClusterIsRecommended.beforeEach,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Options" }));
    const chosen = canvas.getByRole("radio", { name: /kind-team/ });
    await expect(chosen).toBeChecked();
    chosen.focus();
    await userEvent.keyboard("{ArrowDown}");
    await expect(canvas.getByRole("radio", { name: "prod-eu" })).toBeChecked();
    await expectFocusInSight();
  },
};

export const NoClusterInTheKubeconfig: Story = {
  beforeEach: faked(
    shown(
      "where",
      [
        step("where", "Where chats run", "attention", {
          findings: [
            "Your kubeconfig names no cluster, so there is none to set up here. Ask your team for access to its cluster, or use Docker on this machine.",
          ],
          next: act(
            "use-docker",
            "Use Docker instead",
            "Saves your choice. Nothing on this machine changes yet.",
          ),
          also: USE_DOCKER,
        }),
        ...AFTER_CHECK(0),
      ],
      { names: [], recommended: null, chosen: null },
    ),
  ),
};

export const NoKubectl: Story = {
  beforeEach: faked(
    shown(
      "where",
      [
        step("where", "Where chats run", "attention", {
          findings: [
            "A cluster is reached through kubectl, with your own sign-in, and purlis found no kubectl on this machine. You can use Docker on this machine instead.",
          ],
          next: act(
            "get-kubectl",
            "Open kubectl's install page",
            "Opens Kubernetes' own page in your browser. purlis doesn't install anything, and checks again by itself.",
            "https://kubernetes.io/docs/tasks/tools/",
          ),
          also: USE_DOCKER,
        }),
        ...AFTER_CHECK(0),
      ],
      { names: [], recommended: null, chosen: null },
    ),
  ),
};

const ok = (title: string) => ({ about: "cluster", verdict: "ok", title, more: null });

export const ReadyForTheInstall: Story = {
  beforeEach: faked(
    shown("install", [
      DONE_WHERE,
      step("check", "Check the cluster", "done", {
        line: "Ready for the install",
        findings: ["The cluster has what purlis needs. What it lacks, the Install step adds."],
        checks: [
          ok("asked context kind-team, API server https://127.0.0.1:6443/"),
          ok("Kubernetes v1.36.4"),
          ok("you may apply the install's cluster-scoped definitions"),
          {
            about: "cluster",
            verdict: "todo",
            title: "upstream agent-sandbox v1.0.6: not installed",
            more: "The Install step adds it.",
          },
        ],
        see: DOCTOR_SEE,
      }),
      step("install", "Install", "current", {
        findings: [
          "Install puts purlis's cluster operator on the cluster, a part at a time, each on its own button.",
        ],
      }),
      ...AFTER_CHECK(3),
    ]),
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: /Check the cluster/ }));
    const list = await canvas.findByRole("list", { name: "What doctor found" });
    await expect(within(list).getAllByRole("listitem")).toHaveLength(4);
    await canvas.findByRole("button", { name: "Continue" });
  },
};

export const TheClusterLacksSomething: Story = {
  beforeEach: faked(
    shown("check", [
      DONE_WHERE,
      step("check", "Check the cluster", "attention", {
        findings: [
          "The cluster lacks something the install doesn't add, so nothing changes until it has it. Each line says what to do, and this step checks again.",
        ],
        checks: [
          ok("asked context kind-team, API server https://127.0.0.1:6443/"),
          {
            about: "cluster",
            verdict: "missing",
            title: "Kubernetes 1.36 or later: this cluster runs v1.33.2",
            more: "Next: upgrade the cluster to Kubernetes 1.36 or later; its pod user namespaces are what runners need.",
          },
          {
            about: "cluster",
            verdict: "missing",
            title:
              "permission to apply the install's cluster-scoped definitions: you may not create customresourcedefinitions.apiextensions.k8s.io",
            more: "Next: run doctor as a cluster admin, or ask one for these rights.",
          },
        ],
        next: act("check-again", "Check again", "Asks the cluster again. It changes nothing."),
        see: DOCTOR_SEE,
      }),
      ...AFTER_CHECK(1),
    ]),
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const again = await canvas.findByRole("button", { name: "Check again" });
    again.focus();
    await expect(again).toHaveFocus();
    await expectFocusInSight();
  },
};

export const AnOperatorIsInstalled: Story = {
  beforeEach: faked(
    shown("pool", [
      DONE_WHERE,
      step("check", "Check the cluster", "done", {
        line: "purlis's cluster operator runs here",
        findings: [
          "purlis's cluster operator is installed and running here, from ghcr.io/purlis/purlis-cluster-operator@sha256:6a06111c3b0a, so doctor, which checks a cluster before the install, isn't asked.",
        ],
        checks: [
          {
            about: "cluster",
            verdict: "ok",
            title: "purlis's cluster operator runs",
            more: "ghcr.io/purlis/purlis-cluster-operator@sha256:6a06111c3b0a",
          },
        ],
      }),
      step("install", "Install", "done", { line: "Installed" }),
      step("pool", "Pool", "current", {
        findings: [
          "A pool is a named place your chats run. On a cluster, each chat's runner is a pod the cluster operator makes from it.",
        ],
      }),
      ...AFTER_CHECK(4),
    ]),
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: /Check the cluster/ }));
    await canvas.findByText("purlis's cluster operator runs");
  },
};
