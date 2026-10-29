// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ListedRepo, RepoListing, RepoReach } from "./bindings";
import { matches, YourRepos } from "./YourRepos";
import { GetProject } from "./GetProject";

/**
 * **Your repos, beside Get a project's URL box** (#1818): what the list draws from the core's
 * answer, the search, a pick, and what each forge says with no account. The listing itself, the
 * projects-first order and the manifest probes are the core's, tested against the
 * `forge_contract` recordings (`purlis-core`'s `the_repo_list_puts_projects_first`); here the
 * core is a mock that answers as it does.
 */

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  cleanup();
  clearMocks();
});

function repo(path: string, project = false, host = "github.com"): ListedRepo {
  const at = path.lastIndexOf("/");
  return {
    kind: host.includes("gitlab") ? "gitlab" : "github",
    host,
    login: host.includes("gitlab") ? "jdoe" : "octocat",
    path,
    owner: path.slice(0, at),
    name: path.slice(at + 1),
    description: "",
    url: `https://${host}/${path}`,
    project,
  };
}

const SIGNED_IN: RepoReach = {
  kind: "github",
  host: "github.com",
  login: "octocat",
  listed: 3,
  trouble: null,
};
const NO_GITLAB: RepoReach = {
  kind: "gitlab",
  host: "gitlab.com",
  login: null,
  listed: 0,
  trouble:
    "Not signed in to gitlab.com. Sign in to list your GitLab repos, or paste a project's URL above.",
};

/** The core: the list it answers now, changed with `next`, and every call the list made. */
function core(first: RepoListing) {
  const asked: string[] = [];
  let now = first;
  mockIPC((cmd) => {
    asked.push(cmd);
    if (cmd === "forge_repo_list") return now;
    if (cmd === "forge_token_help")
      return {
        host: "github.com",
        scopes: ["repo"],
        create_at: "https://github.com/settings/tokens",
      };
    if (cmd === "forge_sign_in_with_token")
      return {
        id: "01ACCOUNT",
        kind: "github",
        host: "github.com",
        login: "octocat",
        method: "token",
        signed_in: true,
        scopes: ["repo"],
        wider: [],
      };
    return null;
  });
  return {
    lists: () => asked.filter((c) => c === "forge_repo_list").length,
    next: (listing: RepoListing) => {
      now = listing;
    },
  };
}

const LISTED: RepoListing = {
  repos: [repo("acme/team-project", true), repo("acme/api"), repo("octocat/dots")],
  reaches: [SIGNED_IN, NO_GITLAB],
};

/** Each repo's button, by its words, which are its name: its path, and its mark (WCAG 2.5.3). */
function rows(): string[] {
  return within(screen.getByRole("list", { name: "Your repos" }))
    .getAllByRole("button")
    .map((b) => b.textContent ?? "");
}

describe("matches", () => {
  it("finds a repo by its name, its owner, or both, whatever the case", () => {
    const api = repo("acme/API");
    expect(matches(api, "")).toBe(true);
    expect(matches(api, "api")).toBe(true);
    expect(matches(api, "ACME")).toBe(true);
    expect(matches(api, "acme api")).toBe(true);
    expect(matches(api, "acme/api")).toBe(true);
    expect(matches(api, "acme web")).toBe(false);
    expect(matches(repo("grp/sub/handbook"), "sub hand")).toBe(true);
  });
});

describe("your repos (#1818)", () => {
  it("lists the repos as the core ordered them, the projects marked", async () => {
    core(LISTED);
    render(<YourRepos onPick={vi.fn()} busy={false} />);

    await screen.findByRole("list", { name: "Your repos" });
    expect(rows()).toEqual(["acme/team-project purlis project", "acme/api", "octocat/dots"]);
    expect(screen.getAllByText("purlis project")).toHaveLength(1);
  });

  it("searches by name and owner, and says what to do when nothing matches", async () => {
    core(LISTED);
    const person = userEvent.setup();
    render(<YourRepos onPick={vi.fn()} busy={false} />);
    await screen.findByRole("list", { name: "Your repos" });

    await person.type(screen.getByLabelText("Search your repos"), "octocat");
    expect(rows()).toEqual(["octocat/dots"]);

    await person.clear(screen.getByLabelText("Search your repos"));
    await person.type(screen.getByLabelText("Search your repos"), "acme api");
    expect(rows()).toEqual(["acme/api"]);

    await person.type(screen.getByLabelText("Search your repos"), "zzz");
    expect(screen.queryByRole("list", { name: "Your repos" })).toBeNull();
    expect(screen.getByText(/No repo matches/)).toHaveTextContent("Paste its URL above instead");
  });

  it("hands a picked repo's address to the page", async () => {
    core(LISTED);
    const onPick = vi.fn();
    const person = userEvent.setup();
    render(<YourRepos onPick={onPick} busy={false} />);
    await screen.findByRole("list", { name: "Your repos" });

    await person.click(screen.getByRole("button", { name: /^acme\/team-project/ }));

    expect(onPick).toHaveBeenCalledWith("https://github.com/acme/team-project");
  });

  it("picks nothing while a clone runs", async () => {
    core(LISTED);
    render(<YourRepos onPick={vi.fn()} busy />);
    await screen.findByRole("list", { name: "Your repos" });
    expect(screen.getByRole("button", { name: /^acme\/api/ })).toBeDisabled();
  });

  it("says, with no account at all, to sign in or paste a URL, and signs in inline", async () => {
    const nobody: RepoListing = {
      repos: [],
      reaches: [
        {
          ...NO_GITLAB,
          kind: "github",
          host: "github.com",
          trouble:
            "Not signed in to github.com. Sign in to list your GitHub repos, or paste a project's URL above.",
        },
        NO_GITLAB,
      ],
    };
    const forge = core(nobody);
    const person = userEvent.setup();
    render(<YourRepos onPick={vi.fn()} busy={false} />);

    const forges = await screen.findByRole("list", { name: "Your forges" });
    expect(forges).toHaveTextContent("Not signed in to github.com. Sign in");
    expect(forges).toHaveTextContent("paste a project's URL above");
    expect(screen.queryByRole("list", { name: "Your repos" })).toBeNull();

    await person.click(within(forges).getAllByRole("button", { name: "Sign in…" })[0]);
    // The token sign-in opens here, on GitHub; once it signs in, the list is read again.
    const dialog = await screen.findByRole("dialog");
    const asked = forge.lists();
    forge.next(LISTED);
    await person.type(within(dialog).getByLabelText("Token"), "pasted-for-the-test");
    await vi.waitFor(() =>
      expect(within(dialog).getByRole("button", { name: "Sign in" })).toBeEnabled(),
    );
    await person.click(within(dialog).getByRole("button", { name: "Sign in" }));

    await vi.waitFor(() => expect(rows()).toHaveLength(3));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(forge.lists()).toBe(asked + 1);
  });

  it("says why an account would not list, with Sign in again and Retry, and lists the rest", async () => {
    const refused: RepoListing = {
      repos: [repo("grp/api", false, "gitlab.com")],
      reaches: [
        {
          ...SIGNED_IN,
          listed: 0,
          trouble: "github.com refused purlis's sign-in as octocat. Sign in again.",
        },
        { ...NO_GITLAB, login: "jdoe", listed: 1, trouble: null },
      ],
    };
    const forge = core(refused);
    const person = userEvent.setup();
    render(<YourRepos onPick={vi.fn()} busy={false} />);

    const forges = await screen.findByRole("list", { name: "Your forges" });
    expect(forges).toHaveTextContent("refused purlis's sign-in as octocat");
    expect(forges).toHaveTextContent("1 repo on gitlab.com as jdoe.");
    expect(within(forges).getByRole("button", { name: "Sign in again…" })).toBeInTheDocument();
    expect(rows()).toEqual(["grp/api"]);

    const asked = forge.lists();
    forge.next(LISTED);
    await person.click(within(forges).getByRole("button", { name: "Retry" }));
    await vi.waitFor(() => expect(rows()).toHaveLength(3));
    expect(forge.lists()).toBe(asked + 1);
  });

  it("says why the list could not be asked for at all", async () => {
    mockIPC((cmd) => {
      if (cmd === "forge_repo_list") throw new Error("the window lost the core");
      return null;
    });
    render(<YourRepos onPick={vi.fn()} busy={false} />);
    expect(await screen.findByRole("status")).toHaveTextContent("the window lost the core");
  });
});

describe("a repo picked on Get a project (#1818 with #1815)", () => {
  it("is cloned and opened as a pasted URL is", async () => {
    const asked: { cmd: string; args: Record<string, unknown> }[] = [];
    mockIPC((cmd, args) => {
      const given = (args ?? {}) as Record<string, unknown>;
      asked.push({ cmd, args: given });
      if (cmd === "forge_repo_list") return LISTED;
      if (cmd === "project_url_folder")
        return { name: "team-project", folder: "/Users/someone/Projects/team-project" };
      if (cmd === "get_project") return "/Users/someone/Projects/team-project";
      return null;
    });
    const onOpen = vi.fn();
    const person = userEvent.setup();
    render(<GetProject onBack={vi.fn()} onOpen={onOpen} onOpenFolder={vi.fn()} />);

    await person.click(await screen.findByRole("button", { name: /^acme\/team-project/ }));

    await vi.waitFor(() =>
      expect(onOpen).toHaveBeenCalledWith("/Users/someone/Projects/team-project"),
    );
    const got = asked.find((one) => one.cmd === "get_project");
    expect(got?.args.url).toBe("https://github.com/acme/team-project");
    expect(got?.args.folder).toBe("/Users/someone/Projects/team-project");
    // The box shows what was got, so Retry gets it again.
    expect(screen.getByLabelText("Project URL")).toHaveValue(
      "https://github.com/acme/team-project",
    );
  });
});
