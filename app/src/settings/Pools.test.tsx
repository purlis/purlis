// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "../App";
import type { PoolLink, PoolRow, PoolVersion, PoolsShown } from "../bindings";
import { forgetThisLaunch } from "../regions";
import { GLOBAL } from "../windowprefs";

/**
 * **Settings › You › This machine › Pools** (#2679), in the whole window: nothing about pools
 * shows until one is added; Add a Docker pool… in the palette adds the first, with the limits
 * typed and the defaults for the rest; and a pool whose runner answered with another key has a
 * Notice whose Remove removes it. A runner of another version than this machine's says so, and
 * what follows (#2749).
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
let version: PoolVersion;
let added: unknown[];
let removed: unknown[];
let notHeld: { root: string; name: string; why: string; pools: string[] }[];

beforeEach(() => {
  forgetThisLaunch();
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: { path: "/home/op/.config/charter/layout.json", found: false, document: null },
    theme: { path: "", found: false, document: null, trouble: null },
  };
  pools = [];
  link = { state: "linked" };
  version = {
    runner: "0.4.2",
    why: null,
    desktop: "0.4.2",
    follows: true,
    draining: 0,
    named: null,
    waiting: 0,
  };
  added = [];
  removed = [];
  notHeld = [];
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
      if (cmd === "docker_pool_version") return version;
      if (cmd === "projects_not_held") return structuredClone(notHeld);
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

  it("asks before the row's Remove removes a pool", async () => {
    pools = [a_pool("builds")];
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    await userEvent.click(await within(group).findByRole("button", { name: "Remove pool builds" }));
    const asked = await screen.findByRole("alertdialog", { name: "Remove pool builds?" });
    expect(removed).toEqual([]);
    await userEvent.click(within(asked).getByRole("button", { name: "Remove pool" }));
    await waitFor(() => expect(removed).toEqual(["builds"]));
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
    // It asks first, and Cancel changes nothing.
    let asked = await screen.findByRole("alertdialog", { name: "Remove pool builds?" });
    expect(asked).toHaveTextContent(/There is no undo/);
    await userEvent.click(within(asked).getByRole("button", { name: "Cancel" }));
    await waitFor(() =>
      expect(screen.queryByRole("alertdialog", { name: "Remove pool builds?" })).toBeNull(),
    );
    expect(removed).toEqual([]);

    await userEvent.click(within(notice).getByRole("button", { name: "Remove" }));
    asked = await screen.findByRole("alertdialog", { name: "Remove pool builds?" });
    await userEvent.click(within(asked).getByRole("button", { name: "Remove pool" }));

    await waitFor(() => expect(removed).toEqual(["builds"]));
    // With none left, nothing about pools shows.
    await waitFor(() => expect(within(nav).queryByRole("button", { name: "Pools" })).toBeNull());
  });

  it("says a suspended runner is suspended, not that it would not link (#2051)", async () => {
    pools = [a_pool("builds")];
    link = { state: "suspended" };
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    await within(group).findByText("suspended");
    expect(group).not.toHaveTextContent(/not linked/);
  });

  it("says nothing of a runner of this machine's version", async () => {
    pools = [a_pool("builds")];
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    await within(group).findByText("linked");
    expect(group).not.toHaveTextContent(/runs purlis/);
  });

  it("says a runner of purlis's own image is behind, and that its older runner drains", async () => {
    pools = [a_pool("builds")];
    version = {
      runner: "0.4.2-dev.16",
      why: null,
      desktop: "0.4.2-dev.17",
      follows: true,
      draining: 1,
      named: null,
      waiting: 2,
    };
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    expect(
      await within(group).findByText(
        /Its runner runs purlis 0\.4\.2-dev\.16, and this machine runs 0\.4\.2-dev\.17\. New chats start on a runner of 0\.4\.2-dev\.17 once that version's runner image is published\./,
      ),
    ).toHaveTextContent(/1 older runner keeps its chats until they end, then purlis removes it\./);
    expect(
      within(group).getByText(
        /2 chats still run there: end them to let purlis remove the older runners\./,
      ),
    ).toBeInTheDocument();
  });

  it("tells you to build your own image again when its runner is behind", async () => {
    pools = [a_pool("builds")];
    version = {
      runner: "0.4.1",
      why: null,
      desktop: "0.4.2",
      follows: false,
      draining: 0,
      named: null,
      waiting: 0,
    };
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    expect(
      await within(group).findByText(
        /Build your image from purlis 0\.4\.2, then add the pool again\./,
      ),
    ).toBeInTheDocument();
  });

  it("marks a project the runner would not hold on its pool's row, in the runner's words", async () => {
    pools = [a_pool("builds")];
    notHeld = [
      {
        root: "/projects/alpha",
        name: "alpha",
        why: "that folder is not there",
        pools: ["builds"],
      },
    ];
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    const notice = await waitFor(() => {
      const found = group.querySelector('[data-cause="project-not-held:/projects/alpha"]');
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    expect(notice).toHaveTextContent(
      "The runner that alpha's chats run on does not hold alpha, so no chat starts or answers there.",
    );
    // On the row of the pool its chats run on, the runner's words set apart.
    expect(notice.closest("li")).toHaveTextContent("builds");
    expect(notice.querySelector("bdi")).toHaveTextContent("that folder is not there");

    await userEvent.click(within(notice).getByRole("button", { name: "Dismiss" }));
    await waitFor(() =>
      expect(group.querySelector('[data-cause="project-not-held:/projects/alpha"]')).toBeNull(),
    );
  });

  it("says a pool does not move to this version when its release has no runner image", async () => {
    pools = [a_pool("builds")];
    version = {
      runner: "0.4.2-dev.16",
      why: null,
      desktop: "0.4.2-dev.17",
      follows: true,
      draining: 0,
      named: false,
      waiting: 0,
    };
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    expect(
      await within(group).findByText(
        /The release of 0\.4\.2-dev\.17 has no runner image, so the pool does not move to 0\.4\.2-dev\.17 and stays on its runner\./,
      ),
    ).toBeInTheDocument();
  });

  it("keeps the old wording when purlis has said nothing of this version's release", async () => {
    pools = [a_pool("builds")];
    version = {
      runner: "0.4.2-dev.16",
      why: null,
      desktop: "0.4.2-dev.17",
      follows: true,
      draining: 0,
      named: null,
      waiting: 0,
    };
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    expect(
      await within(group).findByText(
        /New chats start on a runner of 0\.4\.2-dev\.17 once that version's runner image is published\./,
      ),
    ).toBeInTheDocument();
    expect(group).not.toHaveTextContent(/has no runner image/);
  });
});
