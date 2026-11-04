// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { Button as AriaButton } from "react-aria-components";

afterEach(cleanup);

describe("a part written in JSX carries its classes (#4351)", () => {
  it("on an element", () => {
    const { container } = render(<p data-scope="card" data-part="title" className="mt-2" />);
    expect(container.firstElementChild?.getAttribute("class")).toBe("mt-2 ds-card dp-title");
  });

  it("on a React Aria component, whose class may be a function of its state", () => {
    const { getByRole } = render(
      <AriaButton
        data-scope="dialog"
        data-part="close"
        className={({ isHovered }) => (isHovered ? "hot" : "cold")}
      >
        Close
      </AriaButton>,
    );
    expect(getByRole("button").getAttribute("class")).toBe("cold ds-dialog dp-close");
  });
});
