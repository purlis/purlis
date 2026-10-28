// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { DEVICES_ID, catalogue, type Now } from "./actions";
import { rereadPools } from "./pools";
import { noTabs } from "./tabs";

/**
 * **The palette's Devices row** (#2052): offered once this machine keeps a pool, and not before,
 * so nothing about runners shows until the first pool is added (spec #2041 story 6).
 */

const IMAGE = `ghcr.io/purlis/purlis-runner@sha256:${"0".repeat(64)}`;
const DEFAULTS = { cpus: "2", memory_mib: 4096, processes: 1024, tmp_mib: 1024 };

function now(): Now {
  return { tabs: noTabs(), workspaces: [], needsYou: [], nameOf: (session) => String(session) };
}

/** This machine keeps `pools`, as the Pools store last read it. */
async function keeps(pools: string[]) {
  mockIPC((cmd) =>
    cmd === "docker_pools"
      ? {
          pools: pools.map((name) => ({
            name,
            image: IMAGE,
            limits: { ...DEFAULTS },
            suspend_after: "24h",
          })),
          defaults: { ...DEFAULTS },
          suspend_after: "24h",
        }
      : null,
  );
  await rereadPools();
}

afterEach(() => {
  clearMocks();
});

describe("the palette's Devices row", () => {
  it("is not offered while this machine keeps no pool", async () => {
    await keeps([]);
    expect(catalogue(now()).some((offer) => offer.id === DEVICES_ID)).toBe(false);
  });

  it("is offered once this machine keeps a pool, and opens the Devices tab", async () => {
    await keeps(["builds"]);
    const row = catalogue(now()).find((offer) => offer.id === DEVICES_ID);
    expect(row?.title).toBe("Devices");
    expect(row?.does).toEqual({
      verb: "openView",
      view: { from: null, view: "devices", key: "" },
      title: "Devices",
    });
  });
});
