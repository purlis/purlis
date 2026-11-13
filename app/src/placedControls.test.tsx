// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState, type ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Said } from "./actions";
import { ActionResultNotice } from "./ActionResultNotice";
import type { MachineProfileRow, PlaneSaving, PoolProfileRow, PoolSignInStep } from "./bindings";
import { SaveConflict } from "./SavingResolve";
import { OPEN_SAVING } from "./saving";
import { SKILL_STUDIO_ASKED, type SkillStudioAsk } from "./skillStudio";
import { forgetStudio, renderStudioAt, STUDIO_HISTORY } from "./SkillStudioTab.testkit";
import { need, skill, studio } from "./skillsListing.testkit";
import { openViewAt } from "./ui/panel/viewHistory";
import { forgetEntering, enterSettings, useEnteringFocus } from "./settings/entering";
import type { LiveSetting, SettingsGroup } from "./settings/groups";
import { linkToGroup, settingsPlace, type SettingsLink } from "./settings/links";
import { PoolProfiles } from "./settings/PoolProfiles";
import { POOLS, poolsGroup } from "./settings/pools";
import { profilesGroup } from "./settings/profiles";

// The runner's terminal, drawn without xterm: a group by its name.
vi.mock("./settings/SignInTerminal", () => ({
  SignInTerminal: ({ label }: { label: string }) => <div role="group" aria-label={label} />,
}));

/**
 * **Each window message that names a control links to it focused** (R-357, T1-48): a result
 * whose words name Resolve, Update the pin, Accept the folders, Open in terminal or Remove on
 * the pool carries Go to that control, and pressing it opens where the control stands (the
 * Saving view, the Skill studio's Needs you, Settings › You › Profiles, Settings › Pools) with
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
        <>
          <Result words="notes.md on the remote and this machine both changed. Nothing was changed here: press Resolve in the Saving view to settle it" />
          {open && <SaveConflict plane={PLANE} saving={saving} />}
        </>
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
      renderStudioAt({ page: at.page, scope: at.scope, item: at.item });

      await waitFor(() =>
        expect(screen.getByRole("button", { name: "Accept the folders" })).toHaveFocus(),
      );
    } finally {
      window.removeEventListener(SKILL_STUDIO_ASKED, hear);
    }
  });

  it("Open in terminal: opens Settings › Pools with the keyboard on the profile's Open in terminal", async () => {
    const a_profile = (name: string): PoolProfileRow => ({
      name,
      kind: "claude",
      harness: "Claude Code",
      standing: "signed-out",
      not_kept: null,
      limit: null,
      plans: [],
    });
    const steps: PoolSignInStep[] = [
      { at: "could-not-follow", said: "Choose your sign-in method: [1] [2]" },
    ];
    mockIPC((cmd) => {
      if (cmd === "pool_profiles") return { kept: [a_profile("work")], addable: [] };
      if (cmd === "pool_sign_in_start") return 7;
      if (cmd === "pool_sign_in_next") return steps.shift() ?? new Promise(() => undefined);
      return null;
    });
    render(
      <>
        <Result
          words={
            "purlis could not follow this sign-in, so it ended it. Sign in with Open in terminal on 'work''s line in Settings › Pools, where the harness asks you itself. It said:\nChoose"
          }
        />
        <Row id={`${POOLS}.list`}>
          <PoolProfiles pool="builds" plane={PLANE} />
        </Row>
      </>,
    );
    await userEvent.click(
      await screen.findByRole("button", { name: "Sign in: work on pool builds" }),
    );
    const signing = await screen.findByRole("group", { name: "Signing work in on builds" });
    await within(signing).findByRole("button", { name: "Open in terminal" });

    await go("Open in terminal");

    await waitFor(() =>
      expect(within(signing).getByRole("button", { name: "Open in terminal" })).toHaveFocus(),
    );
  });

  it("Remove on the pool: opens Settings › You › This machine › Pools with the keyboard on that pool's Remove", async () => {
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
      return null;
    });
    render(
      <>
        <Result words="No command takes work off yet. Pool tests names none of its profiles, so it keeps the profile a new chat in your project gets. Tick another profile and this shows a command for it, or use Remove on the pool in Settings › You › This machine › Pools." />
        <SettingAt group={poolsGroup(PLANE)} id={`${POOLS}.list`} />
      </>,
    );
    await screen.findByRole("button", { name: "Remove pool tests" });

    await go("Remove on the pool");

    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Remove pool tests" })).toHaveFocus(),
    );
  });
});
