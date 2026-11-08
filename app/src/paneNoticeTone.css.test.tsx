// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A pane's warning box draws its own tone** (#4875): trouble on `danger.wash`, what needs a
 * decision on `attention.surface`, a consent on `wash.warning` whatever its tone, and any other
 * on `wash.warning`. The base rule once outranked trouble's by specificity (0,4,0 against
 * 0,3,0), so "MCP server figma did not start" drew as a warning. Each tone's rule now takes the
 * base's whole selector, and the file's order decides: base, trouble, attention, consent.
 *
 * vitest applies no stylesheet, so the cascade is asked of the window's sheets
 * (`cascade.testkit.ts`). The mark, a `::before` the testkit cannot match, is held in the real
 * engines by `ui/notice/Notice.stories.tsx`'s `TroubleInAPaneTop`.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { drawnWith } from "./cascade.testkit";
import { NoticeBox, type NoticeTone } from "./ui/notice/Notice";

afterEach(cleanup);

/** A warning box in a headed pane's top, as a band with no bell draws one. */
function boxIn({ tone, grants = false }: { tone: NoticeTone; grants?: boolean }): Element {
  const { container } = render(
    <div className="pane-frame pane-headed">
      <div className="pane-top">
        <NoticeBox tone={tone} placement="pane" opened={false} asks>
          <div role="status">
            Says
            {grants && <button data-variant="grants">Allow</button>}
          </div>
        </NoticeBox>
      </div>
    </div>,
  );
  const box = container.querySelector('[data-scope="notice"][data-part="root"]');
  if (box === null) throw new Error("no notice box drawn");
  return box;
}

describe("a pane's warning box by its tone (#4875)", () => {
  it("draws trouble on the danger wash, not the warning wash", () => {
    expect(drawnWith(boxIn({ tone: "trouble" }), "background-color")).toBe("var(--danger-wash)");
  });

  it("draws a warning on the warning wash", () => {
    expect(drawnWith(boxIn({ tone: "news" }), "background-color")).toBe("var(--wash-warning)");
  });

  it("draws what needs a decision on the attention surface", () => {
    expect(drawnWith(boxIn({ tone: "attention" }), "background-color")).toBe(
      "var(--attention-surface)",
    );
  });

  it("draws a consent on the warning wash even when its tone is trouble", () => {
    expect(drawnWith(boxIn({ tone: "trouble", grants: true }), "background-color")).toBe(
      "var(--wash-warning)",
    );
  });
});
