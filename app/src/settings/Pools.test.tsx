// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "../App";
import type { PoolLink, PoolRow, PoolsShown } from "../bindings";
import { forgetThisLaunch } from "../regions";
import { GLOBAL } from "../windowprefs";

/**
 * **Settings › You › This machine › Pools** (#2679), in the whole window: nothing about pools
 * shows until one is added; Add a Docker pool… in the palette adds the first, with the limits
 * typed and the defaults for the rest; and a pool whose runner answered with another key has a
 * Notice whose Remove removes it.
 */

vi.mock("../SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const IMAGE = `ghcr.io/purlis/purlis-runner@sha256:${"0".repeat(64)}`;
const DEFAULTS = { cpus: "2", memory_mib: 4096, processes: 1024, tmp_mib: 1024 };

const a_pool = (name: string): PoolRow => ({ name, image: IMAGE, limits: { ...DEFAULTS } });

let pools: PoolRow[];
let link: PoolLink;
let added: unknown[];
let removed: unknown[];

beforeEach(() => {
  forgetThisLaunch();
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: { path: "/home/op/.config/charter/layout.json", found: false, document: null },
    theme: { path: "", found: false, document: null, trouble: null },
  };
  pools = [];
  link = { state: "linked" };
  added = [];
  removed = [];
  mockIPC(
    (cmd, args) => {
      const a = (args ?? {}) as Record<string, unknown>;
      if (cmd === "plane_at_launch") return { plane: null, from: null, why: "no plane here" };
      if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
      if (cmd === "planes_to_restore") return { windows: [], dropped: [], gone: [] };
      if (cmd === "this_machine") return { projects: [], dropped: [], forgetful: null };
      if (cmd === "update_channel") return "stable";
      if (cmd === "docker_pools")
        return { pools: structuredClone(pools), defaults: { ...DEFAULTS } } satisfies PoolsShown;
      if (cmd === "docker_pool_link") return link;
      if (cmd === "add_docker_pool") {
        added.push(a.adding);
        const row = a_pool((a.adding as { name: string }).name);
        pools = [...pools, row];
        return row;
      }
      if (cmd === "remove_docker_pool") {
        removed.push(a.name);
        pools = pools.filter((one) => one.name !== a.name);
        return null;
      }
      return null;
    },
    { shouldMockEvents: true },
  );
});
afterEach(() => {
  cleanup();
  clearMocks();
  Reflect.deleteProperty(globalThis, GLOBAL);
});

async function palette(row: string) {
  await userEvent.keyboard("{F2}");
  await screen.findByRole("dialog", { name: "Command palette" });
  await userEvent.keyboard(row);
  await userEvent.keyboard("{Enter}");
}

async function yourSettings() {
  await palette("Your settings…");
  return screen.findByRole("navigation", { name: "Groups" });
}

describe("You › This machine › Pools", () => {
  it("shows nothing about pools while this machine keeps none", async () => {
    render(<App />);
    const nav = await yourSettings();
    await within(nav).findByRole("button", { name: "Browsers" });
    expect(within(nav).queryByRole("button", { name: "Pools" })).toBeNull();
  });

  it("adds the first pool from the palette, with the limits typed and the defaults for the rest", async () => {
    render(<App />);
    await palette("Add a Docker pool…");
    const dialog = await screen.findByRole("dialog", { name: "Add a Docker pool" });
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Name" }), "builds");
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Image" }), IMAGE);
    await userEvent.type(within(dialog).getByRole("textbox", { name: "CPUs" }), "1.5");
    await userEvent.click(within(dialog).getByRole("button", { name: "Add pool" }));

    await waitFor(() =>
      expect(added).toEqual([
        {
          name: "builds",
          image: IMAGE,
          cpus: "1.5",
          memory_mib: null,
          processes: null,
          tmp_mib: null,
        },
      ]),
    );
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Add a Docker pool" })).toBeNull(),
    );
    const nav = await yourSettings();
    expect(await within(nav).findByRole("button", { name: "Pools" })).toBeInTheDocument();
  });

  it("says a runner whose key changed in a Notice, whose Remove removes the pool", async () => {
    pools = [a_pool("builds")];
    link = { state: "key_changed" };
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    const notice = await waitFor(() => {
      const found = group.querySelector('[data-cause="pool-key-changed:builds"]');
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    expect(notice).toHaveTextContent(/answered with another key/);

    await userEvent.click(within(notice).getByRole("button", { name: "Remove" }));

    await waitFor(() => expect(removed).toEqual(["builds"]));
    // With none left, nothing about pools shows.
    await waitFor(() => expect(within(nav).queryByRole("button", { name: "Pools" })).toBeNull());
  });
});
