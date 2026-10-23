// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { ChatsSection } from "./ChatsSection";
import { ChatsHere, fixedChats } from "./chatState";
import { chatsTree, type ListedChat } from "./chatsTree";
import { facetsOf } from "./projectViews";
import { forgetThisLaunch } from "./regions";
import { GLOBAL } from "./windowprefs";
import { getByTip } from "./ui/tooltip/tooltip.testkit";

/**
 * **The Chats list's folds across a relaunch** (#1687): a fold set by hand is kept by the
 * chat's id, which it keeps across a launch, and comes back on the chat under the number the
 * next launch deals it.
 */

const PLANE = "/plane";
const STEWARD = "01JB6Q9X7ZK3M4N5P6R7S8T9VW";
const DEVOPS = "01JB6Q9X7ZK3M4N5P6R7S8T9VX";

function listed(session: number, id: string, more: Partial<ListedChat> = {}): ListedChat {
  return {
    session,
    id,
    name: `steward ${session}`,
    persona: "steward",
    workspace: "alpha",
    shell: false,
    parent: null,
    mode: null,
    from: null,
    tab: true,
    branch: null,
    report: null,
    outcome: null,
    asking: null,
    harness: "claude",
    ...more,
  };
}

/** A steward chat `steward` with its task `task` under it. */
const chats = (steward: number, task: number) =>
  chatsTree([
    listed(steward, STEWARD),
    listed(task, DEVOPS, {
      name: `devops ${task}`,
      persona: "devops",
      parent: steward,
      mode: "task",
      from: `steward ${steward}`,
      tab: false,
    }),
  ]);

function draw(steward: number, task: number) {
  return render(
    <ChatsHere.Provider value={{ ...fixedChats(), plane: PLANE }}>
      <ChatsSection rows={chats(steward, task)} here="alpha" onOpen={() => {}} />
    </ChatsHere.Provider>,
  );
}

const names = () =>
  within(screen.getByRole("tree"))
    .queryAllByRole("treeitem")
    .map((one) => one.querySelector(".session")?.textContent);

beforeEach(forgetThisLaunch);
afterEach(() => {
  cleanup();
  Reflect.deleteProperty(globalThis, GLOBAL);
});

describe("the Chats list's folds (#1687)", () => {
  it("come back after a relaunch on the same chats under their new numbers", async () => {
    const first = draw(1, 2);
    expect(names()).toEqual(["steward 1", "devops 2"]);
    await userEvent.click(getByTip("Fold the chats under steward 1"));
    expect(names()).toEqual(["steward 1"]);
    // What the window writes for the project, read back from the file at the next launch.
    const written = facetsOf(PLANE);
    first.unmount();

    forgetThisLaunch();
    globalThis.sessionStorage.clear();
    (globalThis as Record<string, unknown>)[GLOBAL] = {
      layout: {
        path: "/cfg/layout.json",
        found: true,
        document: { version: 2, regions: [], projects: { [PLANE]: written } },
        trouble: null,
      },
      theme: { path: "", found: false, document: null, trouble: null },
    };
    draw(7, 8);

    await waitFor(() => expect(names()).toEqual(["steward 7"]));
    await userEvent.click(getByTip("Show the chats under steward 7"));
    expect(names()).toEqual(["steward 7", "devops 8"]);
  });
});
