// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { catalogued, type Offer } from "./actions";
import { CatalogueRows, useCatalogueRows } from "./catalogueRows";

const row = (id: string, title = id): Offer => ({
  id,
  title,
  available: true,
  reason: "",
  does: { verb: "tab.new" },
});

const notTabs = (id: string) => !id.startsWith("tab.");

describe("a reader of the catalogue's rows by id (FS-W2, #7910)", () => {
  it("is handed only the rows it reads, in the catalogue's order", () => {
    const store = new CatalogueRows(catalogued([row("a"), row("tab.select:1"), row("b")]));
    const { result } = renderHook(() => useCatalogueRows(store, notTabs));
    expect([...result.current.keys()]).toEqual(["a", "b"]);
  });

  it("is not drawn again for a catalogue made anew whose rows it reads say the same", () => {
    const store = new CatalogueRows(catalogued([row("a"), row("tab.select:1")]));
    let draws = 0;
    const { result } = renderHook(() => {
      draws += 1;
      return useCatalogueRows(store, notTabs);
    });
    const first = result.current;
    act(() => store.hold(catalogued([row("a"), row("tab.select:1", "Switch to tab 1")])));
    expect(result.current).toBe(first);
    expect(draws).toBe(1);
  });

  it("is drawn with the new rows when one it reads moves, comes or goes", () => {
    const store = new CatalogueRows(catalogued([row("a")]));
    const { result } = renderHook(() => useCatalogueRows(store, notTabs));
    act(() => store.hold(catalogued([row("a", "Renamed")])));
    expect(result.current.get("a")?.title).toBe("Renamed");
    act(() => store.hold(catalogued([row("a", "Renamed"), row("b")])));
    expect([...result.current.keys()]).toEqual(["a", "b"]);
    act(() => store.hold(catalogued([row("b")])));
    expect([...result.current.keys()]).toEqual(["b"]);
  });

  it("is handed the rows it reads now when what it reads changes", () => {
    const store = new CatalogueRows(catalogued([row("a"), row("b")]));
    const { result, rerender } = renderHook(
      ({ id }) => useCatalogueRows(store, (at) => at === id),
      {
        initialProps: { id: "a" },
      },
    );
    rerender({ id: "b" });
    expect([...result.current.keys()]).toEqual(["b"]);
  });
});
