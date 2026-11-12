// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PoolSetupPart, PoolSetupStep } from "../bindings";
import { InstallStep } from "./ClusterSteps";

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
  also: null,
  parts,
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
