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
import type { PoolSetupMachine, PoolSetupShown, PoolSetupStep } from "../bindings";
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
  contexts: { names: [], recommended: null, chosen: null, pools: [] },
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

/** The core faked: the SSH hosts it reads, and a press held. */
const faked = (hosts: readonly string[]) => () => {
  mockIPC((cmd) =>
    cmd === "pool_setup_ssh_hosts"
      ? hosts
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
    await expect(list.getAllByRole("listitem")).toHaveLength(7);
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

/** Plain TCP, refused on an attention card with Use SSH instead. */
export const PlainTcpRefused: Story = {
  beforeEach: faked([]),
  args: { step: REFUSED, shown: shown(REFUSED) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const instead = canvas.getByRole("button", { name: "Use SSH instead" });
    instead.focus();
    await expect(instead).toHaveFocus();
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
    const again = canvas.getByRole("button", { name: "Check again" });
    again.focus();
    await expect(again).toHaveFocus();
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

/** Docker's API over TLS chosen: the address, and the files under Options, the key a file. */
export const RouteTlsApi: Story = {
  beforeEach: faked([]),
  args: { step: TLS_NAMING, shown: shown(TLS_NAMING) },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
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
  play: playFinding(6),
};

const EXPIRED = tlsFinding(
  "tls-expired",
  "lab.example.internal's certificate has expired. Make Docker a new one on lab.example.internal, the file its --tlscert names there, then check again.",
);

/** The server's certificate has expired: the file on the machine to renew is named. */
export const FindsCertificateExpired: Story = {
  beforeEach: faked([]),
  args: { step: EXPIRED, shown: shown(EXPIRED) },
  play: playFinding(6),
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
  play: playFinding(6),
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
    const again = canvas.getByRole("button", { name: "Check again" });
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
    await expect(canvas.getByText(CA)).toBeVisible();
    const go = canvas.getByRole("button", { name: "Continue" });
    go.focus();
    await expect(go).toHaveFocus();
    await expectFocusInSight();
  },
};
