// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { ActivityCount, withNew } from "./ActivityBar";

/** **A "new" pip beside a view's badge** (#6002, ADR 0121 decision 10). */

afterEach(cleanup);

describe("withNew", () => {
  it("draws a pip after the badge of a view with something New, and says how many", () => {
    const badges = withNew(
      { asks: <ActivityCount count={2} said="2 things wait on you" tone="needs-you" /> },
      { asks: 1 },
    );
    const { container } = render(<span>{badges.asks}</span>);

    expect(container.querySelector('[data-part="count"]')).not.toBeNull();
    expect(container.querySelector('[data-part="pip"]')).not.toBeNull();
    expect(container).toHaveTextContent("2 things wait on you, 1 new");
  });

  it("draws a pip where a view has no badge of its own", () => {
    const { container } = render(<span>{withNew({}, { notices: 3 }).notices}</span>);

    expect(container.querySelector('[data-part="pip"]')).not.toBeNull();
    expect(container).toHaveTextContent(/^3 new$/);
  });

  it("leaves a view with nothing New as it was", () => {
    const badge = <ActivityCount count={1} said="1 unread update from your chats" />;
    expect(withNew({ updates: badge }, { updates: 0 }).updates).toBe(badge);
  });
});
