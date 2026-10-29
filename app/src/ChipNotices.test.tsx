// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { OUTSIDE, type Said } from "./actions";
import { mostUrgent, useChipNotices } from "./ChipNotices";
import { reposCloned, reposCloning, reposNotCloned } from "./actionResult";
import type { KeptResult } from "./keptResults";
import type { Chip } from "./noticePlace";

/**
 * **Which chip's card is up** (#3648's review): a card comes up for a new result about its
 * workspace, in the project it was said in, and never in another project, nor for a result
 * placed anywhere else.
 */

const A = "/p/a";
const B = "/p/b";
const CHIPS: readonly Chip[] = [
  { workspace: OUTSIDE, look: "root" },
  { workspace: "docs", look: "pinned" },
];
const made = (workspace: string): Said => ({
  from: "workspace.create",
  refused: false,
  title: `Workspace ${workspace} is ready`,
  words: "",
  workspace,
});
const refused: Said = { from: "workspace.pin", refused: true, words: "Not pinned." };

type Props = {
  said: Said | undefined;
  kept: readonly KeptResult[];
  plane: string;
  chips?: readonly Chip[];
};

function hook(first: Props) {
  return renderHook(
    ({ said, kept, plane, chips = CHIPS }: Props) =>
      useChipNotices({
        said,
        kept,
        plane,
        chips,
        drop: () => undefined,
        onSettings: () => undefined,
      }),
    { initialProps: first },
  );
}

describe("a chip's card comes up", () => {
  it("for a new result about its workspace, in the project it was said in", () => {
    const docs = made("docs");
    const { result, rerender } = hook({ said: undefined, kept: [], plane: A });
    rerender({ said: docs, kept: [{ id: 1, said: docs, plane: A }], plane: A });
    expect(result.current.open).toBe("docs");
  });

  it("never in another project brought to the front, whatever that project keeps", () => {
    const docs = made("docs");
    const behind = made("docs");
    const kept: KeptResult[] = [
      { id: 1, said: docs, plane: A },
      { id: 2, said: behind, plane: B },
    ];
    const { result, rerender } = hook({ said: undefined, kept: [], plane: A });
    rerender({ said: docs, kept, plane: A });
    expect(result.current.open).toBe("docs");
    // B in front, its newest result the one it said while behind, with no press anywhere: its
    // docs notice stays behind its dot.
    rerender({ said: behind, kept, plane: B });
    expect(result.current.on.get("docs")).toHaveLength(1);
    expect(result.current.open).toBeUndefined();
  });

  it("goes down when the newest result is placed anywhere but a chip", () => {
    const docs = made("docs");
    const kept: KeptResult[] = [{ id: 1, said: docs, plane: A }];
    const { result, rerender } = hook({ said: undefined, kept: [], plane: A });
    rerender({ said: docs, kept, plane: A });
    expect(result.current.open).toBe("docs");
    rerender({ said: refused, kept: [{ id: 2, said: refused, plane: A }, ...kept], plane: A });
    expect(result.current.open).toBeUndefined();
  });

  it("never for a workspace that lost its chip and came back (#3648's review)", () => {
    const docs = made("docs");
    const kept: KeptResult[] = [{ id: 1, said: docs, plane: A }];
    const { result, rerender } = hook({ said: undefined, kept: [], plane: A });
    rerender({ said: docs, kept, plane: A });
    expect(result.current.open).toBe("docs");
    // docs unpinned and not in front: no chip, so no card, and none wanted.
    rerender({ said: docs, kept, plane: A, chips: [CHIPS[0]] });
    expect(result.current.open).toBeUndefined();
    // Much later its chip is back: its notice is behind its dot, and no card comes up unasked.
    rerender({ said: docs, kept, plane: A, chips: CHIPS });
    expect(result.current.open).toBeUndefined();
    expect(result.current.on.get("docs")).toHaveLength(1);
  });
});

describe("a chip's notices stacked (#3649)", () => {
  it("are newest first, each of its kind, and the dot takes the most urgent", () => {
    const docs = made("docs");
    const failed = reposNotCloned(
      "docs",
      [{ repo: "api", said: "No access." }],
      () => undefined,
      1,
    );
    const later = { ...made("docs"), title: "Later" };
    // Fed out of order: the card's order is the notices' own, newest first.
    const kept: KeptResult[] = [
      { id: 2, said: failed, plane: A },
      { id: 1, said: docs, plane: A },
      { id: 3, said: later, plane: A },
    ];
    const { result } = hook({ said: later, kept, plane: A });
    const on = result.current.on.get("docs") ?? [];
    expect(on.map((one) => [one.id, one.kind])).toEqual([
      [3, "info"],
      [2, "warning"],
      [1, "info"],
    ]);
    expect(mostUrgent(on)).toBe("warning");
    expect(mostUrgent(on.filter((one) => one.kind === "info"))).toBe("info");
  });
});

describe("a clone's notice updated in place (#3649)", () => {
  const cloning = reposCloning("docs", ["api", "web"], 0, 1);
  const halfway = reposCloning("docs", ["api", "web"], 1, 1);
  const cloned = reposCloned("docs", ["api", "web"], 1);
  const failed = reposNotCloned("docs", [{ repo: "api", said: "No access." }], () => undefined, 1);
  const line = (said: Said, id = 1): KeptResult => ({ id, said, plane: A });

  it("leaves a card the person hid behind its dot as it counts and as it lands", () => {
    const { result, rerender } = hook({ said: undefined, kept: [], plane: A });
    rerender({ said: cloning, kept: [line(cloning)], plane: A });
    expect(result.current.open).toBe("docs");
    act(() => result.current.hide());
    rerender({ said: halfway, kept: [line(halfway)], plane: A });
    expect(result.current.open).toBeUndefined();
    rerender({ said: cloned, kept: [line(cloned)], plane: A });
    expect(result.current.open).toBeUndefined();
  });

  it("keeps a card up that is up", () => {
    const { result, rerender } = hook({ said: undefined, kept: [], plane: A });
    rerender({ said: cloning, kept: [line(cloning)], plane: A });
    rerender({ said: cloned, kept: [line(cloned)], plane: A });
    expect(result.current.open).toBe("docs");
  });

  it("brings the card back up for a failure, which asks for the person", () => {
    const { result, rerender } = hook({ said: undefined, kept: [], plane: A });
    rerender({ said: cloning, kept: [line(cloning)], plane: A });
    act(() => result.current.hide());
    rerender({ said: failed, kept: [line(failed)], plane: A });
    expect(result.current.open).toBe("docs");
    expect(result.current.on.get("docs")?.map((one) => one.kind)).toEqual(["warning"]);
  });

  it("leaves another workspace's card up when a result is said between two of its lines", () => {
    const chips: Chip[] = [...CHIPS, { workspace: "delta", look: "pinned" }];
    const delta = made("delta");
    const { result, rerender } = hook({ said: undefined, kept: [], plane: A, chips });
    rerender({ said: cloning, kept: [line(cloning)], plane: A, chips });
    expect(result.current.open).toBe("docs");
    const both = [line(delta, 2), line(cloning)];
    rerender({ said: delta, kept: both, plane: A, chips });
    expect(result.current.open).toBe("delta");
    rerender({ said: halfway, kept: [line(delta, 2), line(halfway)], plane: A, chips });
    expect(result.current.open).toBe("delta");
  });

  it("stays hidden when a refusal is said between two of its lines", () => {
    const { result, rerender } = hook({ said: undefined, kept: [], plane: A });
    rerender({ said: cloning, kept: [line(cloning)], plane: A });
    act(() => result.current.hide());
    const kept = [line(refused, 2), line(cloning)];
    rerender({ said: refused, kept, plane: A });
    rerender({ said: halfway, kept: [line(refused, 2), line(halfway)], plane: A });
    expect(result.current.open).toBeUndefined();
  });

  it("stays down once its counting line is dismissed, as it counts on and as it lands", () => {
    const { result, rerender } = hook({ said: undefined, kept: [], plane: A });
    rerender({ said: cloning, kept: [line(cloning)], plane: A });
    expect(result.current.open).toBe("docs");
    // Dismissed: the kept list no longer holds it, and keeps no more of its counting.
    rerender({ said: cloning, kept: [], plane: A });
    expect(result.current.open).toBeUndefined();
    rerender({ said: halfway, kept: [], plane: A });
    expect(result.current.open).toBeUndefined();
    // Its result is kept again, behind the dot.
    rerender({ said: cloned, kept: [line(cloned, 2)], plane: A });
    expect(result.current.open).toBeUndefined();
    expect(result.current.on.get("docs")).toHaveLength(1);
  });
});
