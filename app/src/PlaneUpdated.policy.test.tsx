// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { act, cleanup, render, renderHook, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Offer } from "./actions";
import type { PlaneId, PlaneUpdated } from "./bindings";

const answer = vi.hoisted(() => ({ data: [] as PlaneUpdated[], asked: 0 }));

vi.mock("./bindings", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./bindings")>()),
  commands: {
    chatsPlaneUpdated: () => {
      answer.asked += 1;
      return Promise.resolve({ status: "ok", data: answer.data });
    },
  },
}));

import { FreshMark, PlaneUpdatedMark, usePlaneUpdated } from "./PlaneUpdated";

const PLANE = "plane-policy" as PlaneId;

/** The core's own phrases (`policy::running::Why::said`), never ones made up here. */
const NEVER = "a Never was given";
const PATH = "a path rule tightened";

function updated(over: Partial<PlaneUpdated>): PlaneUpdated {
  return {
    session: 1,
    files: [],
    skills: false,
    skills_added: [],
    skills_gone: [],
    skills_edited: [],
    policy: [],
    ...over,
  };
}

/** Asks once and waits until the answer was asked for and applied: a hook that has not yet been
 *  answered reads as no mark, so an assertion of "no mark" before this would pass for nothing. */
async function settled(change: number, then: number) {
  const hook = renderHook(() => usePlaneUpdated(PLANE, change));
  await waitFor(() => {
    expect(answer.asked).toBe(then);
  });
  await act(async () => {
    await Promise.resolve();
  });
  return hook;
}

/**
 * **A chat whose shell sandbox the policy has tightened past is marked, but not as "project
 * updated"** (M75-23): a hook decision already follows a tightening, so what is old is the sandbox
 * its shell commands run in, and the way out is a restart.
 */
describe("the policy phrases on a chat's mark", () => {
  beforeEach(() => {
    answer.data = [];
    answer.asked = 0;
  });

  it("marks a chat whose only change is the policy as an old sandbox, not as updated files", async () => {
    answer.data = [updated({ session: 4, policy: [NEVER] })];
    const { result } = renderHook(() => usePlaneUpdated(PLANE, 1));
    await waitFor(() => {
      expect(result.current.sandbox[4]).toEqual([NEVER]);
    });
    expect(result.current.files).toEqual({});
  });

  it("keeps the files and the policy phrases apart", async () => {
    answer.data = [updated({ session: 2, files: ["CLAUDE.md"], policy: [PATH] })];
    const { result } = renderHook(() => usePlaneUpdated(PLANE, 2));
    await waitFor(() => {
      expect(result.current.sandbox[2]).toEqual([PATH]);
    });
    expect(result.current.files[2]).toEqual(["CLAUDE.md"]);
  });

  it("marks no chat that neither a file nor the policy moved under, once the answer is in", async () => {
    answer.data = [updated({ session: 3 })];
    const { result } = await settled(3, 1);
    expect(result.current.files).toEqual({});
    expect(result.current.sandbox).toEqual({});
  });

  it("asks again when the core says the chats changed", async () => {
    answer.data = [];
    const { rerender, result } = renderHook(({ at }) => usePlaneUpdated(PLANE, at), {
      initialProps: { at: 10 },
    });
    await waitFor(() => {
      expect(answer.asked).toBe(1);
    });
    answer.data = [updated({ session: 6, policy: [PATH] })];
    rerender({ at: 11 });
    await waitFor(() => {
      expect(result.current.sandbox[6]).toEqual([PATH]);
    });
    expect(answer.asked).toBe(2);
  });
});

describe("the mark of an old sandbox", () => {
  afterEach(cleanup);

  const RESTART: Offer = {
    id: "chat.restart:3",
    title: "Restart chat steward",
    available: true,
    reason: "",
    does: { verb: "restartChat", session: 3 },
  };

  it("says the shell sandbox is old, offers a restart, and never says the project was updated", () => {
    const pressed: Offer[] = [];
    render(
      <FreshMark
        id="fresh-3"
        restart={RESTART}
        sandbox={[NEVER]}
        onPress={(offer) => pressed.push(offer)}
      />,
    );
    const mark = screen.getByRole("button", {
      name: "Restart chat steward — shell sandbox older than the policy",
    });
    mark.click();
    expect(pressed).toEqual([RESTART]);
    expect(mark.getAttribute("aria-label")).not.toMatch(/project updated/);
  });

  it("is not drawn where the chat cannot be restarted", () => {
    render(<FreshMark id="fresh-3" sandbox={[NEVER]} onPress={() => undefined} />);
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("on a tab, is an image that names the old sandbox", () => {
    render(<PlaneUpdatedMark sandbox={[PATH]} />);
    expect(screen.getByRole("img", { name: "shell sandbox older than the policy" })).toBeTruthy();
  });
});
