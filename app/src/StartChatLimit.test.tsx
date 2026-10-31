// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { PoolLimitOffer } from "./bindings";
import { StartChatLimit } from "./StartChatLimit";

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
const apply = () => screen.getByRole("button", { name: "Save to the pool and start" });

describe("a start over a pool's limit", () => {
  it("offers the suggested value, and applies it", () => {
    const onApply = vi.fn();
    render(<StartChatLimit offer={offer()} onApply={onApply} />);

    expect((box() as HTMLInputElement).value).toBe("11264");
    fireEvent.click(apply());

    expect(onApply).toHaveBeenCalledWith([["profileFiles", 11264]]);
  });

  it("applies the number the person edited", () => {
    const onApply = vi.fn();
    render(<StartChatLimit offer={offer()} onApply={onApply} />);

    fireEvent.change(box(), { target: { value: "20000" } });
    fireEvent.click(apply());

    expect(onApply).toHaveBeenCalledWith([["profileFiles", 20000]]);
  });

  it("takes no value that is not a whole number above zero", () => {
    render(<StartChatLimit offer={offer()} onApply={vi.fn()} />);

    fireEvent.change(box(), { target: { value: "lots" } });

    expect(apply()).toHaveProperty("disabled", true);
  });

  it("offers nothing where no value would hold it", () => {
    render(<StartChatLimit offer={offer([{ ...hit, suggested: null }])} onApply={vi.fn()} />);

    expect(screen.queryByRole("button")).toBeNull();
  });
});
