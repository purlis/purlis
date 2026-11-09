// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { OUTSIDE, type Said } from "./actions";
import {
  AT_STATUS,
  chipNoticeCause,
  freshOn,
  mostUrgent,
  PASSING,
  useChipNotices,
} from "./ChipNotices";
import { reposCloned, reposCloning, reposNotCloned } from "./actionResult";
import type { KeptResult } from "./keptResults";
import type { Chip } from "./noticePlace";

/**
 * **Which chip's card is up** (#3648; ADR 0121 decision 1, #6004): a card is up only once the
 * person asks for it, by its dot's press, in the project it was asked in. Nothing said ever brings
 * one up by itself: a new result, a failure, a project's result and a line under way are each a
 * dot, listed in the Notices centre as New.
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

describe("a chip's card never comes up by itself (#6004)", () => {
  it("for a new result about its workspace: the result is on the chip, behind its dot", () => {
    const docs = made("docs");
    const { result, rerender } = hook({ said: undefined, kept: [], plane: A });
    rerender({ said: docs, kept: [{ id: 1, said: docs, plane: A }], plane: A });
    expect(result.current.on.get("docs")?.map((one) => one.id)).toEqual([1]);
    expect(result.current.open).toBeUndefined();
  });

  it("for a failure either, which the dot says with its ▲", () => {
    const failed = reposNotCloned("docs", [{ repo: "api", said: "No." }], () => undefined, 1);
    const { result, rerender } = hook({ said: undefined, kept: [], plane: A });
    rerender({ said: failed, kept: [{ id: 1, said: failed, plane: A }], plane: A });
    expect(mostUrgent(result.current.on.get("docs") ?? [])).toBe("warning");
    expect(result.current.open).toBeUndefined();
  });

  it("comes up when its dot is pressed, in that project alone", () => {
    const docs = made("docs");
    const kept: KeptResult[] = [
      { id: 1, said: docs, plane: A },
      { id: 2, said: made("docs"), plane: B },
    ];
    const { result, rerender } = hook({ said: docs, kept, plane: A });
    act(() => result.current.show("docs"));
    expect(result.current.open).toBe("docs");
    // B in front: its docs notice stays behind its dot.
    rerender({ said: docs, kept, plane: B });
    expect(result.current.open).toBeUndefined();
    // Back on A, the card the person asked for there is up again.
    rerender({ said: docs, kept, plane: A });
    expect(result.current.open).toBe("docs");
  });

  it("goes down when its chip goes, and stays down when the chip comes back (#3648's review)", () => {
    const docs = made("docs");
    const kept: KeptResult[] = [{ id: 1, said: docs, plane: A }];
    const { result, rerender } = hook({ said: docs, kept, plane: A });
    act(() => result.current.show("docs"));
    expect(result.current.open).toBe("docs");
    rerender({ said: docs, kept, plane: A, chips: [CHIPS[0]] });
    expect(result.current.open).toBeUndefined();
    rerender({ said: docs, kept, plane: A, chips: CHIPS });
    expect(result.current.open).toBeUndefined();
    expect(result.current.on.get("docs")).toHaveLength(1);
  });

  it("goes down once nothing is left on it, and a later result stays behind the dot", () => {
    const docs = made("docs");
    const { result, rerender } = hook({
      said: docs,
      kept: [{ id: 1, said: docs, plane: A }],
      plane: A,
    });
    act(() => result.current.show("docs"));
    rerender({ said: docs, kept: [], plane: A });
    expect(result.current.open).toBeUndefined();
    const again = made("docs");
    rerender({ said: again, kept: [{ id: 2, said: again, plane: A }], plane: A });
    expect(result.current.open).toBeUndefined();
  });

  it("hides on a press elsewhere, dismissing nothing", () => {
    const docs = made("docs");
    const { result } = hook({ said: docs, kept: [{ id: 1, said: docs, plane: A }], plane: A });
    act(() => result.current.show("docs"));
    act(() => result.current.hide());
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
  const line = (said: Said, id = 1): KeptResult => ({ id, said, plane: A });

  it("keeps a card the person opened up as it counts and as it lands", () => {
    const { result, rerender } = hook({ said: cloning, kept: [line(cloning)], plane: A });
    act(() => result.current.show("docs"));
    rerender({ said: halfway, kept: [line(halfway)], plane: A });
    expect(result.current.open).toBe("docs");
    rerender({ said: cloned, kept: [line(cloned)], plane: A });
    expect(result.current.open).toBe("docs");
    expect(result.current.on.get("docs")?.map((one) => one.id)).toEqual([1]);
  });

  it("brings no card up as it counts, lands or fails (#6004)", () => {
    const failed = reposNotCloned("docs", [{ repo: "api", said: "No." }], () => undefined, 1);
    const { result, rerender } = hook({ said: undefined, kept: [], plane: A });
    for (const said of [cloning, halfway, failed]) {
      rerender({ said, kept: [line(said)], plane: A });
      expect(result.current.open).toBeUndefined();
    }
  });
});

describe("standing lines and dormant chips on a card (#3651)", () => {
  const DORMANT: readonly Chip[] = [...CHIPS, { workspace: "gone", look: "dormant" }];
  const line = (workspace: string) => ({
    workspace,
    cause: `pin-dormant:${workspace}`,
    says: `${workspace} is gone, kept dormant`,
    fixes: [],
    dismiss: () => undefined,
  });

  it("sorts the results newest first, then the standing line after them, of kind info", () => {
    const older: Said = { ...made("gone"), title: "older" };
    const newer: Said = { ...made("gone"), title: "newer" };
    const { result } = renderHook(() =>
      useChipNotices({
        said: undefined,
        kept: [
          { id: 1, said: older, plane: A },
          { id: 2, said: newer, plane: A },
        ],
        plane: A,
        chips: DORMANT,
        drop: () => undefined,
        onSettings: () => undefined,
        standing: [line("gone")],
      }),
    );
    const on = result.current.on.get("gone") ?? [];
    expect(on.map((one) => one.id)).toEqual([2, 1, "pin-dormant:gone"]);
    expect(on[2].kind).toBe("info");
  });

  it("offers no way to act in a dormant workspace: its words, without its ways", () => {
    const ready: Said = {
      ...made("gone"),
      ways: [{ label: "Add a vision", onPress: () => undefined, primary: true }],
    };
    const { result } = renderHook(() =>
      useChipNotices({
        said: undefined,
        kept: [{ id: 1, said: ready, plane: A }],
        plane: A,
        chips: DORMANT,
        drop: () => undefined,
        onSettings: () => undefined,
      }),
    );
    const [one] = result.current.on.get("gone") ?? [];
    expect(one !== undefined && "said" in one ? one.said.ways : undefined).toEqual([]);
    expect(one !== undefined && "said" in one ? one.said.title : undefined).toBe(
      "Workspace gone is ready",
    );
  });

  it("brings back a standing line dismissed before, as a dormant chip's press asks", () => {
    let standing = [line("gone")];
    const asked: string[] = [];
    const { result, rerender } = renderHook(() =>
      useChipNotices({
        said: undefined,
        kept: [],
        plane: A,
        chips: DORMANT,
        drop: () => undefined,
        onSettings: () => undefined,
        standing,
        bringBack: (workspace) => {
          asked.push(workspace);
          standing = [line(workspace)];
        },
      }),
    );
    // Dismissed: nothing on the chip, and no card.
    standing = [];
    rerender();
    expect(result.current.on.has("gone")).toBe(false);
    // The chip's press brings it back, and its card is up in the same render.
    act(() => result.current.show("gone"));
    rerender();
    expect(asked).toEqual(["gone"]);
    expect(result.current.open).toBe("gone");
  });
});

describe("a workspace's findings on its chip (#3650)", () => {
  it("say their own kind, so a stale repo's ▲ is the dot over a result's ●", () => {
    const { result } = renderHook(() =>
      useChipNotices({
        said: undefined,
        kept: [{ id: 1, said: made("docs"), plane: A }],
        plane: A,
        chips: CHIPS,
        drop: () => undefined,
        onSettings: () => undefined,
        standing: [
          {
            workspace: "docs",
            cause: "stale-clone:docs/web|branch:chat-3",
            says: "web in docs is on chat-3, not main.",
            fixes: [{ label: "Switch to main", onPress: () => undefined }],
            kind: "warning",
            dismiss: () => undefined,
          },
        ],
      }),
    );
    const on = result.current.on.get("docs") ?? [];
    expect(on.map((one) => one.kind)).toEqual(["info", "warning"]);
    expect(mostUrgent(on)).toBe("warning");
  });
});

describe("the status bar's card (#3653)", () => {
  it("holds the project's results, newest first, behind its dot until it is pressed (#6004)", () => {
    const older: Said = { from: "workspace.pin", refused: true, words: "Older." };
    const kept: KeptResult[] = [
      { id: 2, said: refused, plane: A },
      { id: 1, said: older, plane: A },
      { id: 3, said: made("docs"), plane: A },
    ];
    const { result, rerender } = hook({ said: undefined, kept: [], plane: A });
    rerender({ said: refused, kept, plane: A });
    expect(result.current.open).toBeUndefined();
    expect(result.current.on.get(AT_STATUS)?.map((one) => one.id)).toEqual([2, 1]);
    expect(result.current.on.get(AT_STATUS)?.map((one) => one.kind)).toEqual([
      "warning",
      "warning",
    ]);
    expect(result.current.on.get("docs")?.map((one) => one.id)).toEqual([3]);
    act(() => result.current.show(AT_STATUS));
    expect(result.current.open).toBe(AT_STATUS);
  });

  it("is a project's alone: another project's results stay behind its own dot", () => {
    const kept: KeptResult[] = [{ id: 1, said: refused, plane: B }];
    const { result } = hook({ said: undefined, kept, plane: A });
    expect(result.current.on.get(AT_STATUS)).toBeUndefined();
  });

  it("lists what the window said with no project in front on its first project, behind the dot", () => {
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
    rerender({ said: took, plane: undefined });
    rerender({ said: took, plane: A });
    expect(result.current.open).toBeUndefined();
    expect(result.current.on.get(AT_STATUS)?.map((one) => one.id)).toEqual([1]);
  });

  it("stands a line under way on it, never kept, and put away by its × until the next", () => {
    const late: Said = { from: "answer.late.plane_pins", refused: true, words: "Still waiting." };
    const { result, rerender } = hook({ said: undefined, kept: [], plane: A });
    rerender({ said: late, kept: [], plane: A });
    expect(result.current.open).toBeUndefined();
    expect(result.current.on.get(AT_STATUS)?.map((one) => one.id)).toEqual([PASSING]);
    act(() => result.current.dismiss(PASSING));
    expect(result.current.on.get(AT_STATUS)).toBeUndefined();
    const answered: Said = { from: "workspace.pin", refused: false, words: "Pinned." };
    rerender({ said: answered, kept: [{ id: 1, said: answered, plane: A }], plane: A });
    expect(result.current.open).toBeUndefined();
    expect(result.current.on.get(AT_STATUS)?.map((one) => one.id)).toEqual([1]);
  });
});

describe("a chip's 'new' pip (ADR 0121 decision 10, #6004)", () => {
  const docs = made("docs");
  const kept: KeptResult[] = [
    { id: 1, said: docs, plane: A },
    { id: 2, said: refused, plane: A },
  ];

  it("marks each place holding a notice the person has not seen, by the Notices centre's key", () => {
    const { result } = hook({ said: undefined, kept, plane: A });
    const fresh = freshOn(result.current.on, (key) => key !== "notice:result:workspace.pin:2");
    expect([...fresh]).toEqual(["docs"]);
    expect(chipNoticeCause({ id: 1, said: docs, kind: "info" })).toBe("result:workspace.create:1");
  });

  it("marks none once each is seen, and none for a line under way, which no list holds", () => {
    const late: Said = { from: "answer.late.plane_pins", refused: true, words: "Still waiting." };
    const { result } = hook({ said: late, kept: [], plane: A });
    expect(result.current.on.get(AT_STATUS)?.map((one) => one.id)).toEqual([PASSING]);
    expect([...freshOn(result.current.on, () => true)]).toEqual([]);
  });
});
