// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PoolLimitOffer } from "./bindings";
import { RaiseLimit } from "./RaiseLimit";

const hit = {
  key: "profileFiles",
  plain: "Most a profile may send: files",
  unit: "files",
  in_force: 8192,
  needed: 9000,
  suggested: 11264 as number | null,
};
const offer = (hits = [hit]): PoolLimitOffer => ({ pool: "builds", hits });
const box = () => screen.getByRole("textbox", { name: /Most a profile may send: files/ });
const press = () =>
  screen.getByRole<HTMLButtonElement>("button", { name: "Save to the pool and start" });

function draw(given: PoolLimitOffer, onApply = vi.fn()) {
  render(<RaiseLimit offer={given} action="Save to the pool and start" onApply={onApply} />);
  return onApply;
}

beforeEach(() => {
  mockIPC((cmd, args) => {
    if (cmd !== "pool_limit_check") return null;
    const { typed } = args as { typed: string };
    if (!/^\d+$/.test(typed.trim())) throw "Most a profile may send: files is a whole number";
    if (Number(typed) > 65536) throw "Most a profile may send: files is at most 65536 files";
    return Number(typed);
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
});

describe("raising a limit where it was hit", () => {
  it("offers the suggestion as an editable number, and says what is needed", () => {
    const onApply = draw(offer());
    expect((box() as HTMLInputElement).value).toBe("11264");
    expect(box().getAttribute("aria-describedby")).toBeTruthy();
    expect(screen.getByText("It needs 9000 files. The limit is 8192 files.")).toBeTruthy();
    fireEvent.click(press());
    expect(onApply).toHaveBeenCalledWith([["profileFiles", 11264]]);
  });

  it("applies the value the core read from what was typed", async () => {
    const onApply = draw(offer());
    fireEvent.change(box(), { target: { value: " 12000 " } });
    // Until the core answers, the press can't send the older value.
    expect(press().disabled).toBe(true);
    await waitFor(() => expect(press().disabled).toBe(false));
    fireEvent.click(press());
    expect(onApply).toHaveBeenCalledWith([["profileFiles", 12000]]);
  });

  it("refuses what the core refuses, in its words, as it is typed", async () => {
    draw(offer());
    fireEvent.change(box(), { target: { value: "70000" } });
    expect(await screen.findByText(/is at most 65536 files/)).toBeTruthy();
    expect(press().disabled).toBe(true);
  });

  it("refuses a value too small to get through", async () => {
    draw(offer());
    fireEvent.change(box(), { target: { value: "8500" } });
    expect(await screen.findByText(/It needs 9000 files, so this would be refused/)).toBeTruthy();
    expect(press().disabled).toBe(true);
  });

  it("offers nothing where one limit no value holds", () => {
    const { container } = render(
      <RaiseLimit
        offer={offer([hit, { ...hit, key: "profileSize", suggested: null }])}
        action="Save to the pool and start"
        onApply={vi.fn()}
      />,
    );
    expect(container.firstChild).toBeNull();
  });
});
