// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSkillStudioOn } from "../skillStudio";
import { useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react";
import { LoaderCircle } from "lucide-react";
import { commands, type PlaneId, type SettingsEdit, type SettingsWhich } from "../bindings";
import { NewPersona } from "../NewPersona";
import { NewWorkspace } from "../NewWorkspace";
import { settled } from "../PlaneEdits";
import { cloneRepos } from "../repoClones";
import { DEFAULT_THEME, inForce } from "../theme/theme";
import { atCreation } from "../windowprefs";
import {
  Choice,
  Field,
  SettingGroup,
  SettingRow,
  SettingsLayout,
  type Badge,
  type RowIds,
} from "./components";
import { heldIn, keysOf, type Driven, type Files } from "./driver";
import { entries, type Entry } from "./fileControls";
import {
  inAFile,
  LEVELS,
  type FileSetting,
  type Level,
  type LiveSetting,
  type Setting,
  type SettingsFileId,
  type SettingsGroup,
} from "./groups";
import {
  askSettingsAction,
  chooseGroup,
  focusedSetting,
  levelOf,
  linkToGroup,
  PICK_VAULT,
  settingsPlace,
  showPersona,
  useShownGroup,
} from "./links";
import { collectionFile, kept, projectGroups, useProjectLevel } from "./project";
import { named, RawEditor, RawLinks, type RawDraft, type RawFile } from "./RawToml";
import { useWorkspaceLevel, workspaceGroups } from "./workspace";
import { youGroups } from "./you";
import { CollectionView, type Asked } from "./Collection";
import { useEnteringFocus } from "./entering";
import { OutLink } from "./OutLink";
import { standingIn, type Standing } from "./standing";
import {
  OffWhenReadOnly,
  ReadOnlyFieldset,
  useReadOnlyMendable,
  useReadOnlySaid,
} from "../ReadOnlyLine";
import { LivesHere } from "../LivesHere";
import { ColorWell } from "../ui/choice/ColorWell";
import { Button } from "../ui/button/Button";
import { Menu, MenuItem, MenuSection, MenuTrigger, MenuWords } from "../ui/menu/Menu";
import { MenuTip, useRested } from "../ui/menu/MenuTip";

/**
 * **Settings** (SE-16, #1166; the spec on #558, rulings V89a–i): the one tab where a setting is
 * read and changed, one level at a time (`CONTEXT.md`, **Settings** and **Level**). A view tab
 * (`tabs.settingsView`), opened from the app menu's Settings…, `⌘,` and the palette.
 *
 * **Two columns.** The level switcher and what that level is sit at the top; the left nav lists
 * the level's groups; the right column shows the chosen group and nothing else. **You**; where
 * the tab is in a project, **Project** (SE-17); and where it is about a workspace — opened at
 * one, or on its strip — **Workspace** (SE-20) are offered. Persona joins the switcher as its
 * groups land, declared as data the same way (`groups.ts`).
 *
 * **The level is the tab's** (D-SE17a): the tab is keyed by it, and the switcher asks for the tab
 * to show another level (`onLevelChange`) rather than keeping one of its own — so opening
 * Settings at a level whose tab is open brings that tab forward, whichever level it was opened
 * at.
 *
 * "Preferences" was this tab's You level before it had a name (charter-app#283), and is retired.
 * So is the long Project settings page with its Form / Raw TOML switch (charter-app#252, SE-19):
 * its forms are the Project level's groups, and its raw view is the level's **Edit as TOML**
 * link per file (`RawToml.tsx`).
 */
export function SettingsTab({
  plane,
  workspace,
  level = "you",
  onLevelChange,
}: {
  /** The project the tab is in; without one only You is offered. */
  plane?: PlaneId;
  /** The workspace the tab is about, in that project; without one Workspace is not offered. */
  workspace?: string;
  level?: Level;
  onLevelChange?: (level: Level) => void;
}) {
  const offered = LEVELS.filter(
    (one) =>
      one.id === "you" ||
      (one.id === "project" && plane !== undefined) ||
      (one.id === "workspace" && plane !== undefined && workspace !== undefined),
  );
  const at = offered.some((one) => one.id === level) ? level : "you";
  const change = (to: string) => onLevelChange?.(to as Level);
  if (at === "workspace" && plane !== undefined && workspace !== undefined)
    return (
      <WorkspaceLevelTab
        key={`${plane}\u0000${workspace}`}
        plane={plane}
        workspace={workspace}
        levels={offered}
        onLevelChange={change}
      />
    );
  return at === "project" && plane !== undefined ? (
    <ProjectLevelTab key={plane} plane={plane} levels={offered} onLevelChange={change} />
  ) : (
    <YouLevel plane={plane} levels={offered} onLevelChange={change} />
  );
}

type Switcher = {
  levels: readonly { id: Level; label: string }[];
  onLevelChange: (level: string) => void;
};

function YouLevel({ plane, levels, onLevelChange }: Switcher & { plane?: PlaneId }) {
  // The Pools group is listed with or without a pool (the person's ruling of 2026-10-30): it
  // says what a pool is until one is added.
  const groups = useMemo(() => youGroups(plane), [plane]);
  const where = atCreation().layout.path || "the layout file";
  return (
    <Shown
      level="you"
      place={settingsPlace("you")}
      levels={levels}
      onLevelChange={onLevelChange}
      about={`This machine only, in every project. Kept in ${where}.`}
      groups={groups}
    />
  );
}

/** What New… on a picker makes, and what is done with its name once it is made (ST-1). */
type Making = {
  entry: "persona" | "workspace";
  then: (name: string) => void;
  trouble?: string;
  busy: boolean;
};

/** Where New… on a picker goes (ST-1): `then` is handed the name of what was made. */
type OnNew = (entry: Entry, then: (name: string) => void) => void;

function ProjectLevelTab({ plane, ...switcher }: Switcher & { plane: PlaneId }) {
  const project = useProjectLevel(plane);
  const [making, setMaking] = useState<Making>();
  /**
   * **Makes what New… asked for, then picks it** (ST-1, D-ST1-2): through the same core
   * commands the sidebar's New persona and New workspace use, so the window and a terminal
   * refuse the same names in the same words; a refusal stays in the dialog. What was made is
   * listed again before it is picked, so the picker names it rather than a value it lacks.
   */
  const make = async (made: Promise<{ status: "ok" } | { status: "error"; error: string }>) => {
    if (making === undefined) return false;
    setMaking({ ...making, busy: true, trouble: undefined });
    const answer = await made;
    if (answer.status === "error") {
      setMaking({ ...making, busy: false, trouble: answer.error });
      return false;
    }
    setMaking(undefined);
    if (project.state === "read") await project.readEntries();
    return true;
  };
  const groups = useMemo(
    () => (project.state === "read" ? projectGroups(project.read, project.reread) : []),
    [project],
  );
  // Each file as its whole text (SE-19, V89d), written through the level's driver: the core
  // checks it and writes it, in the queue every setting's write is in.
  const raw: RawFile[] =
    project.state === "read"
      ? (["shared", "local"] as const).map((which: SettingsWhich) => ({
          id: which,
          as: "TOML",
          file: project.read[which],
          kept: kept(project.read[which]),
          save: (base, text) => project.writeRaw(which, base, text),
        }))
      : [];
  // Each with a link to the setting it names, where this level has one (NO-7, #1232).
  const standing: Standing[] =
    project.state === "read"
      ? (["shared", "local"] as const).flatMap((which) => {
          const file = project.read[which];
          return standingIn(file.refusals, which, groups).map((one) => ({
            ...one,
            why: `${file.file}: ${one.why}`,
          }));
        })
      : [];
  return (
    <Shown
      level="project"
      plane={plane}
      place={settingsPlace("project", plane)}
      {...switcher}
      about={
        <>
          {withVaults(
            "This project, for everyone who opens it. Never put a secret in its files: keep it in a vault and name it as vault:<vault>/<key>.",
            plane,
          )}
          {/* Where it lives, when that is only here, with Publish beside it (#1820). */}
          <LivesHere plane={plane} as="words" />
        </>
      }
      groups={groups}
      waiting={waitingFor(project)}
      standing={standing}
      mend={
        project.state === "read"
          ? [project.read.shared, project.read.local]
              .filter((file) => !file.parsed)
              .map((file) => `Open ${named(file)} under Edit as TOML to mend it.`)
          : []
      }
      driver={project.state === "read" ? project : undefined}
      raw={raw}
      onNew={(entry, then) => {
        if (entry !== "profile") setMaking({ entry, then, busy: false });
      }}
    >
      {making?.entry === "persona" && (
        <NewPersona
          plane={plane}
          trouble={making.trouble}
          making={making.busy}
          onCreate={(name, role, when, parent) =>
            void make(settled(commands.personaCreate(plane, name, role, when, parent))).then(
              (made) => made && making.then(name),
            )
          }
          onCancel={() => setMaking(undefined)}
        />
      )}
      {making?.entry === "workspace" && (
        <NewWorkspace
          plane={plane}
          planeId={plane}
          trouble={making.trouble}
          making={making.busy}
          onCreate={(name, vision, live, repos) =>
            void make(
              settled(
                commands.workspaceCreate(plane, name, vision.trim() === "" ? null : vision, live),
              ),
            ).then((made) => {
              if (!made) return;
              making.then(name);
              // The repos land after it, each on its own, as they do from the sidebar.
              if (repos.length > 0) void cloneRepos(plane, name, repos);
            })
          }
          onCancel={() => setMaking(undefined)}
        />
      )}
    </Shown>
  );
}

/**
 * A level's sentence that names a vault, with the link to the vaults (#1388): a vault stays in
 * its own tab, and Settings links to it. Settings names "a vault", not one, so the link opens
 * the vault picker, as the palette's Open vault… does.
 */
function withVaults(sentence: string, plane: PlaneId): ReactNode {
  return (
    <>
      {sentence}{" "}
      <OutLink plane={plane} action={PICK_VAULT}>
        Open vault…
      </OutLink>
    </>
  );
}

/** What stands in for a level's groups until it has been read. */
function waitingFor(
  level: { state: "reading" } | { state: "trouble"; trouble: string } | Driven<unknown>,
) {
  return level.state === "reading" ? (
    <p className="pending" aria-busy="true">
      <LoaderCircle className="node-icon spinning" />
      Reading the settings…
    </p>
  ) : level.state === "trouble" ? (
    <p className="trouble" role="alert">
      {level.trouble}
    </p>
  ) : undefined;
}

function WorkspaceLevelTab({
  plane,
  workspace,
  ...switcher
}: Switcher & { plane: PlaneId; workspace: string }) {
  const level = useWorkspaceLevel(plane, workspace);
  const skillStudio = useSkillStudioOn();
  const groups = useMemo(
    () => (level.state === "read" ? workspaceGroups(level.read, level.reread, skillStudio) : []),
    [level, skillStudio],
  );
  // The manifest as its whole text (NO-7, #1232), written through the level's driver as Edit as
  // TOML's files are: the core checks it, against the text the edit began from.
  const raw: RawFile[] =
    level.state === "read"
      ? [
          {
            id: "workspace",
            as: "JSON",
            file: level.read.settings,
            kept: level.read.settings.live
              ? "It is committed with this LIVE workspace: your team sees it."
              : "It stays on this machine while the workspace is not LIVE.",
            save: (base, text) => level.writeRaw("workspace", base, text),
          },
        ]
      : [];
  const settings = level.state === "read" ? level.read.settings : undefined;
  return (
    <Shown
      level="workspace"
      plane={plane}
      place={settingsPlace("workspace", plane, workspace)}
      {...switcher}
      about={withVaults(
        `The workspace ${workspace}, read between its project's shared settings and this machine's own: it refines its project for the team, and this machine has the last word. Never put a secret in its settings: keep it in a vault and name it as vault:<vault>/<key>.`,
        plane,
      )}
      groups={groups}
      waiting={waitingFor(level)}
      standing={settings ? standingIn(settings.refusals, "workspace", groups) : []}
      mend={
        settings && !settings.parsed
          ? [`Open ${named(settings)} under Edit as JSON to mend it.`]
          : []
      }
      driver={level.state === "read" ? level : undefined}
      raw={raw}
    />
  );
}

/**
 * One level, drawn: its groups that have a setting in the nav, and the chosen one on the right.
 * `waiting` stands in for the groups until the level has been read.
 *
 * **Which group is the place's** (SE-22, `links.ts`): the one last picked at this level and
 * target, or the one a link last landed on — so a level comes back at the group it was left
 * at, and a link to a group shows it even in a tab already open.
 */
function Shown({
  level,
  plane,
  place,
  levels,
  onLevelChange,
  about,
  groups: declared,
  waiting,
  standing = [],
  mend = [],
  driver,
  raw = [],
  onNew,
  children,
}: Switcher & {
  level: Level;
  /** The project the level is in, whose window runs a link out of Settings (#1387). */
  plane?: PlaneId;
  /** Where the group shown is remembered (`links.settingsPlace`). */
  place: string;
  about: ReactNode;
  groups: readonly SettingsGroup[];
  waiting?: ReactNode;
  /** What charter refuses in the level's files as they stand, each with the setting it is about
   *  where the level has one (NO-7). */
  standing?: readonly Standing[];
  /** Where a file that is not read as it stands is mended: said under {@link standing}. */
  mend?: readonly string[];
  /** What writes the level's file settings, once the level has been read. */
  driver?: Driven<unknown>;
  /** The level's files as their whole text, each with its link at the foot of the nav (SE-19,
   *  NO-7). */
  raw?: readonly RawFile[];
  /** Where a picker's New… goes for a persona or a workspace (ST-1). */
  onNew?: OnNew;
  /** What the level draws over the tab: the dialog New… opened. */
  children?: ReactNode;
}) {
  // Per tab and not remembered (V89c): a level drawn afresh starts with the whole nav.
  const [filter, setFilter] = useState("");
  /** The file whose text is on the right in place of a group, while one is. */
  const [editing, setEditing] = useState<string>();
  const shown = useShownGroup(place);
  // A link that lands here clears the filter and puts away a file's text, so the group it
  // names is on screen. Adjusted during render, React's way for state that follows a value
  // that changed. A draft typed into the text is kept, as it is when a group is picked.
  const linked = shown?.linked ?? 0;
  const [landed, setLanded] = useState(linked);
  if (landed !== linked) {
    setLanded(linked);
    setFilter("");
    setEditing(undefined);
  }
  const choose = (group: string) => chooseGroup(place, group);
  const groups = narrowed(
    // A collection is offered while it has no entry: that is where one is added. A group with
    // a link out is offered too: the link is the way to a setting (#1387).
    declared.filter(
      (one) =>
        one.settings.length > 0 || one.collection !== undefined || (one.links?.length ?? 0) > 0,
    ),
    filter,
  );
  const group = landsOn(groups, shown?.group) ?? groups[0];
  /** The group on screen, once the level is read and no file's text is in its place. */
  const drawn = waiting === undefined && editing === undefined ? group?.id : undefined;
  /**
   * **A link that names a setting focuses it** (NO-7, #1232): once its group is drawn, the
   * setting's control takes the focus, so the person lands on the field and not only near it.
   * Done once per link: the place forgets the setting once it is focused. Asked again as the
   * level is read and its groups are declared, since a link can land before either.
   *
   * The row is looked for in this tab's own element (#1292): a second Settings tab of the same
   * level, split beside this one, draws a row by the same name.
   */
  const target = shown?.setting;
  const own = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (target === undefined) return;
    const rows = own.current?.querySelectorAll<HTMLElement>("[data-setting]") ?? [];
    const row = [...rows].find((one) => one.dataset.setting === target);
    if (row === undefined) return;
    row.scrollIntoView?.({ block: "nearest" });
    // The control, never the label's info mark or badge, which come before it (DS-8).
    (row.querySelector(".ui-setting-control") ?? row)
      .querySelector<HTMLElement>("input, select, textarea, button, [tabindex]")
      ?.focus({ preventScroll: true });
    focusedSetting(place);
  }, [linked, place, target, drawn, declared]);
  /** Every way into Settings leaves the keyboard on the nav's current group (#1206). */
  useEnteringFocus(place, own);
  /** What is typed into each file's text, kept while a group is looked at. */
  const [drafts, setDrafts] = useState<Partial<Record<string, RawDraft>>>({});
  const rawFile = raw.find((one) => one.id === editing);
  /** Whether Edit as TOML on the shared file is how a manifest that does not read is mended. */
  const mendable = useReadOnlyMendable();
  /** Whether the file on screen is still written on a read-only project ({@link OffUnless}). */
  const rawOn =
    rawFile !== undefined &&
    (stillWritten(level, rawFile.id === "local" ? "local" : undefined) ||
      (rawFile.id === "shared" && mendable));
  const readOnly = useReadOnlySaid();
  const found =
    waiting === undefined ? groups.reduce((all, one) => all + one.settings.length, 0) : undefined;
  /** The Add form New profile… opened, on the group it is on, until it adds or is left. */
  const [asking, setAsking] = useState<{ group: string; asked: Asked }>();
  /**
   * New… on a picker (ST-1). New profile… opens the profiles collection's Add form (ST-4,
   * superseding D-ST1-4's Edit as TOML), on its group, and picks the profile once it is added;
   * a persona or a workspace is the level's to make.
   */
  const create: OnNew = (entry, then) => {
    const home = declared.find(
      (one) => one.collection?.name === "profiles" && one.collection.adds !== false,
    );
    if (entry !== "profile") onNew?.(entry, then);
    else if (home !== undefined) {
      linkToGroup(place, home.id);
      setAsking((was) => ({
        group: home.id,
        asked: {
          ask: (was?.asked.ask ?? 0) + 1,
          then: (name) => {
            setAsking(undefined);
            then(name);
          },
        },
      }));
    } else if (raw.some((one) => one.id === "local")) setEditing("local");
  };
  return (
    <SettingsLayout
      holder={own}
      levels={levels}
      level={level}
      onLevelChange={onLevelChange}
      about={about}
      groups={groups}
      group={rawFile ? "" : (group?.id ?? "")}
      onGroupChange={(to) => {
        setEditing(undefined);
        choose(to);
      }}
      filter={filter}
      onFilterChange={(to) => {
        // The group on screen stays the chosen one while it is still matched, and is the one
        // shown again once the box is cleared.
        if (group) choose(group.id);
        setEditing(undefined);
        setFilter(to);
      }}
      found={rawFile ? undefined : found}
      foot={
        raw.length > 0 && waiting === undefined ? (
          <RawLinks files={raw} editing={rawFile?.id} onEdit={setEditing} />
        ) : undefined
      }
    >
      {waiting ?? (
        <>
          {standing.length + mend.length > 0 && (
            <div className="ui-settings-standing">
              <p className="note">purlis does not take this from the files as they stand:</p>
              <ul>
                {standing.map(({ why, to }, at) => (
                  <li key={at} className="trouble">
                    {why}
                    {to && (
                      <>
                        {" "}
                        <Button size="sm" onPress={() => linkToGroup(place, to.group, to.setting)}>
                          {`Go to ${to.label}`}
                        </Button>
                      </>
                    )}
                  </li>
                ))}
              </ul>
              {mend.map((where) => (
                <p key={where} className="note">
                  {where}
                </p>
              ))}
            </div>
          )}
          {/* **A read-only project's committed files are read, not written** (#826, FR-24,
              #1759): each control that writes one is off with the project's sentence as its
              reason, decided by the file it writes ({@link stillWritten}); the nav, the filter
              and the links still work. */}
          {rawFile ? (
            <OffUnless on={rawOn}>
              <RawEditor
                key={rawFile.id}
                raw={rawFile}
                // The text is CodeMirror's, which the fieldset cannot turn off on its own.
                off={!rawOn && readOnly !== undefined}
                draft={drafts[rawFile.id]}
                onDraft={(to) =>
                  setDrafts((was) => ({
                    ...was,
                    [rawFile.id]: typeof to === "function" ? to(was[rawFile.id]) : to,
                  }))
                }
              />
            </OffUnless>
          ) : (
            group && (
              <ShownGroup
                key={group.id}
                level={level}
                group={group}
                driver={driver}
                onNew={create}
                adding={asking?.group === group.id ? asking.asked : undefined}
                // SE-22's link, at this level: it lands on the group and clears the filter. A
                // referrer at another level carries its own link, which the project's window
                // follows (`referrerElsewhere` in links.ts, #1241), so this is never asked.
                reachable={(to) => levelOf(to) === level && declared.some((one) => one.id === to)}
                onGo={(to) => linkToGroup(place, to)}
                onAction={
                  plane === undefined ? undefined : (action) => askSettingsAction(plane, action)
                }
              />
            )
          )}
        </>
      )}
      {children}
    </SettingsLayout>
  );
}

/**
 * **Whether a control stays on in a read-only project** (#826, #1759), by the file it writes:
 * everything at You, which is this machine's; at the Project level, what is kept in
 * `charter.local.toml` alone (`file` "local"), which the core still writes there. Anything else
 * writes a committed file, or a file this cannot tell (`undefined`), and is off.
 */
export function stillWritten(level: Level, file: SettingsFileId | undefined): boolean {
  return level === "you" || (level === "project" && file === "local");
}

/** {@link stillWritten} for a row drawn by a hook of its own, by what it says it writes
 *  (`LiveSetting.writes`): one that does not say is off. */
function liveStillWritten(level: Level, writes: LiveSetting["writes"]): boolean {
  if (writes === "nothing" || writes === "this machine" || writes === "per control") return true;
  return stillWritten(level, writes === "local" ? "local" : undefined);
}

/** `children` as they are where `on`, and otherwise off on a read-only project with its
 *  sentence as their reason ({@link OffWhenReadOnly}). `on` never follows the read-only answer,
 *  so the controls are never remounted when it arrives. */
function OffUnless({
  on,
  steady = false,
  children,
}: {
  on: boolean;
  /** `on` changes while it is drawn (a movable row's file, #1759): always the same box, so
   *  the controls are not drawn again and the focus stays where it was. */
  steady?: boolean;
  children: ReactNode;
}) {
  const said = useReadOnlySaid();
  if (!steady) return on ? <>{children}</> : <OffWhenReadOnly>{children}</OffWhenReadOnly>;
  return <ReadOnlyFieldset said={on ? undefined : said}>{children}</ReadOnlyFieldset>;
}

/**
 * **Which group an address lands on** (#1296): the group it names, or else the longest group
 * whose address it starts with, a part at a time. A sub-page that is not there — a profile that
 * has no page, or was renamed away since the link was made — lands on the group it is under,
 * never on the level's first. `undefined` for an address under no group, or none.
 */
function landsOn(
  groups: readonly SettingsGroup[],
  address: string | undefined,
): SettingsGroup | undefined {
  if (address === undefined) return undefined;
  let under: SettingsGroup | undefined;
  for (const one of groups) {
    if (one.id === address) return one;
    if (address.startsWith(`${one.id}.`) && one.id.length > (under?.id.length ?? 0)) under = one;
  }
  return under;
}

/**
 * **The groups and settings a filter leaves** (SE-21, V89c): a group whose own label or help
 * holds the words keeps every setting; any other group keeps the settings whose label or help
 * holds them, and is left out when none does. Case is ignored, and so is space around the
 * words; with none, every group is left as it is.
 */
function narrowed(groups: readonly SettingsGroup[], filter: string): readonly SettingsGroup[] {
  const words = filter.trim().toLocaleLowerCase();
  if (words === "") return groups;
  const holds = (...texts: string[]) =>
    texts.some((text) => text.toLocaleLowerCase().includes(words));
  return groups.flatMap((one) => {
    if (holds(one.label, one.help)) return [one];
    // Where a value is kept is said once in its group's header (DS-8), and still finds it.
    const settings = one.settings.filter((setting) =>
      holds(setting.label, setting.help, (inAFile(setting) && setting.kept) || ""),
    );
    const after = one.after?.filter((setting) => holds(setting.label, setting.help));
    // A group a filter finds only by what it explains is found too (#1340).
    return settings.length > 0 || (after?.length ?? 0) > 0 ? [{ ...one, settings, after }] : [];
  });
}

/** The chosen group, drawn from its data. Keyed by the group, so each setting's hook is always
 *  the same one in a given row. */
function ShownGroup({
  level,
  group,
  driver,
  onNew,
  onGo,
  reachable,
  adding,
  onAction,
}: {
  /** The level the group is of: what decides, with each row's file, whether the row stays on in
   *  a read-only project ({@link stillWritten}). */
  level: Level;
  group: SettingsGroup;
  driver?: Driven<unknown>;
  onNew?: OnNew;
  /** The Add form a picker's New… opened in this group's collection (ST-4). */
  adding?: Asked;
  /** Opens another group of this level: a link a collection's refusal carries. */
  onGo: (group: string) => void;
  /** Whether {@link onGo} can open `group`. */
  reachable: (group: string) => boolean;
  /** Runs a link out of Settings (#1387): none where no project's window can. */
  onAction?: (action: string) => void;
}) {
  // Where the group keeps its values: said once in its header, and a row kept elsewhere badged.
  const home = homeOf(group.settings);
  // Each row by the file it writes (#1759). A row that may move between the files writes where
  // its value is held: one held in the local file stays on, and only its move, which writes the
  // committed file too, is off (`FileRow`); one held nowhere writes the committed file when it
  // is set. A row drawn by a hook of its own is on by what it says it writes.
  const row = (setting: Setting) =>
    inAFile(setting) ? (
      driver && (
        <OffUnless
          key={setting.id}
          on={stillWritten(
            level,
            !setting.movable
              ? setting.file
              : heldIn(setting, driver.files) === "local"
                ? "local"
                : undefined,
          )}
          steady={setting.movable === true}
        >
          <FileRow
            setting={setting}
            driver={driver}
            home={home?.file}
            onNew={onNew}
            onAction={onAction}
          />
        </OffUnless>
      )
    ) : (
      <OffUnless key={setting.id} on={liveStillWritten(level, setting.writes)}>
        <LiveRow setting={setting} />
      </OffUnless>
    );
  return (
    <SettingGroup label={group.label} help={group.help} kept={home?.kept ?? group.kept}>
      {group.notes?.map((why, at) => (
        <p key={at} className="ui-setting-help">
          {why}
        </p>
      ))}
      {group.settings.length === 0 && group.collection === undefined && group.empty && (
        <p className="ui-setting-help">{group.empty}</p>
      )}
      {onAction && (group.links?.length ?? 0) > 0 && (
        <p className="ui-setting-links">
          {group.links?.map((one) => (
            <Button size="sm" key={one.action} onPress={() => onAction(one.action)}>
              {one.label}
            </Button>
          ))}
        </p>
      )}
      {group.collection && driver ? (
        <OffUnless on={stillWritten(level, collectionFile(group.collection.name))}>
          <CollectionView
            id={group.id}
            collection={group.collection}
            settings={group.settings}
            driver={driver}
            row={row}
            onGo={onGo}
            reachable={reachable}
            adding={adding}
          />
        </OffUnless>
      ) : (
        group.settings.map(row)
      )}
      {group.after?.map(row)}
    </SettingGroup>
  );
}

function LiveRow({ setting }: { setting: LiveSetting }) {
  const { control, reset, grouped, error, undo } = setting.useControl();
  return (
    <SettingRow
      setting={setting.id}
      label={setting.label}
      help={setting.help}
      reset={reset}
      grouped={grouped}
      error={error}
      undo={undo}
      control={control}
    />
  );
}

/**
 * **Where a group keeps its values** (DS-8, #1840): the file most of its settings are kept in,
 * the first one's on a tie, with the sentence its header says once. A row kept anywhere else is
 * badged ({@link badgeOf}). `undefined` for a group with no setting kept in a file.
 */
function homeOf(settings: readonly Setting[]): { file: SettingsFileId; kept?: string } | undefined {
  const inFiles = settings.filter(inAFile);
  const count = new Map<SettingsFileId, number>();
  for (const one of inFiles) count.set(one.file, (count.get(one.file) ?? 0) + 1);
  let home: FileSetting | undefined;
  for (const one of inFiles)
    if (home === undefined || (count.get(one.file) ?? 0) > (count.get(home.file) ?? 0)) home = one;
  return home && { file: home.file, kept: home.kept };
}

/** What a badge calls each file's level (D3): who sees a value kept there. */
const LEVEL_OF: Readonly<Record<SettingsFileId, string>> = {
  shared: "Team",
  local: "This machine",
  workspace: "Workspace",
};

/**
 * **Where a value comes from** (SE-18, V89d): the level and the file, or that no file at this
 * level holds it. A movable value kept on this machine over one the Shared file holds says what
 * the Shared file has, so the person sees why their value is not their team's. Only a movable
 * one: a Local-only setting at a key charter.toml also has (the default profile, where
 * charter.toml has the default harness) is another setting, not an override of it.
 */
function originOf(setting: FileSetting, files: Files, from: SettingsFileId | undefined): string {
  if (from === undefined) return "Not set at this level, so the value beneath it is in force.";
  const file = files[from]?.file ?? "";
  if (from === "workspace") return `From ${file}, at the Workspace level.`;
  if (from === "shared") return `From ${file}, at the Project level: shared with your team.`;
  const shared = sharedUnder(setting, files);
  const committed = files.shared?.file ?? "purlis.toml";
  const under = shared === undefined ? "" : ` ${committed} has ${shared}, which this overrides.`;
  return `From ${file}, at the Project level: this machine only.${under}`;
}

/**
 * **A row's badge** (DS-8, #1840; D3): only where its value is kept elsewhere than its group
 * keeps its values — or, while no file holds it, where its next value would go — named by that
 * level, with {@link originOf}'s sentence in its tooltip.
 */
function badgeOf(
  setting: FileSetting,
  files: Files,
  from: SettingsFileId | undefined,
  into: SettingsFileId,
  home: SettingsFileId | undefined,
): Badge | undefined {
  const at = from ?? into;
  if (home === undefined || at === home) return undefined;
  const origin = originOf(setting, files, from);
  const name = files[at]?.file ?? (at === "local" ? "purlis.local.toml" : "purlis.toml");
  const next = from === undefined ? ` A value set here is kept in ${name}.` : "";
  return { label: LEVEL_OF[at], says: `${origin}${next}` };
}

/**
 * What charter.toml has for a movable setting whose value this machine's file overrides — `a
 * value` when it reads as nothing — or `undefined` when nothing is overridden.
 */
function sharedUnder(setting: FileSetting, files: Files): string | undefined {
  const shared = files.shared;
  if (!setting.movable || shared === undefined || keysOf(setting, shared).length === 0)
    return undefined;
  return setting.read(shared) || "a value";
}

/** The project's two files' names, as read; purlis's own names before they are read. */
function namesIn(files: Files): Record<SettingsWhich, string> {
  return {
    shared: files.shared?.file ?? "purlis.toml",
    local: files.local?.file ?? "purlis.local.toml",
  };
}

/** Every key the setting has in `file`, taken out. */
function removals(setting: FileSetting, files: Files, from: SettingsFileId | undefined) {
  const file = from === undefined ? undefined : files[from];
  return file === undefined
    ? []
    : keysOf(setting, file).map((path): SettingsEdit => ({ path, value: null }));
}

/** What an on/off key's switch can show: on, off, or not set here (the value in force). */
const ON_OFF: ReadonlySet<string> = new Set(["", "on", "off"]);

/**
 * **A setting kept in a file**: drawn at what is on disk — or at what is being written, until
 * that write settles — and written as it changes. A pick is written at once; a typed value when
 * the field is left (`Field`'s `onCommit`).
 *
 * Every row knows where its value comes from and offers a reset, which takes it out of that
 * file so the value beneath shows through (SE-18). Drawn quietly (DS-8, #1840): a value set here
 * is marked, and one kept elsewhere than its group (`home`) keeps its values is badged. A movable
 * row's "Move to…" menu moves a value between `charter.toml` and `charter.local.toml`: a move
 * writes both files and has no Undo, so it is made by choosing a row of the menu, never as the
 * arrows pass over it (`ui-primitives.md`). While no file holds the value, the menu only says
 * where the next value goes, and writes nothing.
 *
 * An on/off key whose value in force is known is a switch (#1865): not set is the switch at that
 * value with no Reset and no mark, and turning it writes the key here, which Reset takes out.
 */
function FileRow({
  setting,
  driver: project,
  home,
  onNew,
  onAction,
}: {
  setting: FileSetting;
  driver: Driven<unknown>;
  /** The file the row's group keeps its values in ({@link homeOf}). */
  home?: SettingsFileId;
  onNew?: OnNew;
  /** Runs a link out of Settings: a picked persona's Show (#1388). */
  onAction?: (action: string) => void;
}) {
  const from = heldIn(setting, project.files);
  const [pick, setPick] = useState<SettingsWhich>();
  /** The file a new value goes to: where it is, else where it was picked to go. */
  const into: SettingsFileId = from ?? pick ?? setting.file;
  const file = project.files[into];
  const onDisk = file === undefined ? "" : setting.read(file);
  const [draft, setDraft] = useState<string>();
  const value = draft ?? project.pending[setting.id] ?? onDisk;
  const writing = project.pending[setting.id] !== undefined;
  const write = (to: string) => project.write(setting, to, into);
  // Always handed to the queue, which skips a write that would change nothing at its turn: a
  // value typed back to what is on disk while another write of it is pending is still written.
  const commit = () => {
    if (draft === undefined) return;
    setDraft(undefined);
    write(draft);
  };
  const out = removals(setting, project.files, from);
  // Never one that would loosen what the level holds fast: the sandbox (D-SE17g, D-SE18e).
  const mayTakeOut = from !== undefined && !setting.oneWay && project.mayChange(out);
  /** What charter.toml has under this machine's value, which a move to it would replace. */
  const overridden = from === "local" ? sharedUnder(setting, project.files) : undefined;
  return (
    <SettingRow
      setting={setting.id}
      label={setting.label}
      help={setting.help}
      // A row of boxes or a status line is named by its label, as a group (#1340).
      grouped={setting.kind === "checks" || setting.kind === "status"}
      error={
        project.refused[setting.id] ??
        // The core's word on a value that names nothing, until a write replaces it (ST-1).
        (setting.standing !== undefined && !writing ? [setting.standing] : undefined)
      }
      undo={project.undoable === setting.id && !writing ? project.undo : undefined}
      origin={originOf(setting, project.files, from)}
      badge={badgeOf(setting, project.files, from, into, home)}
      modified={from !== undefined}
      reset={
        mayTakeOut
          ? {
              label: setting.resets ?? "Reset",
              disabled: writing,
              onReset: () => project.reset(setting),
            }
          : undefined
      }
      place={
        setting.movable && (
          // A move writes the committed file whichever way it goes: off on a read-only project,
          // where the value itself may still be on (#1759).
          <OffWhenReadOnly>
            <MoveTo
              names={namesIn(project.files)}
              held={from === "shared" || from === "local" ? from : undefined}
              pick={pick ?? (from === "shared" || from === "local" ? from : "shared")}
              onPick={setPick}
              mayMove={mayTakeOut}
              replacing={overridden}
              disabled={writing}
              onMove={(to) => {
                setPick(undefined);
                project.move(setting, to);
              }}
            />
          </OffWhenReadOnly>
        )
      }
      control={(ids) =>
        setting.kind === "status" ? (
          <Status ids={ids} setting={setting} value={value} writing={writing} onWrite={write} />
        ) : setting.kind === "checks" ? (
          <Choice
            kind="checks"
            ids={ids}
            options={setting.options ?? []}
            checked={new Set(entries(value))}
            onCheckedChange={(one, on) => {
              const ticked = new Set(entries(value));
              if (on) ticked.add(one);
              else ticked.delete(one);
              // In the boxes' order, whatever order they were ticked in.
              write(
                (setting.options ?? [])
                  .map((option) => option.value)
                  .filter((option) => ticked.has(option))
                  .join("\n"),
              );
            }}
          />
        ) : setting.kind === "colour" ? (
          <Colour ids={ids} setting={setting} value={value} onValueChange={write} />
        ) : setting.names !== undefined ? (
          <>
            <Picker
              ids={ids}
              setting={setting}
              names={setting.names}
              value={value}
              onValueChange={write}
              onNew={onNew}
            />
            {/* A persona stays in its own tab, and the picker links to it (#1388): only one the
                project lists, so the link never names a persona that is not there. */}
            {onAction &&
              setting.names === "persona" &&
              value !== "" &&
              (setting.choices ?? []).includes(value) && (
                <Button size="sm" onPress={() => onAction(showPersona(value))}>
                  {`Show ${value}`}
                </Button>
              )}
          </>
        ) : setting.reading && value === "" ? (
          // What is in force is still being read (#1875): a placeholder, so the row does not
          // draw a select that turns into a switch once the core answers. Nothing to pick yet.
          <span className="ui-setting-help" aria-labelledby={ids.labelledBy} aria-busy="true">
            Reading…
          </span>
        ) : (setting.inForce !== undefined || setting.reading) && ON_OFF.has(value) ? (
          // An on/off key whose value in force is known (#1865): a switch at what is in force —
          // the file's value, or the one beneath while the file does not set it — and the row's
          // Reset takes it back to not set. A value the file holds that is neither stays a
          // select below, so it is shown as held. One the file sets is a switch at once, even
          // while what is beneath is still read (#1875): the file's value is what it shows.
          <Choice
            kind="switch"
            ids={ids}
            checked={value === "" ? setting.inForce === true : value === "on"}
            onCheckedChange={(on) => write(on ? "on" : "off")}
          />
        ) : setting.kind === "choice" ? (
          <Choice
            kind="select"
            ids={ids}
            options={[
              // A value the file holds that is not one of the choices is still shown as held:
              // the core decides what it means, and a select that showed another would write it.
              ...(value !== "" && !setting.choices?.includes(value) ? [value] : []),
              ...(setting.choices ?? []),
            ].map((one) => ({ value: one, label: setting.labels?.[one] ?? one }))}
            value={value}
            unset={
              setting.oneWay && writing
                ? undefined
                : (setting.unset ?? (value === "" ? "not set" : undefined))
            }
            onValueChange={write}
          />
        ) : (
          <Field
            kind={setting.kind === "lines" ? "list" : "text"}
            ids={ids}
            value={value}
            onChange={setDraft}
            onCommit={commit}
          />
        )
      }
    />
  );
}

/**
 * **A setting shown as a status line** (#1340): what the value means, in a sentence, and at most
 * one button, offered while no file sets the value — the Sandbox's Turn the sandbox on, which has
 * no way back here (D-SE17g). A group the row's label names, so the line is read with its name.
 */
function Status({
  ids,
  setting,
  value,
  writing,
  onWrite,
}: {
  ids: RowIds;
  setting: FileSetting;
  value: string;
  writing: boolean;
  onWrite: (to: string) => void;
}) {
  const { turnOn, goTo } = setting;
  return (
    <div
      id={ids.id}
      role="group"
      className="ui-setting-status"
      aria-labelledby={ids.labelledBy}
      aria-describedby={ids.describedBy}
    >
      <p>{setting.status?.(value) ?? value}</p>
      {turnOn !== undefined && value === "" && (
        <Button isDisabled={writing} onPress={() => onWrite(turnOn.value)}>
          {turnOn.label}
        </Button>
      )}
      {goTo !== undefined && <Button onPress={goTo.onPress}>{goTo.label}</Button>}
    </div>
  );
}

/**
 * **"Move to…"** (SE-18, V89d; a menu since DS-8, #1840, in place of the "Where it is kept"
 * radio): where a file holds the value, a row per file it may move to, and choosing one moves it
 * — both files, no Undo, so only a row chosen moves anything, never an arrow passing over one.
 * Where none holds it, the menu picks where the next value goes, and writes nothing. Closed, the
 * focus goes back to its button, which stays.
 */
function MoveTo({
  names,
  held,
  pick,
  onPick,
  mayMove,
  replacing,
  disabled,
  onMove,
}: {
  /** The project's two files, as it names them. */
  names: Readonly<Record<SettingsWhich, string>>;
  /** The file that holds the value, if one does. */
  held: SettingsWhich | undefined;
  pick: SettingsWhich;
  onPick: (to: SettingsWhich) => void;
  mayMove: boolean;
  /** What a move to charter.toml writes over: the team's value, which no Undo puts back. */
  replacing: string | undefined;
  disabled: boolean;
  onMove: (to: SettingsWhich) => void;
}) {
  // A value the level holds fast (the sandbox) is moved nowhere.
  if (held !== undefined && !mayMove) return null;
  const places = [
    { value: "shared", label: LEVEL_OF.shared, says: `${names.shared}, which your team sees` },
    { value: "local", label: LEVEL_OF.local, says: `${names.local}, on this machine only` },
  ] as const;
  return (
    // **Under its button, lined up with its end** (R59): the button stands at the row's end.
    <MenuTrigger>
      <Button variant="ghost" isDisabled={disabled} className="ui-setting-reset">
        Move to…
      </Button>
      <Menu className="ui-setting-move" align="end">
        {held !== undefined ? (
          places
            .filter((one) => one.value !== held)
            .map((one) => {
              // What the move writes over, which no Undo puts back: the one line that stays
              // under the row, because it changes the decision (the menus' rule 2).
              const over =
                one.value === "shared" && replacing !== undefined ? replacing : undefined;
              return (
                <Place
                  key={one.value}
                  label={one.label}
                  says={over === undefined ? one.says : `${one.says}, replacing ${over}`}
                  under={over === undefined ? undefined : `Replaces ${over} there.`}
                >
                  {(row) => (
                    <MenuItem id={one.value} onAction={() => onMove(one.value)} {...row.props}>
                      {row.inside}
                    </MenuItem>
                  )}
                </Place>
              );
            })
        ) : (
          <MenuSection
            title="Where the next value is kept"
            selectionMode="single"
            disallowEmptySelection
            selectedKeys={[pick]}
            onSelectionChange={(keys) => {
              const [to] = keys === "all" ? [] : [...keys];
              if (to !== undefined) onPick(to as SettingsWhich);
            }}
          >
            {places.map((one) => (
              <Place key={one.value} label={one.label} says={one.says}>
                {(row) => (
                  <MenuItem id={one.value} {...row.props}>
                    {row.inside}
                  </MenuItem>
                )}
              </Place>
            ))}
          </MenuSection>
        )}
      </Menu>
    </MenuTrigger>
  );
}

/**
 * One place a value can be kept, as a row of "Move to…"'s menu, by the menus' rules
 * (`docs/ui-primitives.md`, "Menus"): the row says the level, and which file that is, and who
 * sees it, is its tooltip and its description rather than a line under it.
 */
function Place({
  label,
  says,
  under,
  children,
}: {
  label: string;
  /** Which file, and who sees it: the row's tooltip and description. */
  says: string;
  /** The one line under the row: what choosing it writes over. */
  under?: string;
  children: (row: {
    props: {
      textValue: string;
      "aria-label": string;
      "aria-describedby": string;
      onFocus: () => void;
      onBlur: () => void;
    };
    inside: ReactNode;
  }) => ReactNode;
}) {
  const noted = useId();
  const { rested, rest, wake, hold } = useRested(true);
  return children({
    props: { textValue: label, "aria-label": label, "aria-describedby": noted, ...rest },
    inside: (
      <>
        <MenuWords said={label} under={under} />
        <span id={noted} hidden>
          {says}
        </span>
        {rested && (
          <MenuTip side="left" onClose={wake} hold={hold}>
            {says}
          </MenuTip>
        )}
      </>
    ),
  });
}

/** What New… is called in a picker over each collection (ST-1). */
const NEW: Record<Entry, string> = {
  persona: "New persona…",
  workspace: "New workspace…",
  profile: "New profile…",
};

/** The value of a picker's New… option: one no name can be (a NUL is in none). */
const MAKE_ONE = "\u0000new";

/**
 * **A picker over one of the project's collections** (ST-1, #1225; V91r): what the project has,
 * and New…, which opens the matching create flow and writes nothing until something is made —
 * then that is picked. A value the file holds that is not listed is shown as held; where the core
 * says it names nothing, it is marked so and the row says the core's sentence, until any pick
 * replaces it.
 */
function Picker({
  ids,
  setting,
  names,
  value,
  onValueChange,
  onNew,
}: {
  ids: RowIds;
  setting: FileSetting;
  names: Entry;
  value: string;
  onValueChange: (to: string) => void;
  onNew?: OnNew;
}) {
  const listed = setting.choices;
  const held = value !== "" && !(listed ?? []).includes(value);
  return (
    <Choice
      kind="select"
      ids={ids}
      options={[
        // Marked as naming nothing only where the core says so (V91l): a workspace declared
        // before it is made, or an entry no list here shows, is the core's to judge.
        ...(held
          ? [
              {
                value,
                label: setting.standing === undefined ? value : `${value} — names nothing here`,
              },
            ]
          : []),
        ...(listed ?? []).map((one) => ({ value: one, label: one })),
        ...(onNew ? [{ value: MAKE_ONE, label: NEW[names] }] : []),
      ]}
      value={value}
      unset={setting.unset}
      onValueChange={(to) => {
        if (to === MAKE_ONE) onNew?.(names, onValueChange);
        else onValueChange(to);
      }}
    />
  );
}

/** The pick that says a workspace's colour is its own `#rrggbb` rather than a palette name. */
const CUSTOM = "custom";

/**
 * **A colour** (a workspace's, charter-app#281): the palette as a select, and — while the pick
 * is custom — the platform's own colour well beside it, labelled, whose value is the `#rrggbb`
 * the file holds. Picking Custom writes the accent the window is drawn in, as a start; the well
 * writes its colour once it is picked — the native `change`, when the picker closes — or left,
 * never at every step of a drag (React's `onChange` is every step).
 */
function Colour({
  ids,
  setting,
  value,
  onValueChange,
}: {
  ids: RowIds;
  setting: FileSetting;
  value: string;
  onValueChange: (to: string) => void;
}) {
  const well = useId();
  const [dragged, setDragged] = useState<string>();
  // A `#rrggbb`: what the colour well can hold. Anything else the file holds that is not a
  // palette name — `#fff`, say — is shown as held rather than as a custom colour the well would
  // silently turn black.
  const custom = /^#[0-9a-fA-F]{6}$/.test(value);
  const names = (setting.choices ?? []).filter((one) => one !== CUSTOM);
  /** Where a new custom colour starts: the accent the window is drawn in, as `#rrggbb`. */
  const start = () =>
    /^#[0-9a-fA-F]{6}/.exec(inForce().values["accent.base"])?.[0] ??
    DEFAULT_THEME.values["accent.base"];
  const commit = (to: string | undefined) => {
    setDragged(undefined);
    if (to !== undefined && to !== value) onValueChange(to);
  };
  return (
    <>
      <Choice
        kind="select"
        ids={ids}
        options={[
          ...(value !== "" && !custom && !names.includes(value) ? [value] : []),
          ...(setting.choices ?? []),
        ].map((one) => ({ value: one, label: setting.labels?.[one] ?? one }))}
        value={custom ? CUSTOM : value}
        unset={setting.unset}
        onValueChange={(to) => {
          if (to === CUSTOM) {
            if (!custom) onValueChange(start());
          } else onValueChange(to);
        }}
      />
      {custom && (
        <>
          <label htmlFor={well}>Custom colour</label>
          <ColorWell id={well} value={dragged ?? value} onDraft={setDragged} onCommit={commit} />
        </>
      )}
    </>
  );
}
