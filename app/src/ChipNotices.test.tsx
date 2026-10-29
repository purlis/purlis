// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { OUTSIDE, type Said } from "./actions";
import { AT_STATUS, PASSING, useChipNotices } from "./ChipNotices";
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

  it("goes down when the newest result is a project's, whose card is the status bar's", () => {
    const docs = made("docs");
    const kept: KeptResult[] = [{ id: 1, said: docs, plane: A }];
    const { result, rerender } = hook({ said: undefined, kept: [], plane: A });
    rerender({ said: docs, kept, plane: A });
    expect(result.current.open).toBe("docs");
    rerender({ said: refused, kept: [{ id: 2, said: refused, plane: A }, ...kept], plane: A });
    expect(result.current.open).toBe(AT_STATUS);
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

describe("the status bar's card (#3653)", () => {
  it("holds the project's results, newest first, and comes up for a new one", () => {
    const older: Said = { from: "workspace.pin", refused: true, words: "Older." };
    const kept: KeptResult[] = [
      { id: 2, said: refused, plane: A },
      { id: 1, said: older, plane: A },
      { id: 3, said: made("docs"), plane: A },
    ];
    const { result, rerender } = hook({ said: undefined, kept: [], plane: A });
    rerender({ said: refused, kept, plane: A });
    expect(result.current.open).toBe(AT_STATUS);
    expect(result.current.on.get(AT_STATUS)?.map((one) => one.id)).toEqual([2, 1]);
    expect(result.current.on.get("docs")?.map((one) => one.id)).toEqual([3]);
  });

  it("keeps up what the window said with no project in front once its first project comes", () => {
    const took: Said = { from: "open_repo", refused: false, words: "GitLab was added." };
    const kept: KeptResult[] = [{ id: 1, said: took, plane: undefined }];
    const { result, rerender } = renderHook(
      ({ said, plane }: { said: Said | undefined; plane: string | undefined }) =>
        useChipNotices({
          said,
          kept: said === undefined ? [] : kept,
          plane,
          chips: CHIPS,
          drop: () => undefined,
          onSettings: () => undefined,
        }),
      {
        initialProps: {
          said: undefined as Said | undefined,
          plane: undefined as string | undefined,
        },
      },
    );
    // Said on the opener, with no project in front; then the project it opened comes.
    rerender({ said: took, plane: undefined });
    rerender({ said: took, plane: A });
    expect(result.current.open).toBe(AT_STATUS);
    expect(result.current.on.get(AT_STATUS)?.map((one) => one.id)).toEqual([1]);
  });

  it("is a project's alone: another project's results stay behind its own dot", () => {
    const kept: KeptResult[] = [{ id: 1, said: refused, plane: B }];
    const { result } = hook({ said: undefined, kept, plane: A });
    expect(result.current.on.get(AT_STATUS)).toBeUndefined();
  });

  it("stands a line under way on it, never kept, and put away by its × until the next", () => {
    const cloning: Said = {
      from: "workspace.create",
      refused: false,
      words: "Cloning 2 repo(s) into docs…",
      progress: true,
    };
    const { result, rerender } = hook({ said: undefined, kept: [], plane: A });
    rerender({ said: cloning, kept: [], plane: A });
    expect(result.current.open).toBe(AT_STATUS);
    expect(result.current.on.get(AT_STATUS)).toEqual([
      { id: PASSING, said: cloning, kind: "info" },
    ]);
    act(() => result.current.dismiss(PASSING));
    expect(result.current.on.get(AT_STATUS)).toBeUndefined();
    expect(result.current.open).toBeUndefined();
    // The result that replaces it is kept, and comes up.
    const cloned: Said = { from: "workspace.create", refused: false, words: "Cloned." };
    rerender({ said: cloned, kept: [{ id: 1, said: cloned, plane: A }], plane: A });
    expect(result.current.open).toBe(AT_STATUS);
    expect(result.current.on.get(AT_STATUS)?.map((one) => one.id)).toEqual([1]);
  });
});
