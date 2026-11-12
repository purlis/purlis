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
    await expect(await canvas.findByRole("status")).toHaveTextContent(
      "Checking Docker on build-box",
    );
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
    await expect(canvas.getByText(/shows a different key/)).toBeVisible();
    const again = canvas.getByRole("button", { name: "Check again" });
    again.focus();
    await expect(again).toHaveFocus();
    await expectFocusInSight();
  },
};
