// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { lazy, Suspense, useEffect, useState, type ReactNode } from "react";
import { MessageSquarePlus, Plus, RotateCw, Search, Sparkles } from "lucide-react";
import {
  commands,
  type DeletedSkill,
  type StudioLevel,
  type SkillChanged,
  type SkillGrant,
  type SkillLevel,
  type SkillTemplate,
  type SkillTemplateId,
  type SkillUsers,
  type Studio,
  type StudioRouter,
  type StudioSkill,
} from "./bindings";
import { counted } from "./dispatches";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { SkillStudioHarnesses } from "./SkillStudioHarnesses";
import { Button, ButtonRow } from "./ui/button/Button";
import { Badge, type BadgeVariant } from "./ui/badge/Badge";
import { Fact, Facts } from "./ui/card/Facts";
import { TextField } from "./ui/field/Field";
import { List, ListRow, type Selection } from "./ui/list/List";
import { RowBelow, RowLabel, RowTrail } from "./ui/list/Row";
import { ViewFrame, ViewListHead, ViewPageHeader, type ViewPage } from "./ui/panel/ViewFrame";
import { ViewBar, type Crumb } from "./ui/panel/ViewBar";
import { useViewHistory } from "./ui/panel/viewHistory";
import { SkillPinSwitch, SkillPinsRefusedLine } from "./SkillPins";
import { RouterGrade } from "./RouterGrade";
import { Notice } from "./Notice";
import { SkillCompatBadges } from "./SkillCompatBadges";
import { SkillDrafts } from "./SkillDrafts";
import { DRAFT_WITH_A_CHAT, NEW_SKILL, SkillStarts, type SkillStart } from "./SkillStarts";
import { askDraft, useSkillDrafts } from "./skillDraftAsks";
import { SkillSources } from "./SkillSources";
import { Select } from "./ui/select/Select";
import { Choice, Field, SettingRow } from "./settings/components";
import {
  grantChoices,
  grantLabel,
  labelOf,
  levelFrom,
  levelOf,
  levelsFor,
  picking,
  whoseOf,
  sameGrant,
  sameLevel,
} from "./skillChanges";
import {
  AT_THE_ROOT,
  SKILL_STUDIO_VIEW,
  STUDIO_PAGE_NAMES,
  STUDIO_START,
  inWorkspace,
  pinnerOf,
  sameScope,
  scopeChat,
  scopeKey,
  scopeOfKey,
  samePlace,
  skillOfRowId,
  skillRowId,
  skillRowNamed,
  studioHistoryOf,
  useStudioStart,
  whoseChats,
  type StudioPage,
  type StudioPlace,
  type StudioScope,
} from "./skillStudio";
import { BUILT_IN_STAYS, PINNABLE, turnOf } from "./skillsListing";
import { skillView } from "./skillRuns";
import type { ViewRef } from "./tabs";
import { PersonaSkillAdd, changePersonaSkill } from "./personaSkills";

/** The light editor, its own chunk (`tools/main-chunk-has-no-codemirror.mjs`). */
const SkillEditor = lazy(() => import("./SkillEditor"));

/** What the studio says after a change: a refusal, or what the change left to say. */
type Said = { tone: "news" | "trouble"; label: string; text: string; restore?: string };

/** One change's answer, as the core's commands give it. */
type Answer = { status: "ok"; data: SkillChanged } | { status: "error"; error: string };

/** Runs one change and says what came of it. */
type Change = (
  label: string,
  run: () => Promise<Answer>,
  then?: () => Promise<Said | undefined>,
) => void;

/** A change in progress: New skill, or one on the skill chosen, drawn in the item pane. */
type Doing =
  | { what: "new" }
  | { what: "edit"; skill: StudioSkill; level: SkillLevel; was: string }
  | { what: "rename"; skill: StudioSkill; level: SkillLevel }
  | { what: "move"; skill: StudioSkill; from: SkillLevel; to: SkillLevel }
  | { what: "delete"; skill: StudioSkill; level: SkillLevel; users: SkillUsers };

/** What a change in progress is called in the path. */
const DOING_NAMES: Record<Doing["what"], string> = {
  new: NEW_SKILL,
  edit: "Edit",
  rename: "Rename",
  move: "Move",
  delete: "Delete",
};

/**
 * **The row of each name that carries its Pin switch** (SK-15d): of its rows at one of the
 * project's levels, the copy a chat gets, or else the first, where a pin of it may still be
 * written.
 */
function pinRowsOf(skills: readonly StudioSkill[]): Map<string, StudioSkill> {
  const rows = new Map<string, StudioSkill>();
  for (const skill of skills) {
    if (!PINNABLE.has(skill.level)) continue;
    const was = rows.get(skill.name);
    if (was === undefined || (!was.on && skill.on)) rows.set(skill.name, skill);
  }
  return rows;
}

// --- The Library's list ------------------------------------------------------------------------

/** **The Library's filter chips** (S2): each narrows the list, and none ticked shows all. */
const CHIPS = [
  { value: "own", label: "The project's own" },
  { value: "built-in", label: "Built-in" },
  { value: "on", label: "On" },
  { value: "out", label: "Off or left out" },
] as const;
type Chip = (typeof CHIPS)[number]["value"];

/** Whether `skill` is one a chip asks for. Chips of a kind (own, built-in) widen each other, as
 *  chips of a state (on, off or left out) do; a kind and a state both have to hold. */
function chipsKeep(skill: StudioSkill, chips: ReadonlySet<Chip>): boolean {
  const kinds = (["own", "built-in"] as const).filter((one) => chips.has(one));
  const states = (["on", "out"] as const).filter((one) => chips.has(one));
  const kind =
    kinds.length === 0 ||
    kinds.some((one) => (one === "own" ? PINNABLE.has(skill.level) : skill.level === "built-in"));
  const state = states.length === 0 || states.some((one) => (one === "on") === skill.on);
  return kind && state;
}

/** Whether `skill` matches what is typed in the search box: its name or its description. */
function searchKeeps(skill: StudioSkill, query: string): boolean {
  const typed = query.trim().toLowerCase();
  if (typed === "") return true;
  return `${skill.name}\n${skill.description}`.toLowerCase().includes(typed);
}

/**
 * **A row's one mark** (S2): what most needs saying of it, or nothing for a skill a chat simply
 * gets. Off where the workspace turned it off, left out where a chat does not get it, hidden
 * where a router left it off a chat's list, pinned where a pin keeps it on that list.
 */
function markOf(skill: StudioSkill): { said: string; variant: BadgeVariant } | undefined {
  if (skill.turned_off) return { said: "Off", variant: "waiting" };
  if (!skill.on) return { said: "Left out", variant: "waiting" };
  if (skill.hidden.length > 0) return { said: "Hidden by the router", variant: "neutral" };
  if (skill.pinned) return { said: "Pinned", variant: "accent" };
  return undefined;
}

/** The row's line under its name: what it does, or why it is left out where it says nothing. */
function lineOf(skill: StudioSkill): string {
  if (skill.description !== "") return skill.description;
  return skill.left_out === null
    ? "It says nothing of what it does."
    : `Left out: ${skill.left_out}`;
}

/** **Rows held while the skills are read** (S11): the layout stays, and after about a second a
 *  line says what is happening. */
function Placeholders() {
  const [late, setLate] = useState(false);
  useEffect(() => {
    const after = window.setTimeout(() => setLate(true), 1000);
    return () => window.clearTimeout(after);
  }, []);
  return (
    <div className="skill-studio-placeholders" aria-busy="true" data-testid="skill-studio-reading">
      {[0, 1, 2, 3, 4].map((at) => (
        <div key={at} className="skill-studio-placeholder" aria-hidden="true" />
      ))}
      <p role="status" className="honest">
        {late ? "Reading the project's skills…" : ""}
      </p>
    </div>
  );
}

/**
 * **The Skill studio's tab** (SK-2, #2765; spec #3679, the amendment's S2): one view tab per
 * project, laid out by the view frame (`ui/panel/ViewFrame.tsx`) as pages in a left navigation,
 * with its own back and forward and a path (`ViewBar.tsx`), and a scope picker for whose chats it
 * covers. Each place it goes to is a step of the tab's own history (`useViewHistory`), which an
 * entry point opens it at (`skillStudioAt`, `studioOpening`).
 *
 * - **Library**: every skill the scope's chats can get, a row each (its name, its skill level,
 *   one mark, and its one-line description under them), with a search box and filter chips. The
 *   skill chosen opens beside the list, or alone with a way back where the tab is narrow. Its
 *   page is the next ticket's (#3682); until then it holds the controls each row of the old
 *   table had (SK-4, SK-7, SK-15d).
 * - **Needs you** and **Drafts** are in the navigation only while they hold something.
 * - **Sources**, **Deleted**, **Harness skills** and **Routing** hold what the old page's
 *   sections held, each scrolling on its own, until their own tickets build them.
 *
 * Each change is one commit (the core's `skills::verbs`). **What it offers when it opens**
 * (SK-22, #2785; decision 42) is the core's (`skills::first_open`).
 */
export function SkillStudioTab({
  plane,
  tab = "",
  changed,
  narrow,
  onOpenView,
}: {
  plane: string;
  /** The studio tab's own key: `""` for the project's studio, `#2` for one opened beside it. */
  tab?: string;
  changed: number;
  /** Opens a skill's own tab, with its history and runs (SK-3), until its page in the studio
   *  takes them (#3682). */
  onOpenView?: (view: ViewRef, title: string) => void;
  /** Narrow or wide whatever the width: for a story or a test. Measured when left out. */
  narrow?: boolean;
}) {
  const history = useViewHistory<StudioPlace>(
    studioHistoryOf(plane, { ...SKILL_STUDIO_VIEW, key: tab }),
    STUDIO_START,
  );
  const at = history.at;
  const scope = at.scope;
  const [read, setRead] = useState<{ key: string; studio?: Studio; refused?: string }>();
  const [asked, setAsked] = useState(0);
  const [said, setSaid] = useState<Said>();
  const [done, setDone] = useState(0);
  // **A change in progress belongs to the place it was started at**: back, forward or an entry
  // point leaves it behind, so it never stands under another skill's name. One started now binds
  // to the place the tab is drawn at next, which a step taken with it has already reached.
  const [inHand, setInHand] = useState<{ doing: Doing; at: StudioPlace | "next" }>();
  const setDoing = (next: Doing | undefined) =>
    setInHand(next === undefined ? undefined : { doing: next, at: "next" });
  // Bound as it is drawn (React's state adjusted while rendering, not in an effect).
  if (inHand?.at === "next") setInHand({ doing: inHand.doing, at });
  const doing =
    inHand !== undefined && (inHand.at === "next" || samePlace(inHand.at, at))
      ? inHand.doing
      : undefined;
  const [busy, setBusy] = useState(false);
  const [deleted, setDeleted] = useState<readonly DeletedSkill[]>([]);
  const [switched, setSwitched] = useState(0);
  const [moveLater, setMoveLater] = useState(false);
  const [query, setQuery] = useState("");
  const [chips, setChips] = useState<ReadonlySet<Chip>>(new Set());
  const { workspace, persona } = whoseChats(scope);
  const key = scopeKey(scope);
  // What was read for another scope is not this one's: placeholders until this one's is.
  const studio = read?.key === key ? read.studio : undefined;
  const refused = read?.key === key ? read.refused : undefined;
  const on = studio?.on === true;
  // Read once the studio has said it is on: a scope picked later reads its skills, not these.
  const [opened, setOpened] = useState(false);
  if (on && !opened) setOpened(true);
  const drafts = useSkillDrafts(plane, changed + done);
  // A start the Skills view asked this studio to open on (#3266): the New skill form.
  useStudioStart(tab, on, () => setDoing({ what: "new" }));
  useEffect(() => {
    let live = true;
    void commands.skillStudio(plane, workspace, persona).then((answer) => {
      if (!live) return;
      if (answer.status === "ok") setRead({ key, studio: answer.data });
      else setRead({ key, refused: answer.error });
    });
    return () => {
      live = false;
    };
  }, [plane, key, workspace, persona, changed, done, switched, asked]);
  // A delete a crash or a quit cut short is finished or undone as the studio opens (SK-35),
  // said in one line each, and the lists read again.
  useEffect(() => {
    let live = true;
    if (!opened) return;
    void commands
      .skillDeletesCutShort(plane)
      .then((answer) => {
        if (!live || answer.status !== "ok") return;
        const { settled, unsettled } = answer.data;
        if (settled.length + unsettled.length === 0) return;
        // One it could neither finish nor put back is trouble, and named first.
        setSaid(
          unsettled.length > 0
            ? {
                tone: "trouble",
                label: "A skill delete could not be finished",
                text: [...unsettled, ...settled].join(" "),
              }
            : { tone: "news", label: "Skill delete cut short", text: settled.join(" ") },
        );
        setDone((n) => n + 1);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [plane, opened]);
  useEffect(() => {
    let live = true;
    if (!opened) return;
    void commands
      .skillsDeleted(plane)
      .then((answer) => {
        if (live && answer.status === "ok") setDeleted(answer.data ?? []);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [plane, opened, changed, done]);

  if (studio !== undefined && !studio.on)
    return (
      <EmptyState
        headline="The Skill studio is not on yet"
        body="Turn it on in Settings › You › Previews to try it while it is being built."
        mark={Sparkles}
        testid="skill-studio-off"
      />
    );

  /** Goes to `place`, a step of the tab's history; a change in progress is left. */
  const go = (place: StudioPlace) => {
    setDoing(undefined);
    history.go(place);
  };

  const change: Change = (label, run, then) => {
    setBusy(true);
    void run()
      .then(async (answer) => {
        if (answer.status === "error") {
          setSaid({ tone: "trouble", label, text: answer.error });
          return;
        }
        setDoing(undefined);
        const next = then === undefined ? undefined : await then();
        const notes = answer.data.notes;
        if (next !== undefined) setSaid(next);
        else if (notes.length > 0)
          setSaid({ tone: "news", label: "Skill changed", text: notes.join(" ") });
        else setSaid(undefined);
      })
      .catch((err: unknown) => setSaid({ tone: "trouble", label, text: String(err) }))
      .finally(() => {
        setBusy(false);
        setDone((n) => n + 1);
      });
  };
  const share = (skill: StudioSkill, level: SkillLevel) =>
    change("The skill was not shared", () =>
      commands
        .skillShare(plane, whoseOf(level), skill.name)
        .then((answer): Answer =>
          answer.status === "ok" ? { status: "ok", data: { notes: [] } } : answer,
        ),
    );
  const open = (skill: StudioSkill, level: SkillLevel) => {
    void commands.skillText(plane, level, skill.name).then((answer) => {
      if (answer.status === "error")
        setSaid({ tone: "trouble", label: "The skill could not be opened", text: answer.error });
      else setDoing({ what: "edit", skill, level, was: answer.data });
    });
  };
  const askDelete = (skill: StudioSkill, level: SkillLevel) => {
    void commands.skillUsers(plane, level, skill.name).then((answer) => {
      if (answer.status === "error")
        setSaid({ tone: "trouble", label: "The skill was not deleted", text: answer.error });
      else setDoing({ what: "delete", skill, level, users: answer.data });
    });
  };
  /** Moving `skill` to `to`: at once, or after its grant is asked where it becomes a project
   *  skill (decision 14). */
  const moveTo = (skill: StudioSkill, from: SkillLevel, to: SkillLevel) => {
    if (sameLevel(from, to)) return;
    void commands.skillMoveNeedsGrant(from, to).then((asks) => {
      if (asks) setDoing({ what: "move", skill, from, to });
      else
        change("The skill was not moved", () =>
          commands.skillMove(plane, from, skill.name, to, null),
        );
    });
  };
  /** Adds `written` to the persona's skills, or takes it out (SK-5, #2768), as a change. */
  const personaSkill = (who: string, written: string, how: "add" | "remove") =>
    change(how === "add" ? "The skill was not added" : "The skill was not removed", () =>
      changePersonaSkill(plane, who, written, how).then((why): Answer =>
        why === undefined ? { status: "ok", data: { notes: [] } } : { status: "error", error: why },
      ),
    );
  const restore = (id: string) =>
    change("The skill was not restored", () => commands.skillRestore(plane, id));
  /** After a delete: Restore on the Notice, for the copy just deleted. */
  const restoreOffered = async (name: string, level: SkillLevel): Promise<Said | undefined> => {
    const answer = await commands.skillsDeleted(plane);
    const one =
      answer.status === "ok"
        ? answer.data.find((d) => d.name === name && sameLevel(d.level, level))
        : undefined;
    return one === undefined
      ? undefined
      : { tone: "news", label: "Skill deleted", text: `${name} is deleted.`, restore: one.id };
  };

  /** Moves the project's skills into the studio's: the doctor's fix, said as it said it. */
  const moveSkills = (fix: string) => {
    setBusy(true);
    void commands
      .planeDoctorFix(plane, fix)
      .then((answer) => {
        if (answer.status === "error")
          setSaid({ tone: "trouble", label: "The skills were not moved", text: answer.error });
        else if (answer.data.refused !== null)
          setSaid({
            tone: "trouble",
            label: "The skills were not moved",
            text: answer.data.refused,
          });
        else
          setSaid({
            tone: answer.data.complete ? "news" : "trouble",
            label: answer.data.complete ? "Skills moved" : "Some skills were not moved",
            text: answer.data.said.join(" "),
          });
      })
      .catch((err: unknown) =>
        setSaid({ tone: "trouble", label: "The skills were not moved", text: String(err) }),
      )
      .finally(() => {
        setBusy(false);
        setDone((n) => n + 1);
      });
  };

  // --- What each page holds ----------------------------------------------------------------

  const unmoved = studio?.first.unmoved ?? null;
  const moving = unmoved !== null && !moveLater;
  const pinner = pinnerOf(scope);
  const refusedPins = pinner === null ? null : (studio?.pins_refused ?? null);
  const needing = (moving ? 1 : 0) + (refusedPins === null ? 0 : 1);
  const pages: ViewPage[] = [
    { id: "library", label: STUDIO_PAGE_NAMES.library },
    ...(needing > 0
      ? [
          {
            id: "needs",
            label: STUDIO_PAGE_NAMES.needs,
            count: needing,
            waiting: true,
            countSaid: `${needing} need you`,
          },
        ]
      : []),
    ...(drafts.length > 0
      ? [
          {
            id: "drafts",
            label: STUDIO_PAGE_NAMES.drafts,
            count: drafts.length,
            countSaid: counted(drafts.length, "draft", "drafts"),
          },
        ]
      : []),
    { id: "sources", label: STUDIO_PAGE_NAMES.sources },
    {
      id: "deleted",
      label: STUDIO_PAGE_NAMES.deleted,
      count: deleted.length,
      countSaid: counted(deleted.length, "deleted skill", "deleted skills"),
    },
    { id: "harness", label: STUDIO_PAGE_NAMES.harness, startsGroup: true },
    { id: "routing", label: STUDIO_PAGE_NAMES.routing },
  ];
  // A page that holds nothing now (Needs you, Drafts) is not in the navigation: Library instead.
  const page: StudioPage = pages.some((one) => one.id === at.page) ? at.page : "library";
  const skills = studio?.skills ?? [];
  const chosen =
    page === "library" && at.skill !== undefined
      ? (skills.find((skill) => skillRowId(skill) === at.skill) ??
        // A skill renamed, shared or moved has a new row: the one of its name is it.
        skills.find((skill) => skill.name === skillOfRowId(at.skill ?? "")))
      : undefined;
  // The project's, each workspace's and the persona's this studio shows (SK-32).
  const levels = studio === undefined ? [] : levelsFor(studio.levels, scope);
  const start = levelFrom(scope, levels);

  const picker = (
    <Select
      size="sm"
      aria-label="Whose chats"
      value={key}
      options={[
        AT_THE_ROOT,
        ...(studio?.workspaces ?? []).map(inWorkspace),
        // The scope shown is always among them, while its skills are still being read too.
        ...(scope.at === "persona" ||
        (scope.at === "workspace" && !(studio?.workspaces ?? []).includes(scope.name))
          ? [scope]
          : []),
      ].map((one) => ({ value: scopeKey(one), label: scopeChat(one) }))}
      onValueChange={(to) => {
        const next = scopeOfKey(to);
        if (!sameScope(next, scope)) go({ page, scope: next });
      }}
    />
  );
  const controls = (
    <>
      {picker}
      <Button
        size="sm"
        icon={<Plus aria-hidden="true" />}
        isDisabled={studio === undefined || doing?.what === "new"}
        onPress={() => {
          if (page !== "library" || at.skill !== undefined) history.go({ page: "library", scope });
          setDoing({ what: "new" });
        }}
      >
        {`${NEW_SKILL}…`}
      </Button>
    </>
  );
  /** A chat's draft, asked for where drafts are listed and where skills are: the Library's tools
   *  and the Drafts page, not the bar, which keeps to the prototype's and fits a narrow tab. */
  const draftAsk = (
    <Button
      size="sm"
      icon={<MessageSquarePlus aria-hidden="true" />}
      isDisabled={studio === undefined}
      onPress={() => studio !== undefined && askDraft(plane, scope, studio.levels)}
    >
      {DRAFT_WITH_A_CHAT}
    </Button>
  );
  const path: Crumb[] = [
    { id: page, label: STUDIO_PAGE_NAMES[page] },
    ...(chosen === undefined ? [] : [{ id: "skill", label: chosen.name }]),
    ...(doing === undefined ? [] : [{ id: "doing", label: DOING_NAMES[doing.what] }]),
  ];
  const restoring = said?.restore;
  const notice = said !== undefined && (
    <Notice
      cause="skill-change"
      tone={said.tone}
      label={said.label}
      fixes={
        restoring === undefined
          ? undefined
          : [{ label: "Restore", onPress: () => restore(restoring) }]
      }
      onDismiss={() => setSaid(undefined)}
    >
      {said.text}
    </Notice>
  );
  const bar = (
    <>
      <ViewBar
        history={history}
        path={path}
        onGo={(crumb, step) => {
          if (step === 0) go({ page, scope });
          else if (crumb.id === "skill") setDoing(undefined);
        }}
      >
        {controls}
      </ViewBar>
      {notice}
    </>
  );

  /** The list pane of a page that is not Library: its heading, its lead line, what it holds. */
  const pageList = (lead: string, body: ReactNode, tools?: ReactNode) => (
    <>
      <ViewListHead title={STUDIO_PAGE_NAMES[page]} lead={lead}>
        {tools !== undefined && <div className="skill-library-tools">{tools}</div>}
      </ViewListHead>
      <div className="skill-studio-page">{body}</div>
    </>
  );
  const reading = studio === undefined && refused === undefined;
  const notRead = refused !== undefined && (
    <EmptyState
      headline="purlis could not read this project's skills"
      body={refused}
      testid="skill-studio-refused"
      action={
        <Button icon={<RotateCw aria-hidden="true" />} onPress={() => setAsked((n) => n + 1)}>
          Try again
        </Button>
      }
    />
  );
  const held = (body: () => ReactNode) => notRead || (reading ? <Placeholders /> : body());

  let list: ReactNode;
  let item: ReactNode;
  if (page === "library") {
    const shown = skills.filter((skill) => chipsKeep(skill, chips) && searchKeeps(skill, query));
    const starts =
      studio?.first.starts === true && doing === undefined ? (
        <SkillStarts
          busy={busy}
          onStart={(how: SkillStart) => {
            if (how === "draft") {
              if (studio !== undefined) askDraft(plane, scope, studio.levels);
            } else if (how === "sources") go({ page: "sources", scope });
            else setDoing({ what: "new" });
          }}
        />
      ) : null;
    // The first row of a name is the copy the chat gets, or its only copy; each row after it is
    // a copy that one hides.
    const seen = new Map<string, number>();
    const testid = (name: string) => {
      const before = seen.get(name) ?? 0;
      seen.set(name, before + 1);
      return before === 0 ? `skill-${name}` : `skill-${name}-hidden-${before}`;
    };
    const filtering = query.trim() !== "" || chips.size > 0;
    list = (
      <>
        <ViewListHead
          title={STUDIO_PAGE_NAMES.library}
          lead={`Every skill ${lowerFirst(scopeChat(scope))} can get, and why any other is left out.${persona === null ? "" : " A persona brings its skills into every workspace."}`}
        >
          <div className="skill-library-tools">
            <TextField
              type="search"
              size="sm"
              icon={<Search aria-hidden="true" />}
              aria-label="Search skills"
              placeholder="Search skills"
              keepsFocus="skill-library-search"
              value={query}
              onChange={setQuery}
            />
            <Choice
              ids={{ id: `skill-library-chips-${tab}`, label: "Show only" }}
              kind="chips"
              options={CHIPS}
              checked={chips}
              onCheckedChange={(value, ticked) =>
                setChips((was) => {
                  const next = new Set(was);
                  if (ticked) next.add(value as Chip);
                  else next.delete(value as Chip);
                  return next;
                })
              }
            />
            {draftAsk}
            {persona !== null && studio !== undefined && (
              /* The persona's own skills list (#2768): every skill choice is the studio's. A name
                 the studio lists has Add on its page; this takes one it does not. */
              <PersonaSkillAdd
                plane={plane}
                persona={persona}
                known={studio.skills.filter((skill) => !skill.named).map((skill) => skill.written)}
                onChanged={() => setDone((n) => n + 1)}
              />
            )}
          </div>
        </ViewListHead>
        {held(() => (
          <>
            {starts}
            {shown.length === 0 && skills.length > 0 ? (
              <EmptyState
                headline={
                  query.trim() === ""
                    ? "No skill is left by these filters"
                    : `No skill matches “${query.trim()}”`
                }
                body="Skills that match what is typed and the chips ticked are listed here."
                testid="skill-library-none"
                action={
                  filtering && (
                    <Button
                      onPress={() => {
                        setQuery("");
                        setChips(new Set());
                      }}
                    >
                      Show every skill
                    </Button>
                  )
                }
              />
            ) : skills.length === 0 && starts !== null ? null : skills.length === 0 ? (
              <EmptyState
                headline="No skill reaches these chats yet"
                body="Every skill these chats can get is listed here, with why any other is left out."
                testid="skill-library-empty"
                action={
                  <Button onPress={() => setDoing({ what: "new" })}>{`${NEW_SKILL}…`}</Button>
                }
              />
            ) : (
              <List
                aria-label="Skills"
                className="skill-library"
                selectionMode="single"
                disallowEmptySelection
                selectedKeys={chosen === undefined ? new Set() : new Set([skillRowId(chosen)])}
                onSelectionChange={(keys: Selection) => {
                  const [first] = keys === "all" ? [] : [...keys];
                  if (first === undefined) return;
                  go({ page: "library", scope, skill: String(first) });
                }}
              >
                {shown.map((skill) => {
                  const mark = markOf(skill);
                  return (
                    <ListRow
                      key={skillRowId(skill)}
                      id={skillRowId(skill)}
                      textValue={skill.name}
                      data-testid={testid(skill.name)}
                      data-row={skillRowId(skill)}
                    >
                      <RowLabel>{skill.name}</RowLabel>
                      <RowTrail>
                        <Badge>{skill.label}</Badge>
                        {mark !== undefined && <Badge variant={mark.variant}>{mark.said}</Badge>}
                      </RowTrail>
                      <RowBelow clamp={1}>{lineOf(skill)}</RowBelow>
                    </ListRow>
                  );
                })}
              </List>
            )}
          </>
        ))}
      </>
    );
    item =
      doing !== undefined && studio !== undefined ? (
        <div className="skill-studio-item" data-testid="skill-studio-doing">
          {chosen !== undefined && doing.what !== "new" && (
            <ViewPageHeader
              name={chosen.name}
              tag={<Badge>{chosen.label}</Badge>}
              description={chosen.description}
            />
          )}
          <ChangePanel
            plane={plane}
            scope={scope}
            doing={doing}
            busy={busy}
            workspaces={studio.workspaces}
            levels={levels}
            onCancel={() => setDoing(undefined)}
            change={change}
            restoreOffered={restoreOffered}
            onRenamed={(to) => history.replace({ ...at, skill: skillRowNamed(to) })}
          />
        </div>
      ) : chosen !== undefined && studio !== undefined ? (
        <SkillItem
          plane={plane}
          studio={studio}
          skill={chosen}
          scope={scope}
          levels={levels}
          busy={busy}
          change={change}
          onShare={share}
          onEdit={open}
          onRename={(skill, level) => setDoing({ what: "rename", skill, level })}
          onDelete={askDelete}
          onMove={moveTo}
          onPersona={personaSkill}
          onOpenTab={
            onOpenView === undefined
              ? undefined
              : (skill) => onOpenView(skillView(skill.name), skill.name)
          }
        />
      ) : (
        <EmptyState
          headline="Pick a skill"
          body="What it does, who gets it and what can be changed open here."
          testid="skill-studio-unpicked"
        />
      );
  } else if (page === "needs") {
    list = pageList(
      "What needs a decision of yours before chats get what they should.",
      held(() => (
        <>
          {moving && unmoved !== null && (
            <Notice
              cause="skill-move"
              tone="news"
              label="Skills to move"
              fixes={[{ label: "Move them", busy, onPress: () => moveSkills(unmoved.fix) }]}
              onDismiss={() => setMoveLater(true)}
            >
              <span data-testid="skill-studio-unmoved">
                {`This project keeps ${counted(unmoved.skills, "skill", "skills")} where only some chat programs read them. Moving them gives every chat each one, whichever program it runs on, in one commit that can be undone. The commit reaches everyone who works in the project, and a teammate on an older purlis opens the project read-only until they update.`}
              </span>
            </Notice>
          )}
          {refusedPins !== null && (
            <p>
              <SkillPinsRefusedLine
                refused={refusedPins}
                busy={busy}
                onRemove={(bad) =>
                  change("The pin was not removed", () =>
                    commands.skillPin(plane, refusedPins.by, bad, false),
                  )
                }
              />
            </p>
          )}
        </>
      )),
    );
  } else if (page === "drafts") {
    list = pageList(
      "Skills a chat drafted, waiting for you to try, accept or reject.",
      held(() => (
        /* A chat's drafts (SK-9, #2772): Accept is one commit, Reject deletes the draft. */
        <SkillDrafts
          plane={plane}
          drafts={drafts}
          levels={studio?.levels ?? []}
          busy={busy}
          onAccept={(draft) =>
            change("The draft was not accepted", () => commands.skillDraftAccept(plane, draft.id))
          }
          onReject={(draft) =>
            change("The draft was not rejected", () =>
              commands
                .skillDraftReject(plane, draft.id)
                .then((answer): Answer =>
                  answer.status === "ok" ? { status: "ok", data: { notes: [] } } : answer,
                ),
            )
          }
        />
      )),
      draftAsk,
    );
  } else if (page === "sources") {
    list = pageList(
      "Where skills are installed from: a git repository, reviewed before anything lands.",
      held(() => (
        /* Skill sources (SK-10): install from a git repository, and each newer commit's review. */
        <SkillSources
          plane={plane}
          levels={levels}
          start={start}
          busy={busy}
          change={change}
          changed={changed + done}
        />
      )),
    );
  } else if (page === "deleted") {
    list = pageList(
      "Skills deleted from this project, each of which can be restored.",
      held(() =>
        deleted.length === 0 ? (
          <EmptyState
            headline="No skill is deleted"
            body="A skill you delete waits here, and can be restored from here."
            testid="skill-deleted-none"
            action={<Button onPress={() => go({ page: "library", scope })}>Go to Library</Button>}
          />
        ) : (
          <ul className="skill-deleted" aria-label="Deleted skills">
            {deleted.map((one) => (
              <li key={one.id} data-testid={`deleted-${one.name}`}>
                <code>{one.name}</code> · {one.label} · {one.stays}{" "}
                <Button
                  size="sm"
                  aria-label={`Restore ${one.name}`}
                  isDisabled={busy}
                  onPress={() => restore(one.id)}
                >
                  Restore
                </Button>
              </li>
            ))}
          </ul>
        ),
      ),
    );
  } else if (page === "harness") {
    list = pageList(
      "The skills each chat program brings on its own, which purlis lists and never edits.",
      held(() =>
        studio === undefined ? null : (
          <SkillStudioHarnesses
            plane={plane}
            workspace={workspace}
            harnesses={studio.harnesses}
            notSupported={studio.not_supported}
            onSwitched={() => setSwitched((n) => n + 1)}
          />
        ),
      ),
    );
  } else {
    const router = studio?.router ?? null;
    list = pageList(
      "Which router picks the skills a chat is shown, and how it did at each chat's start.",
      held(() =>
        router === null || (router.extension === null && !router.problem) ? (
          <EmptyState
            headline="These chats use purlis's own router"
            body="An extension's router chosen for a workspace or a persona is shown here, with how it did at each chat's start."
            testid="skill-routing-built-in"
          />
        ) : (
          <StudioRouterLine
            router={router}
            grading={{ plane, workspace, persona, onGraded: () => setDone((was) => was + 1) }}
          />
        ),
      ),
    );
  }

  return (
    <div className="skill-studio" data-testid="skill-studio">
      <ViewFrame
        label="Skill studio pages"
        pages={pages}
        page={page}
        onPage={(id) => go({ page: id as StudioPage, scope })}
        list={list}
        item={item}
        opened={chosen !== undefined || doing !== undefined}
        onBack={() => go({ page, scope })}
        narrow={narrow}
        bar={bar}
        history={history}
      />
    </div>
  );
}

/**
 * **The skill chosen in the Library**, beside the list: its name, its skill level and its mark,
 * then the controls its row of the old table had, as facts, until its page (#3682) takes them.
 */
function SkillItem({
  plane,
  studio,
  skill,
  scope,
  levels,
  busy,
  change,
  onShare,
  onEdit,
  onRename,
  onDelete,
  onMove,
  onPersona,
  onOpenTab,
}: {
  plane: string;
  studio: Studio;
  skill: StudioSkill;
  scope: StudioScope;
  levels: readonly StudioLevel[];
  busy: boolean;
  change: Change;
  onShare: (skill: StudioSkill, level: SkillLevel) => void;
  onEdit: (skill: StudioSkill, level: SkillLevel) => void;
  onRename: (skill: StudioSkill, level: SkillLevel) => void;
  onDelete: (skill: StudioSkill, level: SkillLevel) => void;
  onMove: (skill: StudioSkill, from: SkillLevel, to: SkillLevel) => void;
  onPersona: (who: string, written: string, how: "add" | "remove") => void;
  /** Its own tab, with its history and runs. */
  onOpenTab?: (skill: StudioSkill) => void;
}) {
  const level = levelOf(skill, scope);
  const { workspace, persona } = whoseChats(scope);
  const mark = markOf(skill);
  // The Pin switch (SK-15d): for the workspace's or the persona's chats the studio shows, on the
  // copy of a name a chat could have pinned (one of the project's levels), where the chat gets it
  // or a pin of it is still written, so a pin can always be undone.
  const pinner = pinnerOf(scope);
  const refusedPins = pinner === null ? null : (studio.pins_refused ?? null);
  const pinned =
    pinner === null || pinRowsOf(studio.skills).get(skill.name) !== skill
      ? null
      : skill.pinned_by.some((by) => by.at === pinner.at && by.name === pinner.name);
  const pinShown = pinner !== null && pinned !== null && (skill.on || pinned);
  const clashes = studio.clashes.filter(
    (clash) =>
      skill.on &&
      clash.name === skill.name &&
      clash.level === skill.level &&
      clash.local === skill.local,
  );
  const personaAct =
    persona === null || skill.level === "persona"
      ? null
      : skill.named
        ? {
            label: "Remove",
            name: `Remove ${skill.written} from ${persona}`,
            how: "remove" as const,
          }
        : {
            label: `Add to ${persona}`,
            name: `Add ${skill.written} to ${persona}`,
            how: "add" as const,
          };
  return (
    <div
      className="skill-studio-item"
      data-testid="skill-studio-skill"
      data-row={skillRowId(skill)}
    >
      <ViewPageHeader
        name={skill.name}
        tag={<Badge>{skill.label}</Badge>}
        mark={mark !== undefined && <Badge variant={mark.variant}>{mark.said}</Badge>}
        description={skill.description}
        actions={
          level === null && personaAct === null && onOpenTab === undefined ? undefined : (
            <>
              {level !== null && (
                <>
                  <Button
                    aria-label={`Edit ${skill.name}`}
                    isDisabled={busy}
                    onPress={() => onEdit(skill, level)}
                  >
                    Edit
                  </Button>
                  <Button
                    aria-label={`Rename ${skill.name}`}
                    isDisabled={busy}
                    onPress={() => onRename(skill, level)}
                  >
                    Rename
                  </Button>
                </>
              )}
              {/* A persona's own list (SK-5, #2768). A persona's own skill is in its skills
                  already, and needs no name (SK-32). */}
              {personaAct !== null && persona !== null && (
                <Button
                  aria-label={personaAct.name}
                  isDisabled={busy}
                  onPress={() => onPersona(persona, skill.written, personaAct.how)}
                >
                  {personaAct.label}
                </Button>
              )}
              {onOpenTab !== undefined && (
                <Button isDisabled={busy} onPress={() => onOpenTab(skill)}>
                  History and runs
                </Button>
              )}
              {level !== null && (
                <Button
                  variant="danger"
                  aria-label={`Delete ${skill.name}`}
                  isDisabled={busy}
                  onPress={() => onDelete(skill, level)}
                >
                  Delete
                </Button>
              )}
            </>
          )
        }
      />
      <Facts aria-label={`About ${skill.name}`} className="skill-studio-facts">
        <Fact label="In a chat">
          <span data-testid="skill-in-a-chat">
            {skill.on
              ? (skill.unchecked ??
                (skill.hidden.length > 0
                  ? `On, ${skill.hidden[0].hidden.said} (on ${skill.hidden.map((one) => one.title).join(", ")})`
                  : skill.pinned
                    ? "On, pinned"
                    : "On"))
              : `Left out: ${skill.left_out ?? ""}`}
          </span>
          {pinShown && pinner !== null && (
            <>
              {" "}
              <SkillPinSwitch
                name={skill.name}
                by={pinner}
                pinned={pinned === true}
                refused={refusedPins}
                busy={busy}
                onPin={(to) =>
                  change(to ? "The skill was not pinned" : "The skill was not unpinned", () =>
                    commands.skillPin(plane, pinner, skill.name, to),
                  )
                }
              />
            </>
          )}
          {skill.on && skill.level === "built-in" && (
            <span className="honest"> {BUILT_IN_STAYS}</span>
          )}
          {clashes.map((clash) => (
            <span
              key={`${clash.harness}\u0000${clash.outcome}`}
              className="honest"
              data-testid={`clash-${skill.name}`}
            >
              {` Also: ${clash.said}. `}
              {level !== null && (
                <Button size="sm" isDisabled={busy} onPress={() => onRename(skill, level)}>
                  Rename the project's copy
                </Button>
              )}
            </span>
          ))}
          {typeof skill.accept === "string" && (
            <>
              {" "}
              <Button
                size="sm"
                isDisabled={busy}
                onPress={() => {
                  const folder = skill.accept;
                  if (typeof folder === "string")
                    change("The folder was not accepted", () =>
                      commands.skillLevelAccept(plane, folder),
                    );
                }}
              >
                Accept its folder
              </Button>
            </>
          )}
          {skill.not_granted !== null && (
            <span className="honest" data-testid={`not-granted-${skill.name}`}>
              {" "}
              {skill.not_granted}
            </span>
          )}
          {workspace !== null && (
            <TurnSwitch
              skill={skill}
              workspace={workspace}
              busy={busy}
              onTurn={(turn) =>
                change(turn.refused, () =>
                  commands.skillTurn(plane, workspace, skill.name, turn.on),
                )
              }
            />
          )}
        </Fact>
        <Fact label="Skill level">
          {skill.label}
          {skill.local && level !== null && (
            <>
              {" "}
              <Button
                size="sm"
                onPress={() => onShare(skill, level)}
                tooltip="Give it to the team: it is committed with the project"
              >
                Share
              </Button>
            </>
          )}
          {level !== null && (
            <>
              {" "}
              <LevelPicker
                label={`Move ${skill.name} to`}
                levels={levels.filter((to) => !sameLevel(to.level, level))}
                disabled={busy}
                onPick={(to) => onMove(skill, level, to)}
              />
            </>
          )}
        </Fact>
        {skill.granted && (
          <Fact label="Granted to">
            <GrantPicker
              label={`Where ${skill.name} reaches`}
              workspaces={studio.workspaces}
              now={skill.granted}
              disabled={busy}
              onPick={(grant) =>
                change("The grant was not changed", () =>
                  commands.skillGrant(plane, skill.name, grant),
                )
              }
            />
          </Fact>
        )}
        <Fact label="Works on">
          <SkillCompatBadges
            name={skill.name}
            compat={skill.compat}
            lint={skill.lint}
            notSupported={studio.not_supported}
          />
        </Fact>
      </Facts>
    </div>
  );
}

/** The panel under the toolbar for the change in progress: New skill, the editor, Rename, the
 *  grant a move into the project asks for, and Delete's who-uses-it. */
function ChangePanel({
  plane,
  scope,
  doing,
  busy,
  workspaces,
  levels,
  onCancel,
  change,
  restoreOffered,
  onRenamed,
}: {
  plane: string;
  scope: StudioScope;
  doing: Doing;
  busy: boolean;
  workspaces: readonly string[];
  levels: readonly StudioLevel[];
  onCancel: () => void;
  change: Change;
  restoreOffered: (name: string, level: SkillLevel) => Promise<Said | undefined>;
  /** The skill was renamed `to`: the place follows it. */
  onRenamed?: (to: string) => void;
}) {
  switch (doing.what) {
    case "new":
      return (
        <NewSkill
          plane={plane}
          scope={scope}
          levels={levels}
          busy={busy}
          onCancel={onCancel}
          onMake={(level, name, description, template) =>
            change("The skill was not made", () =>
              commands.skillNew(plane, level, name, description, template),
            )
          }
        />
      );
    case "edit":
      return (
        <Suspense
          fallback={
            <p className="pending" aria-busy="true">
              Opening the editor…
            </p>
          }
        >
          <SkillEditor
            plane={plane}
            level={doing.level}
            name={doing.skill.name}
            was={doing.was}
            saving={busy}
            onClose={onCancel}
            onSave={(now) =>
              change("The skill was not saved", () =>
                commands.skillSave(plane, doing.level, doing.skill.name, doing.was, now),
              )
            }
          />
        </Suspense>
      );
    case "rename":
      return (
        <Rename
          name={doing.skill.name}
          busy={busy}
          onCancel={onCancel}
          onRename={(to) =>
            change(
              "The skill was not renamed",
              () => commands.skillRename(plane, doing.level, doing.skill.name, to),
              () => {
                onRenamed?.(to);
                return Promise.resolve(undefined);
              },
            )
          }
        />
      );
    case "move":
      return (
        <GrantPrompt
          name={doing.skill.name}
          from={doing.from}
          to={doing.to}
          workspaces={workspaces}
          levels={levels}
          busy={busy}
          onCancel={onCancel}
          onMove={(grant) =>
            change("The skill was not moved", () =>
              commands.skillMove(plane, doing.from, doing.skill.name, doing.to, grant),
            )
          }
        />
      );
    case "delete":
      return (
        <section className="skill-change" aria-label={`Delete ${doing.skill.name}`}>
          <p>
            <strong>Delete {doing.skill.name}?</strong> It can be restored afterwards.
          </p>
          <UsedBy users={doing.users} />
          <ButtonRow>
            <Button
              variant="danger"
              isDisabled={busy}
              onPress={() =>
                change(
                  "The skill was not deleted",
                  () => commands.skillDelete(plane, doing.level, doing.skill.name),
                  () => restoreOffered(doing.skill.name, doing.level),
                )
              }
            >
              Delete
            </Button>
            <Button onPress={onCancel}>Cancel</Button>
          </ButtonRow>
        </section>
      );
  }
}

/** Who uses a skill, as Delete asks with it. */
function UsedBy({ users }: { users: SkillUsers }) {
  const chats = [
    ...(users.root ? ["chats at the project's root"] : []),
    ...users.workspaces.map((ws) => `chats in ${ws}`),
    ...(users.persona === null ? [] : [`chats as ${users.persona}, in every workspace`]),
  ];
  return (
    <ul className="skill-users" data-testid="skill-users">
      <li>{chats.length === 0 ? "No chat gets it now." : `Used by ${chats.join(", ")}.`}</li>
      {users.personas.length > 0 && (
        <li>
          Named by {counted(users.personas.length, "persona", "personas")}:{" "}
          {users.personas.join(", ")}.
        </li>
      )}
      {users.skills.length > 0 && (
        <li>
          Named by {counted(users.skills.length, "skill", "skills")}: {users.skills.join(", ")}.
        </li>
      )}
    </ul>
  );
}

/** **New skill** (decisions 13 and 37): a name, a description and a template, at the level the
 *  person came from, said in one line they can change. */
/** What the template picker holds a template by: its kind, or `<extension>/<template>`. */
function templateKey(id: SkillTemplateId): string {
  return id.kind === "extension" ? `${id.extension}/${id.template}` : id.kind;
}

function NewSkill({
  plane,
  scope,
  levels,
  busy,
  onCancel,
  onMake,
}: {
  plane: string;
  scope: StudioScope;
  levels: readonly StudioLevel[];
  busy: boolean;
  onCancel: () => void;
  /** `template` is purlis's own, or one an extension offers (ADR 0101). */
  onMake: (level: SkillLevel, name: string, description: string, template: SkillTemplateId) => void;
}) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [templates, setTemplates] = useState<readonly SkillTemplate[]>([]);
  const [template, setTemplate] = useState<SkillTemplateId>({ kind: "blank" });
  const [level, setLevel] = useState<SkillLevel>(() => levelFrom(scope, levels));
  useEffect(() => {
    let live = true;
    void commands
      .skillTemplates(plane)
      .then((all) => {
        if (live && all.status === "ok") setTemplates(all.data);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [plane]);
  return (
    <form
      className="skill-change"
      aria-label="New skill"
      onSubmit={(event) => {
        event.preventDefault();
        onMake(level, name.trim(), description.trim(), template);
      }}
    >
      <SettingRow
        label="Name"
        help="Lowercase letters, digits and hyphens."
        control={(ids) => (
          <Field kind="text" ids={ids} value={name} placeholder="pr-review" onChange={setName} />
        )}
      />
      <SettingRow
        label="What it does and when to use it"
        help="One line: a chat reads it to know when the skill applies."
        control={(ids) => (
          <Field kind="text" ids={ids} value={description} onChange={setDescription} />
        )}
      />
      <Select
        aria-label="Starts from"
        value={templateKey(template)}
        options={templates.map((one) => ({
          value: templateKey(one.id),
          label: one.label,
          tooltip: one.about,
        }))}
        // One of the offered ids, or the blank template where the list does not name it.
        onValueChange={(to) =>
          setTemplate(templates.find((one) => templateKey(one.id) === to)?.id ?? { kind: "blank" })
        }
      />
      <p className="honest" data-testid="skill-new-level">
        It lands in the {labelOf(levels, level)} skills.{" "}
        <LevelPicker label="Lands in" levels={levels} now={level} onPick={setLevel} />
      </p>
      <ButtonRow>
        <Button
          variant="primary"
          type="submit"
          isDisabled={busy || name.trim() === "" || description.trim() === ""}
        >
          Make it
        </Button>
        <Button onPress={onCancel}>Cancel</Button>
      </ButtonRow>
    </form>
  );
}

/** Rename: the new name, and that every persona and skill naming it follows. */
function Rename({
  name,
  busy,
  onCancel,
  onRename,
}: {
  name: string;
  busy: boolean;
  onCancel: () => void;
  onRename: (to: string) => void;
}) {
  const [to, setTo] = useState(name);
  return (
    <form
      className="skill-change"
      aria-label={`Rename ${name}`}
      onSubmit={(event) => {
        event.preventDefault();
        onRename(to.trim());
      }}
    >
      <SettingRow
        label="New name"
        control={(ids) => <Field kind="text" ids={ids} value={to} onChange={setTo} />}
      />
      <p className="honest">Every persona and skill that names it is changed with it.</p>
      <ButtonRow>
        <Button
          variant="primary"
          type="submit"
          isDisabled={busy || to.trim() === "" || to.trim() === name}
        >
          Rename
        </Button>
        <Button onPress={onCancel}>Cancel</Button>
      </ButtonRow>
    </form>
  );
}

/** **The grant a move into the project's skills asks for** (decision 14): a project skill
 *  reaches only the chats its grant names. It starts at the workspace the skill came from. */
function GrantPrompt({
  name,
  from,
  to,
  workspaces,
  levels,
  busy,
  onCancel,
  onMove,
}: {
  name: string;
  from: SkillLevel;
  to: SkillLevel;
  workspaces: readonly string[];
  levels: readonly StudioLevel[];
  busy: boolean;
  onCancel: () => void;
  onMove: (grant: SkillGrant) => void;
}) {
  const [grant, setGrant] = useState<SkillGrant>(
    from.at === "workspace" ? { to: "only", workspaces: [from.name] } : { to: "root" },
  );
  return (
    <section
      className="skill-change"
      aria-label={`Move ${name} to the ${labelOf(levels, to)} skills`}
    >
      <p>
        <strong>
          Move {name} to the {labelOf(levels, to)} skills.
        </strong>{" "}
        A project skill reaches only the chats its grant names. Where does it reach?
      </p>
      {from.at === "persona" && (
        <p className="honest" data-testid="persona-keeps">
          {from.name} keeps it in every workspace: it is added to {from.name}'s skills.
        </p>
      )}
      <GrantPicker
        label={`Where ${name} will reach`}
        workspaces={workspaces}
        now={grant}
        onPick={setGrant}
      />
      <ButtonRow>
        <Button variant="primary" isDisabled={busy} onPress={() => onMove(grant)}>
          Move
        </Button>
        <Button onPress={onCancel}>Cancel</Button>
      </ButtonRow>
    </section>
  );
}

/** A picker over the project's levels, by the core's words. */
function LevelPicker({
  label,
  levels,
  now,
  disabled,
  onPick,
}: {
  label: string;
  levels: readonly StudioLevel[];
  /** The level it holds, or none for a picker that only says what to do (Move to…). */
  now?: SkillLevel;
  disabled?: boolean;
  onPick: (level: SkillLevel) => void;
}) {
  const picker = picking(
    levels.map((one) => one.level),
    (level) => labelOf(levels, level),
    sameLevel,
    now,
  );
  return (
    <Select
      aria-label={label}
      value={picker.value}
      placeholder={now === undefined ? "Move to…" : undefined}
      isDisabled={disabled}
      options={picker.options}
      onValueChange={(to) => {
        const level = picker.chosen(to);
        if (level !== undefined) onPick(level);
      }}
    />
  );
}

/** A picker over the grants a project skill can have. */
function GrantPicker({
  label,
  workspaces,
  now,
  disabled,
  onPick,
}: {
  label: string;
  workspaces: readonly string[];
  now: SkillGrant;
  disabled?: boolean;
  onPick: (grant: SkillGrant) => void;
}) {
  const picker = picking(grantChoices(workspaces, now), grantLabel, sameGrant, now);
  return (
    <Select
      aria-label={label}
      value={picker.value}
      isDisabled={disabled}
      options={picker.options}
      onValueChange={(to) => {
        const grant = picker.chosen(to);
        if (grant !== undefined) onPick(grant);
      }}
    />
  );
}

/** **A project skill's switch in one workspace** (#2770), where it has one (`turnOf`): the
 *  Skills view's menu offers the same. */
function TurnSwitch({
  skill,
  workspace,
  busy,
  onTurn,
}: {
  skill: StudioSkill;
  workspace: string;
  busy: boolean;
  onTurn: (turn: { on: boolean; refused: string }) => void;
}) {
  const turn = turnOf(skill, workspace);
  if (!("label" in turn)) return null;
  return (
    <>
      {" "}
      <Button size="sm" isDisabled={busy} onPress={() => onTurn(turn)}>
        {turn.label}
      </Button>
    </>
  );
}

/** `text` with its first letter lower-cased, to go inside a sentence. */
function lowerFirst(text: string): string {
  return text.charAt(0).toLowerCase() + text.slice(1);
}

/**
 * **Which router this studio's chats are routed by** (SK-15b, ADR 0102 §5, §6), where it is an
 * extension's or the choice names none, and how the router did at the start of each such chat
 * open now: its timing, or why it fell back on the built-in router. Nothing for the built-in
 * router, which every chat has unless a workspace or persona chooses another.
 */
export function StudioRouterLine({
  router,
  grading,
}: {
  router: StudioRouter;
  /** Where the studio is, to grade the router chosen there (SK-15e): none offers no Grade. */
  grading?: {
    plane: string;
    workspace: string | null;
    persona: string | null;
    onGraded: () => void;
  };
}) {
  if (router.extension === null && !router.problem) return null;
  return (
    <div
      className={router.problem ? "honest use-skill-over" : "honest"}
      data-testid="skill-studio-router"
    >
      <p>{router.said}</p>
      {router.extension !== null && (
        <p data-testid="skill-studio-router-grade">
          {router.grade ??
            "This router has not been graded on this library yet: a grade runs the library's trigger tests with it and with the built-in router."}
        </p>
      )}
      {router.extension !== null && grading !== undefined && (
        <RouterGrade
          plane={grading.plane}
          router={router.extension}
          workspace={grading.workspace}
          persona={grading.persona}
          onGraded={grading.onGraded}
        />
      )}
      {router.chats.length > 0 && (
        <ul className="skill-studio-routed">
          {router.chats.map((one) => (
            <li key={one.session} data-fell-back={one.routing.fell_back}>
              {`Chat ${one.session}: ${one.routing.said}`}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
