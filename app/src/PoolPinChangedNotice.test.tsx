// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { POOL_PIN_CHANGED, type PoolPinChanged } from "./bindings";
import { PoolPinChangedNotice } from "./PoolPinChangedNotice";
import { removePool } from "./pools";

const KEY_CHANGED: PoolPinChanged = {
  pool: "builds",
  host: "build-box",
  what: "key",
  pinned: "SHA256:dmeCC9oOb+c7VYVBSUAdgQchZHZQyXKWgKnGQxfh2rA",
  now: "SHA256:tGq2HWbnxdQ1Q3yxD6vk2u0bxs6WtJ5gT3f7a6Fe8E0",
  title: "Pool builds: build-box shows a different key",
  said: "Pool builds: build-box shows a different key than the one you trusted (SHA256:tGq2HWbnxdQ1Q3yxD6vk2u0bxs6WtJ5gT3f7a6Fe8E0 now, SHA256:dmeCC9oOb+c7VYVBSUAdgQchZHZQyXKWgKnGQxfh2rA trusted). This happens when the machine is reinstalled, and can mean something else is answering in its place. purlis starts no runner, opens no new link and sends no vault value there until you check its key and trust it again. Links already open stay open.",
};

function core(
  found: () => PoolPinChanged[],
  left = "Runner builds's container stays on build-box.",
  removing: (name: string) => Promise<string> | string = () => left,
) {
  const calls: { cmd: string; args: unknown }[] = [];
  mockIPC(
    (cmd, args) => {
      calls.push({ cmd, args });
      if (cmd === "pools_pin_changed") return found();
      if (cmd === "remove_docker_pool") return removing((args as { name: string }).name);
      if (cmd === "pool_named_by") return [];
      if (cmd === "pool_held_runners") return [];
      if (cmd === "docker_pools") return { pools: [], clusters: [] };
      return null;
    },
    { shouldMockEvents: true },
  );
  return calls;
}

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("the notice that a pool's pin no longer holds", () => {
  it("draws nothing where no pool's pin changed", async () => {
    const calls = core(() => []);
    render(<PoolPinChangedNotice onCheckAndTrust={() => {}} />);
    await waitFor(() => expect(calls.some((call) => call.cmd === "pools_pin_changed")).toBe(true));
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("names the pool, what changed and both fingerprints, keyed by its cause", async () => {
    core(() => [KEY_CHANGED]);
    render(<PoolPinChangedNotice onCheckAndTrust={() => {}} />);
    const notice = await screen.findByRole("status");
    expect(notice.textContent).toContain(KEY_CHANGED.said);
    expect(notice.closest("[data-cause]")?.getAttribute("data-cause")).toBe(
      "pool-pin-changed:builds",
    );
  });

  it("offers Check and trust again for the pool, and Remove pool, never only a command", async () => {
    core(() => [KEY_CHANGED]);
    const onCheckAndTrust = vi.fn();
    render(<PoolPinChangedNotice onCheckAndTrust={onCheckAndTrust} />);
    await userEvent.click(await screen.findByRole("button", { name: "Check and trust again" }));
    expect(onCheckAndTrust).toHaveBeenCalledWith("builds");
    expect(screen.getByRole("button", { name: "Remove pool…" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Copy command/ })).toBeNull();
  });

  it("asks before Remove pool forgets it here, then says what stays on the machine", async () => {
    const calls = core(() => [KEY_CHANGED], "Runner builds's container stays on build-box.");
    render(<PoolPinChangedNotice onCheckAndTrust={() => {}} />);
    await userEvent.click(await screen.findByRole("button", { name: "Remove pool…" }));
    const asked = await screen.findByRole("alertdialog", { name: "Remove pool builds?" });
    expect(asked.textContent).toMatch(/purlis sends nothing to build-box/);
    expect(calls.some((call) => call.cmd === "remove_docker_pool")).toBe(false);
    await userEvent.click(within(asked).getByRole("button", { name: "Remove pool" }));
    await waitFor(() =>
      expect(calls.find((call) => call.cmd === "remove_docker_pool")?.args).toEqual({
        name: "builds",
        elsewhere: true,
        discarding: [],
      }),
    );
    expect(await screen.findByText(/Runner builds's container stays on build-box/)).toBeVisible();
  });

  it("keeps the card and says why where Remove pool is refused, never that the pool is forgotten", async () => {
    // Another pool's removal is still going, so this one is refused with nothing removed.
    let finish: (left: string) => void = () => {};
    const calls = core(
      () => [KEY_CHANGED],
      "",
      (name) => (name === "other" ? new Promise<string>((done) => (finish = done)) : "stays"),
    );
    const other = removePool("other");
    render(<PoolPinChangedNotice onCheckAndTrust={() => {}} />);
    await userEvent.click(await screen.findByRole("button", { name: "Remove pool…" }));
    const asked = await screen.findByRole("alertdialog", { name: "Remove pool builds?" });
    await userEvent.click(within(asked).getByRole("button", { name: "Remove pool" }));
    const card = await screen.findByText(
      /purlis is removing pool other: remove builds once it is gone/,
    );
    expect(card.closest("[data-cause]")?.getAttribute("data-cause")).toBe(
      "pool-pin-changed:builds",
    );
    expect(screen.queryByText(/forgotten/)).toBeNull();
    expect(
      calls.filter((call) => call.cmd === "remove_docker_pool").map((call) => call.args),
    ).toEqual([{ name: "other", elsewhere: false, discarding: [] }]);
    finish("");
    await other;
  });

  it("keeps the card with the core's own words where the core refuses Remove pool", async () => {
    core(
      () => [KEY_CHANGED],
      "",
      () => Promise.reject("Pool builds isn't set up here any more, so nothing was removed."),
    );
    render(<PoolPinChangedNotice onCheckAndTrust={() => {}} />);
    await userEvent.click(await screen.findByRole("button", { name: "Remove pool…" }));
    const asked = await screen.findByRole("alertdialog", { name: "Remove pool builds?" });
    await userEvent.click(within(asked).getByRole("button", { name: "Remove pool" }));
    const card = await screen.findByText(
      /Pool builds isn't set up here any more, so nothing was removed/,
    );
    expect(card.closest("[data-cause]")?.getAttribute("data-cause")).toBe(
      "pool-pin-changed:builds",
    );
    expect(screen.queryByText(/forgotten/)).toBeNull();
  });

  it("reads again when a pool's pin is found changed or holds again", async () => {
    let found: PoolPinChanged[] = [KEY_CHANGED];
    core(() => found);
    render(<PoolPinChangedNotice onCheckAndTrust={() => {}} />);
    await screen.findByRole("status");
    found = [];
    await emit(POOL_PIN_CHANGED);
    await waitFor(() => expect(screen.queryByRole("status")).toBeNull());
  });
});
