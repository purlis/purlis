// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PoolSetupPart, PoolSetupStep } from "../bindings";
import { CheckStep, ClusterPoolStep, InstallStep } from "./ClusterSteps";

afterEach(() => {
  cleanup();
  clearMocks();
});

const part = (
  id: string,
  name: string,
  done: boolean,
  next: string | null = null,
): PoolSetupPart => ({
  id,
  name,
  done,
  line: `${name}: what it is.`,
  next:
    next === null
      ? null
      : { id: next, label: `Install ${name}`, does: `Applies ${name}.`, command: null, url: null },
});

const install = (parts: PoolSetupPart[], undo = true): PoolSetupStep => ({
  id: "install",
  name: "Install",
  state: "current",
  line: null,
  findings: [],
  next: null,
  skippable: false,
  see: { note: "The install's commands.", text: "kubectl apply --server-side -k …" },
  checks: [],
  other: null,
  machine: null,
  cause: null,
  also: null,
  prefill: null,
  parts,
  admin: null,
  undo: undo
    ? {
        note: "purlis never uninstalls from a cluster: 3 claims by 2 people would end.",
        text: "kubectl delete claims.runners.purlis.dev --all-namespaces --all",
      }
    : null,
});

const FIRST = install([
  part("upstream", "agent-sandbox", true),
  part("definitions", "purlis's definitions", false, "install-definitions"),
  part("signing-key", "Signing key", false),
  part("operator", "Cluster operator", false),
]);

function core(refuse?: string) {
  const sent: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    sent.push({ cmd, args });
    if (cmd === "pool_setup_install" && refuse) throw refuse;
    return null;
  });
  return sent;
}

describe("Install (#4493)", () => {
  it("applies a part only on its own button, and reads the setup again after", async () => {
    const sent = core();
    const changed = vi.fn();
    render(<InstallStep step={FIRST} onNext={() => {}} changed={changed} />);
    const parts = screen.getByRole("list", { name: "Parts of the install" });
    // One button, for the one part that is next.
    expect(within(parts).getAllByRole("button")).toHaveLength(1);
    expect(sent.filter((one) => one.cmd === "pool_setup_install")).toEqual([]);
    await userEvent.click(
      within(parts).getByRole("button", { name: "Install purlis's definitions" }),
    );
    await waitFor(() => expect(changed).toHaveBeenCalled());
    expect(sent).toContainEqual({ cmd: "pool_setup_install", args: { part: "definitions" } });
  });

  it("says what failed under the part and offers to try it again, alone", async () => {
    const sent = core("kubectl apply said: forbidden");
    render(<InstallStep step={FIRST} onNext={() => {}} changed={() => {}} />);
    await userEvent.click(screen.getByRole("button", { name: "Install purlis's definitions" }));
    expect(await screen.findByText("purlis's definitions wasn't installed")).toBeInTheDocument();
    expect(screen.getByText("kubectl apply said: forbidden")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() =>
      expect(sent.filter((one) => one.cmd === "pool_setup_install")).toHaveLength(2),
    );
  });

  it("keeps the operator's image typed under Options", async () => {
    const sent = core();
    const step = install([
      part("upstream", "agent-sandbox", true),
      part("definitions", "purlis's definitions", true),
      part("signing-key", "Signing key", true),
      part("operator", "Cluster operator", false, "choose-operator-image"),
    ]);
    render(<InstallStep step={step} onNext={() => {}} changed={() => {}} />);
    // Waiting for its image, the operator has no button, and Options is open.
    expect(screen.queryByRole("button", { name: "Install Cluster operator" })).toBeNull();
    const image = "ghcr.io/purlis/purlis-cluster-operator@sha256:" + "a".repeat(64);
    await userEvent.type(screen.getByRole("textbox", { name: "Operator image" }), image);
    await waitFor(() =>
      expect(sent).toContainEqual({ cmd: "pool_setup_operator_image", args: { image } }),
    );
  });

  it("never offers uninstalling as a button, only its commands and who they affect", async () => {
    core();
    render(<InstallStep step={FIRST} onNext={() => {}} changed={() => {}} />);
    // The one button that names it only opens its commands.
    const [only, ...more] = screen.getAllByRole("button", { name: /uninstall/i });
    expect(more).toEqual([]);
    expect(only).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(only);
    expect(screen.getByText(/3 claims by 2 people would end/)).toBeInTheDocument();
  });
});

const poolStep = (next: string): PoolSetupStep => ({
  ...install([]),
  id: "pool",
  name: "Pool",
  undo: null,
  prefill: null,
  see: null,
  next: {
    id: next,
    label: next === "create-cluster-pool" ? "Create the pool" : "Add the pool here",
    does: "What it does.",
    command: null,
    url: null,
  },
});

const CONTEXTS = (pools: string[]) => ({
  names: ["kind-team"],
  recommended: "kind-team",
  chosen: "kind-team",
  pools,
  joining: false,
  pinned: null,
  key_changed: false,
});

const KEY = "ed25519:" + "b".repeat(64);
const IMAGE = "ghcr.io/purlis/purlis-runner@sha256:" + "c".repeat(64);

/** The key as the core reads a paste: spaces, line breaks and capitals put aside. */
const plain = (pasted: string) =>
  pasted
    .replace(/\s/g, "")
    .toLowerCase()
    .replace(/^ed25519:/, "");

/** A stand-in core, whose Pool names KEY: shown only where it was made here or pasted (#4495). */
function cluster(madeHere = false) {
  const sent: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    sent.push({ cmd, args });
    if (cmd === "cluster_pool_key") {
      const pasted = (args as { pasted: string | null }).pasted;
      const matches = pasted === null ? null : plain(pasted) === plain(KEY);
      return {
        key: madeHere || matches === true ? KEY : null,
        made_here: madeHere,
        matches,
        namespace: "purlis-runners-ab12",
      };
    }
    if (cmd === "add_cluster_pool") return { name: "team" };
    if (cmd === "docker_pools")
      return { pools: [], clusters: [], defaults: null, suspend_after: null, yours: true };
    return null;
  });
  return sent;
}

describe("The Pool step on a cluster (#4494)", () => {
  it("makes the Pool in the cluster on its own button, of this build's runner image", async () => {
    const sent = cluster();
    const changed = vi.fn();
    render(
      <ClusterPoolStep
        step={poolStep("create-cluster-pool")}
        contexts={CONTEXTS([])}
        image={IMAGE}
        onNext={() => {}}
        changed={changed}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Create the pool" }));
    await waitFor(() => expect(changed).toHaveBeenCalled());
    expect(sent).toContainEqual({
      cmd: "pool_setup_cluster_pool",
      args: { name: "team", image: IMAGE },
    });
  });

  it("shows the key this setup made at once, and pins it only on the second press, with no paste", async () => {
    const sent = cluster(true);
    const changed = vi.fn();
    render(
      <ClusterPoolStep
        step={poolStep("add-cluster-pool")}
        contexts={CONTEXTS(["team"])}
        image={IMAGE}
        onNext={() => {}}
        changed={changed}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Add the pool here" }));
    expect(await screen.findByText("Check the cluster operator key")).toBeInTheDocument();
    expect(screen.getByText(KEY)).toBeInTheDocument();
    expect(sent.map((one) => one.cmd)).not.toContain("add_cluster_pool");
    await userEvent.click(screen.getByRole("button", { name: "Pin the key and add the pool" }));
    await waitFor(() => expect(changed).toHaveBeenCalled());
    expect(sent).toContainEqual({
      cmd: "add_cluster_pool",
      args: {
        adding: {
          name: "team",
          kubeconfig: null,
          context: "kind-team",
          pool: "team",
          pasted: null,
          vouch_wait: null,
        },
      },
    });
    expect(sent).toContainEqual({ cmd: "pool_setup_pool", args: { name: "team" } });
  });
});

const JOINING = (pools: string[]) => ({ ...CONTEXTS(pools), joining: true });
const PASTED = "B".repeat(32) + " " + "B".repeat(32);

describe("Joining a team's pool (#4495)", () => {
  it("asks for the pool's name where the pools can't be listed, and asks about that pool's key", async () => {
    const sent = cluster();
    render(
      <ClusterPoolStep
        step={poolStep("add-cluster-pool")}
        contexts={JOINING([])}
        image={IMAGE}
        onNext={() => {}}
        changed={() => {}}
      />,
    );
    await userEvent.type(screen.getByRole("textbox", { name: "Pool's name" }), "team");
    await userEvent.click(screen.getByRole("button", { name: "Add the pool here" }));
    await screen.findByRole("textbox", { name: "Key your admin gave you" });
    expect(sent).toContainEqual({
      cmd: "cluster_pool_key",
      args: { kubeconfig: null, context: "kind-team", pool: "team", pasted: null },
    });
  });

  it("shows the pool's key only once the admin's key pasted matches it, and pins only then", async () => {
    const sent = cluster();
    const changed = vi.fn();
    render(
      <ClusterPoolStep
        step={poolStep("add-cluster-pool")}
        contexts={JOINING(["team"])}
        image={IMAGE}
        onNext={() => {}}
        changed={changed}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Add the pool here" }));
    const box = await screen.findByRole("textbox", { name: "Key your admin gave you" });
    // Nothing of the cluster's key before the paste, so it can't be copied into the box.
    expect(screen.queryByText(KEY)).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Check the key" }));
    expect(
      await screen.findByText(/Paste the key your cluster's admin gave you first/),
    ).toBeInTheDocument();

    // A paste that isn't the pool's key shows the mismatch, and still not the key.
    await userEvent.type(box, "ed25519:" + "e".repeat(64));
    await userEvent.click(screen.getByRole("button", { name: "Check the key" }));
    expect(await screen.findByText(/isn't the one pool team names/)).toBeInTheDocument();
    expect(screen.queryByText(KEY)).toBeNull();
    expect(sent.map((one) => one.cmd)).not.toContain("add_cluster_pool");

    await userEvent.clear(box);
    await userEvent.type(box, PASTED);
    await userEvent.click(screen.getByRole("button", { name: "Check the key" }));
    expect(await screen.findByText(KEY)).toBeInTheDocument();
    expect(sent.map((one) => one.cmd)).not.toContain("add_cluster_pool");
    await userEvent.click(screen.getByRole("button", { name: "Pin the key and add the pool" }));
    await waitFor(() => expect(changed).toHaveBeenCalled());
    const added = sent.find((one) => one.cmd === "add_cluster_pool");
    // The pasted key, as pasted: the core reads it and compares it with the Pool's again.
    expect(added?.args).toMatchObject({ adding: { pasted: PASTED } });
  });

  it("asks again when the paste changes after a match", async () => {
    cluster();
    render(
      <ClusterPoolStep
        step={poolStep("add-cluster-pool")}
        contexts={CONTEXTS(["team"])}
        image={IMAGE}
        onNext={() => {}}
        changed={() => {}}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Add the pool here" }));
    const box = await screen.findByRole("textbox", { name: "Key your admin gave you" });
    await userEvent.type(box, PASTED);
    await userEvent.click(screen.getByRole("button", { name: "Check the key" }));
    expect(await screen.findByText(KEY)).toBeInTheDocument();
    await userEvent.type(box, "x");
    expect(screen.queryByText(KEY)).toBeNull();
    expect(screen.getByRole("button", { name: "Check the key" })).toBeInTheDocument();
  });

  it("shows the key pinned here once the pool is added, for teammates to check theirs against", () => {
    cluster();
    render(
      <ClusterPoolStep
        step={{ ...poolStep("add-cluster-pool"), state: "done", next: null, line: "team" }}
        contexts={{ ...CONTEXTS(["team"]), pinned: KEY }}
        image={IMAGE}
        onNext={() => {}}
        changed={() => {}}
      />,
    );
    expect(screen.getByText(KEY)).toBeInTheDocument();
    expect(
      screen.getByText(/Teammates who join this pool check theirs against it/),
    ).toBeInTheDocument();
  });
});

describe("A changed key asks again (#4495)", () => {
  const NEW = "ed25519:" + "d".repeat(64);
  const again: PoolSetupStep = {
    ...poolStep("add-cluster-pool"),
    state: "attention",
    next: {
      id: "check-key-again",
      label: "Check the new key",
      does: "Shows the key.",
      command: null,
      url: null,
    },
  };

  it("shows the pinned key and not the new one, and pins the new one only with the admin's pasted", async () => {
    const sent = cluster();
    const changed = vi.fn();
    render(
      <ClusterPoolStep
        step={again}
        contexts={{ ...CONTEXTS(["team"]), pinned: KEY, key_changed: true }}
        image={IMAGE}
        pool="team"
        onNext={() => {}}
        changed={changed}
      />,
    );
    expect(screen.getByText(KEY)).toBeInTheDocument();
    expect(screen.queryByText(NEW)).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Pin the new key" }));
    expect(
      await screen.findByText(/Paste the key your cluster's admin gave you first/),
    ).toBeInTheDocument();
    expect(sent.map((one) => one.cmd)).not.toContain("pool_setup_pin_again");
    await userEvent.type(screen.getByRole("textbox", { name: "Key your admin gave you" }), NEW);
    await userEvent.click(screen.getByRole("button", { name: "Pin the new key" }));
    await waitFor(() => expect(changed).toHaveBeenCalled());
    expect(sent).toContainEqual({
      cmd: "pool_setup_pin_again",
      args: { pool: "team", key: NEW },
    });
  });
});

describe("Send to your admin, and waiting (#4496)", () => {
  const WORDS = "Hello. I'd like to run purlis chats on our Kubernetes cluster.";
  const check = (state: string, also: { id: string; label: string }): PoolSetupStep => ({
    ...install([]),
    id: "check",
    name: "Check the cluster",
    state,
    undo: null,
    see: null,
    admin: WORDS,
    line: state === "waiting" ? "Waiting for your admin" : null,
    next: {
      id: "check-again",
      label: "Check again",
      does: "Asks again.",
      command: null,
      url: null,
    },
    also: { ...also, does: "Shows the words.", command: null, url: null },
  });

  it("keeps that the person asked their admin, from this project, on Send to your admin", async () => {
    const sent: { cmd: string; args: unknown }[] = [];
    mockIPC((cmd, args) => {
      sent.push({ cmd, args });
      return null;
    });
    const recheck = vi.fn();
    render(
      <CheckStep
        step={check("attention", { id: "send-to-admin", label: "Send to your admin" })}
        plane="/home/ada/team"
        onNext={() => {}}
        onRecheck={recheck}
      />,
    );
    expect(screen.queryByText(WORDS)).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Send to your admin" }));
    await waitFor(() => expect(recheck).toHaveBeenCalled());
    expect(sent).toContainEqual({ cmd: "pool_setup_ask_admin", args: { plane: "/home/ada/team" } });
  });

  it("shows the words to copy while it waits, and stops waiting on its own press", async () => {
    const sent: { cmd: string; args: unknown }[] = [];
    mockIPC((cmd, args) => {
      sent.push({ cmd, args });
      return null;
    });
    const recheck = vi.fn();
    render(
      <CheckStep
        step={check("waiting", { id: "stop-waiting", label: "Stop waiting" })}
        plane="/home/ada/team"
        onNext={() => {}}
        onRecheck={recheck}
      />,
    );
    expect(screen.getByText("Waiting for your admin")).toBeInTheDocument();
    expect(screen.getByText(WORDS)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Copy/ })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Stop waiting" }));
    await waitFor(() => expect(recheck).toHaveBeenCalled());
    expect(sent.map((one) => one.cmd)).toContain("pool_setup_stop_waiting");
  });
});
