// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Channel } from "@tauri-apps/api/core";
import type { CreateProgress, ForgeOwners } from "./bindings";
import { LIVES_HERE, LivesHere } from "./LivesHere";
import { settingsPageIn } from "./PublishProject";
import { ReadOnlySaid } from "./ReadOnlyLine";
import { pickIn } from "./ui/select/select.testkit";
import { tipOf } from "./ui/tooltip/tooltip.testkit";

/**
 * **A project that lives only on this machine says so, and publishes** (#1820), as the window
 * draws it: the line and its Publish while the core says the project lives only here; the
 * dialog's form (#1819's: forge, name, owner, private by default) reaching the core; each stage
 * with Cancel; a refusal with Retry and, for a repo the forge kept, its settings page; and the
 * line gone once the publish lands. The publish itself is the core's, tested against the forge
 * contract's recordings in `purlis-core`'s `publishproject`; here the core is a mock that
 * answers as it does.
 */

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

const PLANE = "/Users/someone/notebook";
const STAGES = [
  "Checking the owner on GitHub",
  "Committing what is not committed yet",
  "Creating the repo on GitHub",
  "Pushing the project's history",
];
const OWNERS: ForgeOwners = {
  host: "github.com",
  login: "octocat",
  owners: [
    { name: "octocat", title: "The Octocat", mine: true },
    { name: "acme", title: "acme", mine: false },
  ],
};
const LINE = { name: "Only on this machine" };
/** The line, also while a dialog hides the rest of the window from the accessibility tree. */
const LINE_BEHIND = { ...LINE, hidden: true };

afterEach(() => {
  cleanup();
  clearMocks();
});

/** The core: the project lives only here until a publish lands. */
function core({ owners = OWNERS, livesHere = true } = {}) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  let here = livesHere;
  let finish: (answer: string | null) => void = () => {};
  let fail: (sentence: string) => void = () => {};
  let progress: Channel<CreateProgress> | undefined;
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "project_lives_here") return here ? "notebook" : null;
    if (cmd === "forge_owners") return owners;
    if (cmd === "publish_project") {
      progress = given.progress as Channel<CreateProgress>;
      return new Promise((resolve, reject) => {
        finish = (answer) => {
          if (answer !== null) here = false;
          resolve(answer);
        };
        fail = (sentence) => reject(sentence);
      });
    }
    return null;
  });
  return {
    asked,
    published: () => asked.filter((one) => one.cmd === "publish_project"),
    finish: (answer: string | null) => finish(answer),
    fail: (sentence: string) => fail(sentence),
    tell: (at: number) => progress?.onmessage({ stage: STAGES[at], at, stages: STAGES }),
  };
}

async function openPublish() {
  const person = userEvent.setup();
  const line = await screen.findByRole("note", LINE);
  expect(line).toHaveTextContent(LIVES_HERE);
  await person.click(within(line).getByRole("button", { name: "Publish to GitHub or GitLab…" }));
  const dialog = await screen.findByRole("dialog");
  await within(dialog).findByLabelText("Owner");
  return { person, dialog };
}

describe("a project that lives only on this machine (#1820)", () => {
  it("says so plainly on its tab, with Publish beside it", async () => {
    core();
    render(<LivesHere plane={PLANE} />);
    const line = await screen.findByRole("note", LINE);
    expect(line).toHaveTextContent("This project lives only on this machine.");
    expect(
      within(line).getByRole("button", { name: "Publish to GitHub or GitLab…" }),
    ).toBeEnabled();
  });

  it("draws nothing for a project that has a remote", async () => {
    const mocked = core({ livesHere: false });
    render(<LivesHere plane={PLANE} />);
    await vi.waitFor(() =>
      expect(mocked.asked.some((one) => one.cmd === "project_lives_here")).toBe(true),
    );
    expect(screen.queryByRole("note", LINE)).not.toBeInTheDocument();
  });

  it("says it in the settings' sentence too, with the same Publish", async () => {
    core();
    render(
      <p>
        This project, for everyone who opens it.
        <LivesHere plane={PLANE} as="words" />
      </p>,
    );
    expect(await screen.findByRole("note", LINE)).toHaveTextContent(LIVES_HERE);
    expect(screen.getByRole("button", { name: "Publish to GitHub or GitLab…" })).toBeEnabled();
  });

  it("turns Publish off on a project this purlis opens read-only, saying why", async () => {
    core();
    render(
      <ReadOnlySaid.Provider value="purlis opens the project read-only.">
        <LivesHere plane={PLANE} />
      </ReadOnlySaid.Provider>,
    );
    const publish = await screen.findByRole("button", { name: "Publish to GitHub or GitLab…" });
    expect(publish).toHaveAttribute("aria-disabled", "true");
    expect(tipOf(publish)).toEqual("purlis opens the project read-only.");
  });

  it("publishes with #1819's form, private by default, and the line goes once it lands", async () => {
    const publishing = core();
    render(<LivesHere plane={PLANE} />);
    const { person, dialog } = await openPublish();
    expect(within(dialog).getByLabelText("Name")).toHaveValue("notebook");
    expect(within(dialog).getByLabelText("Owner")).toHaveValue("octocat");
    expect(within(dialog).getByRole("radio", { name: /GitHub/ })).toBeChecked();
    expect(within(dialog).getByRole("radio", { name: /Private/ })).toBeChecked();

    await person.click(within(dialog).getByRole("button", { name: "Publish" }));
    await vi.waitFor(() => expect(publishing.published()).toHaveLength(1));
    expect(publishing.published()[0].args).toMatchObject({
      plane: PLANE,
      ask: { forge: "github", owner: "octocat", name: "notebook", private: true },
    });

    publishing.tell(3);
    const list = await within(dialog).findByRole("list", { name: "Publishing notebook" });
    expect(list.querySelectorAll("li")).toHaveLength(4);
    expect(within(dialog).getByText("Pushing the project's history…")).toHaveAttribute(
      "aria-current",
      "step",
    );

    publishing.finish("https://github.com/octocat/notebook");
    expect(
      await within(dialog).findByText("https://github.com/octocat/notebook"),
    ).toBeInTheDocument();
    // The project has its remote now: the line and its Publish are gone.
    await vi.waitFor(() => expect(screen.queryByRole("note", LINE_BEHIND)).not.toBeInTheDocument());
    await person.click(within(dialog).getByRole("button", { name: "Done" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Publish to GitHub/ })).not.toBeInTheDocument();
  });

  it("sends GitLab, another owner and public when the person picks them", async () => {
    const publishing = core();
    render(<LivesHere plane={PLANE} />);
    const { person, dialog } = await openPublish();
    await person.click(within(dialog).getByRole("radio", { name: /GitLab/ }));
    await pickIn(await within(dialog).findByLabelText("Owner"), "acme", person);
    await person.clear(within(dialog).getByLabelText("Name"));
    await person.type(within(dialog).getByLabelText("Name"), "garden");
    await person.click(within(dialog).getByRole("radio", { name: /Public/ }));
    await person.click(within(dialog).getByRole("button", { name: "Publish" }));
    await vi.waitFor(() => expect(publishing.published()).toHaveLength(1));
    expect(publishing.published()[0].args.ask).toEqual({
      forge: "gitlab",
      owner: "acme",
      name: "garden",
      private: false,
    });
  });

  it("calls the publish off with Cancel, and the line stays", async () => {
    const publishing = core();
    render(<LivesHere plane={PLANE} />);
    const { person, dialog } = await openPublish();
    await person.click(within(dialog).getByRole("button", { name: "Publish" }));
    publishing.tell(2);
    await person.click(await within(dialog).findByRole("button", { name: "Cancel" }));
    expect(publishing.asked.some((one) => one.cmd === "cancel_create_project")).toBe(true);
    publishing.finish(null);
    expect(
      await within(dialog).findByText("Cancelled. Nothing was left behind."),
    ).toBeInTheDocument();
    expect(screen.getByRole("note", LINE_BEHIND)).toBeInTheDocument();
  });

  it("says why it failed, links the settings of a repo the forge kept, and Retry pushes again", async () => {
    const publishing = core();
    const opened: string[] = [];
    render(<LivesHere plane={PLANE} />);
    const { person, dialog } = await openPublish();
    mockIPC((cmd, args) => {
      const given = (args ?? {}) as Record<string, unknown>;
      publishing.asked.push({ cmd, args: given });
      if (cmd === "plugin:opener|open_url") opened.push(String(given.url));
      if (cmd === "project_lives_here") return "notebook";
      if (cmd === "forge_owners") return OWNERS;
      if (cmd === "publish_project")
        return Promise.reject(
          "git could not push the project to https://github.com/octocat/notebook: denied. purlis " +
            "made https://github.com/octocat/notebook and could not delete it (forbidden), so it " +
            "is kept, empty: Retry pushes the project into it. To remove it instead, delete it " +
            "yourself at https://github.com/octocat/notebook/settings, under Danger Zone, Delete " +
            "this repository.",
        );
      return null;
    });
    await person.click(within(dialog).getByRole("button", { name: "Publish" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "Retry pushes the project into it",
    );
    await person.click(within(dialog).getByRole("button", { name: "Open the repo's settings" }));
    await vi.waitFor(() =>
      expect(opened).toEqual(["https://github.com/octocat/notebook/settings"]),
    );
    await person.click(within(dialog).getByRole("button", { name: "Retry" }));
    await vi.waitFor(() => expect(publishing.published()).toHaveLength(2));
  });

  it("asks for a sign-in when this machine has none to the forge picked", async () => {
    core({ owners: { host: null, login: "", owners: [] } });
    render(<LivesHere plane={PLANE} />);
    const person = userEvent.setup();
    await person.click(await screen.findByRole("button", { name: "Publish to GitHub or GitLab…" }));
    const dialog = await screen.findByRole("dialog");
    expect(
      await within(dialog).findByRole("button", { name: "Sign in to GitHub…" }),
    ).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Publish" })).toBeDisabled();
  });
});

describe("the settings page a kept repo's sentence names", () => {
  it("is the GitHub settings or the GitLab edit page, and nothing else", () => {
    expect(
      settingsPageIn("delete it yourself at https://github.com/a/b/settings, under Danger Zone"),
    ).toBe("https://github.com/a/b/settings");
    expect(settingsPageIn("at https://gitlab.com/a/b/edit, under Advanced")).toBe(
      "https://gitlab.com/a/b/edit",
    );
    expect(settingsPageIn("GitHub already has a repo named a/b. Nothing was left behind.")).toBe(
      undefined,
    );
  });
});
