// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PoolSetupShown, PoolSetupStep } from "./bindings";
import { PoolSetupTab } from "./PoolSetupTab";

const step = (id: string, name: string, state: string, more: Partial<PoolSetupStep> = {}) =>
  ({
    id,
    name,
    state,
    line: null,
    findings: [],
    next: null,
    skippable: false,
    see: null,
    checks: [],
    other: null,
    machine: null,
    cause: null,
    also: null,
    parts: [],
    undo: null,
    ...more,
  }) as PoolSetupStep;

const action = (id: string, label: string, url: string | null = null) => ({
  id,
  label,
  does: `${label}: what it does.`,
  command: null,
  url,
});

const setup = (at: string, steps: PoolSetupStep[], pool: string | null = null): PoolSetupShown => ({
  kind: "docker",
  pool,
  name: pool,
  at,
  steps,
  noticed: false,
  recommended: "docker",
  others: [],
  contexts: { names: [], recommended: null, chosen: null, pools: [] },
});

const READY = setup("where", [
  step("where", "Where chats run", "current", {
    findings: ["Docker is ready."],
    next: action("use-docker", "Continue with Docker"),
  }),
  step("pool", "Pool", "later"),
]);

const CHOSEN = setup("pool", [
  step("where", "Where chats run", "done", { line: "Docker on this machine" }),
  step("pool", "Pool", "current", { next: action("create-pool", "Create the pool") }),
]);

const MADE = setup(
  "signins",
  [
    step("where", "Where chats run", "done", { line: "Docker on this machine" }),
    step("pool", "Pool", "done", { line: "dev-box" }),
    step("signins", "Sign-ins", "current", {
      findings: ["This pool has none yet."],
      next: action("choose-profiles", "Choose profiles"),
    }),
  ],
  "dev-box",
);

const MISSING = setup("where", [
  step("where", "Where chats run", "attention", {
    findings: ["You need Docker first."],
    next: action("get-docker", "Open Docker's install page", "https://docs.docker.com/"),
  }),
  step("pool", "Pool", "later"),
]);

const mock = vi.hoisted(() => ({
  openUrl: vi.fn(async () => undefined),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: mock.openUrl }));

type Calls = { cmd: string; args: unknown }[];

function core(
  answers: () => PoolSetupShown,
  more: Record<string, (args: unknown) => unknown> = {},
) {
  const calls: Calls = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    if (cmd === "pool_setup") return answers();
    if (cmd in more) return more[cmd]?.(args);
    if (cmd === "runner_image_of_this_version") return { state: "ours", image: "img@sha256:a" };
    return null;
  });
  return calls;
}

afterEach(() => {
  cleanup();
  clearMocks();
  vi.useRealTimers();
});

describe("PoolSetupTab", () => {
  it("shows each step's state as the core read it", async () => {
    core(() => READY);
    render(<PoolSetupTab plane="p" />);
    expect(
      await screen.findByRole("heading", { name: "Where should your chats run?" }),
    ).toBeTruthy();
    expect(screen.getByText("Docker is ready.")).toBeTruthy();
    expect(screen.getByRole("button", { name: /^Pool/ })).toHaveProperty("disabled", true);
  });

  it("says what the button does before it is pressed", async () => {
    core(() => READY);
    render(<PoolSetupTab plane="p" />);
    expect(await screen.findByText("Continue with Docker: what it does.")).toBeTruthy();
  });

  it("keeps the choice of Docker and moves on to the Pool step", async () => {
    let now = READY;
    const calls = core(() => now, {
      pool_setup_choose_docker: () => {
        now = CHOSEN;
        return null;
      },
    });
    render(<PoolSetupTab plane="p" />);
    await userEvent.click(await screen.findByRole("button", { name: "Continue with Docker" }));
    expect(await screen.findByRole("heading", { name: "Name your pool" })).toBeTruthy();
    expect(calls.some((call) => call.cmd === "pool_setup_choose_docker")).toBe(true);
  });

  it("makes a real pool on the step's own button and goes on", async () => {
    let now = CHOSEN;
    const calls = core(() => now, {
      pool_setup_create: () => {
        now = MADE;
        return { breaks: [] };
      },
    });
    render(<PoolSetupTab plane="p" />);
    await userEvent.type(await screen.findByRole("textbox", { name: /Name/ }), "dev-box");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Create the pool" })).toHaveProperty(
        "disabled",
        false,
      ),
    );
    expect(calls.some((call) => call.cmd === "pool_setup_create")).toBe(false);
    await userEvent.click(screen.getByRole("button", { name: "Create the pool" }));
    expect(
      await screen.findByRole("heading", { name: "Choose the profiles for this pool" }),
    ).toBeTruthy();
    const added = calls.find((call) => call.cmd === "pool_setup_create");
    expect(added?.args).toMatchObject({ adding: { name: "dev-box", image: "img@sha256:a" } });
  });

  it("keeps what was typed when the pool is refused, and says why", async () => {
    const calls = core(() => CHOSEN, {
      pool_setup_create: () => {
        throw "a pool named dev-box is on this machine";
      },
    });
    render(<PoolSetupTab plane="p" />);
    const box = await screen.findByRole("textbox", { name: /Name/ });
    await userEvent.type(box, "dev-box");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Create the pool" })).toHaveProperty(
        "disabled",
        false,
      ),
    );
    await userEvent.click(screen.getByRole("button", { name: "Create the pool" }));
    expect(await screen.findByText("a pool named dev-box is on this machine")).toBeTruthy();
    expect(screen.getByRole("textbox", { name: /Name/ })).toHaveProperty("value", "dev-box");
    expect(calls.filter((call) => call.cmd === "pool_setup_create")).toHaveLength(1);
  });

  it("says when the image is a newer version's, and why it couldn't name one", async () => {
    core(() => CHOSEN, {
      runner_image_of_this_version: () => ({
        state: "newer",
        image: "new@sha256:b",
        version: "9.9",
      }),
    });
    const { unmount } = render(<PoolSetupTab plane="p" />);
    expect(await screen.findByText(/from purlis 9\.9\. An update to it is waiting/)).toBeTruthy();
    unmount();
    core(() => CHOSEN, {
      runner_image_of_this_version: () => ({ state: "none_named" }),
    });
    render(<PoolSetupTab plane="p" />);
    expect(await screen.findByRole("textbox", { name: /Image, by digest/ })).toBeTruthy();
  });

  it("makes nothing where Docker changed since the step was read", async () => {
    let now = CHOSEN;
    const calls = core(() => now);
    render(<PoolSetupTab plane="p" />);
    await userEvent.type(await screen.findByRole("textbox", { name: /Name/ }), "dev-box");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Create the pool" })).toHaveProperty(
        "disabled",
        false,
      ),
    );
    now = MISSING;
    await userEvent.click(screen.getByRole("button", { name: "Create the pool" }));
    expect(await screen.findByRole("button", { name: "Open Docker's install page" })).toBeTruthy();
    expect(calls.some((call) => call.cmd === "pool_setup_create")).toBe(false);
  });

  it("keeps the name as it is typed, without waiting to leave the box", async () => {
    const calls = core(() => CHOSEN);
    render(<PoolSetupTab plane="p" />);
    await userEvent.type(await screen.findByRole("textbox", { name: /Name/ }), "dev");
    await waitFor(() => expect(calls.some((call) => call.cmd === "pool_setup_name")).toBe(true));
  });

  it("keeps a name typed just before the tab closes, at once, and leaves no wait behind (#4823)", async () => {
    const calls = core(() => CHOSEN);
    const { unmount } = render(<PoolSetupTab plane="p" />);
    const box = await screen.findByRole("textbox", { name: /Name/ });
    vi.useFakeTimers();
    fireEvent.change(box, { target: { value: "dev" } });
    const kept = () => calls.filter((call) => call.cmd === "pool_setup_name");
    expect(kept()).toEqual([]);
    unmount();
    expect(kept().map((call) => call.args)).toEqual([{ name: "dev" }]);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("tells the rest of the window a pool was made", async () => {
    let now = CHOSEN;
    const calls = core(() => now, {
      pool_setup_create: () => {
        now = MADE;
        return { breaks: [] };
      },
    });
    render(<PoolSetupTab plane="p" />);
    await userEvent.type(await screen.findByRole("textbox", { name: /Name/ }), "dev-box");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Create the pool" })).toHaveProperty(
        "disabled",
        false,
      ),
    );
    await userEvent.click(screen.getByRole("button", { name: "Create the pool" }));
    await screen.findByRole("heading", { name: "Choose the profiles for this pool" });
    expect(calls.some((call) => call.cmd === "docker_pools")).toBe(true);
  });

  it("goes on with the pool it made, and only then", async () => {
    let now = CHOSEN;
    const calls = core(() => now, {
      pool_setup_create: () => {
        now = MADE;
        return { breaks: [] };
      },
    });
    render(<PoolSetupTab plane="p" />);
    await userEvent.type(await screen.findByRole("textbox", { name: /Name/ }), "dev-box");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Create the pool" })).toHaveProperty(
        "disabled",
        false,
      ),
    );
    expect(calls.some((call) => call.cmd === "pool_setup_pool")).toBe(false);
    await userEvent.click(screen.getByRole("button", { name: "Create the pool" }));
    await screen.findByRole("heading", { name: "Choose the profiles for this pool" });
    expect(calls.find((call) => call.cmd === "pool_setup_pool")?.args).toMatchObject({
      name: "dev-box",
    });
  });

  it("keeps the name typed when the person goes back a step and returns", async () => {
    core(() => CHOSEN);
    render(<PoolSetupTab plane="p" />);
    await userEvent.type(await screen.findByRole("textbox", { name: /Name/ }), "dev");
    await userEvent.click(screen.getByRole("button", { name: /^Where chats run/ }));
    await screen.findByRole("heading", { name: "Where should your chats run?" });
    await userEvent.click(screen.getByRole("button", { name: /^Pool/ }));
    expect(await screen.findByRole("textbox", { name: /Name/ })).toHaveProperty("value", "dev");
  });

  it("resumes with the name given", async () => {
    core(() => ({ ...CHOSEN, name: "kept-name" }));
    render(<PoolSetupTab plane="p" />);
    expect(await screen.findByRole("textbox", { name: /Name/ })).toHaveProperty(
      "value",
      "kept-name",
    );
  });

  it("opens the official page without installing anything, and notices Docker by itself", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    let now = MISSING;
    core(() => now);
    render(<PoolSetupTab plane="p" />);
    const open = await screen.findByRole("button", { name: "Open Docker's install page" });
    await userEvent.click(open);
    expect(mock.openUrl).toHaveBeenCalledWith("https://docs.docker.com/");
    now = READY;
    await vi.advanceTimersByTimeAsync(6000);
    expect(await screen.findByRole("button", { name: "Continue with Docker" })).toBeTruthy();
  });

  it("goes back to a step reached", async () => {
    core(() => CHOSEN);
    render(<PoolSetupTab plane="p" />);
    await screen.findByRole("heading", { name: "Name your pool" });
    await userEvent.click(screen.getByRole("button", { name: /^Where chats run/ }));
    expect(
      await screen.findByRole("heading", { name: "Where should your chats run?" }),
    ).toBeTruthy();
  });

  it("says what went wrong where the read failed, with something to try", async () => {
    let fails = true;
    mockIPC((cmd) => {
      if (cmd !== "pool_setup") return null;
      if (fails) throw "the choices can't be read";
      return READY;
    });
    render(<PoolSetupTab plane="p" />);
    expect(await screen.findByText("the choices can't be read")).toBeTruthy();
    fails = false;
    await userEvent.click(screen.getByRole("button", { name: "Check again" }));
    expect(await screen.findByRole("button", { name: "Continue with Docker" })).toBeTruthy();
  });
});

describe("Options and See what this does (#4481)", () => {
  const typeName = async () => {
    await userEvent.type(await screen.findByRole("textbox", { name: /^Name/ }), "dev-box");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Create the pool" })).toHaveProperty(
        "disabled",
        false,
      ),
    );
  };

  it("offers purlis's image as recommended and a team's own under Options", async () => {
    core(() => CHOSEN);
    render(<PoolSetupTab plane="p" />);
    await userEvent.click(await screen.findByRole("button", { name: "Options" }));
    const ours = screen.getByRole("radio", { name: /purlis's image/ });
    expect((ours as HTMLInputElement).checked).toBe(true);
    expect(screen.getByText("Recommended")).toBeTruthy();
    expect(screen.getByRole("radio", { name: /My team's own image/ })).toBeTruthy();
  });

  it("shows the command for what the button would send", async () => {
    const calls = core(() => CHOSEN, {
      pool_setup_command: () => "purlis pool add dev-box --image img@sha256:a",
    });
    render(<PoolSetupTab plane="p" />);
    await typeName();
    await userEvent.click(screen.getByRole("button", { name: "See what this does" }));
    expect(await screen.findByText("purlis pool add dev-box --image img@sha256:a")).toBeTruthy();
    const asked = calls.filter((call) => call.cmd === "pool_setup_command").pop();
    expect(asked?.args).toMatchObject({ adding: { name: "dev-box", image: "img@sha256:a" } });
  });

  it("sends the button exactly what the shown command was made from", async () => {
    let now = CHOSEN;
    const calls = core(() => now, {
      pool_setup_command: () => "shown",
      pool_setup_create: () => {
        now = MADE;
        return { breaks: [] };
      },
    });
    render(<PoolSetupTab plane="p" />);
    await typeName();
    await userEvent.click(screen.getByRole("button", { name: "Options" }));
    await userEvent.type(screen.getByRole("textbox", { name: /^Memory/ }), "8192");
    await waitFor(() => {
      const shown = calls.filter((call) => call.cmd === "pool_setup_command").pop();
      expect(shown?.args).toMatchObject({ adding: { memory_mib: 8192 } });
    });
    await userEvent.click(screen.getByRole("button", { name: "Create the pool" }));
    await screen.findByRole("heading", { name: "Choose the profiles for this pool" });
    const shown = calls.filter((call) => call.cmd === "pool_setup_command").pop();
    const sent = calls.find((call) => call.cmd === "pool_setup_create");
    expect(sent?.args).toEqual(shown?.args);
  });

  it("refuses a limit that isn't a number before anything is sent", async () => {
    const calls = core(() => CHOSEN);
    render(<PoolSetupTab plane="p" />);
    await typeName();
    await userEvent.click(screen.getByRole("button", { name: "Options" }));
    await userEvent.type(screen.getByRole("textbox", { name: /^Memory/ }), "lots");
    await userEvent.click(screen.getByRole("button", { name: "Create the pool" }));
    expect(await screen.findByText("A limit under Options isn't a whole number.")).toBeTruthy();
    expect(calls.some((call) => call.cmd === "pool_setup_create")).toBe(false);
  });

  it("names a team's image, and says what it breaks with nothing made", async () => {
    const calls = core(() => CHOSEN, {
      pool_setup_create: () => ({ breaks: ["It runs as root.", "It has no HOME."] }),
    });
    render(<PoolSetupTab plane="p" />);
    await typeName();
    await userEvent.click(screen.getByRole("button", { name: "Options" }));
    await userEvent.click(screen.getByRole("radio", { name: /My team's own image/ }));
    await userEvent.type(
      screen.getByRole("textbox", { name: /Image, by digest/ }),
      "reg.example/r@sha256:b",
    );
    await userEvent.click(screen.getByRole("button", { name: "Create the pool" }));
    expect(await screen.findByText("This image is missing 2 things a runner needs")).toBeTruthy();
    // Create is at the page's foot: why nothing was made is moved to, not left out of sight.
    await waitFor(() =>
      expect(document.activeElement?.textContent).toContain("This image is missing 2 things"),
    );
    const line = screen.getByRole("list", { name: "What the image is missing" });
    expect(line.textContent).toContain("It runs as root.");
    expect(calls.some((call) => call.cmd === "pool_setup_pool")).toBe(false);
    expect(calls.find((call) => call.cmd === "pool_setup_create")?.args).toMatchObject({
      adding: { image: "reg.example/r@sha256:b" },
    });
  });

  it("goes back to purlis's image from a team's that breaks the contract", async () => {
    core(() => CHOSEN, { pool_setup_create: () => ({ breaks: ["It runs as root."] }) });
    render(<PoolSetupTab plane="p" />);
    await typeName();
    await userEvent.click(screen.getByRole("button", { name: "Options" }));
    await userEvent.click(screen.getByRole("radio", { name: /My team's own image/ }));
    await userEvent.type(screen.getByRole("textbox", { name: /Image, by digest/ }), "r@sha256:b");
    await userEvent.click(screen.getByRole("button", { name: "Create the pool" }));
    await userEvent.click(
      await screen.findByRole("button", { name: "Use purlis's image instead" }),
    );
    expect(screen.queryByRole("list", { name: "What the image is missing" })).toBeNull();
    const ours = screen.getByRole("radio", { name: /purlis's image/ });
    expect((ours as HTMLInputElement).checked).toBe(true);
  });

  it("notices a pool made by hand, and follows it on a press", async () => {
    let now: PoolSetupShown = {
      ...setup(
        "signins",
        [
          step("where", "Where chats run", "done"),
          step("pool", "Pool", "done", { line: "dev-box" }),
          step("signins", "Sign-ins", "current"),
        ],
        "dev-box",
      ),
      noticed: true,
    };
    const calls = core(() => now, {
      pool_setup_pool: () => {
        now = { ...now, noticed: false };
        return null;
      },
    });
    render(<PoolSetupTab plane="p" />);
    await userEvent.click(await screen.findByRole("button", { name: /^Pool/ }));
    expect(await screen.findByRole("heading", { name: "Your pool is ready" })).toBeTruthy();
    expect(screen.getByText("Found pool dev-box")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Continue to sign-ins" }));
    await waitFor(() =>
      expect(calls.find((call) => call.cmd === "pool_setup_pool")?.args).toEqual({
        name: "dev-box",
      }),
    );
  });

  it("says a name is taken, and a pool already there is picked only by a press", async () => {
    const calls = core(() => ({ ...CHOSEN, others: ["dev-box"] }));
    render(<PoolSetupTab plane="p" />);
    await userEvent.type(await screen.findByRole("textbox", { name: /^Name/ }), "dev-box");
    expect(await screen.findByText("You already have a pool called dev-box.")).toBeTruthy();
    expect(calls.some((call) => call.cmd === "pool_setup_pool")).toBe(false);
    await userEvent.click(screen.getByRole("button", { name: "Use pool dev-box" }));
    await waitFor(() =>
      expect(calls.find((call) => call.cmd === "pool_setup_pool")?.args).toEqual({
        name: "dev-box",
      }),
    );
  });

  it("shows a step's own commands and what running them means", async () => {
    const withCommands = setup("where", [
      step("where", "Where chats run", "current", {
        next: action("use-docker", "Continue with Docker"),
        see: { note: "It only reads.", text: "docker version" },
      }),
    ]);
    core(() => withCommands);
    render(<PoolSetupTab plane="p" />);
    await userEvent.click(await screen.findByRole("button", { name: "See what this does" }));
    expect(screen.getByText("It only reads.")).toBeTruthy();
    expect(screen.getByText("docker version")).toBeTruthy();
  });
});

describe("work done elsewhere (#4481)", () => {
  it("reads again as the person comes back to the window", async () => {
    const calls = core(() => CHOSEN);
    render(<PoolSetupTab plane="p" />);
    await screen.findByRole("heading", { name: "Name your pool" });
    const before = calls.filter((call) => call.cmd === "pool_setup").length;
    window.dispatchEvent(new Event("focus"));
    await waitFor(() =>
      expect(calls.filter((call) => call.cmd === "pool_setup").length).toBe(before + 1),
    );
  });

  it("shows no command while a box holds what no pool could have", async () => {
    core(() => CHOSEN, {
      pool_setup_command: () => "purlis pool add dev-box --image img@sha256:a",
    });
    render(<PoolSetupTab plane="p" />);
    await userEvent.type(await screen.findByRole("textbox", { name: /^Name/ }), "dev-box");
    expect(await screen.findByRole("button", { name: "See what this does" })).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Options" }));
    await userEvent.type(screen.getByRole("textbox", { name: /^Memory/ }), "lots");
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "See what this does" })).toBeNull(),
    );
  });
});

describe("Where, on a cluster (#4492)", () => {
  const onACluster = (): PoolSetupShown => ({
    ...setup("where", [
      step("where", "Where chats run", "current", {
        findings: ["Your kubeconfig reaches a cluster through context kind-team."],
        next: action("use-cluster", "Continue with this cluster"),
        also: action("use-docker", "Use Docker on this machine instead"),
      }),
      step("check", "Check the cluster", "later"),
    ]),
    kind: "cluster",
    contexts: {
      names: ["kind-team", "prod-eu"],
      recommended: "kind-team",
      chosen: null,
      pools: [],
    },
  });

  const asking = () => {
    const sent: { cmd: string; args: unknown }[] = [];
    mockIPC((cmd, args) => {
      sent.push({ cmd, args });
      return cmd === "pool_setup" ? onACluster() : null;
    });
    return sent;
  };

  it("keeps the recommended context's name when the person continues", async () => {
    const sent = asking();
    render(<PoolSetupTab plane="p" />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Continue with this cluster" }),
    );
    await waitFor(() =>
      expect(sent).toContainEqual({
        cmd: "pool_setup_choose_cluster",
        args: { context: "kind-team" },
      }),
    );
  });

  it("keeps the context chosen under Options instead", async () => {
    const sent = asking();
    render(<PoolSetupTab plane="p" />);
    await userEvent.click(await screen.findByRole("button", { name: "Options" }));
    await userEvent.click(screen.getByRole("radio", { name: "prod-eu" }));
    await userEvent.click(screen.getByRole("button", { name: "Continue with this cluster" }));
    await waitFor(() =>
      expect(sent).toContainEqual({
        cmd: "pool_setup_choose_cluster",
        args: { context: "prod-eu" },
      }),
    );
  });

  it("offers Docker as the place purlis didn't recommend", async () => {
    const sent = asking();
    render(<PoolSetupTab plane="p" />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Use Docker on this machine instead" }),
    );
    await waitFor(() => expect(sent.map((one) => one.cmd)).toContain("pool_setup_choose_docker"));
  });
});
