// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Notices item's dot is drawn on the dot alone** (#3653's review, round 2): its geometry
 * (an absolute 24px target over the item's corner) is the `dot` parts' of the status item
 * family, never every status item that carries a `data-kind`. The title bar's Updates pill
 * carries one (`Updates.tsx`, its state's kind), and a rule keyed on `[data-kind]` drew it as a
 * 24px disc lifted out of the title bar.
 *
 * vitest applies no stylesheet, so what a rule draws is asked of the cascade over the window's
 * sheets (`cascade.testkit.ts`), on the markup the wrappers write.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { drawnWith } from "../../cascade.testkit";
import { NoticeDot } from "../titlebar/TitleBarWorkspaces";
import { StatusItem } from "./StatusItem";

afterEach(cleanup);

describe("the Notices item's dot (#3653)", () => {
  it("leaves a status item with a kind of its own where it stands: the Updates pill", () => {
    render(
      <StatusItem variant="pill" data-testid="status-update" data-kind="quiet" onPress={() => {}}>
        Updates
      </StatusItem>,
    );
    const pill = screen.getByTestId("status-update");
    expect(pill).toHaveAttribute("data-kind", "quiet");
    expect(drawnWith(pill, "position")).not.toBe("absolute");
    expect(drawnWith(pill, "inline-size")).not.toBe("24px");
  });

  it("draws each kind's dot as the 24px target over the item's corner", () => {
    for (const kind of ["info", "warning"] as const) {
      render(
        <NoticeDot
          scope="status-item"
          workspace="the status bar"
          count={1}
          kind={kind}
          stop
          onPress={() => {}}
        />,
      );
      const dot = screen.getByRole("button", { name: /^Show the notice on the status bar/ });
      expect(drawnWith(dot, "position"), kind).toBe("absolute");
      expect(drawnWith(dot, "inline-size"), kind).toBe("24px");
      cleanup();
    }
  });
});
