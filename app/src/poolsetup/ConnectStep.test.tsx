// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PoolSetupShown, PoolSetupStep } from "../bindings";
import { ConnectStep, overOf, routeOf, tcpRouteOf, verdictsOf } from "./ConnectStep";

const FINGERPRINT = "SHA256:dmeCC9oOb+c7VYVBSUAdgQchZHZQyXKWgKnGQxfh2rA";
const CA =
  "7A:3F:91:0C:5E:D2:84:B6:19:EE:47:2A:C0:6D:F3:58:91:0B:7C:AA:E4:32:6F:1D:88:C5:09:B7:2E:64:D1:3F";

/** A tcp:// machine, reached over Docker's API over TLS (#6267). */
const tlsMachine = (more: Partial<NonNullable<PoolSetupStep["machine"]>> = {}) => ({
  route: "tcp://lab:2376",
  host: "lab",
  key: null,
  known: null,
  trusted: null,
  now: null,
  engine: null,
  over: "tls",
  tls: null,
  ca: null,
  trusted_ca: null,
  ca_now: null,
  ...more,
});

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
    over: "ssh",
    tls: null,
    ca: null,
    trusted_ca: null,
    ca_now: null,
  },
  cause: null,
  also: null,
  parts: [],
  undo: null,
  ...more,
});

const shown = (one: PoolSetupStep): PoolSetupShown => ({
  kind: "remote-docker",
  pool: null,
  name: null,
  at: "connect",
  noticed: false,
  others: [],
  recommended: "build-box",
  contexts: { names: [], recommended: null, chosen: null, pools: [] },
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

  it("picks the machine kept once the SSH settings that name it are read", async () => {
    core({ pool_setup_ssh_hosts: () => ["gpu", "build-box"] });
    draw(
      step({
        machine: {
          route: "ssh://build-box",
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
        },
      }),
    );
    await waitFor(() => expect(screen.getByRole("radio", { name: /build-box/ })).toBeChecked());
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
          over: "ssh",
          tls: null,
          ca: null,
          trusted_ca: null,
          ca_now: null,
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

  it("checks a made pool's machine again now when Check again is pressed, and shows what it read", async () => {
    const read = shown(step({ state: "done" }));
    const calls = core({ pool_setup_check_again: () => read });
    const failing = step({
      state: "attention",
      cause: "engine-silent",
      findings: ["Docker didn't answer on build-box."],
      next: null,
    });
    const props = { onShown: vi.fn(), changed: vi.fn(), onNext: vi.fn() };
    render(
      <ConnectStep
        step={failing}
        shown={{ ...shown(failing), pool: "build" }}
        plane="p"
        {...props}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Check again" }));
    await waitFor(() => expect(props.onShown).toHaveBeenCalledWith(read));
    expect(calls).toContainEqual({ cmd: "pool_setup_check_again", args: { plane: "p" } });
    expect(props.changed).not.toHaveBeenCalled();
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

  it("offers SSH first, and over TLS keeps the machine as a tcp:// route, contacting nothing", async () => {
    const calls = core();
    draw(step({ machine: null }));
    const ssh = await screen.findByRole("radio", { name: /SSH/ });
    expect(ssh).toBeChecked();
    await userEvent.click(screen.getByRole("radio", { name: /Docker's API over TLS/ }));
    await userEvent.type(
      await screen.findByRole("textbox", { name: "The machine's address" }),
      "lab:2376",
    );
    await waitFor(() =>
      expect(calls).toContainEqual({
        cmd: "pool_setup_machine",
        args: { route: "tcp://lab:2376" },
      }),
    );
    expect(calls.some((call) => call.cmd === "pool_setup_connect")).toBe(false);
  });

  it("keeps the TLS files once all are named, the key from a vault by its names alone", async () => {
    const calls = core({ pool_setup_tls_files: () => shown(step({})) });
    draw(step({ machine: tlsMachine(), cause: "plain-tcp", state: "attention" }));
    // A tcp:// machine without its files is plain TCP, refused, until TLS is chosen.
    await userEvent.click(await screen.findByRole("button", { name: "Use TLS instead" }));
    await userEvent.type(
      await screen.findByRole("textbox", { name: "Authority (CA) file" }),
      "/home/me/ca.pem",
    );
    await userEvent.type(screen.getByRole("textbox", { name: "Certificate" }), "/home/me/cert.pem");
    await userEvent.click(screen.getByRole("radio", { name: /In a vault entry/ }));
    await userEvent.type(screen.getByRole("textbox", { name: "Vault" }), "docker-box");
    await userEvent.type(screen.getByRole("textbox", { name: "Key in the vault" }), "TLS_KEY");
    await waitFor(() =>
      expect(calls).toContainEqual({
        cmd: "pool_setup_tls_files",
        args: {
          plane: "p",
          ca: "/home/me/ca.pem",
          cert: "/home/me/cert.pem",
          key: null,
          keyVault: "docker-box",
          keyName: "TLS_KEY",
        },
      }),
    );
    // Connect to waits, and isn't named for the machine, until what is typed is what is kept.
    expect(screen.getByRole("button", { name: "Connect" })).toBeDisabled();
  });

  it("connects to a tcp:// machine once its files are kept", async () => {
    const calls = core({ pool_setup_connect: () => shown(step({})) });
    draw(
      step({
        machine: tlsMachine({
          tls: {
            ca: "/ca.pem",
            cert: "/cert.pem",
            key: "/key.pem",
            key_vault: null,
            key_name: null,
          },
        }),
        next: {
          id: "connect-machine",
          label: "Connect to lab",
          does: "Reads the CA file.",
          command: null,
          url: null,
        },
      }),
    );
    await userEvent.click(await screen.findByRole("button", { name: "Connect to lab" }));
    await waitFor(() =>
      expect(calls).toContainEqual({ cmd: "pool_setup_connect", args: { plane: "p" } }),
    );
  });

  it("trusts a tcp:// machine by the fingerprint of the authority it showed", async () => {
    const calls = core();
    draw(
      step({
        next: {
          id: "trust-machine",
          label: "Trust this machine",
          does: "Keeps this CA for the pool.",
          command: null,
          url: null,
        },
        machine: tlsMachine({
          tls: {
            ca: "/ca.pem",
            cert: "/cert.pem",
            key: "/key.pem",
            key_vault: null,
            key_name: null,
          },
          ca: { fingerprint: CA, subject: "CN=lab CA", file: "/ca.pem" },
        }),
      }),
    );
    expect(await screen.findByText("CN=lab CA")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Trust this machine" }));
    await waitFor(() =>
      expect(calls).toContainEqual({
        cmd: "pool_setup_trust",
        args: { plane: "p", fingerprint: CA },
      }),
    );
  });

  it("checks a TLS machine's certificate in place of a sign-in", () => {
    expect(verdictsOf("tls-wrong-name", false, "tls")).toEqual([
      "ok",
      "ok",
      "ok",
      "missing",
      "todo",
      "todo",
    ]);
    expect(verdictsOf("tls-client-refused", false, "tls")[3]).toBe("missing");
  });

  it("spells a TLS machine as tcp:// and tells the routes apart", () => {
    expect(tcpRouteOf(" lab:2376 ")).toBe("tcp://lab:2376");
    expect(tcpRouteOf("tcp://lab")).toBe("tcp://lab");
    const files = { ca: "/ca", cert: "/c", key: "/k", key_vault: null, key_name: null };
    expect(overOf(tlsMachine({ tls: files }))).toBe("tls");
    expect(overOf(tlsMachine()), "plain TCP, no files").toBe("ssh");
    expect(overOf(tlsMachine({ route: "ssh://lab", tls: null }))).toBe("ssh");
    expect(overOf(null)).toBe("ssh");
  });
});
