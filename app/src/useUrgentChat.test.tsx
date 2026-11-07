// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { act, cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { useUrgentChat } from "./ChatPersonaState";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
import { NONE_BELOW, type TasksBelow } from "./sessionTasks";
import { TasksBelowLent } from "./TasksBelow";

afterEach(cleanup);

/** How many times each tab's mark was drawn, by the tab's first chat. */
const drawn = new Map<number, number>();

/** A tab's mark, as `TabMarks` reads it: the most urgent of its panes' chats. */
function Mark({ chats }: { chats: readonly number[] }) {
  drawn.set(chats[0], (drawn.get(chats[0]) ?? 0) + 1);
  useUrgentChat(chats.map((session) => ({ session, shell: false, harness: "claude" })));
  return null;
}

/** Chat `session` with `n` tasks open below it. */
function withTasks(n: number): TasksBelow {
  return {
    ...NONE_BELOW,
    open: Array.from({ length: n }, (_, at) => ({
      session: 100 + at,
      asker: null,
      shell: false,
      direct: true,
      byYou: false,
      report: null,
      outcome: null,
      asking: null,
      harness: null,
    })),
  };
}

/** The chats' states, one value for every draw: none is known. */
const CHATS = fixedChats(nothingKnown);

const MARKS = [<Mark key={1} chats={[1, 2]} />, <Mark key={3} chats={[3]} />];

function Window({ below }: { below: ReadonlyMap<number, TasksBelow> }) {
  return (
    <ChatsHere.Provider value={CHATS}>
      <TasksBelowLent below={below}>{MARKS}</TasksBelowLent>
    </ChatsHere.Provider>
  );
}

describe("a tab's mark (#5577)", () => {
  it("is drawn again when its own panes' tasks change, and never for another tab's", () => {
    drawn.clear();
    const { rerender } = render(<Window below={new Map([[3, withTasks(1)]])} />);
    const before = new Map(drawn);

    // A task of chat 3 starts: only the tab holding chat 3 reads it.
    act(() => rerender(<Window below={new Map([[3, withTasks(2)]])} />));

    expect(drawn.get(1)).toBe(before.get(1));
    expect(drawn.get(3)).toBeGreaterThan(before.get(3) ?? 0);

    // A task of chat 2, in the split tab's second pane: only that tab.
    const then = new Map(drawn);
    act(() =>
      rerender(
        <Window
          below={
            new Map([
              [3, withTasks(2)],
              [2, withTasks(1)],
            ])
          }
        />,
      ),
    );
    expect(drawn.get(3)).toBe(then.get(3));
    expect(drawn.get(1)).toBeGreaterThan(then.get(1) ?? 0);
  });
});
