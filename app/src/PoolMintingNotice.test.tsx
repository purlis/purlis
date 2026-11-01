// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { POOL_MINTS_WITHOUT_ASKING } from "./bindings";
import { PoolMintingNotice } from "./PoolMintingNotice";

function core(due: () => string[]): { asked: string[]; putAway: unknown[] } {
  const asked: string[] = [];
  const putAway: unknown[] = [];
  mockIPC(
    (cmd, args) => {
      asked.push(cmd);
      if (cmd === "pools_minting_without_asking") return due();
      if (cmd === "put_away_pool_minting_notice") putAway.push(args);
      return null;
    },
    { shouldMockEvents: true },
  );
  return { asked, putAway };
}

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("the notice that a pool began minting without asking", () => {
  it("draws nothing where no pool changed", async () => {
    const { asked } = core(() => []);
    render(<PoolMintingNotice onOpenPools={() => {}} />);
    await waitFor(() => expect(asked).toContain("pools_minting_without_asking"));
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("says what changed and who controls it, once for each pool, and offers Pools settings", async () => {
    core(() => ["team"]);
    const onOpenPools = vi.fn();
    render(<PoolMintingNotice onOpenPools={onOpenPools} />);
    const notice = await screen.findByRole("status");
    expect(notice.textContent).toMatch(/pool team no longer ask you before each GitHub App token/);
    expect(notice.textContent).toMatch(/the admin controls this setting/);
    expect(screen.getAllByRole("status")).toHaveLength(1);
    await userEvent.click(screen.getByRole("button", { name: "Open Pools settings" }));
    expect(onOpenPools).toHaveBeenCalled();
  });

  it("is put away in the core when dismissed", async () => {
    const { putAway } = core(() => ["team"]);
    render(<PoolMintingNotice onOpenPools={() => {}} />);
    await screen.findByRole("status");
    await userEvent.click(screen.getByRole("button", { name: /Dismiss/ }));
    await waitFor(() => expect(screen.queryByRole("status")).toBeNull());
    expect(putAway).toEqual([{ name: "team" }]);
  });

  it("appears when a link finds the change", async () => {
    let due: string[] = [];
    core(() => due);
    render(<PoolMintingNotice onOpenPools={() => {}} />);
    await waitFor(() => expect(screen.queryByRole("status")).toBeNull());
    due = ["team"];
    await emit(POOL_MINTS_WITHOUT_ASKING);
    expect((await screen.findByRole("status")).textContent).toMatch(/pool team/);
  });
});
