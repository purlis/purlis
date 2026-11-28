// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, renderHook } from "@testing-library/react";
import { catalogued, type Offer } from "./actions";
import { useBesideRows } from "./besideRows";

afterEach(cleanup);

const row = (id: string): Offer => ({ id, title: id, available: true, reason: "" }) as Offer;

describe("the rows the views beside the tabs read", () => {
  it("are the same map while a catalogue made anew says the same, in the same order", () => {
    const { result, rerender } = renderHook(({ rows }) => useBesideRows(catalogued(rows)), {
      initialProps: { rows: [row("chat.new"), row("charter.quit")] },
    });
    const first = result.current;
    rerender({ rows: [row("chat.new"), row("charter.quit")] });
    expect(result.current).toBe(first);
  });

  it("move when the same rows come in a new order, since the views read them in that order", () => {
    const { result, rerender } = renderHook(({ rows }) => useBesideRows(catalogued(rows)), {
      initialProps: { rows: [row("chat.new"), row("charter.quit")] },
    });
    rerender({ rows: [row("charter.quit"), row("chat.new")] });
    expect([...result.current.keys()]).toEqual(["charter.quit", "chat.new"]);
  });
});
