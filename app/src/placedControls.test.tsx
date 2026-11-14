// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState, type ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Said } from "./actions";
import { ActionResultNotice } from "./ActionResultNotice";
import type { MachineProfileRow, PlaneSaving, PoolSetupStep } from "./bindings";
import { PLACED_CONTROLS, goToPlaced, type PlacedControl } from "./placedControls";
import { SignInsStep } from "./poolsetup/SignInsStep";
import { SaveConflict } from "./SavingResolve";
import { OPEN_SAVING } from "./saving";
import { SKILL_STUDIO_ASKED, type SkillStudioAsk } from "./skillStudio";
import { forgetStudio, renderStudioAt, STUDIO_HISTORY } from "./SkillStudioTab.testkit";
import { need, skill, studio } from "./skillsListing.testkit";
import { openViewAt } from "./ui/panel/viewHistory";
import { forgetEntering, enterSettings, PATIENCE, useEnteringFocus } from "./settings/entering";
import { Button } from "./ui/button/Button";
import type { LiveSetting, SettingsGroup } from "./settings/groups";
import {
  linkToGroup,
  SETTINGS_LINK,
  settingsPlace,
  type SettingsLink,
  type SettingsLinkAsk,
} from "./settings/links";
import { POOLS, poolsGroup } from "./settings/pools";
import { profilesGroup } from "./settings/profiles";

// The runner's terminal, drawn without xterm: a group by its name.
vi.mock("./settings/SignInTerminal", () => ({
  SignInTerminal: ({ label }: { label: string }) => <div role="group" aria-label={label} />,
}));

/**
 * **Each window message that names a control links to it focused** (R-357, T1-48): a result
 * whose words name Resolve, Update the pin, Accept the folders or Remove on the pool carries
 * Go to that control, and pressing it opens where the control stands (the
 * Saving view, the Skill studio's Needs you, Settings › You › Profiles, Settings › You › This machine › Pools) with
 * the keyboard on it. Each message is the core's own words.
 */

afterEach(() => {
  cleanup();
  clearMocks();
  forgetEntering();
  forgetStudio();
});

const PLANE = "/home/dev/plane";
const YOU = settingsPlace("you");

/** Follows a link into Settings as the project's window does (`PlaneView.openSettingsAt`): the
 *  group shown at its place, and the way in asked. */
const follow = (link: SettingsLink) => {
  linkToGroup(YOU, link.group, link.setting, link.control);
  enterSettings(YOU);
};

/** A result that says `words`, refused, about `project`. */
function Result({ words, project = PLANE }: { words: string; project?: string }) {
  const said: Said = { from: "test", refused: true, words };
  return (
    <ActionResultNotice
      said={said}
      project={project}
      cause="test:result"
      onClose={() => undefined}
      onSettings={follow}
    />
  );
}

/** One setting of `group`, `id`, drawn as Settings draws its row: its control in the row. */
function SettingAt({ group, id }: { group: SettingsGroup; id: string }) {
  const setting = group.settings.find((one) => one.id === id) as LiveSetting;
  const { control } = setting.useControl();
  return <Row id={id}>{control({ id: "row", labelledBy: "row-label" })}</Row>;
}

/** A setting's row in a Settings tab at You, which lands the keyboard a link asks for. */
function Row({ id, children }: { id: string; children: ReactNode }) {
  const holder = useRef<HTMLDivElement>(null);
  useEnteringFocus(YOU, holder);
  return (
    <div ref={holder}>
      <div data-setting={id}>
        <div className="ui-setting-control">{children}</div>
      </div>
    </div>
  );
}

/** Pool setup's Sign-ins step, as the core reads it. */
const SIGN_INS: PoolSetupStep = {
  id: "signins",
  name: "Sign-ins",
  state: "current",
  line: null,
  findings: [],
  next: { id: "choose-profiles", label: "Choose profiles", does: "", command: null, url: null },
  skippable: true,
  also: null,
  parts: [],
  undo: null,
  see: null,
  other: null,
  machine: null,
  cause: null,
  checks: [],
  admin: null,
  prefill: null,
};

const go = async (name: string) =>
  userEvent.click(await screen.findByRole("button", { name: `Go to ${name}` }));

describe("each window message that names a control links to it focused", () => {
  it("Resolve: opens the Saving view with the keyboard on Resolve", async () => {
    mockIPC(() => null);
    const saving = {
      stage: "blocked",
      changed: [],
      ahead: 1,
      pr: null,
      request: "pull request",
      blocked: null,
      branch: "main",
      pushes: true,
      behind: 3,
      pushFailed: null,
      live: [],
      conflicts: [],
      conflictKeys: {},
      notice: null,
      mode: null,
      modeFrom: "default",
      journal: [],
      toResolve: "Press Resolve.",
    } as PlaneSaving;
    /** The project's window, which opens its Saving tab when asked (`PlaneView`). */
    function Window() {
      const [open, setOpen] = useState(false);
      useEffect(() => {
        const asked = () => setOpen(true);
        window.addEventListener(OPEN_SAVING, asked);
        return () => window.removeEventListener(OPEN_SAVING, asked);
      }, []);
      return (
        <div className="project-view" data-plane={PLANE}>
          <Result words="notes.md on the remote and this machine both changed. Nothing was changed here: press Resolve in the Saving view to settle it" />
          {open && <SaveConflict plane={PLANE} saving={saving} />}
        </div>
      );
    }
    render(<Window />);

    await go("Resolve");

    await waitFor(() => expect(screen.getByRole("button", { name: "Resolve" })).toHaveFocus());
  });

  it("Update the pin: opens Settings › You › Profiles with the keyboard on the profile's Update the pin", async () => {
    const profile = (name: string): MachineProfileRow => ({
      name,
      kind: "claude",
      harness: "Claude Code",
      origin: "made",
      folder: `/data/profiles/${name}/config`,
      imported_at: 1_760_000_000,
      imported_from: "/home/dev/.claude",
      signs_in: "through Claude Code's own sign-in, on this Mac",
      standing: "signed-in",
      plans: [],
      pinned: "2.1.300",
      installed: "3.0.0",
      offered: "3.0.0",
      sandbox_only: false,
      commands_can_read_sign_in: null,
      icon: { glyph: null, own: null, monogram: "C" },
    });
    mockIPC((cmd) => (cmd === "machine_profiles" ? [profile("home"), profile("work")] : null));
    render(
      <>
        <Result words="This chat resumes a conversation of the profile 'work', which is pinned to Claude Code 2.1.300, and Claude Code 3.0.0 is installed. purlis resumes a profile's conversations only on its pinned version or a newer one of the same major version, so nothing was started. To resume it on 3.0.0, press Update the pin in Settings › You › Profiles, or install Claude Code 2.1.300 again." />
        <SettingAt group={profilesGroup(PLANE)} id="you.profiles.list" />
      </>,
    );
    await screen.findByRole("button", { name: "Update the pin of work to 3.0.0" });

    await go("Update the pin");

    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Update the pin of work to 3.0.0" })).toHaveFocus(),
    );
  });

  it("Accept the folders: opens the Skill studio's Needs you at the project's folders, with the keyboard on Accept the folders", async () => {
    const needs = [
      need("grilling"),
      need("review:project-folders", {
        title: "The project's folders",
        tag: null,
        reason: "Purlis can't confirm which of this project's folders you accepted before.",
        fix: { kind: "accept-folders", folders: ["workspaces/ops"] },
        fix_label: "Accept the folders",
        fix_does: "Their skills reach chats from then on.",
      }),
    ];
    mockIPC((cmd) => {
      if (cmd === "skill_studio") return studio([skill("deploy")], { needs });
      if (cmd === "skill_updates") return { offers: [], unconfirmed: [], unasked: [] };
      if (cmd === "skill_drafts" || cmd === "skills_deleted") return [];
      return null;
    });
    let asked: SkillStudioAsk | undefined;
    const hear = (event: Event) => {
      asked = (event as CustomEvent<SkillStudioAsk>).detail;
    };
    window.addEventListener(SKILL_STUDIO_ASKED, hear);
    try {
      render(
        <Result
          project="p"
          words="purlis can't confirm which of this project's folders you accepted before: open the project once in the purlis app and press Accept the folders in the Skill studio's Needs you"
        />,
      );

      await go("Accept the folders");

      expect(asked).toMatchObject({ plane: "p", page: "needs", item: "review:project-folders" });
      // The window opens its studio where it was asked (`PlaneView`).
      const at = asked as SkillStudioAsk;
      act(() => openViewAt(STUDIO_HISTORY, { page: at.page, scope: at.scope, item: at.item }));
      renderStudioAt(
        { page: at.page, scope: at.scope, item: at.item },
        {
          wrap: (tab) => (
            <div className="project-view" data-plane="p">
              {tab}
            </div>
          ),
        },
      );

      await waitFor(() =>
        expect(screen.getByRole("button", { name: "Accept the folders" })).toHaveFocus(),
      );
    } finally {
      window.removeEventListener(SKILL_STUDIO_ASKED, hear);
    }
  });

  it("Remove on the pool: Pool setup › Sign-ins › See what this does opens Settings › You › This machine › Pools with the keyboard on that pool's Remove", async () => {
    const limits = { cpus: "2", memory_mib: 4096, processes: 1024, tmp_mib: 1024 };
    const pool = (name: string) => ({
      name,
      image: "purlis/runner:0.4.2",
      limits,
      suspend_after: "24h",
    });
    mockIPC((cmd) => {
      if (cmd === "docker_pools")
        return {
          pools: [pool("builds"), pool("tests")],
          defaults: limits,
          suspend_after: "24h",
          frames: 15,
          width: 1280,
          clusters: [],
        };
      if (cmd === "docker_pool_link") return { state: "linked" };
      if (cmd === "pool_profiles") return { kept: [], addable: [] };
      if (cmd === "pool_named_by" || cmd === "pool_held_runners" || cmd === "projects_not_held")
        return [];
      if (cmd === "pool_setup_sizes") return { most_mib: 128, most_files: 8192, profiles: [] };
      if (cmd === "pool_setup_sign_ins_by_hand")
        return {
          note: "No command takes work off yet. Pool tests names none of its profiles, so it keeps the profile a new chat in your project gets. Tick another profile and this shows a command for it, or use Remove on the pool in Settings › You › This machine › Pools.",
          text: "",
        };
      return null;
    });
    // The project's window follows a link a tab asks for (`PlaneView`).
    const heard = (event: Event) => follow((event as CustomEvent<SettingsLinkAsk>).detail.link);
    window.addEventListener(SETTINGS_LINK, heard);
    try {
      render(
        <>
          <SignInsStep step={SIGN_INS} pool="tests" plane={PLANE} changed={() => undefined} />
          <SettingAt group={poolsGroup(PLANE)} id={`${POOLS}.list`} />
        </>,
      );
      await userEvent.click(await screen.findByRole("button", { name: "See what this does" }));
      await screen.findByRole("button", { name: "Remove pool tests" });

      await go("Remove on the pool");

      await waitFor(() =>
        expect(screen.getByRole("button", { name: "Remove pool tests" })).toHaveFocus(),
      );
    } finally {
      window.removeEventListener(SETTINGS_LINK, heard);
    }
  });
});

describe("a link whose control never appears", () => {
  it("forgets itself when it lapses, so a later plain way into Settings lands on the group nav", async () => {
    /** A Settings tab at You: its nav, and a row whose control the link names is never drawn. */
    function Tab() {
      const holder = useRef<HTMLDivElement>(null);
      useEnteringFocus(YOU, holder);
      return (
        <>
          <Button>Elsewhere</Button>
          <div ref={holder}>
            <nav className="ui-settings-nav" aria-label="Groups">
              <Button aria-current="page">Pools</Button>
            </nav>
            <div data-setting={`${POOLS}.list`}>
              <div className="ui-setting-control">
                <Button>Add a pool</Button>
              </div>
            </div>
          </div>
        </>
      );
    }
    render(<Tab />);
    follow({
      group: "you.machine.pools",
      setting: `${POOLS}.list`,
      control: { words: "Remove pool", about: "gone" },
    });
    await waitFor(() => expect(screen.getByRole("button", { name: "Add a pool" })).toHaveFocus());

    // The way in lapses with the named control never drawn.
    await act(() => new Promise((done) => setTimeout(done, PATIENCE + 2 * 50)));
    act(() => screen.getByRole("button", { name: "Elsewhere" }).focus());
    // ⌘,: a plain way in, naming no setting.
    act(() => enterSettings(YOU));

    await waitFor(() => expect(screen.getByRole("button", { name: "Pools" })).toHaveFocus());
  });
});

describe("a Go to into a view", () => {
  it("puts the keyboard on the control in its own project's view, not another project's", async () => {
    mockIPC(() => null);
    render(
      <>
        <div className="project-view" data-plane="/home/dev/other">
          <Button>Resolve</Button>
        </div>
        <div className="project-view" data-plane={PLANE}>
          <Button aria-describedby="here">Resolve</Button>
          <span id="here">this project</span>
        </div>
      </>,
    );
    const resolve = PLACED_CONTROLS.find((one) => one.words === "Resolve") as PlacedControl;

    act(() => goToPlaced(resolve, PLANE, "press Resolve in the Saving view", () => undefined));

    await waitFor(() =>
      expect(screen.getByRole("button", { description: "this project" })).toHaveFocus(),
    );
  });
});

describe("a Settings link that names a row's control", () => {
  it("lands on the control whose name holds its words and its pool as whole words", async () => {
    render(
      <Row id={`${POOLS}.list`}>
        <Button>Remove pooling tests</Button>
        <Button>Remove pool tests-2</Button>
        <Button>Remove pool tests</Button>
      </Row>,
    );

    follow({
      group: "you.machine.pools",
      setting: `${POOLS}.list`,
      control: { words: "Remove pool", about: "tests" },
    });

    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Remove pool tests" })).toHaveFocus(),
    );
  });
});
