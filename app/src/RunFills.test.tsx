// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, describe, expect, it } from "vitest";
import type { RunShown } from "./bindings";
import { RunFills, valuesOf } from "./RunFills";

function run(over: Partial<RunShown> = {}): RunShown {
  return {
    persona: "devops",
    vault: "team",
    program: "/usr/bin/curl",
    values: [
      { var: "A", key: "TOKEN", in_a_file: false },
      { var: "B", key: "OTHER", in_a_file: true },
    ],
    hosts: [],
    every_host: false,
    not_its_vault: false,
    command: "curl https://api.example.com",
    cwd: "/work",
    fills: [{ key: "TOKEN" }, { key: "OTHER", declared: "the API it calls" }],
    offer: "program",
    ahead: false,
    ...over,
  };
}

afterEach(cleanup);

describe("RunFills", () => {
  it("draws one masked value box for each key the run lacks, in one notice", () => {
    const boxes = createRef<Map<string, HTMLInputElement>>() as {
      current: Map<string, HTMLInputElement>;
    };
    boxes.current = new Map();
    render(<RunFills run={run()} boxes={boxes} />);
    const token = screen.getByLabelText("Value of TOKEN");
    const other = screen.getByLabelText("Value of OTHER");
    expect(token.getAttribute("type")).toBe("password");
    expect(other.getAttribute("type")).toBe("password");
    expect(screen.getByTestId("run-fills").textContent).toContain("the API it calls");
    expect(screen.getByTestId("run-fills").textContent).toContain("whether or not you allow");
  });

  it("reads each box once, by key, and empties it", () => {
    const boxes = { current: new Map<string, HTMLInputElement>() };
    render(<RunFills run={run()} boxes={boxes} />);
    (screen.getByLabelText("Value of TOKEN") as HTMLInputElement).value = "t0ken";
    (screen.getByLabelText("Value of OTHER") as HTMLInputElement).value = "0ther";
    expect(valuesOf(boxes)).toEqual([
      { key: "TOKEN", value: "t0ken" },
      { key: "OTHER", value: "0ther" },
    ]);
    expect((screen.getByLabelText("Value of TOKEN") as HTMLInputElement).value).toBe("");
  });

  it("says a vault the persona can't use is filled only on an Allow", () => {
    render(<RunFills run={run({ not_its_vault: true })} boxes={{ current: new Map() }} />);
    expect(screen.getByTestId("run-fills").textContent).toContain("only if you allow");
  });

  it("on a notice with no program Allow says what the vault's Allow does, and why policy offers none", () => {
    render(
      <RunFills
        run={run({ offer: "vault", not_its_vault: true, no_allow: "Policy turns it off." })}
        boxes={{ current: new Map() }}
      />,
    );
    const said = screen.getByTestId("run-fills").textContent ?? "";
    expect(said).toContain("every chat opened as devops");
    expect(screen.getByTestId("run-fills-no-allow").textContent).toBe("Policy turns it off.");
  });

  it("draws nothing for a run that lacks no key", () => {
    const { container } = render(
      <RunFills run={run({ fills: [] })} boxes={{ current: new Map() }} />,
    );
    expect(container.textContent).toBe("");
  });
});
