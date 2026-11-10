// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { NAVIGATIONS, placeOffer, type Goes } from "./artifactPlaces";
import { todoView } from "./todos";

describe("where a purlis:// link in an artifact's page goes", () => {
  it("focuses a workspace", () => {
    const offer = placeOffer({ to: "workspace", workspace: "beta" });
    expect(offer.does).toEqual({ verb: "focusWorkspace", workspace: "beta" });
    expect(offer.available).toBe(true);
  });

  it("opens a todo's view in its own workspace", () => {
    expect(
      placeOffer({ to: "todo", workspace: "alpha", todo: "fix-login", title: "Fix the login" })
        .does,
    ).toEqual({
      verb: "openView",
      view: todoView({ workspace: "alpha", slug: "fix-login" }),
      title: "Fix the login",
    });
  });

  it("shows a chat by its session", () => {
    expect(placeOffer({ to: "chat", session: 7 }).does).toEqual({ verb: "showChat", session: 7 });
  });

  it("does nothing for a place it does not know, rather than anything else", () => {
    const offer = placeOffer({ to: "run", command: "rm" } as unknown as Goes);
    expect(offer.available).toBe(false);
    expect(offer.does).toEqual({ verb: "nothing" });
  });

  it("only ever navigates", () => {
    const every: Goes[] = [
      { to: "workspace", workspace: "beta" },
      { to: "todo", workspace: "alpha", todo: "fix-login", title: "Fix the login" },
      { to: "chat", session: 7 },
    ];
    for (const goes of every) expect(NAVIGATIONS).toContain(placeOffer(goes).does.verb);
    expect([...NAVIGATIONS].sort()).toEqual(["focusWorkspace", "openView", "showChat"]);
  });
});
