// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { useWorkedHere, withChatPersonas, type AskRecorded, type WorkedRow } from "./workedHere";

/** A record that kept nobody: what a test about the chats alone reads. */
const NOBODY: AskRecorded = () => Promise.resolve([]);

/** Who worked where, as sorted lists, for comparing. */
const said = (of: ReadonlyMap<string, ReadonlySet<string>>) =>
  Object.fromEntries([...of].map(([at, personas]) => [at, [...personas].sort()]));

describe("who has worked in each workspace (#3566)", () => {
  it("names each persona under the workspace its chat runs in, and no chat run as none", () => {
    const rows: WorkedRow[] = [
      { workspace: "alpha", persona: "devops" },
      { workspace: "alpha", persona: null },
      { workspace: "beta", persona: "release" },
      { workspace: "alpha", persona: "steward" },
    ];
    expect(said(withChatPersonas(new Map(), rows))).toEqual({
      alpha: ["devops", "steward"],
      beta: ["release"],
    });
  });

  it("is what it was when the chats add no one, so nothing that reads it is redrawn", () => {
    const was = withChatPersonas(new Map(), [{ workspace: "alpha", persona: "devops" }]);
    expect(withChatPersonas(was, [{ workspace: "alpha", persona: "devops" }])).toBe(was);
    expect(withChatPersonas(was, [])).toBe(was);
  });

  it("keeps a persona in its workspace after its chat closes", () => {
    const { result, rerender } = renderHook(
      ({ rows }: { rows: WorkedRow[] }) => useWorkedHere("/plane", rows, NOBODY),
      { initialProps: { rows: [{ workspace: "alpha", persona: "devops" }] } },
    );
    expect(said(result.current)).toEqual({ alpha: ["devops"] });

    // The chat is closed: no chat of devops is listed any more.
    rerender({ rows: [] });
    expect(said(result.current)).toEqual({ alpha: ["devops"] });

    rerender({ rows: [{ workspace: "alpha", persona: "ops" }] });
    expect(said(result.current)).toEqual({ alpha: ["devops", "ops"] });
  });

  it("starts afresh for another project", () => {
    const { result, rerender } = renderHook(
      ({ project, rows }: { project: string; rows: WorkedRow[] }) =>
        useWorkedHere(project, rows, NOBODY),
      { initialProps: { project: "/one", rows: [{ workspace: "alpha", persona: "devops" }] } },
    );
    rerender({ project: "/two", rows: [] });
    expect(said(result.current)).toEqual({});
  });

  it("counts who the reopen record kept, so a persona whose chat closed before a relaunch is here after it (#3593)", async () => {
    const asked: string[] = [];
    const record: AskRecorded = (project) => {
      asked.push(project);
      return Promise.resolve(
        project === "/plane" ? [{ workspace: "alpha", persona: "devops" }] : [],
      );
    };
    // After the relaunch: no chat of devops is listed, only ops's, which is open.
    const { result, rerender } = renderHook(
      ({ project, rows }: { project: string; rows: WorkedRow[] }) =>
        useWorkedHere(project, rows, record),
      { initialProps: { project: "/plane", rows: [{ workspace: "alpha", persona: "ops" }] } },
    );
    await waitFor(() => expect(said(result.current)).toEqual({ alpha: ["devops", "ops"] }));

    // Read once for the project, not again as its chats change.
    rerender({ project: "/plane", rows: [] });
    expect(said(result.current)).toEqual({ alpha: ["devops", "ops"] });
    expect(asked).toEqual(["/plane"]);

    // Another project's record, not this one's.
    rerender({ project: "/other", rows: [] });
    await waitFor(() => expect(asked).toEqual(["/plane", "/other"]));
    expect(said(result.current)).toEqual({});
  });
});
