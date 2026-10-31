// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { renderHook } from "@testing-library/react";
import { useWorkedHere, withChatPersonas, type WorkedRow } from "./workedHere";

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
      ({ rows }: { rows: WorkedRow[] }) => useWorkedHere("/plane", rows),
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
      ({ project, rows }: { project: string; rows: WorkedRow[] }) => useWorkedHere(project, rows),
      { initialProps: { project: "/one", rows: [{ workspace: "alpha", persona: "devops" }] } },
    );
    rerender({ project: "/two", rows: [] });
    expect(said(result.current)).toEqual({});
  });
});
