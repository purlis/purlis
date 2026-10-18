// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { ApprovePlane } from "./ApprovePlane";

afterEach(cleanup);

const none = () => undefined;

const NOTHING = { plugins: [], env: [], starts: [], profiles: [], grants: [], fonts: [] };

/** The trust ask for a project's font files (#2555, ADR 0098 as amended 2026-10-18). */
describe("a project's font files in the trust ask", () => {
  it("names each font file a project brings, with its digest", () => {
    render(
      <ApprovePlane
        ask={{
          path: "/p",
          contributes: {
            ...NOTHING,
            fonts: [["themes/plex/fonts/Plex.woff2", "ab12cd"]],
          },
          changes: [],
          first: true,
        }}
        onApprove={none}
        onCancel={none}
      />,
    );

    const said = screen.getByRole("dialog").textContent ?? "";
    expect(screen.getByRole("heading", { name: "Font files its themes bring" })).toBeTruthy();
    expect(said).toContain("themes/plex/fonts/Plex.woff2");
    expect(said).toContain("sha-256 ab12cd");
    expect(said).not.toContain("Nothing purlis can enumerate");
  });

  it("says a project brings no font files when it brings nothing", () => {
    render(
      <ApprovePlane
        ask={{ path: "/p", contributes: NOTHING, changes: [], first: true }}
        onApprove={none}
        onCancel={none}
      />,
    );

    expect(screen.getByRole("dialog").textContent).toContain("brings no font files");
  });
});
