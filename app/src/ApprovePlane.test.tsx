// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { ApprovePlane } from "./ApprovePlane";
import { getAllByTip } from "./ui/tooltip/tooltip.testkit";

afterEach(cleanup);

const none = () => undefined;

const NOTHING = { plugins: [], env: [], starts: [], profiles: [], grants: [], fonts: [] };

/** The trust ask for a project's font files (#2555, ADR 0098 as amended 2026-10-18). */
describe("a project's font files in the trust ask", () => {
  const DIGEST = "ab12cd34ef56".concat("0".repeat(52));

  it("names each font file by its theme folder, with its size and a short digest", () => {
    render(
      <ApprovePlane
        ask={{
          path: "/p",
          contributes: {
            ...NOTHING,
            fonts: [
              ["themes/plex/fonts/Plex-Regular.woff2", `${DIGEST} 40960`],
              ["workspaces/alpha/themes/client/fonts/Brand.woff2", `${DIGEST} 512`],
            ],
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
    expect(said).toContain("themes/plex");
    expect(said).toContain("Plex-Regular.woff2 40 KiB, sha-256 ab12cd34ef56");
    expect(said).toContain("workspaces/alpha/themes/client");
    expect(said).toContain("Brand.woff2 512 bytes");
    expect(said).not.toContain(DIGEST);
    // The whole digest is there on hover.
    expect(getAllByTip(DIGEST)).toHaveLength(2);
    expect(said).not.toContain("Nothing purlis can enumerate");
  });

  it("draws the first twelve font files and counts the rest", () => {
    const fonts: [string, string][] = Array.from({ length: 15 }, (_, at) => [
      `themes/many/fonts/F${String(at).padStart(2, "0")}.woff2`,
      `${DIGEST} 100`,
    ]);
    render(
      <ApprovePlane
        ask={{ path: "/p", contributes: { ...NOTHING, fonts }, changes: [], first: true }}
        onApprove={none}
        onCancel={none}
      />,
    );

    const said = screen.getByRole("dialog").textContent ?? "";
    expect(said).toContain("F11.woff2");
    expect(said).not.toContain("F12.woff2");
    expect(said).toContain("and 3 more font files");
  });

  it("puts what changed above the font files on a re-ask", () => {
    render(
      <ApprovePlane
        ask={{
          path: "/p",
          contributes: { ...NOTHING, fonts: [["themes/plex/fonts/Plex.woff2", `${DIGEST} 100`]] },
          changes: ["the font file themes/plex/fonts/Plex.woff2 has changed"],
          first: false,
        }}
        onApprove={none}
        onCancel={none}
      />,
    );

    const said = screen.getByRole("dialog").textContent ?? "";
    const changed = said.indexOf("the font file themes/plex/fonts/Plex.woff2 has changed");
    expect(changed).toBeGreaterThan(-1);
    expect(changed).toBeLessThan(said.indexOf("Font files its themes bring"));
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
