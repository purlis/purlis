// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createContext, useEffect, useRef, useSyncExternalStore } from "react";
import {
  commands,
  PERSONA_SCOPE_KEY,
  SKILL_NEEDS_IN,
  SKILL_STUDIO_IN,
  type SkillPinner,
  type SkillPinsRefused,
} from "./bindings";
import { contentsOf, type Tabs, type ViewRef } from "./tabs";

/**
 * **The Skill studio** (SK-2, #2765; spec #2762, #3679): every skill a chat in the project gets,
 * with its level, and every one left out with why.
 *
 * It is a view tab, `{ from: null, view: "skill-studio", key: "" }`: **one per project** (#3681,
 * the amendment's S2), holding its own pages, scope and back/forward history. A second studio tab
 * opens only when the person asks for one (⌘-click), keyed `#2`, `#3`…. It is behind the
 * `skill-studio` flag until its finish line (#2786): while the flag is off the palette offers no
 * row for it.
 */
export const SKILL_STUDIO_VIEW: ViewRef = { from: null, view: "skill-studio", key: "" };

/** What the Skill studio's tab is called. */
export const SKILL_STUDIO_TITLE = "Skill studio";

/** Whose chats the studio shows (#2767, #2768): at the project's root, in a workspace, or as a
 *  persona, which brings its skills into every workspace. */
export type StudioAt = "root" | "workspace" | "persona";

/** **Whose chats the studio shows**: `name` is the workspace's or the persona's, empty at the root. */
export type StudioScope = { at: StudioAt; name: string };

/**
 * **Each kind of scope, in one place**: the prefix of its view key (and so of the studio's
 * picker's value, which is the same key), what its tab is called, and what the studio says of
 * whose chats it shows. A workspace's name is one plain name and never holds the `/` a persona's
 * key starts with.
 */
const SCOPES: Record<
  StudioAt,
  { prefix: string; title: (name: string) => string; chat: (name: string) => string }
> = {
  root: { prefix: "", title: () => SKILL_STUDIO_TITLE, chat: () => "A chat at the project's root" },
  workspace: {
    prefix: "",
    title: (name) => `${SKILL_STUDIO_TITLE} · ${name}`,
    chat: (name) => `A chat in ${name}`,
  },
  persona: {
    prefix: PERSONA_SCOPE_KEY,
    title: (name) => `${SKILL_STUDIO_TITLE} · ${name}`,
    chat: (name) => `A chat as ${name}`,
  },
};

/** A chat at the project's root. */
export const AT_THE_ROOT: StudioScope = { at: "root", name: "" };

/** A chat in `workspace`. */
export function inWorkspace(workspace: string): StudioScope {
  return { at: "workspace", name: workspace };
}

/** A chat as `persona`, at the project's root (#2768). */
export function asPersona(persona: string): StudioScope {
  return { at: "persona", name: persona };
}

/** The workspace and the persona a scope names, as the core takes them: `null` for none. */
export function whoseChats(scope: StudioScope): {
  workspace: string | null;
  persona: string | null;
} {
  return {
    workspace: scope.at === "workspace" ? scope.name : null,
    persona: scope.at === "persona" ? scope.name : null,
  };
}

/** What the studio's view is keyed by for `scope`: the one encoding of a scope. */
export function scopeKey(scope: StudioScope): string {
  return `${SCOPES[scope.at].prefix}${scope.name}`;
}

/** The scope a studio view's key names. */
export function scopeOfKey(key: string): StudioScope {
  if (key === "") return AT_THE_ROOT;
  const persona = SCOPES.persona.prefix;
  return key.startsWith(persona) ? asPersona(key.slice(persona.length)) : inWorkspace(key);
}

/** Two scopes are one. */
export function sameScope(one: StudioScope, other: StudioScope): boolean {
  return one.at === other.at && one.name === other.name;
}

// --- Its pages, and the places an entry point opens it at ------------------------------------

/**
 * **The studio's pages** (the amendment's S2): Library, Needs you, Drafts, Sources, Deleted, then
 * Harness skills and Routing. Needs you and Drafts are in the navigation only while they hold
 * something.
 */
export const STUDIO_PAGES = [
  "library",
  "needs",
  "drafts",
  "sources",
  "deleted",
  "harness",
  "routing",
] as const;
export type StudioPage = (typeof STUDIO_PAGES)[number];

/** What each page is called, in the navigation, its path and its heading. */
export const STUDIO_PAGE_NAMES: Record<StudioPage, string> = {
  library: "Library",
  needs: "Needs you",
  drafts: "Drafts",
  sources: "Sources",
  deleted: "Deleted",
  harness: "Harness skills",
  routing: "Routing",
};

/**
 * **The tabs of a skill's page** (#3682, the amendment's S3): Overview, Text, History, Grades and
 * Runs. Overview is where a page opens.
 */
export const SKILL_PAGE_TABS = ["overview", "text", "history", "grades", "runs"] as const;
export type SkillPageTab = (typeof SKILL_PAGE_TABS)[number];

/** What each of a skill's tabs is called, on the tab and in the path. */
export const SKILL_PAGE_TAB_NAMES: Record<SkillPageTab, string> = {
  overview: "Overview",
  text: "Text",
  history: "History",
  grades: "Grades",
  runs: "Runs",
};

/**
 * **A grading page under a skill** (#3688, the amendment's S6): the plan (`afterEdit`: the plan
 * that turns on grading after each edit), a grade running, its scorecard, or one of its chats.
 */
export type GradingAt =
  | { at: "plan"; afterEdit?: true }
  | { at: "running"; grade: string }
  | { at: "scorecard"; grade: string }
  | { at: "chat"; grade: string; chat: string };

/** A grading page read back from a target, or nothing for one that names none. */
function gradingOf(read: unknown): GradingAt | undefined {
  if (typeof read !== "object" || read === null) return undefined;
  const { at, grade, chat, afterEdit } = read as Record<string, unknown>;
  if (at === "plan") return afterEdit === true ? { at, afterEdit } : { at };
  if (typeof grade !== "string") return undefined;
  if (at === "scorecard") return { at, grade };
  if (at === "running") return { at, grade };
  return at === "chat" && typeof chat === "string" ? { at, grade, chat } : undefined;
}

/**
 * **A place in the studio**: one step of its tab's history (`ui/panel/viewHistory.ts`). The
 * page, whose chats it covers, on Library the skill chosen (its row's id, {@link skillRowId}),
 * the tab of its page shown (`tab`, Overview where none is said) and on Runs the run to mark
 * (`run`, from a run's chip), on another page the item chosen in its list (`item`: a draft's
 * id on Drafts, a deleted skill's on Deleted), and on Sources the page inside it (`sources`).
 * `grading` is a grading page under the skill.
 */
export interface StudioPlace {
  page: StudioPage;
  scope: StudioScope;
  skill?: string;
  tab?: SkillPageTab;
  run?: string;
  item?: string;
  grading?: GradingAt;
  /** On Sources, the page inside it: a source's own, or a review (#3687). */
  sources?: SourcesAt;
}

/**
 * **A page inside Sources** (#3687, the amendment's S5): a source's own page, or the review of
 * installing or updating one of its skills. A source is named by its repository and branch
 * ({@link sourceKey}), and `label` is how the path over the page names it.
 */
export type SourcesAt =
  | { at: "source"; source: string; label: string }
  | { at: "install"; source: string; label: string; skill: string }
  | { at: "update"; source: string; label: string; skill: string };

/** **A source's key**: its repository and the branch read from it, which no other source shares. */
export function sourceKey(remote: { url: string; branch: string | null }): string {
  return `${remote.url}\u0000${remote.branch ?? ""}`;
}

/** Where a studio tab starts when nothing opened it anywhere: Library, at the project's root. */
export const STUDIO_START: StudioPlace = { page: "library", scope: AT_THE_ROOT };

/** **One row of the Library**, as a place names it: a skill's level, its folder, whose it is
 *  and its name, which no other row shares (an installed skill has no folder in the project, and
 *  two programs' own may share a name). */
export function skillRowId(skill: {
  level: string;
  local: boolean;
  folder: string;
  name: string;
  owner?: { kind: string; name?: string } | null;
}): string {
  const whose = skill.owner == null ? "" : `${skill.owner.kind}${skill.owner.name ?? ""}`;
  return `${skill.level}${skill.local ? "+local" : ""}:${skill.folder}:${whose}#${skill.name}`;
}

/** The name a row's id ends with. */
export function skillOfRowId(id: string): string {
  return id.slice(id.lastIndexOf("#") + 1);
}

/** **A row named only by its skill's name**: where a change left the skill with a new row
 *  (renamed, shared, moved), the place goes on to the row of that name. */
export function skillRowNamed(name: string): string {
  return `#${name}`;
}

/** Two places are one: the same page, scope, skill and item chosen, whichever tab of its page
 *  shows, and the same page inside Sources. */
export function samePlace(one: StudioPlace, other: StudioPlace): boolean {
  return (
    one.page === other.page &&
    sameScope(one.scope, other.scope) &&
    one.skill === other.skill &&
    one.item === other.item &&
    sameSourcesAt(one.sources, other.sources) &&
    JSON.stringify(one.grading) === JSON.stringify(other.grading)
  );
}

/** **Whether a way back to `to` is a step back** (#3978): only where the step behind is `to`, on
 *  the same tab of its skill's page. Else `to` is said in place of where the person is, so Back
 *  to the scorecard or to a skill's grades lands where it names, never on whatever came before. */
export function backIsTo(behind: StudioPlace | undefined, to: StudioPlace): boolean {
  if (behind === undefined || (behind.tab ?? "overview") !== (to.tab ?? "overview")) return false;
  // A row named by its skill's name alone is the row it was settled on since (#3978).
  const named = (id: string | undefined) => id !== undefined && id.startsWith("#");
  const sameSkill =
    behind.skill === to.skill ||
    ((named(behind.skill) || named(to.skill)) &&
      skillOfRowId(behind.skill ?? "") === skillOfRowId(to.skill ?? ""));
  return sameSkill && samePlace({ ...behind, skill: to.skill }, to);
}

/** Two pages inside Sources are one: the same page, of the same source and skill. */
function sameSourcesAt(one: SourcesAt | undefined, other: SourcesAt | undefined): boolean {
  if (one === undefined || other === undefined) return one === other;
  const skill = (at: SourcesAt) => (at.at === "source" ? undefined : at.skill);
  return one.at === other.at && one.source === other.source && skill(one) === skill(other);
}

/** A target's key starts with this: it names a place to open the studio at, never a tab. */
const TARGET = "at:";

/**
 * **The studio, opened at a place** (#3681): what every entry point hands `openView`, so each
 * lands on its page and scope. It is a target, not a tab: the window's one way of opening views
 * ({@link studioOpening}) turns it into the studio's tab, with the place recorded in that tab's
 * history. `newTab` asks for a studio tab of its own (⌘-click).
 */
export function skillStudioAt(
  scope: StudioScope,
  page: StudioPage = "library",
  {
    skill,
    tab,
    run,
    item,
    grading,
    newTab = false,
  }: {
    skill?: string;
    tab?: SkillPageTab;
    run?: string;
    item?: string;
    grading?: GradingAt;
    newTab?: boolean;
  } = {},
): ViewRef {
  const place: StudioPlace & { newTab?: true } = { page, scope };
  if (skill !== undefined) place.skill = skill;
  if (tab !== undefined) place.tab = tab;
  if (run !== undefined) place.run = run;
  if (item !== undefined) place.item = item;
  if (grading !== undefined) place.grading = grading;
  if (newTab) place.newTab = true;
  return { ...SKILL_STUDIO_VIEW, key: `${TARGET}${JSON.stringify(place)}` };
}

/**
 * **The Skill studio's Library on one scope's chats** (#2767): a Skills summary row's, a
 * persona page's and the palette's way in.
 */
export function skillStudioView(scope: StudioScope): ViewRef {
  return skillStudioAt(scope, "library");
}

/** Whether `key` is a studio tab's own: the first (`""`) or one asked for beside it (`#2`). */
function isTabKey(key: string): boolean {
  return key === "" || /^#[0-9]+$/.test(key);
}

/** A place read back from a target's key, or nothing for a key that names none. */
function placeOfTarget(key: string): (StudioPlace & { newTab?: boolean }) | undefined {
  if (!key.startsWith(TARGET)) return undefined;
  try {
    const read = JSON.parse(key.slice(TARGET.length)) as Partial<StudioPlace & { newTab: boolean }>;
    const page = STUDIO_PAGES.find((one) => one === read.page);
    const scope = read.scope;
    if (page === undefined || scope === undefined || !(scope.at in SCOPES)) return undefined;
    const tab = SKILL_PAGE_TABS.find((one) => one === read.tab);
    const grading = gradingOf(read.grading);
    return {
      page,
      scope: { at: scope.at, name: String(scope.name ?? "") },
      ...(typeof read.skill === "string" ? { skill: read.skill } : {}),
      ...(tab === undefined ? {} : { tab }),
      ...(typeof read.run === "string" ? { run: read.run } : {}),
      ...(typeof read.item === "string" ? { item: read.item } : {}),
      ...(grading === undefined ? {} : { grading }),
      ...(read.newTab === true ? { newTab: true } : {}),
    };
  } catch {
    return undefined;
  }
}

/**
 * **What opening `view` opens** (#3681): a studio target becomes the studio's tab and the place
 * it lands at; a tab put back from an older launch, keyed by its scope, becomes the one tab on
 * that scope's Library; a studio tab is itself. `free` says which extra tab key is not open yet,
 * for a target that asks for a tab of its own.
 */
export function studioOpening(
  view: ViewRef,
  free: (key: string) => boolean,
): { tab: ViewRef; place?: StudioPlace } {
  if (isTabKey(view.key)) return { tab: view };
  const target = placeOfTarget(view.key);
  const place: StudioPlace =
    target === undefined
      ? { page: "library", scope: scopeOfKey(view.key) }
      : {
          page: target.page,
          scope: target.scope,
          ...(target.skill === undefined ? {} : { skill: target.skill }),
          ...(target.tab === undefined ? {} : { tab: target.tab }),
          ...(target.run === undefined ? {} : { run: target.run }),
          ...(target.item === undefined ? {} : { item: target.item }),
          ...(target.grading === undefined ? {} : { grading: target.grading }),
        };
  let key = "";
  if (target?.newTab === true) for (let n = 2; !free(key); n++) key = `#${n}`;
  return { tab: { ...SKILL_STUDIO_VIEW, key }, place };
}

/** The view name a skill's own tab had (SK-3), retired by its page in the studio (#3682). */
const SKILL_TAB = "skill";

/** The view names a grade's scorecard tab and its chat's tab had (SK-59), retired by the grading
 *  pages in the studio (#3688). */
const GRADE_TAB = "skill-grade";
const GRADE_CHAT_TAB = "skill-grade-chat";

/** **The studio at a grading page under skill `name`** (#3688): on its Grades tab. */
export function skillGrading(scope: StudioScope, name: string, grading: GradingAt): ViewRef {
  return skillStudioAt(scope, "library", { skill: skillRowNamed(name), tab: "grades", grading });
}

/**
 * **A skill's own tab an older launch kept** (SK-3), keyed `deploy` or `deploy@<run>`: it comes
 * back as the studio at Library › that skill, on its Runs where it was opened on a run, as the
 * skill's own tab is retired (#3682). A grade's scorecard tab (`deploy@<grade>`) and its chat's
 * tab (`deploy@<grade>@<chat>`) come back as that scorecard and that chat in the studio, as
 * those tabs are retired too (#3688). Nothing for any other view.
 */
export function studioOfSkillTab(view: ViewRef): ViewRef | undefined {
  if (view.from !== null) return undefined;
  if (view.view === GRADE_TAB || view.view === GRADE_CHAT_TAB) {
    const [name, grade, chat, ...rest] = view.key.split("@");
    if (!name || !grade || rest.length > 0) return undefined;
    if (view.view === GRADE_TAB)
      return chat === undefined
        ? skillGrading(AT_THE_ROOT, name, { at: "scorecard", grade })
        : undefined;
    return chat ? skillGrading(AT_THE_ROOT, name, { at: "chat", grade, chat }) : undefined;
  }
  if (view.view !== SKILL_TAB || view.key === "") return undefined;
  const at = view.key.indexOf("@");
  const name = at === -1 ? view.key : view.key.slice(0, at);
  return at === -1
    ? skillStudioAt(AT_THE_ROOT, "library", { skill: skillRowNamed(name) })
    : skillRuns(AT_THE_ROOT, name, view.key.slice(at + 1));
}

/** **The studio at a skill's Runs** (#3682), with `run` marked: where a run's chip goes, on the
 *  chats of the scope the run's chat is in. */
export function skillRuns(scope: StudioScope, name: string, run?: string): ViewRef {
  return skillStudioAt(scope, "library", {
    skill: skillRowNamed(name),
    tab: "runs",
    ...(run === undefined ? {} : { run }),
  });
}

/** **The history a studio tab keeps** (`useViewHistory`), by its project and its key: the
 *  studio's tab of each project is its own. */
export function studioHistoryOf(plane: string, view: ViewRef): string {
  return `skill-studio\u0000${plane}\u0000${view.key}`;
}

/** The studio on `scope`'s chats, as a sentence names it (`Skill studio · ops`). Its tab is
 *  called {@link SKILL_STUDIO_TITLE} whatever scope it shows, as there is one per project. */
export function skillStudioTitle(scope: StudioScope): string {
  return SCOPES[scope.at].title(scope.name);
}

/** Whose chat `scope` is, as the studio's picker names it. */
export function scopeChat(scope: StudioScope): string {
  return SCOPES[scope.at].chat(scope.name);
}

/** The catalogue row that opens the studio on a scope's chats, before the scope's key (#2767): the
 *  core names it once (`skillstudio::STUDIO_IN`). */
export { SKILL_STUDIO_IN };

/** The catalogue row that opens the studio on `persona`'s chats (#2768): what its page's summary
 *  row and a chat's missing-skills Notice run. */
export function skillStudioAs(persona: string): string {
  return `${SKILL_STUDIO_IN}${scopeKey(asPersona(persona))}`;
}

/** The catalogue row that opens the studio's Needs you on a scope's chats, before the scope's key
 *  (#3695): a summary line's "need you". The core names it once (`skillstudio::NEEDS_IN`). */
export { SKILL_NEEDS_IN };

/** The catalogue row that opens Needs you on `persona`'s chats: its page's "need you" row. */
export function skillNeedsAs(persona: string): string {
  return `${SKILL_NEEDS_IN}${scopeKey(asPersona(persona))}`;
}

/** **What opens the studio, in words**: the palette's row and the Skills view's header (#3266). */
export const OPEN_SKILL_STUDIO = "Open Skill studio";

/** The catalogue row that opens the studio on a chat at the project's root (#2765). */
export const SKILL_STUDIO_SHOW = "skills.studio";

/** Whether `view` is the Skill studio. */
export function isSkillStudio(view: ViewRef): boolean {
  return view.from === null && view.view === SKILL_STUDIO_VIEW.view;
}

/** What this machine said of the flag, once asked, and who is drawn from it. */
let said: boolean | undefined;
/** Whether the last ask was refused or failed: the preview counts as off until an ask is
 *  answered, so nothing waits on it (#3308). */
let unanswered = false;
let asking = false;
const listeners = new Set<() => void>();

function told() {
  for (const one of listeners) one();
}

function ask() {
  if (asking) return;
  asking = true;
  void commands
    .skillStudioOn()
    .then((answer) => {
      if (answer.status === "ok") said = answer.data;
      unanswered = answer.status !== "ok";
    })
    .catch(() => {
      unanswered = true;
    })
    .finally(() => {
      asking = false;
      told();
    });
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  if (said === undefined) ask();
  return () => {
    listeners.delete(listener);
  };
}

/** Whether the Skill studio is on in this window: `false` until the core has said. */
export function useSkillStudioOn(): boolean {
  return useSyncExternalStore(subscribe, () => said ?? false);
}

/** What the core said of the Skill studio's preview: `undefined` until it has, which a view
 *  that would otherwise draw something else for a moment holds on (#3308); `false` where it
 *  could not say, as the preview is then drawn off. */
export function useSkillStudioSaid(): boolean | undefined {
  return useSyncExternalStore(subscribe, () => said ?? (unanswered ? false : undefined));
}

/** **Turns the Skill studio's preview on or off on this machine** (Settings › You ›
 *  Previews), then draws what the core says is now in force, or the core's refusal. */
export async function setSkillStudioOn(on: boolean): Promise<string | undefined> {
  const answer = await commands
    .setSkillStudioOn(on)
    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
  if (answer.status === "ok") said = answer.data;
  told();
  return answer.status === "error" ? answer.error : undefined;
}

/**
 * **A start the studio is asked to open on** (#3266): the Skills view's New skill opens the
 * studio's own form, and its Make my own copy the studio's own dialog (#3693), so each start is
 * the studio's one copy. Browse sources is a page
 * ({@link skillStudioAt}'s `sources`), not a start.
 */
export type StudioStart = "new" | "copy";

/** The start asked for, by the studio tab's key, until the tab of that key takes it. */
let startAsked: { key: string; start: StudioStart } | undefined;
const startListeners = new Set<() => void>();

/** Asks the studio tab keyed `tab` (the first is `""`) to open on `start`: the one drawn now,
 *  or the next. */
export function askStudioStart(tab: string, start: StudioStart): void {
  startAsked = { key: tab, start };
  for (const one of startListeners) one();
}

/**
 * **Hands the studio tab keyed `tab` each start asked of it** (#3266), once it is `ready` (its
 * skills read): one asked before it was drawn, then each asked while it is. A start is taken
 * once, so drawing the studio again opens nothing.
 */
export function useStudioStart(
  tab: string,
  ready: boolean,
  onStart: (start: StudioStart) => void,
): void {
  const key = tab;
  const held = useRef(onStart);
  useEffect(() => {
    held.current = onStart;
  });
  useEffect(() => {
    if (!ready) return;
    const take = () => {
      if (startAsked?.key !== key) return;
      const { start } = startAsked;
      startAsked = undefined;
      held.current(start);
    };
    startListeners.add(take);
    // One asked before this studio was drawn, taken after this render.
    void Promise.resolve().then(take);
    return () => {
      startListeners.delete(take);
    };
  }, [key, ready]);
}

/** **The studio a pin is changed in** (SK-15d): its workspace's chats, or its persona's. */
export function scopeOfPinner(by: SkillPinner): StudioScope {
  return by.at === "workspace" ? inWorkspace(by.name) : asPersona(by.name);
}

/** A pinner as a sentence names it: `workspace ops`, `persona reviewer`. */
export function pinnerSaid(by: SkillPinner): string {
  return `${by.at} ${by.name}`;
}

/** Why a chat reads none of a pinner's pins, in one sentence: `workspace ops's pins aren't
 *  used: …`. The studio's Pin switch and the Skills view's menu both say it. */
export function pinsRefusedSaid(refused: SkillPinsRefused): string {
  return `${pinnerSaid(refused.by)}'s pins aren't used: ${refused.why}.`;
}

/** **The pinner the studio of `scope` pins for**, or `null` at the project's root (SK-15d). */
export function pinnerOf(scope: StudioScope): SkillPinner | null {
  if (scope.at === "root") return null;
  return { at: scope.at, name: scope.name };
}

/** **The studio tabs `before` showed that `after` does not**: each closed, so its history goes
 *  with it (`forgetViewHistory`, #3681), whichever way it was closed. */
export function studioTabsGone(before: Tabs, after: Tabs): ViewRef[] {
  const shown = (tabs: Tabs) => {
    const keys = new Set<string>();
    for (const id of tabs.order)
      for (const { content } of contentsOf(tabs, id))
        if (content.kind === "view" && isSkillStudio(content.view)) keys.add(content.view.key);
    return keys;
  };
  const now = shown(after);
  return [...shown(before)]
    .filter((key) => !now.has(key))
    .map((key) => ({ ...SKILL_STUDIO_VIEW, key }));
}

/** **The event a surface without a view opener asks the studio by** (#3681): a chat's notice. */
export const SKILL_STUDIO_ASKED = "purlis-skill-studio-asked";

/** What it asks: which project's window, and the place to open its studio at. */
export type SkillStudioAsk = {
  plane: string;
  scope: StudioScope;
  page: StudioPage;
  /** The item chosen on the page: a draft's id on Drafts. */
  item?: string;
};

/** **Asks the window of `plane` to open its Skill studio at `page` on `scope`'s chats**, at
 *  `item` on it where one is named: what a chat's skills notices' "Open Skill studio" does. */
export function askSkillStudio(
  plane: string,
  scope: StudioScope,
  page: StudioPage,
  item?: string,
): void {
  window.dispatchEvent(
    new CustomEvent<SkillStudioAsk>(SKILL_STUDIO_ASKED, {
      detail: item === undefined ? { plane, scope, page } : { plane, scope, page, item },
    }),
  );
}

// --- The Harness skills and Routing pages' items (#3692) -------------------------------------

/**
 * **A program's own skill, as a place on Harness skills names it** (#3692): its program, then
 * its row's id ({@link skillRowId}), which no other row of that program shares. What a way in
 * that names one skill (Needs you's "Show in Harness skills", #3689) hands {@link skillStudioAt}
 * as its `skill`, so the page opens at that skill with its switch beside the list.
 */
export function harnessSkillId(harness: string, skill: Parameters<typeof skillRowId>[0]): string {
  return `${harness}/${skillRowId(skill)}`;
}

/** **A program's own skill named only by its name** (#3692): where the one asking knows the
 *  program and the name and nothing else, the page goes to the first of that program's rows of
 *  that name. */
export function harnessSkillNamed(harness: string, name: string): string {
  return `${harness}/${skillRowNamed(name)}`;
}

/** The program a Harness skills place names, and the rest of it: `undefined` for one that
 *  names none. */
export function harnessOfId(id: string): { harness: string; row: string } | undefined {
  const at = id.indexOf("/");
  return at <= 0 ? undefined : { harness: id.slice(0, at), row: id.slice(at + 1) };
}

/** **A chat's start, as a place on Routing names it** (#3692): by the chat's number. */
export function routedChatId(session: number): string {
  return `chat-${session}`;
}

/**
 * **The chat a skill's page uses a skill in** (#3682): the chat last in front in this project's
 * window, by its session and the name its tab shows, which keeps its pane while the studio is
 * looked at. `undefined` while no chat is open, and Use in this chat then says so.
 */
export type ChatToUse = { session: number; name: string };

/** {@link ChatToUse}, handed to every pane of a project's window. */
export const ChatToUseHere = createContext<ChatToUse | undefined>(undefined);
