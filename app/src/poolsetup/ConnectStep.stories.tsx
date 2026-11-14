// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Pool setup's Connect, for a pool on another machine** (#6266, accepted in #6265): every state
 * the core reads, drawn, held to axe, walked by the keyboard and pictured in every pictured theme
 * cell. The core is faked at its command boundary; the words are the core's.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";
import { mockIPC } from "@tauri-apps/api/mocks";
import type {
  PoolSetupDockerContext,
  PoolSetupMachine,
  PoolSetupPodmanConnection,
  PoolSetupShown,
  PoolSetupStep,
} from "../bindings";
import { expectFocusInSight } from "../stories.testkit";
import { ConnectStep } from "./ConnectStep";

const FINGERPRINT = "SHA256:dmeCC9oOb+c7VYVBSUAdgQchZHZQyXKWgKnGQxfh2rA";
const OTHER = "SHA256:hX0mBv7pE9yS8Wd6Jf1Lq2Rk3Tn4Uo5Vp6Wq7Xr8Ys";

const SEE = {
  note: "The first command, run on build-box itself, prints its key, to compare with the one shown here. The second asks Docker there for its engine's id, as purlis does. Both only read.",
  text: "ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub\ndocker -H ssh://sam@build-box info --format '{{.ID}}'",
};

const machine = (more: Partial<PoolSetupMachine> = {}): PoolSetupMachine => ({
  route: "ssh://sam@build-box",
  identity: null,
  host: "build-box",
  key: null,
  known: null,
  trusted: null,
  now: null,
  engine: null,
  over: "ssh",
  tls: null,
  ca: null,
  trusted_ca: null,
  ca_now: null,
  context: null,
  ...more,
});

const connect = (more: Partial<PoolSetupStep>): PoolSetupStep => ({
  id: "connect",
  name: "Connect",
  state: "current",
  line: null,
  findings: [],
  next: null,
  skippable: false,
  see: SEE,
  checks: [],
  other: null,
  machine: machine(),
  cause: null,
  also: null,
  parts: [],
  undo: null,
  admin: null,
  prefill: null,
  ...more,
});

const action = (id: string, label: string, does: string) => ({
  id,
  label,
  does,
  command: null,
  url: null,
});

const PICK = connect({
  findings: ["purlis hasn't contacted build-box yet."],
  next: action(
    "connect-machine",
    "Connect to build-box",
    "Reads the key the machine shows, and keeps nothing. Nothing on it changes.",
  ),
});

const MET = connect({
  findings: [
    "This is the key build-box shows. Check it on build-box before you trust it: run the command under See what this does there, and compare the two.",
  ],
  next: action(
    "trust-machine",
    "Trust this machine",
    "Keeps this key for the pool, then checks that Docker answers on build-box. purlis connects only to a machine showing this key.",
  ),
  machine: machine({ key: { algorithm: "ssh-ed25519", fingerprint: FINGERPRINT } }),
});

const shown = (step: PoolSetupStep, pool: string | null = null): PoolSetupShown => ({
  kind: "remote-docker",
  pool,
  name: null,
  at: "connect",
  noticed: false,
  others: [],
  recommended: "build-box",
  contexts: {
    names: [],
    recommended: null,
    chosen: null,
    pools: [],
    joining: false,
    pinned: null,
    key_changed: false,
  },
  steps: [step],
});

const meta = {
  component: ConnectStep,
  args: {
    step: PICK,
    shown: shown(PICK),
    plane: "p",
    onShown: () => undefined,
    changed: () => undefined,
    onNext: () => undefined,
  },
  decorators: [
    (Story) => (
      <div style={{ maxWidth: 720 }} className="pool-setup-page">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ConnectStep>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The core faked: the SSH hosts, Podman connections and Docker contexts it reads, and a press
 * held. */
const faked =
  (
    hosts: readonly string[],
    podmans: readonly PoolSetupPodmanConnection[] = [],
    contexts: readonly PoolSetupDockerContext[] = [],
  ) =>
  () => {
    mockIPC((cmd) =>
      cmd === "pool_setup_ssh_hosts"
        ? hosts
        : cmd === "pool_setup_podman_connections"
          ? podmans
          : cmd === "pool_setup_docker_contexts"
            ? contexts
            : cmd === "pool_setup_connect" || cmd === "pool_setup_trust"
              ? new Promise(() => undefined)
              : null,
    );
  };

/** The machines the person's SSH settings name, and another one, with Connect to it. */
export const PickFromYourSshHosts: Story = {
  beforeEach: faked(["build-box", "gpu"]),
  args: {
    step: connect({ ...PICK, machine: machine({ route: "ssh://build-box" }) }),
  },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    const first = await canvas.findByRole("radio", { name: /build-box/ });
    await expect(first).toBeChecked();
    first.focus();
    await userEvent.keyboard("{ArrowDown}");
    await expect(canvas.getByRole("radio", { name: /gpu/ })).toHaveFocus();
    await expectFocusInSight();
  },
};

/** The person's Podman connections beside their SSH hosts (#6269), each its own machine. */
export const PickFromYourPodmanConnections: Story = {
  beforeEach: faked(
    ["gpu"],
    [
      {
        name: "build-box",
        route: "ssh://sam@build-box:22/run/user/1000/podman/podman.sock",
        identity: "/home/sam/.ssh/id_ed25519",
      },
    ],
  ),
  args: {
    step: connect({
      ...PICK,
      machine: machine({
        route: "ssh://sam@build-box:22/run/user/1000/podman/podman.sock",
        identity: "/home/sam/.ssh/id_ed25519",
      }),
    }),
  },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    const podman = await canvas.findByRole("radio", { name: /build-box/ });
    await expect(podman).toBeChecked();
    await expect(canvas.getByText("From your Podman connections")).toBeVisible();
    podman.focus();
    await userEvent.keyboard("{ArrowUp}");
    await expect(canvas.getByRole("radio", { name: /gpu/ })).toHaveFocus();
    await expectFocusInSight();
  },
};

/** The person's Docker contexts beside their SSH hosts (#6268): one reached over SSH, kept, and
 * one on this machine's own socket, offered as this machine's Docker. */
export const PickFromYourDockerContexts: Story = {
  beforeEach: faked(
    ["gpu"],
    [],
    [
      {
        name: "build-box",
        current: true,
        over: "ssh",
        route: "ssh://sam@build-box",
        refused: null,
      },
      { name: "desktop-linux", current: false, over: "here", route: null, refused: null },
    ],
  ),
  args: {
    step: connect({ ...PICK, machine: machine({ context: "build-box" }) }),
  },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    const context = await canvas.findByRole("radio", { name: /build-box/ });
    await expect(context).toBeChecked();
    await expect(canvas.getByText("From your Docker contexts")).toBeVisible();
    await expect(canvas.getByText(/This machine's own Docker/)).toBeVisible();
    context.focus();
    await userEvent.keyboard("{ArrowUp}");
    await expect(canvas.getByRole("radio", { name: /gpu/ })).toHaveFocus();
    await expectFocusInSight();
  },
};

/** No SSH hosts found: the address is typed. */
export const TypeTheAddress: Story = {
  beforeEach: faked([]),
  args: { step: PICK },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const field = await canvas.findByRole("textbox", { name: "Its address" });
    await expect(field).toHaveValue("sam@build-box");
    const button = canvas.getByRole("button", { name: "Connect to build-box" });
    button.focus();
    await expect(button).toHaveFocus();
    await expectFocusInSight();
  },
};

/** The key build-box shows, to check before trusting it, with This isn't it beside Trust. */
export const CheckTheKey: Story = {
  beforeEach: faked([]),
  args: { step: MET, shown: shown(MET) },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText(FINGERPRINT)).toBeVisible();
    // See what this does is open for a key the person's ssh doesn't know.
    await expect(canvas.getByText(/ssh-keygen -lf/)).toBeVisible();
    const trust = canvas.getByRole("button", { name: "Trust this machine" });
    trust.focus();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "This isn't it" })).toHaveFocus();
    await expectFocusInSight();
  },
};

/** Trust pressed: Docker there is being checked. */
export const CheckingDocker: Story = {
  beforeEach: faked([]),
  args: { step: MET, shown: shown(MET) },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Trust this machine" }));
    await expect(await canvas.findByText(/Checking Docker on build-box/)).toBeVisible();
  },
};

const FOUND = connect({
  state: "attention",
  cause: "not-allowed",
  findings: [
    "sam on build-box isn't allowed to use Docker. On build-box, add sam to the docker group, or use rootless Docker or Podman, then check again.",
  ],
  next: action("check-again", "Check again", "Contacts the machine again. It changes nothing."),
});

/** A finding: the check list with its failing line, and one trouble card. */
export const NotAllowedToUseDocker: Story = {
  beforeEach: faked([]),
  args: { step: FOUND, shown: shown(FOUND) },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    const list = within(canvas.getByRole("list", { name: "What purlis checked on build-box" }));
    await expect(list.getAllByRole("listitem")).toHaveLength(11);
    const again = canvas.getByRole("button", { name: "Check again" });
    again.focus();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Change the machine" })).toHaveFocus();
    await expectFocusInSight();
  },
};

const NEEDS_PODMAN = connect({
  state: "attention",
  cause: "needs-podman",
  machine: machine({ route: "ssh://sam@build-box/run/user/1000/podman/podman.sock" }),
  findings: [
    "build-box runs Podman, and purlis reaches Podman with Podman's own client. Install Podman on this machine, then check again.",
  ],
  next: action("check-again", "Check again", "Contacts the machine again. It changes nothing."),
});

/** Podman there, and no Podman here to reach it with (#6269): the engine's line fails. */
export const PodmanThereNeedsPodmanHere: Story = {
  beforeEach: faked([]),
  args: { step: NEEDS_PODMAN, shown: shown(NEEDS_PODMAN) },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("purlis needs Podman on this machine too")).toBeVisible();
    const again = canvas.getByRole("button", { name: "Check again" });
    again.focus();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Change the machine" })).toHaveFocus();
    await expectFocusInSight();
  },
};

const FORWARDING_REFUSED = connect({
  state: "attention",
  cause: "forwarding-refused",
  findings: [
    "The SSH server on build-box doesn't allow forwarding, and purlis reaches Docker there by forwarding its socket. Ask its admin to allow it for your account (`AllowTcpForwarding`, `AllowStreamLocalForwarding` and `DisableForwarding` in sshd_config, or `restrict` and `no-port-forwarding` on your key), then check again.",
  ],
  next: action("check-again", "Check again", "Contacts the machine again. It changes nothing."),
});

/** An SSH server that signs you in and refuses forwarding (#6349): Docker's line fails, named. */
export const SshRefusesForwarding: Story = {
  beforeEach: faked([]),
  args: { step: FORWARDING_REFUSED, shown: shown(FORWARDING_REFUSED) },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("SSH there doesn't allow forwarding")).toBeVisible();
    const again = canvas.getByRole("button", { name: "Check again" });
    again.focus();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Change the machine" })).toHaveFocus();
    await expectFocusInSight();
  },
};

const REFUSED = connect({
  state: "attention",
  cause: "plain-tcp",
  machine: machine({ route: "tcp://build-box:2375" }),
  findings: [
    "purlis doesn't connect to Docker over plain tcp://: anyone who can reach that port can run anything on that machine. Use ssh:// instead, or turn on TLS for Docker there.",
  ],
  next: action("use-ssh-instead", "Use SSH instead", "Goes back to the machine."),
});

/** Plain TCP, refused on an attention card with Use SSH instead, and Use TLS instead beside it. */
export const PlainTcpRefused: Story = {
  beforeEach: faked([]),
  args: { step: REFUSED, shown: shown(REFUSED) },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    const instead = canvas.getByRole("button", { name: "Use SSH instead" });
    instead.focus();
    await expect(instead).toHaveFocus();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Use TLS instead" })).toHaveFocus();
    await expectFocusInSight();
  },
};

const DONE = connect({
  state: "done",
  line: `build-box, key ${FINGERPRINT}`,
  machine: machine({
    route: "ssh://sam@build-box/var/run/docker.sock",
    trusted: { algorithm: "ssh-ed25519", fingerprint: FINGERPRINT },
    engine: "ABCD:EFGH",
  }),
});

/** Every check passed: the machine trusted, and Continue to the pool. */
export const Trusted: Story = {
  beforeEach: faked([]),
  args: { step: DONE, shown: shown(DONE) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("Every check passed")).toBeVisible();
    const go = canvas.getByRole("button", { name: "Continue" });
    go.focus();
    await expect(go).toHaveFocus();
    await expectFocusInSight();
  },
};

const DONE_ONLY_YOU = connect({
  state: "done",
  line: `build-box, key ${FINGERPRINT}`,
  machine: machine({
    route: "ssh://sam@build-box/var/run/docker.sock",
    trusted: { algorithm: "ssh-ed25519", fingerprint: FINGERPRINT },
    engine: "ABCD:EFGH",
    owned: true,
  }),
});

/** Every check passed, and the person said only they sign in there (#6271): the question
 *  answered, walked by the keyboard back to not sure. */
export const TrustedOnlyYouSignIn: Story = {
  beforeEach: faked([]),
  args: { step: DONE_ONLY_YOU, shown: shown(DONE_ONLY_YOU) },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    await expect(
      canvas.getByRole("radiogroup", { name: "Who can sign in to build-box?" }),
    ).toBeVisible();
    const mine = canvas.getByRole("radio", { name: "Only me, and people I trust with my logins" });
    await expect(mine).toBeChecked();
    mine.focus();
    await userEvent.keyboard("{ArrowUp}");
    await expect(
      canvas.getByRole("radio", { name: /Others might, or I.m not sure/ }),
    ).toHaveFocus();
    await expectFocusInSight();
  },
};

const CHANGED = connect({
  state: "attention",
  cause: "pool-pin-changed",
  findings: [
    `build-box shows a different key than the one you trusted (${OTHER} now, ${FINGERPRINT} trusted). This happens when the machine is reinstalled, and can mean something else is answering in its place, so purlis connects to nothing there until you check it and trust it again.`,
  ],
  next: action(
    "check-and-trust-again",
    "Check and trust again",
    "Shows the key the machine shows now, for you to check and trust.",
  ),
  machine: machine({
    trusted: { algorithm: "ssh-ed25519", fingerprint: FINGERPRINT },
    now: { algorithm: "ssh-ed25519", fingerprint: OTHER },
  }),
});

/** The pool's machine shows another key: nothing connects there, both keys are said. */
export const KeyChanged: Story = {
  beforeEach: faked([]),
  args: { step: CHANGED, shown: shown(CHANGED, "build-box") },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("heading", { name: "Check build-box again" })).toBeVisible();
    await expect(canvas.getByText(/shows a different key than the one you trusted/)).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Check again" })).toBeVisible();
    const again = canvas.getByRole("button", { name: "Check and trust again" });
    again.focus();
    await expect(again).toHaveFocus();
    await expectFocusInSight();
  },
};

const MET_AGAIN = connect({
  findings: [
    `This is the key build-box shows now, and it's different than the one you trusted (${FINGERPRINT} trusted). Check it on build-box before you trust it: run the command under See what this does there, and compare the two.`,
  ],
  next: action(
    "trust-machine",
    "Trust this machine",
    "Keeps this key for the pool in place of the one trusted, and the engine build-box runs now, then checks that Docker answers there.",
  ),
  machine: machine({
    key: { algorithm: "ssh-ed25519", fingerprint: OTHER },
    trusted: { algorithm: "ssh-ed25519", fingerprint: FINGERPRINT },
  }),
});

/** **Check and trust again** (#6270): the made pool's machine met again, the key it shows now
 *  to check beside the one trusted, and Trust this machine, which keeps only that one. */
export const MetAgain: Story = {
  beforeEach: faked([]),
  args: { step: MET_AGAIN, shown: shown(MET_AGAIN, "build-box") },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("heading", { name: "Check build-box again" })).toBeVisible();
    await expect(canvas.getByText(OTHER)).toBeVisible();
    await expect(canvas.getByText(/different than the one you trusted/)).toBeVisible();
    const trust = canvas.getByRole("button", { name: "Trust this machine" });
    trust.focus();
    await expect(trust).toHaveFocus();
    await expectFocusInSight();
  },
};

// --- Over Docker's API over TLS (#6267) ---------------------------------------------------------

const CA =
  "7A:3F:91:0C:5E:D2:84:B6:19:EE:47:2A:C0:6D:F3:58:91:0B:7C:AA:E4:32:6F:1D:88:C5:09:B7:2E:64:D1:3F";
const CA_NOW =
  "C1:08:5B:7E:22:A9:F0:43:6D:91:BC:0E:57:34:A8:F2:6C:1D:E9:80:3B:47:A5:12:D6:9F:0C:68:B3:E1:24:7A";

const TLS_FILES = {
  ca: "/Users/sam/.docker/lab/ca.pem",
  cert: "/Users/sam/.docker/lab/cert.pem",
  key: "/Users/sam/.docker/lab/key.pem",
  key_vault: null,
  key_name: null,
};

const TLS_SEE = {
  note: "The first command, run on lab.example.internal itself with the CA Docker's --tlscacert names there, prints its fingerprint, to compare with the one shown here. The second asks Docker there for its engine's id, as purlis does. Both only read.",
  text: "openssl x509 -noout -fingerprint -sha256 -in /etc/docker/ca.pem\ndocker -H tcp://lab.example.internal:2376 --tlsverify --tlscacert /Users/sam/.docker/lab/ca.pem --tlscert /Users/sam/.docker/lab/cert.pem --tlskey /Users/sam/.docker/lab/key.pem info --format '{{.ID}}'",
};

const tlsMachine = (more: Partial<PoolSetupMachine> = {}) =>
  machine({
    route: "tcp://lab.example.internal:2376",
    host: "lab.example.internal",
    over: "tls",
    tls: TLS_FILES,
    ...more,
  });

const TLS_NAMING = connect({
  state: "attention",
  cause: "plain-tcp",
  see: TLS_SEE,
  findings: [
    "purlis doesn't connect to Docker over plain tcp://: anyone who can reach that port can run anything on that machine. Use ssh:// instead, or turn on TLS for Docker there. If TLS is on for Docker there, name its CA, client certificate and key under Options.",
  ],
  machine: tlsMachine({ tls: null }),
});

/** Plain TCP to a TLS port, Use TLS instead pressed: the address, and the files under Options. */
export const RouteTlsApi: Story = {
  beforeEach: faked([]),
  args: { step: TLS_NAMING, shown: shown(TLS_NAMING) },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Use TLS instead" }));
    const tls = await canvas.findByRole("radio", { name: /Docker's API over TLS/ });
    await expect(tls).toBeChecked();
    tls.focus();
    await userEvent.keyboard("{ArrowUp}");
    await expect(canvas.getByRole("radio", { name: /SSH/ })).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}");
    await expect(await canvas.findByRole("textbox", { name: "Authority (CA) file" })).toBeVisible();
    await userEvent.tab();
    await expect(canvas.getByRole("textbox", { name: "The machine's address" })).toHaveFocus();
    await expectFocusInSight();
  },
};

/** A Docker context reached over TLS (#6268), kept: its files are the context's own, so no
 * field asks for them. */
export const TlsFromADockerContext: Story = {
  beforeEach: faked(
    [],
    [],
    [
      {
        name: "lab",
        current: false,
        over: "tls",
        route: "tcp://lab.example.internal:2376",
        refused: null,
      },
    ],
  ),
  args: {
    step: connect({
      ...TLS_NAMING,
      state: "current",
      cause: null,
      findings: ["purlis hasn't contacted lab.example.internal yet."],
      next: action(
        "connect-machine",
        "Connect to lab.example.internal",
        "Reads the CA file, and checks that the machine's certificate is signed by it and accepts yours. It keeps nothing, and nothing on the machine changes.",
      ),
      machine: tlsMachine({ context: "lab" }),
    }),
  },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    const lab = await canvas.findByRole("radio", { name: /^lab/ });
    await expect(lab).toBeChecked();
    await expect(canvas.queryByRole("textbox", { name: "Authority (CA) file" })).toBeNull();
    lab.focus();
    await userEvent.keyboard("{ArrowDown}");
    await expect(canvas.getByRole("radio", { name: /Another machine/ })).toHaveFocus();
    await expectFocusInSight();
  },
};

/** The key kept in a vault entry: its vault and its key's name, never its value. */
export const TlsKeyInAVault: Story = {
  beforeEach: faked([]),
  args: {
    step: connect({
      ...TLS_NAMING,
      machine: tlsMachine({
        tls: { ...TLS_FILES, key: null, key_vault: "docker-lab", key_name: "TLS_KEY" },
      }),
      state: "current",
      cause: null,
      findings: ["purlis hasn't contacted lab.example.internal yet."],
      next: action(
        "connect-machine",
        "Connect to lab.example.internal",
        "Reads the CA file, and checks that the machine's certificate is signed by it and accepts yours. It keeps nothing, and nothing on the machine changes.",
      ),
    }),
  },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    const options = canvas.getByRole("button", { name: "Options" });
    await userEvent.click(options);
    const vault = await canvas.findByRole("radio", { name: /In a vault entry/ });
    await expect(vault).toBeChecked();
    await expect(canvas.getByRole("textbox", { name: "Vault" })).toHaveValue("docker-lab");
    const go = canvas.getByRole("button", { name: "Connect to lab.example.internal" });
    go.focus();
    await expect(go).toHaveFocus();
    await expectFocusInSight();
  },
};

const MET_CA = connect({
  see: TLS_SEE,
  findings: [
    "lab.example.internal's certificate is signed by this CA. Check it is the CA Docker on lab.example.internal trusts before you trust it: run the command under See what this does there, and compare the two fingerprints.",
  ],
  next: action(
    "trust-machine",
    "Trust this machine",
    "Keeps this CA for the pool, then checks that Docker answers on lab.example.internal. purlis connects only to a machine whose certificate this CA signed, and checks the file still holds it before every run.",
  ),
  machine: tlsMachine({
    ca: {
      fingerprint: CA,
      subject: "CN=lab.example.internal CA,O=Example Lab",
      file: TLS_FILES.ca,
    },
  }),
});

/** The authority lab.example.internal's certificate is signed by, to check before trusting it. */
export const CheckTheAuthority: Story = {
  beforeEach: faked([]),
  args: { step: MET_CA, shown: shown(MET_CA) },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText(CA)).toBeVisible();
    await expect(canvas.getByText(/openssl x509 -noout -fingerprint/)).toBeVisible();
    const trust = canvas.getByRole("button", { name: "Trust this machine" });
    trust.focus();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "This isn't it" })).toHaveFocus();
    await expectFocusInSight();
  },
};

const tlsFinding = (cause: string, finding: string, next = "check-again") =>
  connect({
    state: "attention",
    cause,
    see: TLS_SEE,
    findings: [finding],
    machine: tlsMachine(),
    next:
      next === "change-machine"
        ? action("change-machine", "Change the machine", "Goes back to the machine.")
        : action("check-again", "Check again", "Contacts the machine again. It changes nothing."),
  });

const playFinding =
  (lines: number) =>
  async ({ canvasElement }: { canvasElement: HTMLElement }) => {
    const canvas = within(canvasElement);
    const list = within(
      canvas.getByRole("list", { name: "What purlis checked on lab.example.internal" }),
    );
    await expect(list.getAllByRole("listitem")).toHaveLength(lines);
    const first = canvas.getAllByRole("button", { name: /Check again|Change the machine/ })[0];
    first?.focus();
    await expect(first).toHaveFocus();
    await expectFocusInSight();
  };

const NOT_SIGNED = tlsFinding(
  "tls-not-signed",
  "lab.example.internal's certificate isn't signed by the CA you chose. Check /Users/sam/.docker/lab/ca.pem: it must be the CA Docker's certificates on lab.example.internal are signed with.",
);

/** The server's certificate is from another authority: the CA file to check is named. */
export const FindsCertificateFromAnotherAuthority: Story = {
  beforeEach: faked([]),
  args: { step: NOT_SIGNED, shown: shown(NOT_SIGNED) },
  play: playFinding(10),
};

const EXPIRED = tlsFinding(
  "tls-expired",
  "lab.example.internal's certificate has expired. Make Docker a new one on lab.example.internal, the file its --tlscert names there, then check again.",
);

/** The server's certificate has expired: the file on the machine to renew is named. */
export const FindsCertificateExpired: Story = {
  beforeEach: faked([]),
  args: { step: EXPIRED, shown: shown(EXPIRED) },
  play: playFinding(10),
};

const WRONG_NAME = tlsFinding(
  "tls-wrong-name",
  "lab.example.internal's certificate is for another name (docker, localhost). Connect by that name, or check the file Docker's --tlscert names on lab.example.internal.",
  "change-machine",
);

/** The server's certificate is for another name: the names it is for, and the file to check. */
export const FindsCertificateForAnotherName: Story = {
  beforeEach: faked([]),
  args: { step: WRONG_NAME, shown: shown(WRONG_NAME) },
  play: playFinding(10),
};

const CA_CHANGED = connect({
  state: "attention",
  cause: "pool-pin-changed",
  see: TLS_SEE,
  findings: [
    `/Users/sam/.docker/lab/ca.pem no longer holds the CA you trusted for lab.example.internal (${CA_NOW} now, ${CA} trusted). This happens when the CA is replaced, and can mean something else is answering in its place, so purlis connects to nothing there until you check it and trust it again.`,
  ],
  next: action(
    "check-and-trust-again",
    "Check and trust again",
    "Shows the CA the file holds now, for you to check and trust.",
  ),
  machine: tlsMachine({ trusted_ca: CA, ca_now: CA_NOW }),
});

/** The pool's CA file no longer holds the authority trusted: nothing runs there. */
export const AuthorityChanged: Story = {
  beforeEach: faked([]),
  args: { step: CA_CHANGED, shown: shown(CA_CHANGED, "lab") },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      canvas.getByRole("heading", { name: "Check lab.example.internal again" }),
    ).toBeVisible();
    await expect(canvas.getByText(/no longer holds the CA you trusted/)).toBeVisible();
    await expect(canvas.getByText("The authority isn't the one you trusted")).toBeVisible();
    const again = canvas.getByRole("button", { name: "Check and trust again" });
    again.focus();
    await expect(again).toHaveFocus();
    await expectFocusInSight();
  },
};

const TLS_DONE = connect({
  state: "done",
  see: TLS_SEE,
  line: `lab.example.internal, CA ${CA}`,
  machine: tlsMachine({ trusted_ca: CA, engine: "ABCD:EFGH" }),
});

/** Every check passed over TLS: the authority trusted, and Continue to the pool. */
export const TrustedOverTls: Story = {
  beforeEach: faked([]),
  args: { step: TLS_DONE, shown: shown(TLS_DONE) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("Docker's API over TLS")).toBeVisible();
    await expect(canvas.getByText(`${CA.slice(0, 23)}…`)).toBeVisible();
    const go = canvas.getByRole("button", { name: "Continue" });
    go.focus();
    await expect(go).toHaveFocus();
    await expectFocusInSight();
  },
};

// --- From a Docker context (#6268) ----------------------------------------------------------------

const MOVED = connect({
  state: "attention",
  cause: "context-moved",
  findings: [
    "Your context build-box now points somewhere else than this pool. Connect again to follow it, or keep the pool as it is.",
  ],
  next: action(
    "connect-again",
    "Connect again",
    "Starts setting up a pool on the machine build-box names now. This pool keeps running where it does until you remove it.",
  ),
  other: action(
    "keep-pool-as-it-is",
    "Keep the pool as it is",
    "The pool stays on the machine it runs on now, and purlis stops comparing it with the context.",
  ),
  prefill: { route: "ssh://sam@new-box", context: "build-box" },
  machine: machine({
    route: "ssh://sam@build-box/var/run/docker.sock",
    context: "build-box",
    trusted: { algorithm: "ssh-ed25519", fingerprint: FINGERPRINT },
    engine: "ABCD:EFGH",
  }),
});

/** The pool's Docker context now names another machine (ADR 0123 §7): the pool runs where it
 * did, every check of it holds, and the person connects again or keeps it as it is. */
export const ContextMoved: Story = {
  beforeEach: faked([]),
  args: { step: MOVED, shown: shown(MOVED, "build-box") },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("Your context points somewhere else")).toBeVisible();
    const again = canvas.getByRole("button", { name: "Connect again" });
    again.focus();
    await expect(again).toHaveFocus();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "Keep the pool as it is" })).toHaveFocus();
    await expectFocusInSight();
  },
};

// --- Doctor's every finding (#6272, ADR 0123 §7) ------------------------------------------------
// Each finding's words and its commands are the core's (`connecting::said_and_next`,
// `connecting::see`), as `purlis pool doctor` prints them too.

const CHECK_AGAIN = action(
  "check-again",
  "Check again",
  "Contacts the machine again. It changes nothing.",
);
const CHANGE = action(
  "change-machine",
  "Change the machine",
  "Goes back to the machine, to pick or type it again.",
);

/** A finding over SSH on build-box, with its one way on and its commands. */
const sshFinding = (
  cause: string,
  finding: string,
  see: { note: string; text: string } = SEE,
  next = CHECK_AGAIN,
  more: Partial<PoolSetupStep> = {},
) =>
  connect({
    state: "attention",
    cause,
    findings: [finding],
    see,
    next,
    machine: machine({
      trusted: { algorithm: "ssh-ed25519", fingerprint: FINGERPRINT },
      engine: "ABCD:EFGH:IJKL",
    }),
    ...more,
  });

/** A finding's card, by its title, its failing line among `lines`, its commands and its way on
 * reached by the keyboard. */
const playDoctor =
  (title: string, lines: number, host = "build-box"): Story["play"] =>
  async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText(title)).toBeVisible();
    const list = within(canvas.getByRole("list", { name: `What purlis checked on ${host}` }));
    await expect(list.getAllByRole("listitem")).toHaveLength(lines);
    // Opened by the keyboard, so no pointer is left on it for the picture to catch mid-hover.
    const see = canvas.getByRole("button", { name: "See what this does" });
    see.focus();
    await userEvent.keyboard("{Enter}");
    await expect(canvas.getByRole("button", { name: /Copy/ })).toBeVisible();
    const first = canvas.getAllByRole("button", {
      name: /Check again|Change the machine|Check and trust again/,
    })[0];
    first?.focus();
    await expect(first).toHaveFocus();
    await expectFocusInSight();
  };

const NO_SUCH_NAME = sshFinding(
  "no-such-name",
  "purlis couldn't find a machine called build-box. Check the name, or pick it from your SSH hosts.",
  {
    note: "The first prints what your SSH settings make of build-box, and the second lists the hosts they name, to pick from. Both only read.",
    text: "ssh -G build-box\ngrep -i '^host ' ~/.ssh/config",
  },
  CHANGE,
);

/** The name doesn't resolve: the first line fails, and the way on is another name. */
export const FindsNoSuchName: Story = {
  beforeEach: faked([]),
  args: { step: NO_SUCH_NAME, shown: shown(NO_SUCH_NAME) },
  play: playDoctor("purlis couldn't find that machine", 11),
};

const NO_ANSWER = sshFinding(
  "no-answer",
  "build-box didn't answer on port 22. Check that it's on and on this network, and connect your VPN if you use one.",
  {
    note: "This asks whether anything answers on port 22 of build-box, from this machine. It only reads.",
    text: "nc -vz build-box 22",
  },
);

/** Nothing answers on the port: the machine is off, elsewhere, or behind a VPN. */
export const FindsNoAnswer: Story = {
  beforeEach: faked([]),
  args: { step: NO_ANSWER, shown: shown(NO_ANSWER) },
  play: playDoctor("The machine didn't answer", 11),
};

const NOT_MET = sshFinding(
  "not-met",
  "purlis couldn't reach SSH on build-box (it said: kex_exchange_identification: Connection closed). Check that SSH is running there, then check again.",
  {
    note: "This signs in to build-box as purlis does, and prints what SSH says as it goes. It changes nothing there.",
    text: "ssh sam@build-box -v -o BatchMode=yes true",
  },
);

/** Something answers on the port, and it isn't SSH, or SSH closed on purlis. */
export const FindsSshNotThere: Story = {
  beforeEach: faked([]),
  args: { step: NOT_MET, shown: shown(NOT_MET) },
  play: playDoctor("SSH didn't answer there", 11),
};

const SIGN_IN_REFUSED = sshFinding(
  "sign-in-refused",
  "build-box didn't accept your sign-in. purlis signs in with your SSH agent and never asks for a password. Add your key to the agent with `ssh-add`, then check again.",
  {
    note: "The first lists the keys your SSH agent holds, and the second adds your own key to it. The third signs in to build-box as purlis does, never asking for a password.",
    text: "ssh-add -l\nssh-add\nssh sam@build-box -o BatchMode=yes true",
  },
);

/** No key in the agent the machine takes: the sign-in's line fails. */
export const FindsNoAgentKey: Story = {
  beforeEach: faked([]),
  args: { step: SIGN_IN_REFUSED, shown: shown(SIGN_IN_REFUSED) },
  play: playDoctor("The machine didn't accept your sign-in", 11),
};

const NO_ENGINE = sshFinding(
  "no-engine",
  "build-box has no Docker running that sam can use. Start Docker there, or name its socket in the address, then check again.",
  {
    note: "The first shows which of Docker's sockets build-box has. The second starts Docker there and keeps it on, asking for your password on build-box.",
    text: `ssh sam@build-box 'ls -l /var/run/docker.sock "$XDG_RUNTIME_DIR/docker.sock"'\nssh -t sam@build-box sudo systemctl enable --now docker`,
  },
);

/** No engine's socket there: Docker's line fails. */
export const FindsNoSocket: Story = {
  beforeEach: faked([]),
  args: { step: NO_ENGINE, shown: shown(NO_ENGINE) },
  play: playDoctor("No Docker there that you can use", 11),
};

const ENGINE_SILENT = sshFinding(
  "engine-silent",
  "Docker on build-box didn't answer through SSH. Check that Docker is running there, then check again.",
  {
    note: "This asks Docker on build-box how it is, as purlis reaches it. It only reads.",
    text: "docker -H ssh://sam@build-box info",
  },
);

/** The socket is there and Docker behind it doesn't answer. */
export const FindsDockerSilent: Story = {
  beforeEach: faked([]),
  args: { step: ENGINE_SILENT, shown: shown(ENGINE_SILENT) },
  play: playDoctor("Docker didn't answer there", 11),
};

const ENGINE_CHANGED = sshFinding(
  "pool-engine-changed",
  "The Docker engine on build-box isn't the one you trusted: it was reset or reinstalled. purlis makes and links no runner there until you check it and trust it again. Runners made on the old engine aren't on this one.",
  SEE,
  action(
    "check-and-trust-again",
    "Check and trust again",
    "Checks the engine there again, for you to trust.",
  ),
);

/** A made pool's engine answers with another id under the same key: its line fails. */
export const FindsEngineChanged: Story = {
  beforeEach: faked([]),
  args: { step: ENGINE_CHANGED, shown: shown(ENGINE_CHANGED, "build-box") },
  play: playDoctor("Docker there isn't the one you trusted", 11),
};

const UNCONFINED = sshFinding(
  "unconfined",
  "Docker on build-box doesn't give containers its built-in seccomp profile, so purlis makes no runner there. On build-box, set the daemon's seccomp profile back to its default, then check again.",
  {
    note: "This prints the security options Docker on build-box gives containers, as purlis reads them. It only reads.",
    text: "docker -H ssh://sam@build-box info --format '{{json .SecurityOptions}}'",
  },
);

/** Docker there is set to another seccomp profile, or none: the local kind's daemon check. */
export const FindsDockerUnconfined: Story = {
  beforeEach: faked([]),
  args: { step: UNCONFINED, shown: shown(UNCONFINED, "build-box") },
  play: playDoctor("Docker there doesn't confine runners", 11),
};

const ARCHITECTURE = sshFinding(
  "unsupported-architecture",
  "build-box is linux/s390x, and the runner image is built for linux/amd64 and linux/arm64. Pick a machine that is one of those.",
  {
    note: "This prints the platform of build-box, as purlis reads it. It only reads.",
    text: "docker -H ssh://sam@build-box info --format '{{.OSType}}/{{.Architecture}}'",
  },
  CHANGE,
);

/** A machine of a platform the runner image isn't built for. */
export const FindsUnsupportedArchitecture: Story = {
  beforeEach: faked([]),
  args: { step: ARCHITECTURE, shown: shown(ARCHITECTURE) },
  play: playDoctor("There's no runner image for that machine", 11),
};

const IMAGE_UNREACHABLE = sshFinding(
  "image-unreachable",
  "build-box couldn't download the runner image. Check that build-box can reach ghcr.io, then check again.",
  {
    note: "This asks ghcr.io for an answer from build-box, as Docker there does before it downloads. It only reads.",
    text: "ssh sam@build-box 'curl -sSI https://ghcr.io/v2/'",
  },
);

/** The machine lacks the runner image and can't reach its registry. */
export const FindsImageUnreachable: Story = {
  beforeEach: faked([]),
  args: { step: IMAGE_UNREACHABLE, shown: shown(IMAGE_UNREACHABLE, "build-box") },
  play: playDoctor("The machine couldn't download the runner image", 11),
};

const DISK_FULL = sshFinding(
  "disk-full",
  "Docker's disk on build-box is full: it has 512 MB free, and a runner needs at least 1.0 GB, so purlis makes no runner there. To make room, run docker system df on build-box to see what takes it, then docker system prune there to remove what nothing uses, or give Docker there a bigger disk. Then check again.",
  {
    note: "The first shows what takes Docker's disk on build-box. The second removes what nothing uses there: stopped containers, unused networks and images, and the build cache. It keeps volumes, so a runner's home and sign-ins stay.",
    text: "docker -H ssh://sam@build-box system df\ndocker -H ssh://sam@build-box system prune",
  },
);

/** Docker's disk there has less free than a runner needs: the last line fails. */
export const FindsDiskFull: Story = {
  beforeEach: faked([]),
  args: { step: DISK_FULL, shown: shown(DISK_FULL, "build-box") },
  play: playDoctor("Docker's disk there is full", 11),
};

const HERE = sshFinding("here", "purlis found no ssh on this machine.");

/** Something on this machine stopped the check before the other machine was asked. */
export const FindsSomethingHere: Story = {
  beforeEach: faked([]),
  args: { step: HERE, shown: shown(HERE) },
  play: playDoctor("That didn't work", 11),
};

const NOT_A_ROUTE = connect({
  state: "attention",
  cause: "not-a-route",
  findings: ["a route starts with ssh://, as in ssh://you@build-box"],
  next: CHANGE,
  machine: machine({ route: "sftp://build-box", host: null }),
});

/** What was typed is no address purlis can read: refused on its card, with Change the machine. */
export const NotAnAddress: Story = {
  beforeEach: faked([]),
  args: { step: NOT_A_ROUTE, shown: shown(NOT_A_ROUTE) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("That isn't an address purlis can read")).toBeVisible();
    const change = canvas.getByRole("button", { name: "Change the machine" });
    change.focus();
    await expect(change).toHaveFocus();
    await expectFocusInSight();
  },
};

const tlsDoctor = (cause: string, finding: string, see: { note: string; text: string }) => ({
  ...tlsFinding(cause, finding),
  see,
});

const TLS_FILE = tlsDoctor(
  "tls-file",
  "/Users/sam/.docker/lab/ca.pem isn't there. Check the file named under Options, then check again.",
  {
    note: "This shows whether the file is there and who may read it. It only reads.",
    text: "ls -l /Users/sam/.docker/lab/ca.pem",
  },
);

/** A file named under Options can't be used. */
export const FindsTlsFileMissing: Story = {
  beforeEach: faked([]),
  args: { step: TLS_FILE, shown: shown(TLS_FILE) },
  play: playDoctor("purlis can't use one of the files you named", 10, "lab.example.internal"),
};

const CLIENT_REFUSED = tlsDoctor(
  "tls-client-refused",
  "Docker on lab.example.internal didn't accept your client certificate. Check /Users/sam/.docker/lab/cert.pem: it must be signed by the CA Docker on lab.example.internal trusts, the file its --tlscacert names.",
  {
    note: "This prints who signed your client certificate and when it expires. It only reads.",
    text: "openssl x509 -noout -issuer -enddate -in /Users/sam/.docker/lab/cert.pem",
  },
);

/** Docker there refuses the client certificate. */
export const FindsClientCertificateRefused: Story = {
  beforeEach: faked([]),
  args: { step: CLIENT_REFUSED, shown: shown(CLIENT_REFUSED) },
  play: playDoctor("Docker there didn't accept your certificate", 10, "lab.example.internal"),
};

const CLIENT_EXPIRED = tlsDoctor(
  "tls-client-expired",
  "Your client certificate has expired. Check /Users/sam/.docker/lab/cert.pem, make a new one, then check again.",
  {
    note: "This prints who signed your client certificate and when it expires. It only reads.",
    text: "openssl x509 -noout -issuer -enddate -in /Users/sam/.docker/lab/cert.pem",
  },
);

/** The client certificate has expired. */
export const FindsClientCertificateExpired: Story = {
  beforeEach: faked([]),
  args: { step: CLIENT_EXPIRED, shown: shown(CLIENT_EXPIRED) },
  play: playDoctor("Your certificate has expired", 10, "lab.example.internal"),
};

const FOR_ADMIN = sshFinding(
  "not-allowed",
  "sam on build-box isn't allowed to use Docker. On build-box, add sam to the docker group, or use rootless Docker or Podman, then check again.",
  {
    note: "The first adds sam to the docker group on build-box, asking for your password there: anyone in that group controls Docker on build-box. The second signs in again, so the group counts, and asks Docker for its id as purlis does. Rootless Docker or Podman keeps Docker to your own account instead.",
    text: "ssh -t sam@build-box sudo usermod -aG docker sam\nssh sam@build-box docker info --format '{{.ID}}'",
  },
  CHECK_AGAIN,
  {
    also: action(
      "send-to-admin",
      "Send to your admin",
      "Shows the words to send the machine's admin: what's wrong, the machine's fingerprints and the commands. They hold no password, token, key or credential.",
    ),
    admin: `Hello. I'm setting up purlis to run my chats in Docker on build-box, and its check found something there I can't change with my own account. Could you have a look?

What the check found:
sam on build-box isn't allowed to use Docker. On build-box, add sam to the docker group, or use rootless Docker or Podman, then check again.

The machine, as purlis reaches it:
Route: ssh://sam@build-box
Machine: build-box
Reached by: SSH
Its key, trusted: ${FINGERPRINT} (ssh-ed25519)
Its engine, trusted: ABCD:EFGH:IJKL

The first adds sam to the docker group on build-box, asking for your password there: anyone in that group controls Docker on build-box. The second signs in again, so the group counts, and asks Docker for its id as purlis does. Rootless Docker or Podman keeps Docker to your own account instead.
ssh -t sam@build-box sudo usermod -aG docker sam
ssh sam@build-box docker info --format '{{.ID}}'

Once it's done, tell me, and I'll have purlis check again. Sent from purlis 0.5.2. It holds no password, token, key or credential.`,
  },
);

/** Send to your admin, open: the route, the host and the fingerprints, to copy, and no key. */
export const SendToYourAdmin: Story = {
  beforeEach: faked([]),
  args: { step: FOR_ADMIN, shown: shown(FOR_ADMIN, "build-box") },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Send to your admin" }));
    await expect(canvas.getByText(/Route: ssh:\/\/sam@build-box/)).toBeVisible();
    const copy = canvas.getAllByRole("button", { name: /Copy/ })[0];
    copy?.focus();
    await expect(copy).toHaveFocus();
    await expectFocusInSight();
  },
};
