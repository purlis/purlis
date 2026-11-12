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
    parts: [],
    undo: null,
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

const CONTEXTS = {
  names: ["kind-team", "prod-eu"],
  recommended: "kind-team",
  chosen: null,
  pools: [],
  joining: false,
  pinned: null,
  key_changed: false,
};

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
  recommended: "team",
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
      {
        names: [],
        recommended: null,
        chosen: null,
        pools: [],
        joining: false,
        pinned: null,
        key_changed: false,
      },
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
      {
        names: [],
        recommended: null,
        chosen: null,
        pools: [],
        joining: false,
        pinned: null,
        key_changed: false,
      },
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
      ...AFTER_CHECK(2),
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
      ...AFTER_CHECK(3),
    ]),
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: /Check the cluster/ }));
    await canvas.findByText("purlis's cluster operator runs");
  },
};

const part = (
  id: string,
  name: string,
  done: boolean,
  line: string,
  next: PoolSetupAction | null = null,
) => ({ id, name, done, line, next });

const DONE_CHECK = step("check", "Check the cluster", "done", { line: "Ready for the install" });

const INSTALL_SEE = {
  note: "These are the install's commands, a part after another, as each button runs them.",
  text: "# agent-sandbox\nkubectl --context kind-team apply --server-side -k operator/vendor/agent-sandbox/v1.0.6",
};

const UNDO = {
  note: "purlis never uninstalls from a cluster: other people's chats run there. 3 claims by 2 people would end, with every chat on their runners. Ask them first. Run these yourself, in order.",
  text: "kubectl --context kind-team delete claims.runners.purlis.dev --all-namespaces --all\nkubectl --context kind-team delete --ignore-not-found -f operator/deploy/operator.yaml",
};

const installing = (parts: ReturnType<typeof part>[], undo: typeof UNDO | null = null) =>
  shown("install", [
    DONE_WHERE,
    DONE_CHECK,
    step("install", "Install", "current", {
      findings: [
        "Install puts purlis's cluster operator on the cluster with your own sign-in, a part at a time. Each button says what it applies, and nothing changes until you press it.",
      ],
      parts,
      see: INSTALL_SEE,
      undo,
    }),
    ...AFTER_CHECK(2),
  ]);

export const InstallTheFirstPart: Story = {
  beforeEach: faked(
    installing([
      part(
        "upstream",
        "agent-sandbox",
        false,
        "The project purlis's runners are built on, at the release it is tested on, v1.0.6.",
        act(
          "install-upstream",
          "Install agent-sandbox",
          "Applies agent-sandbox v1.0.6 to the cluster, its controller named by digest, and waits for it to start.",
        ),
      ),
      part(
        "definitions",
        "purlis's definitions",
        false,
        "The kinds a team's pools and people's claims are.",
      ),
      part(
        "signing-key",
        "Signing key",
        false,
        "The key the operator vouches for each runner with.",
      ),
      part(
        "operator",
        "Cluster operator",
        false,
        "The controller that makes each person's runners.",
      ),
    ]),
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const main = await canvas.findByRole("button", { name: "Install agent-sandbox" });
    main.focus();
    await expect(main).toHaveFocus();
    await expectFocusInSight();
  },
};

export const TheOperatorWaitsForItsImage: Story = {
  beforeEach: faked(
    installing(
      [
        part("upstream", "agent-sandbox", true, "agent-sandbox v1.0.6, which purlis is tested on"),
        part(
          "definitions",
          "purlis's definitions",
          true,
          "pools.runners.purlis.dev, claims.runners.purlis.dev and their policies",
        ),
        part(
          "signing-key",
          "Signing key",
          true,
          "Kept in purlis-system/purlis-cluster-operator-signing-key. purlis never replaces it.",
        ),
        part(
          "operator",
          "Cluster operator",
          false,
          "The controller that makes each person's runners. Its image is public, so the cluster needs no pull secret.",
          act(
            "choose-operator-image",
            "Name the operator's image",
            "Give the operator's image by its digest under Options, ghcr.io/purlis/purlis-cluster-operator@sha256:… See what this does says how to find and check it.",
          ),
        ),
      ],
      UNDO,
    ),
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const field = await canvas.findByRole("textbox", { name: "Operator image" });
    field.focus();
    await expect(field).toHaveFocus();
    await expectFocusInSight();
  },
};

export const UninstallingIsCommandsOnly: Story = {
  beforeEach: TheOperatorWaitsForItsImage.beforeEach,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Uninstalling" }));
    await canvas.findByText(/3 claims by 2 people would end/);
  },
};

const INSTALLED = [
  DONE_WHERE,
  step("check", "Check the cluster", "done", { line: "purlis's cluster operator runs here" }),
  step("install", "Install", "done", { line: "Installed" }),
];

const AFTER_POOL = [
  step("signins", "Sign-ins", "later"),
  step("identity", "Agent identity", "later"),
  step("try", "Try it", "later"),
];

const onPool = (findings: string, next: PoolSetupAction, pools: string[]) =>
  shown(
    "pool",
    [...INSTALLED, step("pool", "Pool", "current", { findings: [findings], next }), ...AFTER_POOL],
    {
      names: ["kind-team"],
      recommended: "kind-team",
      chosen: "kind-team",
      pools,
      joining: false,
      pinned: null,
      key_changed: false,
    },
  );

const KEY = "ed25519:9f2c4e6a8b0d1f3e5a7c9b1d3f5e7a9c1b3d5f7e9a1c3b5d7f9e1a3c5b7d9f1e";

export const CreateTheClustersPool: Story = {
  beforeEach: faked(
    onPool(
      "A pool is a named place your chats run. Your cluster has none yet, so this makes one, from purlis's runner image, and then claims your runner of it.",
      act(
        "create-cluster-pool",
        "Create the pool",
        "Applies a Pool to the cluster with your own sign-in. Anyone your cluster lets use it can claim a runner of it.",
      ),
      [],
    ),
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const main = await canvas.findByRole("button", { name: "Create the pool" });
    main.focus();
    await expect(main).toHaveFocus();
    await expectFocusInSight();
  },
};

export const TheKeyToCheckBeforeAdding: Story = {
  beforeEach: () => {
    mockIPC((cmd) => {
      if (cmd === "pool_setup")
        return onPool(
          "Your team has these pools on this cluster: team, gpu. Adding one here claims a runner of it for you and pairs it with this machine.",
          act(
            "add-cluster-pool",
            "Add the pool here",
            "Checks the cluster operator key the pool names against the one your admin gave you, pins it, then claims your runner of it and pairs it with this machine. Nothing else on the cluster changes.",
          ),
          ["team", "gpu"],
        );
      // The admin made the signing key in this setup, so its key is shown at once (#4495).
      if (cmd === "cluster_pool_key")
        return { key: KEY, made_here: true, matches: null, namespace: "purlis-runners-3f9a1c2b" };
      return null;
    });
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Add the pool here" }));
    const pin = await canvas.findByRole("button", { name: "Pin the key and add the pool" });
    await canvas.findByText(KEY);
    pin.focus();
    await expect(pin).toHaveFocus();
    await expectFocusInSight();
  },
};

export const MarkThePoolAsYourOwn: Story = {
  beforeEach: faked(
    shown(
      "signins",
      [
        ...INSTALLED,
        step("pool", "Pool", "done", { line: "team" }),
        step("signins", "Sign-ins", "current", {
          findings: [
            "Pool team isn't marked as your own, so purlis keeps no profile of yours on its runner. Your cluster's admins, and whoever holds its nodes, can read what a runner holds. Mark it on its row once you accept that.",
          ],
          next: act(
            "mark-as-own",
            "Open Settings › Pools",
            "Opens your pools, where This is my own is on the pool's row. Only you can turn it on.",
          ),
          skippable: true,
        }),
        step("identity", "Agent identity", "later"),
        step("try", "Try it", "later"),
      ],
      {
        names: ["kind-team"],
        recommended: "kind-team",
        chosen: "kind-team",
        pools: ["team"],
        joining: false,
        pinned: null,
        key_changed: false,
      },
    ),
  ),
};

/** A teammate joining their team's install (#4495): Check and Install read as the team's. */
const JOINED = [
  DONE_WHERE,
  step("check", "Check the cluster", "done", {
    line: "Installed by your team",
    findings: [
      "purlis is on this cluster already, installed by your team, so there's nothing to check or install. Your sign-in doesn't need to read the install itself.",
    ],
  }),
  step("install", "Install", "done", {
    line: "Installed by your team",
    findings: [
      "purlis's cluster operator is on this cluster, installed by your team. Installing, changing and uninstalling it are your cluster admin's, so there's nothing to do here.",
    ],
  }),
];

const TEAMMATE = {
  names: ["kind-team"],
  recommended: "kind-team",
  chosen: "kind-team",
  pools: [],
  joining: true,
  pinned: null,
  key_changed: false,
};

const ADMINS_KEY = "ed25519:4b7e1a9c3d5f7b2e8a0c6d4f1b3e5a7c9d2f4b6e8a1c3d5f7b9e2a4c6d8f0b1e";

export const ATeammateTypesThePoolsName: Story = {
  beforeEach: () => {
    mockIPC((cmd, args) => {
      if (cmd === "pool_setup")
        return shown(
          "pool",
          [
            ...JOINED,
            step("pool", "Pool", "current", {
              findings: [
                "A pool is a named place your chats run. Your sign-in can't list your team's pools, so type the pool's name your cluster's admin gave you. purlis reads the key it names, for you to check against the one your admin gave you too.",
              ],
              next: act(
                "add-cluster-pool",
                "Read the pool's key",
                "Asks for the key your admin gave you, to check the pool's against. Nothing is pinned or claimed yet.",
              ),
            }),
            ...AFTER_POOL,
          ],
          TEAMMATE,
        );
      // Shown only once the key pasted is the pool's (#4495).
      if (cmd === "cluster_pool_key") {
        const pasted = (args as { pasted: string | null }).pasted;
        const matches = pasted === null ? null : pasted.trim().toLowerCase() === ADMINS_KEY;
        return {
          key: matches === true ? ADMINS_KEY : null,
          made_here: false,
          matches,
          namespace: "purlis-runners-7c2e9a41",
        };
      }
      return null;
    });
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const name = await canvas.findByRole("textbox", { name: "Pool's name" });
    name.focus();
    await expect(name).toHaveFocus();
    await userEvent.keyboard("team");
    await expectFocusInSight();
  },
};

export const ATeammatePastesTheirAdminsKey: Story = {
  beforeEach: ATeammateTypesThePoolsName.beforeEach,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.type(await canvas.findByRole("textbox", { name: "Pool's name" }), "team");
    await userEvent.click(canvas.getByRole("button", { name: "Read the pool's key" }));
    const box = await canvas.findByRole("textbox", { name: "Key your admin gave you" });
    await expect(canvas.queryByText(ADMINS_KEY)).toBeNull();
    await userEvent.type(box, ADMINS_KEY);
    await userEvent.click(canvas.getByRole("button", { name: "Check the key" }));
    await canvas.findByText(ADMINS_KEY);
    const pin = await canvas.findByRole("button", { name: "Pin the key and add the pool" });
    pin.focus();
    await expect(pin).toHaveFocus();
    await expectFocusInSight();
  },
};

export const NothingIsPinnedBeforeThePaste: Story = {
  beforeEach: ATeammateTypesThePoolsName.beforeEach,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.type(await canvas.findByRole("textbox", { name: "Pool's name" }), "team");
    await userEvent.click(canvas.getByRole("button", { name: "Read the pool's key" }));
    const check = await canvas.findByRole("button", { name: "Check the key" });
    check.focus();
    await userEvent.keyboard("{Enter}");
    await canvas.findByText(/Paste the key your cluster's admin gave you first/);
    // The picture is taken with the press's button still focused, as a keyboard user is left.
    const after = canvas.getByRole("button", { name: "Check the key" });
    after.focus();
    await expect(after).toHaveFocus();
  },
};

const NEW_KEY = "ed25519:e1d3c5b7a9f2e4d6c8b0a1f3e5d7c9b2a4f6e8d0c1b3a5f7e9d2c4b6a8f0e1d3";

export const AChangedKeyAsksAgain: Story = {
  beforeEach: faked({
    ...shown(
      "pool",
      [
        ...INSTALLED,
        step("pool", "Pool", "attention", {
          findings: [
            "Your cluster's pool team now names another cluster operator key than the one pinned on this machine, and the pinned key didn't vouch for it. purlis refuses every link to your runner until you check the new key with your cluster's admin and pin it.",
          ],
          next: act(
            "check-key-again",
            "Check the new key",
            "Asks for the key your admin gives you, and pins the pool's new key only if it is the same.",
          ),
        }),
        ...AFTER_POOL,
      ],
      {
        names: ["kind-team"],
        recommended: "kind-team",
        chosen: "kind-team",
        pools: ["team"],
        joining: false,
        pinned: KEY,
        key_changed: true,
      },
    ),
    pool: "team",
  }),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText(KEY);
    // The new key isn't shown before the paste (#4495).
    await expect(canvas.queryByText(NEW_KEY)).toBeNull();
    const box = await canvas.findByRole("textbox", { name: "Key your admin gave you" });
    box.focus();
    await expect(box).toHaveFocus();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Pin the new key" })).toBeVisible();
    await expectFocusInSight();
  },
};

export const TheKeyPinnedHereForTeammates: Story = {
  beforeEach: faked({
    ...shown(
      "pool",
      [...INSTALLED, step("pool", "Pool", "done", { line: "team" }), ...AFTER_POOL],
      {
        names: ["kind-team"],
        recommended: "kind-team",
        chosen: "kind-team",
        pools: ["team"],
        joining: false,
        pinned: KEY,
        key_changed: false,
      },
    ),
    pool: "team",
  }),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText(KEY);
    const go = await canvas.findByRole("button", { name: "Continue" });
    go.focus();
    await expect(go).toHaveFocus();
    await expectFocusInSight();
  },
};

/** Doctor's lines on a cluster where the person may not apply the install (#4496). */
const REFUSED_CHECKS = [
  { about: "cluster", verdict: "ok", title: "asked context kind-team", more: null },
  { about: "cluster", verdict: "ok", title: "Kubernetes v1.36.4", more: null },
  {
    about: "cluster",
    verdict: "missing",
    title: "you may not apply the install's cluster-scoped definitions",
    more: "Next: ask your cluster's admin to install purlis, or for the rights.",
  },
] as PoolSetupStep["checks"];

const ADMIN_WORDS =
  "Hello. I'd like to run purlis chats on our Kubernetes cluster, and purlis's check found it lacks what I can't add with my own sign-in. Could you have a look?\n\nWhat the check found, in its own words:\nok: asked context kind-team\nok: Kubernetes v1.36.4\nmissing: you may not apply the install's cluster-scoped definitions.\n\nThe check only reads. Run it with your own sign-in:\nkubectl version\nkubectl auth can-i create namespaces\n\nSent from purlis. It holds no password, token, key or credential.";

const onCheck = (state: string, also: PoolSetupAction) =>
  shown("check", [
    DONE_WHERE,
    step("check", "Check the cluster", state, {
      line: state === "waiting" ? "Waiting for your admin" : null,
      findings: [
        "The cluster lacks something the install doesn't add, so nothing changes until it has it. Each line says what to do, and this step checks again.",
      ],
      checks: REFUSED_CHECKS,
      next: act("check-again", "Check again", "Asks the cluster again. It changes nothing."),
      also,
      admin: ADMIN_WORDS,
    }),
    ...AFTER_CHECK(1),
  ]);

export const SendToYourAdmin: Story = {
  beforeEach: faked(
    onCheck(
      "attention",
      act(
        "send-to-admin",
        "Send to your admin",
        "Shows the words to send your cluster's admin: what's missing, what purlis needs and the commands. They hold no password, token, key or credential.",
      ),
    ),
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const send = await canvas.findByRole("button", { name: "Send to your admin" });
    send.focus();
    await expect(send).toHaveFocus();
    await expectFocusInSight();
  },
};

export const WaitingForYourAdmin: Story = {
  beforeEach: faked(
    onCheck(
      "waiting",
      act(
        "stop-waiting",
        "Stop waiting",
        "Forgets that you asked your admin. Nothing on the cluster changes.",
      ),
    ),
  ),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findAllByText("Waiting for your admin");
    const copy = await canvas.findByRole("button", { name: /Copy/ });
    copy.focus();
    await expect(copy).toHaveFocus();
    await userEvent.tab();
    await expectFocusInSight();
  },
};

/** Get help under a step that needs attention (#4497): a chat that explains it, and changes nothing. */
export const GetHelpOnAStepThatNeedsYou: Story = {
  beforeEach: SendToYourAdmin.beforeEach,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText(/It changes nothing itself: every fix is made here/);
    const help = await canvas.findByRole("button", { name: "Get help" });
    help.focus();
    await expect(help).toHaveFocus();
    await expectFocusInSight();
  },
};
