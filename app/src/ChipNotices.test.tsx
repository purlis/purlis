// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { renderHook } from "@testing-library/react";
import { OUTSIDE, type Said } from "./actions";
import { useChipNotices } from "./ChipNotices";
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
