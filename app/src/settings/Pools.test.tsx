// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "../App";
import type {
  ClusterPoolRow,
  PoolLink,
  PoolProfileRow,
  PoolRow,
  PoolVersion,
  PoolsShown,
  RunnerImageOffered,
} from "../bindings";
import { doneWithTheAddedPool, stopAddingAPool } from "../pools";
import { forgetThisLaunch } from "../regions";
import { GLOBAL } from "../windowprefs";

/**
 * **Settings › You › This machine › Pools** (#2679), in the whole window: the group is always
 * listed (the person's ruling of 2026-10-30), and says what a pool is until one is added; Add a
 * pool…, there and in the palette, adds the first, with the limits typed and the defaults for the
 * rest, its image the one this version's release names, or a newer one's where an update is
 * waiting; and a pool whose runner answered with another key has a Notice whose Remove removes
 * it. A runner of another version than this machine's says so, and what follows (#2749). Adding
 * a pool ends with this machine's profiles, each to sign in on its runner (#2049).
 */

vi.mock("../SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const IMAGE = `ghcr.io/purlis/purlis-runner@sha256:${"0".repeat(64)}`;
const DEFAULTS = { cpus: "2", memory_mib: 4096, processes: 1024, tmp_mib: 1024 };

const a_pool = (name: string, suspend_after = "24h"): PoolRow => ({
  name,
  image: IMAGE,
  limits: { ...DEFAULTS },
  suspend_after,
});

const OPERATOR_KEY = `ed25519:${"ab".repeat(32)}`;

const a_cluster_pool = (name: string): ClusterPoolRow => ({
  name,
  context: "team",
  pool: "default",
  namespace: "op",
  operator_key: OPERATOR_KEY,
  owned: false,
});

let pools: PoolRow[];
let clusters: ClusterPoolRow[];
let clustersAdded: unknown[];
let link: PoolLink;
let version: PoolVersion;
let added: unknown[];
let removed: unknown[];
let madeAgain: string[];
let notHeld: { root: string; name: string; why: string; pools: string[] }[];
let profilesAsked: unknown[];
/** The runner image a new pool is offered, or why it could not be read. */
let releaseImage: RunnerImageOffered | { trouble: string };
/** The profiles chosen for each pool, by its name, and what Add and Remove asked (#4047). */
let chosen: string[];
let profileChanges: unknown[];
/** What `pool_profiles` keeps, in turn, where a test says; the last again once the rest are. */
let profileAnswers: PoolProfileRow[][] | undefined;
/** Whether `pool_profiles` says the runner is on a fresh claim (#3819). */
let freshClaim: boolean;

const a_profile = (name: string, standing: string): PoolProfileRow => ({
  name,
  kind: "claude",
  harness: "Claude Code",
  standing,
  not_kept: null,
  plans: [],
});

beforeEach(() => {
  forgetThisLaunch();
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: { path: "/home/op/.config/charter/layout.json", found: false, document: null },
    theme: { path: "", found: false, document: null, trouble: null },
  };
  pools = [];
  clusters = [];
  clustersAdded = [];
  link = { state: "linked" };
  version = {
    runner: "0.4.2",
    why: null,
    desktop: "0.4.2",
    follows: true,
    draining: 0,
    held: 0,
    named: null,
    waiting: 0,
  };
  added = [];
  removed = [];
  madeAgain = [];
  notHeld = [];
  profilesAsked = [];
  releaseImage = { state: "local_build" };
  chosen = ["work", "claude"];
  profileChanges = [];
  freshClaim = false;
  profileAnswers = undefined;
  mockIPC(
    (cmd, args) => {
      const a = (args ?? {}) as Record<string, unknown>;
      if (cmd === "runner_image_of_this_version") {
        if ("trouble" in releaseImage) throw releaseImage.trouble;
        return releaseImage;
      }
      if (cmd === "plane_at_launch") return { plane: null, from: null, why: "no plane here" };
      if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
      if (cmd === "planes_to_restore") return { windows: [], dropped: [], gone: [] };
      if (cmd === "this_machine") return { projects: [], dropped: [], forgetful: null };
      if (cmd === "update_channel") return "stable";
      if (cmd === "docker_pools")
        return {
          pools: structuredClone(pools),
          defaults: { ...DEFAULTS },
          suspend_after: "24h",
          frames: 15,
          width: 1280,
          clusters: structuredClone(clusters),
        } satisfies PoolsShown;
      if (cmd === "cluster_contexts")
        return { kubeconfig: "/home/op/.kube/config", contexts: ["team", "kind"], current: "team" };
      if (cmd === "cluster_pool_key") return { key: OPERATOR_KEY, namespace: "op" };
      if (cmd === "add_cluster_pool") {
        clustersAdded.push(a.adding);
        const row = a_cluster_pool((a.adding as { name: string }).name);
        clusters = [...clusters, row];
        return row;
      }
      if (cmd === "set_cluster_pool_owned") {
        clusters = clusters.map((one) =>
          one.name === a.name ? { ...one, owned: a.owned as boolean } : one,
        );
        return null;
      }
      if (cmd === "docker_pool_link") return link;
      if (cmd === "docker_pool_make_again") {
        madeAgain.push(a.name as string);
        link = { state: "linked" };
        return { sign_ins_kept: true };
      }
      if (cmd === "docker_pool_version") return version;
      if (cmd === "projects_not_held") return structuredClone(notHeld);
      if (cmd === "pool_profiles") {
        profilesAsked.push(a.pool);
        if (profileAnswers !== undefined)
          return {
            kept: profileAnswers.length > 1 ? profileAnswers.shift() : profileAnswers[0],
            addable: [],
            fresh_claim: freshClaim,
          };
        const here = [a_profile("work", "signed-out"), a_profile("claude", "signed-in")];
        const home = { ...a_profile("home", "signed-out"), kind: "codex", harness: "Codex" };
        const all = [...here, home];
        return {
          kept: chosen.flatMap((name) => all.filter((one) => one.name === name)),
          addable: all
            .filter((one) => !chosen.includes(one.name))
            .map((one) => ({ name: one.name, harness: one.harness })),
          fresh_claim: freshClaim,
        };
      }
      if (cmd === "pool_profile_add") {
        profileChanges.push(["add", a.pool, a.name]);
        chosen = [...chosen, a.name as string];
        return null;
      }
      if (cmd === "pool_profile_remove") {
        profileChanges.push(["remove", a.pool, a.name]);
        chosen = chosen.filter((one) => one !== a.name);
        return null;
      }
      if (cmd === "add_docker_pool") {
        added.push(a.adding);
        const row = a_pool((a.adding as { name: string }).name);
        pools = [...pools, row];
        return row;
      }
      if (cmd === "remove_docker_pool") {
        removed.push(a.name);
        pools = pools.filter((one) => one.name !== a.name);
        clusters = clusters.filter((one) => one.name !== a.name);
        return null;
      }
      return null;
    },
    { shouldMockEvents: true },
  );
});
afterEach(() => {
  cleanup();
  // The dialogs' open state is the window's, kept across renders: a test that leaves the add
  // dialog, or the sign-in that adding ends with, open must not leave it over the next test's
  // window.
  stopAddingAPool();
  doneWithTheAddedPool();
  clearMocks();
  Reflect.deleteProperty(globalThis, GLOBAL);
});

async function palette(row: string) {
  await userEvent.keyboard("{F2}");
  await screen.findByRole("dialog", { name: "Command palette" });
  await userEvent.keyboard(row);
  await userEvent.keyboard("{Enter}");
}

async function yourSettings() {
  await palette("Your settings…");
  return screen.findByRole("navigation", { name: "Groups" });
}

describe("You › This machine › Pools", () => {
  it("is listed while this machine keeps no pool, says what a pool is, and adds the first", async () => {
    // The person's ruling of 2026-10-30: shown, it announces pools; in the palette alone it hid
    // them.
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    expect(
      await within(group).findByText(
        /No pool yet\. A pool is a runner on your Docker, or one claimed in your team's cluster, that chats and browsers can move to/,
      ),
    ).toBeInTheDocument();
    expect(within(group).queryByText(/There is no undo/)).toBeNull();
    await userEvent.click(within(group).getByRole("button", { name: "Add a pool…" }));
    const dialog = await screen.findByRole("dialog", { name: "Add a pool" });
    await userEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add a pool" })).toBeNull());
  });

  it("lists its pools with Add a pool… beside them", async () => {
    pools = [a_pool("builds")];
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    expect(
      await within(group).findByRole("button", { name: "Remove pool builds" }),
    ).toBeInTheDocument();
    expect(within(group).queryByText(/No pool yet/)).toBeNull();
    expect(
      within(group).getByText(/Remove takes this machine out of the runner's peers/),
    ).toBeInTheDocument();
    expect(within(group).getByRole("button", { name: "Add a pool…" })).toBeInTheDocument();
  });

  it("asks before the row's Remove removes a pool", async () => {
    pools = [a_pool("builds")];
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    await userEvent.click(await within(group).findByRole("button", { name: "Remove pool builds" }));
    const asked = await screen.findByRole("alertdialog", { name: "Remove pool builds?" });
    expect(removed).toEqual([]);
    await userEvent.click(within(asked).getByRole("button", { name: "Remove pool" }));
    await waitFor(() => expect(removed).toEqual(["builds"]));
  });

  it("adds the first pool from the palette, with the limits typed and the defaults for the rest", async () => {
    render(<App />);
    await palette("Add a pool…");
    const dialog = await screen.findByRole("dialog", { name: "Add a pool" });
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Name" }), "builds");
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Image" }), IMAGE);
    await userEvent.type(within(dialog).getByRole("textbox", { name: "CPUs" }), "1.5");
    await userEvent.click(within(dialog).getByRole("button", { name: "Add pool" }));

    await waitFor(() =>
      expect(added).toEqual([
        {
          name: "builds",
          image: IMAGE,
          cpus: "1.5",
          memory_mib: null,
          processes: null,
          tmp_mib: null,
          suspend_after: null,
          frames: null,
          width: null,
        },
      ]),
    );
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add a pool" })).toBeNull());
    // Adding it ends with choosing the profiles that sign in on its runner (#2049, #4048).
    const signIn = await screen.findByRole("dialog", {
      name: "Sign your profiles in on pool builds",
    });
    const choose = await within(signIn).findByRole("group", {
      name: "Profiles that sign in on pool builds",
    });
    expect(within(choose).getByRole("checkbox", { name: "work" })).toBeChecked();
    expect(within(choose).getByRole("checkbox", { name: "home" })).not.toBeChecked();
    await userEvent.click(within(signIn).getByRole("button", { name: "Not now" }));
    const list = await within(signIn).findByRole("list", { name: "Profiles on pool builds" });
    expect(within(list).getByText(/Not signed in on pool builds/)).toBeInTheDocument();
    expect(within(list).getByText("Signed in on pool builds.")).toBeInTheDocument();
    expect(profilesAsked).toEqual(["builds"]);
    await userEvent.click(within(signIn).getByRole("button", { name: "Done" }));
    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "Sign your profiles in on pool builds" }),
      ).toBeNull(),
    );
    const nav = await yourSettings();
    expect(await within(nav).findByRole("button", { name: "Pools" })).toBeInTheDocument();
  });

  it("keeps only the profiles chosen for a pool, and Add a profile… and Remove change them (#4047)", async () => {
    pools = [a_pool("builds")];
    chosen = ["work"];
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    const list = await within(group).findByRole("list", { name: "Profiles on pool builds" });
    expect(within(list).getByText("work")).toBeInTheDocument();
    expect(within(list).queryByText("claude")).toBeNull();

    await userEvent.click(within(group).getByRole("button", { name: "Add a profile…" }));
    const menu = await screen.findByRole("menu", { name: "Add a profile…" });
    expect(within(menu).queryByRole("menuitem", { name: /work/ })).toBeNull();
    await userEvent.click(within(menu).getByRole("menuitem", { name: /home/ }));
    await waitFor(() => expect(profileChanges).toEqual([["add", "builds", "home"]]));
    expect(
      await within(group).findByRole("button", { name: "Remove home from pool builds" }),
    ).toBeInTheDocument();

    // Remove asks first, and says the sign-in there goes with it.
    await userEvent.click(
      within(group).getByRole("button", { name: "Remove work from pool builds" }),
    );
    expect(within(group).getByText(/its sign-in there goes with it/)).toBeInTheDocument();
    expect(profileChanges).toHaveLength(1);
    await userEvent.click(within(group).getByRole("button", { name: "Keep work on pool builds" }));
    expect(profileChanges).toHaveLength(1);
    await userEvent.click(
      within(group).getByRole("button", { name: "Remove work from pool builds" }),
    );
    await userEvent.click(
      within(group).getByRole("button", { name: "Yes, remove work from pool builds" }),
    );
    await waitFor(() =>
      expect(profileChanges).toEqual([
        ["add", "builds", "home"],
        ["remove", "builds", "work"],
      ]),
    );
    await waitFor(() =>
      expect(
        within(group).queryByRole("button", { name: "Remove work from pool builds" }),
      ).toBeNull(),
    );
  });

  it("says a profile's harness is being fetched, and asks again until it is (#4008)", async () => {
    profileAnswers = [[a_profile("work", "fetching")], [a_profile("work", "signed-in")]];
    render(<App />);
    await palette("Add a pool…");
    const dialog = await screen.findByRole("dialog", { name: "Add a pool" });
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Name" }), "builds");
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Image" }), IMAGE);
    await userEvent.click(within(dialog).getByRole("button", { name: "Add pool" }));

    const signIn = await screen.findByRole("dialog", {
      name: "Sign your profiles in on pool builds",
    });
    await within(signIn).findByRole("group", { name: "Profiles that sign in on pool builds" });
    await userEvent.click(within(signIn).getByRole("button", { name: "Not now" }));
    const list = await within(signIn).findByRole("list", { name: "Profiles on pool builds" });
    expect(within(list).getByText("Fetching Claude Code for this runner…")).toBeInTheDocument();
    expect(
      await within(list).findByText("Signed in on pool builds.", {}, { timeout: 6000 }),
    ).toBeInTheDocument();
    expect(profilesAsked).toEqual(["builds", "builds"]);
    await userEvent.click(within(signIn).getByRole("button", { name: "Done" }));
    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "Sign your profiles in on pool builds" }),
      ).toBeNull(),
    );
  }, 10000);

  it("adds a pool with its own time before its runner suspends, and its row says it (#3471)", async () => {
    render(<App />);
    await palette("Add a pool…");
    const dialog = await screen.findByRole("dialog", { name: "Add a pool" });
    const after = within(dialog).getByRole("textbox", { name: "Suspends after" });
    expect(after).toHaveAttribute("placeholder", "24h");
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Name" }), "nightly");
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Image" }), IMAGE);
    await userEvent.type(after, "12h");
    await userEvent.click(within(dialog).getByRole("button", { name: "Add pool" }));
    await waitFor(() =>
      expect(added).toEqual([
        {
          name: "nightly",
          image: IMAGE,
          cpus: null,
          memory_mib: null,
          processes: null,
          tmp_mib: null,
          suspend_after: "12h",
          frames: null,
          width: null,
        },
      ]),
    );
    const signIn = await screen.findByRole("dialog", {
      name: "Sign your profiles in on pool nightly",
    });
    await userEvent.click(within(signIn).getByRole("button", { name: "Done" }));
    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "Sign your profiles in on pool nightly" }),
      ).toBeNull(),
    );
  });

  it("adds a pool whose runner's browsers are watched at frames and a width of its own", async () => {
    // #3297: a pool's browser frame limits over the link are its own settings.
    render(<App />);
    await palette("Add a pool…");
    const dialog = await screen.findByRole("dialog", { name: "Add a pool" });
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Name" }), "fast");
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Image" }), IMAGE);
    const frames = within(dialog).getByRole("textbox", { name: "Browser frames a second" });
    const width = within(dialog).getByRole("textbox", { name: "Browser width in pixels" });
    expect(frames).toHaveAttribute("placeholder", "15");
    expect(width).toHaveAttribute("placeholder", "1280");
    await userEvent.type(frames, "30");
    await userEvent.type(width, "1920");
    await userEvent.click(within(dialog).getByRole("button", { name: "Add pool" }));

    await waitFor(() =>
      expect(added).toEqual([
        {
          name: "fast",
          image: IMAGE,
          cpus: null,
          memory_mib: null,
          processes: null,
          tmp_mib: null,
          suspend_after: null,
          frames: 30,
          width: 1920,
        },
      ]),
    );
    const signIn = await screen.findByRole("dialog", {
      name: "Sign your profiles in on pool fast",
    });
    await userEvent.click(within(signIn).getByRole("button", { name: "Done" }));
    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "Sign your profiles in on pool fast" }),
      ).toBeNull(),
    );
  });

  it("adds a pool of the runner image this version's release names when Image is left empty", async () => {
    releaseImage = { state: "ours", image: IMAGE };
    render(<App />);
    await palette("Add a pool…");
    const dialog = await screen.findByRole("dialog", { name: "Add a pool" });
    const image = within(dialog).getByRole("textbox", { name: "Image" });
    await waitFor(() => expect(image).toHaveAttribute("placeholder", IMAGE));
    expect(image).toHaveAccessibleDescription(/Empty is the default/);
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Name" }), "builds");
    await userEvent.click(within(dialog).getByRole("button", { name: "Add pool" }));
    await waitFor(() =>
      expect(added).toEqual([expect.objectContaining({ name: "builds", image: IMAGE })]),
    );
    const signIn = await screen.findByRole("dialog", {
      name: "Sign your profiles in on pool builds",
    });
    await userEvent.click(within(signIn).getByRole("button", { name: "Done" }));
    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "Sign your profiles in on pool builds" }),
      ).toBeNull(),
    );
  });

  it("offers a newer release's runner image where an update of purlis is waiting, and says so", async () => {
    // A dev machine is behind its channel after every merge: the channel's newest names a newer
    // version's image, signed for that version.
    releaseImage = { state: "newer", image: IMAGE, version: "0.4.3-dev.9" };
    render(<App />);
    await palette("Add a pool…");
    const dialog = await screen.findByRole("dialog", { name: "Add a pool" });
    const image = within(dialog).getByRole("textbox", { name: "Image" });
    await waitFor(() => expect(image).toHaveAttribute("placeholder", IMAGE));
    expect(image).toHaveAccessibleDescription(
      /Empty is the default: the runner image of purlis 0\.4\.3-dev\.9, the newest release on your update channel\. An update of purlis to 0\.4\.3-dev\.9 is waiting: update first/,
    );
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Name" }), "builds");
    await userEvent.click(within(dialog).getByRole("button", { name: "Add pool" }));
    await waitFor(() =>
      expect(added).toEqual([expect.objectContaining({ name: "builds", image: IMAGE })]),
    );
    const signIn = await screen.findByRole("dialog", {
      name: "Sign your profiles in on pool builds",
    });
    await userEvent.click(within(signIn).getByRole("button", { name: "Done" }));
    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "Sign your profiles in on pool builds" }),
      ).toBeNull(),
    );
  });

  it("says a local build names no runner image, and asks for one by its digest", async () => {
    releaseImage = { state: "local_build" };
    render(<App />);
    await palette("Add a pool…");
    const dialog = await screen.findByRole("dialog", { name: "Add a pool" });
    const image = within(dialog).getByRole("textbox", { name: "Image" });
    await waitFor(() =>
      expect(image).toHaveAccessibleDescription(
        /This is a local build of purlis, with no update channel, so it names no runner image\. Name one by its digest/,
      ),
    );
    expect(image).toHaveAttribute("placeholder", "ghcr.io/purlis/purlis-runner@sha256:…");
  });

  it("says a release that names no runner image names none", async () => {
    releaseImage = { state: "none_named" };
    render(<App />);
    await palette("Add a pool…");
    const dialog = await screen.findByRole("dialog", { name: "Add a pool" });
    const image = within(dialog).getByRole("textbox", { name: "Image" });
    await waitFor(() =>
      expect(image).toHaveAccessibleDescription(
        /This version's release names no runner image, as a release whose runner image was not built does\. Name one by its digest/,
      ),
    );
    expect(image).toHaveAttribute("placeholder", "ghcr.io/purlis/purlis-runner@sha256:…");
  });

  it("says why the runner image to offer could not be read, and offers none", async () => {
    releaseImage = {
      trouble: "the runner image's signature is not the updater key's over its line",
    };
    render(<App />);
    await palette("Add a pool…");
    const dialog = await screen.findByRole("dialog", { name: "Add a pool" });
    const image = within(dialog).getByRole("textbox", { name: "Image" });
    await waitFor(() =>
      expect(image).toHaveAccessibleDescription(
        /purlis could not read which runner image to offer, so name one by its digest: the runner image's signature is not the updater key's over its line/,
      ),
    );
    expect(image).toHaveAttribute("placeholder", "ghcr.io/purlis/purlis-runner@sha256:…");
  });

  it("says on each row how long its runner is left without a link before it suspends", async () => {
    pools = [a_pool("nightly", "12h"), a_pool("builds")];
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    expect(
      await within(group).findByText("Suspends after 12h without a link from this machine"),
    ).toBeInTheDocument();
    expect(
      within(group).getByText("Suspends after 24h without a link from this machine"),
    ).toBeInTheDocument();
  });

  it("says a runner made before purlis suspended runners never suspends (#3717)", async () => {
    pools = [a_pool("builds")];
    version = { ...version, suspends: { made: "never" } };
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    expect(
      await within(group).findByText(
        "Never suspends: its runner was made before purlis suspended runners",
      ),
    ).toBeInTheDocument();
    expect(within(group).queryByText(/^Suspends after/)).toBeNull();
  });

  it("says the time a runner was made with where it is not the pool's (#3717)", async () => {
    pools = [a_pool("nightly", "12h")];
    version = { ...version, suspends: { made: "after", after: "24h" } };
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    expect(
      await within(group).findByText(
        "Suspends after 24h without a link from this machine, as its runner was made (the pool says 12h)",
      ),
    ).toBeInTheDocument();
    expect(
      within(group).queryByText("Suspends after 12h without a link from this machine"),
    ).toBeNull();
  });

  it("says a runner whose key changed in a Notice, whose Remove removes the pool", async () => {
    pools = [a_pool("builds")];
    link = { state: "key_changed" };
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    const notice = await waitFor(() => {
      const found = group.querySelector('[data-cause="pool-key-changed:builds"]');
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    expect(notice).toHaveTextContent(/answered with another key/);

    await userEvent.click(within(notice).getByRole("button", { name: "Remove" }));
    // It asks first, and Cancel changes nothing.
    let asked = await screen.findByRole("alertdialog", { name: "Remove pool builds?" });
    expect(asked).toHaveTextContent(/There is no undo/);
    await userEvent.click(within(asked).getByRole("button", { name: "Cancel" }));
    await waitFor(() =>
      expect(screen.queryByRole("alertdialog", { name: "Remove pool builds?" })).toBeNull(),
    );
    expect(removed).toEqual([]);

    await userEvent.click(within(notice).getByRole("button", { name: "Remove" }));
    asked = await screen.findByRole("alertdialog", { name: "Remove pool builds?" });
    await userEvent.click(within(asked).getByRole("button", { name: "Remove pool" }));

    await waitFor(() => expect(removed).toEqual(["builds"]));
    // With none left, the group stays, and says what a pool is (the person's ruling of
    // 2026-10-30), with nothing more about Remove.
    expect(await within(group).findByText(/^No pool yet\./)).toBeInTheDocument();
    expect(within(group).queryByText(/There is no undo/)).toBeNull();
    expect(within(nav).getByRole("button", { name: "Pools" })).toBeInTheDocument();
  });

  it("says a runner removed outside purlis, and Make the runner again makes it and keeps its profiles (#4149)", async () => {
    pools = [a_pool("builds")];
    link = { state: "removed", home_kept: true };
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    await within(group).findByText("removed outside purlis");
    const notice = await waitFor(() => {
      const found = group.querySelector('[data-cause="pool-runner-removed:builds"]');
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    expect(notice).toHaveTextContent(/Its home is still there/);
    profilesAsked = [];

    await userEvent.click(within(notice).getByRole("button", { name: "Make the runner again" }));

    await waitFor(() => expect(madeAgain).toEqual(["builds"]));
    await within(group).findByText("linked");
    expect(group.querySelector('[data-cause="pool-runner-removed:builds"]')).toBeNull();
    expect(group).toHaveTextContent(/made again, with its sign-ins/);
    // Its profiles are kept there again, as after Add a pool.
    await waitFor(() => expect(profilesAsked).toContain("builds"));
  });

  it("says a runner removed with its home that its sign-ins went too (#4149)", async () => {
    pools = [a_pool("builds")];
    link = { state: "removed", home_kept: false };
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    await within(group).findByText(/sign-ins kept there/);
  });

  it("says a runner on another engine is not removed, and offers nothing to make again (#4166)", async () => {
    pools = [a_pool("builds")];
    link = { state: "on_another_engine" };
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    await within(group).findByText("on another Docker engine");
    expect(group).toHaveTextContent(/Point Docker back at that engine/);
    expect(group).not.toHaveTextContent(/removed outside purlis/);
    expect(within(group).queryByRole("button", { name: "Make the runner again" })).toBeNull();
  });

  it("asks Remove in words that say a runner on another engine stays there (#4325)", async () => {
    pools = [a_pool("builds")];
    link = { state: "on_another_engine" };
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    await within(group).findByText("on another Docker engine");
    await userEvent.click(within(group).getByRole("button", { name: "Remove pool builds" }));
    const asked = await screen.findByRole("alertdialog", { name: "Remove pool builds?" });
    expect(asked).toHaveTextContent(/stay there/);
    expect(asked).not.toHaveTextContent(/removed from your Docker/);
  });

  it("says a suspended runner is suspended, not that it would not link (#2051)", async () => {
    pools = [a_pool("builds")];
    link = { state: "suspended" };
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    await within(group).findByText("suspended");
    expect(group).not.toHaveTextContent(/not linked/);
  });

  it("says nothing of a runner of this machine's version", async () => {
    pools = [a_pool("builds")];
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    await within(group).findByText("linked");
    expect(group).not.toHaveTextContent(/runs purlis/);
  });

  it("says a runner of purlis's own image is behind, and that its older runner drains", async () => {
    pools = [a_pool("builds")];
    version = {
      runner: "0.4.2-dev.16",
      why: null,
      desktop: "0.4.2-dev.17",
      follows: true,
      draining: 1,
      held: 0,
      named: null,
      waiting: 2,
    };
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    expect(
      await within(group).findByText(
        /Its runner runs purlis 0\.4\.2-dev\.16, and this machine runs 0\.4\.2-dev\.17\. New chats start on a runner of 0\.4\.2-dev\.17 once that version's runner image is published\./,
      ),
    ).toHaveTextContent(/1 older runner keeps its chats until they end, then purlis removes it\./);
    expect(
      within(group).getByText(
        /2 chats still run there: end them to let purlis remove the older runners\./,
      ),
    ).toBeInTheDocument();
  });

  it("says an older runner held after it stopped stays until you remove it", async () => {
    pools = [a_pool("builds")];
    version = {
      runner: "0.4.2-dev.17",
      why: null,
      desktop: "0.4.2-dev.17",
      follows: true,
      draining: 0,
      held: 1,
      named: null,
      waiting: 0,
    };
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    expect(
      await within(group).findByText(/1 older runner stopped during its drain\./),
    ).toHaveTextContent(/until you remove it in Devices\./);
    expect(within(group).queryByText(/then purlis removes it/)).toBeNull();
  });

  it("tells you to build your own image again when its runner is behind", async () => {
    pools = [a_pool("builds")];
    version = {
      runner: "0.4.1",
      why: null,
      desktop: "0.4.2",
      follows: false,
      draining: 0,
      held: 0,
      named: null,
      waiting: 0,
    };
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    expect(
      await within(group).findByText(
        /Build your image from purlis 0\.4\.2, then add the pool again\./,
      ),
    ).toBeInTheDocument();
  });

  it("marks a project the runner would not hold on its pool's row, in the runner's words", async () => {
    pools = [a_pool("builds")];
    notHeld = [
      {
        root: "/projects/alpha",
        name: "alpha",
        why: "that folder is not there",
        pools: ["builds"],
      },
    ];
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    const notice = await waitFor(() => {
      const found = group.querySelector('[data-cause="project-not-held:/projects/alpha"]');
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    expect(notice).toHaveTextContent(
      "The runner that alpha's chats run on does not hold alpha, so no chat starts or answers there.",
    );
    // On the row of the pool its chats run on, the runner's words set apart.
    expect(notice.closest("li")).toHaveTextContent("builds");
    expect(notice.querySelector("bdi")).toHaveTextContent("that folder is not there");

    await userEvent.click(within(notice).getByRole("button", { name: "Dismiss" }));
    await waitFor(() =>
      expect(group.querySelector('[data-cause="project-not-held:/projects/alpha"]')).toBeNull(),
    );
  });

  it("says a pool does not move to this version when its release has no runner image", async () => {
    pools = [a_pool("builds")];
    version = {
      runner: "0.4.2-dev.16",
      why: null,
      desktop: "0.4.2-dev.17",
      follows: true,
      draining: 0,
      held: 0,
      named: false,
      waiting: 0,
    };
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    expect(
      await within(group).findByText(
        /The release of 0\.4\.2-dev\.17 has no runner image, so the pool does not move to 0\.4\.2-dev\.17 and stays on its runner\./,
      ),
    ).toBeInTheDocument();
  });

  it("keeps the old wording when purlis has said nothing of this version's release", async () => {
    pools = [a_pool("builds")];
    version = {
      runner: "0.4.2-dev.16",
      why: null,
      desktop: "0.4.2-dev.17",
      follows: true,
      draining: 0,
      held: 0,
      named: null,
      waiting: 0,
    };
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    expect(
      await within(group).findByText(
        /New chats start on a runner of 0\.4\.2-dev\.17 once that version's runner image is published\./,
      ),
    ).toBeInTheDocument();
    expect(group).not.toHaveTextContent(/has no runner image/);
  });
});

describe("A cluster pool (#3292)", () => {
  it("is added from the same dialog once its cluster operator key is shown to check", async () => {
    render(<App />);
    await palette("Add a pool…");
    const first = await screen.findByRole("dialog", { name: "Add a pool" });
    await userEvent.click(within(first).getByRole("radio", { name: /Cluster/ }));
    const dialog = await screen.findByRole("dialog", { name: "Add a pool" });
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Name" }), "team");
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Pool" }), "default");
    // Nothing is added before its key is shown.
    expect(within(dialog).queryByRole("button", { name: "Add pool" })).toBeNull();
    await userEvent.click(within(dialog).getByRole("button", { name: "Check its key" }));
    expect(await within(dialog).findByText(OPERATOR_KEY)).toBeInTheDocument();
    expect(clustersAdded).toEqual([]);
    await userEvent.click(within(dialog).getByRole("button", { name: "Add pool" }));
    await waitFor(() =>
      expect(clustersAdded).toEqual([
        {
          name: "team",
          kubeconfig: null,
          context: "team",
          pool: "default",
          key: OPERATOR_KEY,
          vouch_wait: null,
        },
      ]),
    );
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add a pool" })).toBeNull());
    // Its runner keeps none of this machine's profiles: nothing to sign in there.
    expect(screen.queryByRole("dialog", { name: /Sign your profiles in/ })).toBeNull();
  });

  it("takes the key away when what names the cluster changes after it was checked", async () => {
    render(<App />);
    await palette("Add a pool…");
    const first = await screen.findByRole("dialog", { name: "Add a pool" });
    await userEvent.click(within(first).getByRole("radio", { name: /Cluster/ }));
    const dialog = await screen.findByRole("dialog", { name: "Add a pool" });
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Pool" }), "default");
    await userEvent.click(within(dialog).getByRole("button", { name: "Check its key" }));
    await within(dialog).findByText(OPERATOR_KEY);
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Pool" }), "s");
    expect(within(dialog).queryByText(OPERATOR_KEY)).toBeNull();
    expect(within(dialog).getByRole("button", { name: "Check its key" })).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add a pool" })).toBeNull());
  });

  it("is off until the person says This is my own, which says who else can read it", async () => {
    clusters = [a_cluster_pool("team")];
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    const mine = await within(group).findByRole("switch", { name: "This is my own" });
    expect(mine).not.toBeChecked();
    expect(mine).toHaveAccessibleDescription(/cluster's admins.*can read what it holds/);
    await userEvent.click(mine);
    await waitFor(() => expect(clusters[0].owned).toBe(true));
    await waitFor(() =>
      expect(within(group).getByRole("switch", { name: "This is my own" })).toBeChecked(),
    );
    await userEvent.click(within(group).getByRole("switch", { name: "This is my own" }));
    await waitFor(() => expect(clusters[0].owned).toBe(false));
  });

  it("lists no profile until it is marked, then its profiles with Sign in, as a Docker pool's (#3819)", async () => {
    clusters = [a_cluster_pool("team")];
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    expect(
      await within(group).findByText(
        /go to pool team's runner, and sign in there, only once you turn on This is my own/,
      ),
    ).toBeInTheDocument();
    expect(within(group).queryByRole("list", { name: "Profiles on pool team" })).toBeNull();
    expect(profilesAsked).toEqual([]);
    await userEvent.click(within(group).getByRole("switch", { name: "This is my own" }));
    const profiles = await within(group).findByRole("list", { name: "Profiles on pool team" });
    expect(profilesAsked).toContain("team");
    expect(
      within(profiles).getByRole("button", { name: "Sign work in on pool team" }),
    ).toBeInTheDocument();
    expect(within(group).queryByText(/lasts only for this claim/)).toBeNull();
  });

  it("on a fresh claim says a sign-in there lasts only for this claim (#3819)", async () => {
    clusters = [{ ...a_cluster_pool("team"), owned: true }];
    freshClaim = true;
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    expect(
      await within(group).findByText(
        "Pool team's runner is on a fresh claim, which keeps nothing once it ends: a sign-in there lasts only for this claim.",
      ),
    ).toBeInTheDocument();
  });

  it("whose cluster names another operator key has the key-changed Notice with Remove", async () => {
    clusters = [a_cluster_pool("team")];
    link = { state: "operator_key_changed" };
    render(<App />);
    const nav = await yourSettings();
    await userEvent.click(await within(nav).findByRole("button", { name: "Pools" }));
    const group = await screen.findByRole("region", { name: "Pools" });
    expect(await within(group).findByText(/another cluster operator key/)).toBeInTheDocument();
    await userEvent.click(within(group).getAllByRole("button", { name: "Remove" })[0]);
    const asked = await screen.findByRole("alertdialog", { name: "Remove pool team?" });
    expect(
      within(asked).getByText(/claim is deleted from your team's cluster/),
    ).toBeInTheDocument();
    await userEvent.click(within(asked).getByRole("button", { name: "Remove pool" }));
    await waitFor(() => expect(removed).toEqual(["team"]));
  });
});
