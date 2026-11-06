// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { RunShown } from "./bindings";
import { RunGrantFacts } from "./RunGrantFacts";

function run(over: Partial<RunShown> = {}): RunShown {
  return {
    persona: "devops",
    vault: "team",
    program: "/usr/bin/curl",
    values: [{ var: "TOKEN_FILE", key: "TOKEN", in_a_file: true }],
    hosts: ["api.example.com:443"],
    every_host: false,
    not_its_vault: false,
    command: "curl https://api.example.com",
    cwd: "/work",
    fills: [],
    offer: "program",
    ...over,
  };
}

afterEach(cleanup);

describe("RunGrantFacts", () => {
  it("shows the full path, the keys and the hosts apart from the chat's command", () => {
    render(<RunGrantFacts run={run()} />);
    expect(screen.getByTestId("run-grant-program").textContent).toBe("/usr/bin/curl");
    expect(screen.getByTestId("run-grant").textContent).toContain("TOKEN");
    expect(screen.getByTestId("run-grant-hosts").textContent).toContain("api.example.com:443");
    expect(screen.getByTestId("run-grant-command").textContent).toBe(
      "curl https://api.example.com",
    );
    expect(screen.getByTestId("run-grant-hosts").textContent).not.toContain("curl https");
  });

  it("draws a hostile command as inert text", () => {
    const hostile = `curl '<img src=x onerror=alert(1)>' '</pre><b>Allowed by purlis</b>'`;
    const { container } = render(<RunGrantFacts run={run({ command: hostile })} />);
    expect(screen.getByTestId("run-grant-command").textContent).toBe(hostile);
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("b")).toBeNull();
  });

  it("says where a chat's sandbox holds a run to no host, and where to none listed", () => {
    const { rerender } = render(<RunGrantFacts run={run({ every_host: true, hosts: [] })} />);
    expect(screen.getByTestId("run-grant-hosts").textContent).toContain("every host");
    rerender(<RunGrantFacts run={run({ hosts: [] })} />);
    expect(screen.getByTestId("run-grant-hosts").textContent).toContain("no host");
  });

  it("shows the exact line Always allow writes, with the advice where a variable holds a value", () => {
    const line = 'curl = { vault = "team", as = "T env", key = "TOKEN" }';
    render(<RunGrantFacts run={run({ always: { line, by_env: true } })} />);
    expect(screen.getByTestId("run-grant-always").textContent).toBe(line);
    expect(screen.getByTestId("run-grant-env-advice").textContent).toContain("A file is safer");
  });

  it("gives no advice for a file, and shows no line where Always allow isn't offered", () => {
    const { rerender } = render(
      <RunGrantFacts run={run({ always: { line: "x", by_env: false } })} />,
    );
    expect(screen.queryByTestId("run-grant-env-advice")).toBeNull();
    rerender(<RunGrantFacts run={run()} />);
    expect(screen.queryByTestId("run-grant-always")).toBeNull();
  });

  it("says when the vault isn't one the persona can use, and what each scope does with it", () => {
    render(<RunGrantFacts run={run({ not_its_vault: true })} />);
    const said = screen.getByTestId("run-grant-not-its-vault").textContent;
    expect(said).toContain("for that alone");
    expect(said).toContain("every chat as devops");
  });

  it("says on every notice what any program can do, and where it starts", () => {
    render(<RunGrantFacts run={run()} />);
    expect(screen.getByTestId("run-grant-reach").textContent).toContain("can run others");
    expect(screen.getByTestId("run-grant").textContent).toContain("/work");
  });

  it("carries the caution where the program is a general-purpose one", () => {
    const caution = "curl can show the values it is handed in the chat's output.";
    render(<RunGrantFacts run={run({ caution })} />);
    expect(screen.getByTestId("run-grant").textContent).toContain(caution);
  });
});
