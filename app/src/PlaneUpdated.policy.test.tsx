// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PlaneId, PlaneUpdated } from "./bindings";

const answer = vi.hoisted(() => ({ data: [] as PlaneUpdated[] }));

vi.mock("./bindings", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./bindings")>()),
  commands: { chatsPlaneUpdated: () => Promise.resolve({ status: "ok", data: answer.data }) },
}));

import { usePlaneUpdated } from "./PlaneUpdated";

const PLANE = "plane-policy" as PlaneId;

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

/**
 * **A chat the policy has tightened past is marked like one whose files moved** (M75-23): its
 * sandbox was compiled at its start, so the operator is told why, and starts it fresh.
 */
describe("the policy phrases on a chat's mark", () => {
  beforeEach(() => {
    answer.data = [];
  });

  it("marks a chat whose only change is the policy, with the phrase as the reason", async () => {
    answer.data = [updated({ session: 4, policy: ["the sandbox now forbids ~/.ssh"] })];
    const { result } = renderHook(() => usePlaneUpdated(PLANE, 1));
    await waitFor(() => {
      expect(result.current.files[4]).toEqual(["the sandbox now forbids ~/.ssh"]);
    });
  });

  it("lists the files first and the policy phrases after them", async () => {
    answer.data = [updated({ session: 2, files: ["CLAUDE.md"], policy: ["a rule was added"] })];
    const { result } = renderHook(() => usePlaneUpdated(PLANE, 2));
    await waitFor(() => {
      expect(result.current.files[2]).toEqual(["CLAUDE.md", "a rule was added"]);
    });
  });

  it("marks no chat that neither a file nor the policy moved under", async () => {
    answer.data = [updated({ session: 3 })];
    const { result } = renderHook(() => usePlaneUpdated(PLANE, 3));
    await waitFor(() => {
      expect(result.current.files).toEqual({});
    });
  });
});
