// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PoolSetupShown, PoolSetupStep } from "../bindings";
import { ConnectStep, routeOf, verdictsOf } from "./ConnectStep";

const FINGERPRINT = "SHA256:dmeCC9oOb+c7VYVBSUAdgQchZHZQyXKWgKnGQxfh2rA";

const step = (more: Partial<PoolSetupStep>): PoolSetupStep => ({
  id: "connect",
  name: "Connect",
  state: "current",
  line: null,
  findings: [],
  next: {
    id: "connect-machine",
    label: "Connect to build-box",
    does: "Reads the key the machine shows, and keeps nothing.",
    command: null,
    url: null,
  },
  skippable: false,
  see: null,
  checks: [],
  other: null,
  machine: {
    route: "ssh://sam@build-box",
    host: "build-box",
    key: null,
    known: null,
    trusted: null,
    now: null,
    engine: null,
  },
  cause: null,
  ...more,
});

const shown = (one: PoolSetupStep): PoolSetupShown => ({
  kind: "remote-docker",
  pool: null,
  name: null,
  at: "connect",
  noticed: false,
  others: [],
  steps: [one],
});

function core(more: Record<string, (args: unknown) => unknown> = {}) {
  const calls: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    if (cmd in more) return more[cmd]?.(args);
    if (cmd === "pool_setup_ssh_hosts") return [];
    return null;
  });
  return calls;
}

const draw = (one: PoolSetupStep) => {
  const props = { onShown: vi.fn(), changed: vi.fn(), onNext: vi.fn() };
  render(<ConnectStep step={one} shown={shown(one)} plane="p" {...props} />);
  return props;
};

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("ConnectStep", () => {
  it("contacts the machine only when Connect to it is pressed, and shows what the core read", async () => {
    const met = shown(step({ next: null }));
    const calls = core({ pool_setup_connect: () => met });
    const props = draw(step({}));
    await screen.findByRole("textbox", { name: "Its address" });
    expect(calls.some((call) => call.cmd === "pool_setup_connect")).toBe(false);
    await userEvent.click(screen.getByRole("button", { name: "Connect to build-box" }));
    await waitFor(() => expect(props.onShown).toHaveBeenCalledWith(met));
  });

  it("keeps a machine typed as Docker's ssh:// route, and contacts nothing", async () => {
    const calls = core();
    const props = draw(step({ machine: null }));
    await userEvent.type(await screen.findByRole("textbox", { name: "Its address" }), "me@gpu");
    await waitFor(() =>
      expect(calls).toContainEqual({ cmd: "pool_setup_machine", args: { route: "ssh://me@gpu" } }),
    );
    expect(props.changed).toHaveBeenCalled();
    expect(calls.some((call) => call.cmd === "pool_setup_connect")).toBe(false);
  });

  it("trusts exactly the fingerprint it showed", async () => {
    const calls = core({ pool_setup_trust: () => shown(step({ state: "done" })) });
    draw(
      step({
        next: {
          id: "trust-machine",
          label: "Trust this machine",
          does: "Keeps this key for the pool.",
          command: null,
          url: null,
        },
        machine: {
          route: "ssh://sam@build-box",
          host: "build-box",
          key: { algorithm: "ssh-ed25519", fingerprint: FINGERPRINT },
          known: null,
          trusted: null,
          now: null,
          engine: null,
        },
      }),
    );
    await userEvent.click(screen.getByRole("button", { name: "Trust this machine" }));
    await waitFor(() =>
      expect(calls).toContainEqual({
        cmd: "pool_setup_trust",
        args: { plane: "p", fingerprint: FINGERPRINT },
      }),
    );
  });

  it("marks the failing check, those before it passed and those after not reached", () => {
    expect(verdictsOf("not-allowed", false)).toEqual([
      "ok",
      "ok",
      "ok",
      "ok",
      "ok",
      "missing",
      "todo",
    ]);
    expect(verdictsOf(null, true).every((one) => one === "ok")).toBe(true);
    expect(verdictsOf("no-such-name", false)[0]).toBe("missing");
  });

  it("spells a host as Docker's ssh:// route", () => {
    expect(routeOf(" sam@build-box ")).toBe("ssh://sam@build-box");
    expect(routeOf("ssh://build-box")).toBe("ssh://build-box");
    expect(routeOf("")).toBe("");
  });
});
