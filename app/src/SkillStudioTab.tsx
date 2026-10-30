// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { lazy, Suspense, useEffect, useRef, useState, type ReactNode } from "react";
import { MessageSquarePlus, Plus, RotateCw, Search, Sparkle, Sparkles } from "lucide-react";
import {
  commands,
  type DeletedSkill,
  type NeedsYou,
  type StudioLevel,
  type SkillGrant,
  type SkillLevel,
  type SkillTemplate,
  type SkillTemplateId,
  type SkillDraft,
  type SkillPinner,
  type Studio,
  type StudioSkill,
} from "./bindings";
import { BUILT_IN_FIXED, MAKE_MY_OWN_COPY, SkillCopy } from "./SkillCopy";
import { counted } from "./dispatches";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { HarnessItem, HarnessList, harnessGroups, harnessRowAt } from "./SkillStudioHarnesses";
import { RoutedChatItem, RoutedChats, RoutingFacts } from "./SkillStudioRouting";
import { Button, ButtonRow } from "./ui/button/Button";
import { Badge } from "./ui/badge/Badge";
import { TextField } from "./ui/field/Field";
import { List, ListRow, type Selection } from "./ui/list/List";
import { ListRun } from "./ui/list/Fold";
import { RowBelow, RowLabel, RowTrail } from "./ui/list/Row";
import { ViewFrame, ViewListHead, type ViewPage } from "./ui/panel/ViewFrame";
import { ViewBar, type Crumb } from "./ui/panel/ViewBar";
import { useViewHistory } from "./ui/panel/viewHistory";
import { NEEDS_YOU_LEAD, NeedsYouItem, NeedsYouList, needShown } from "./SkillStudioNeedsYou";
import { proposeOldKeysRemoval } from "./oldHarnessPlugins";
import { Notice } from "./Notice";
import { DraftPage, DraftsList } from "./SkillStudioDrafts";
import { DeletedList, DeletedPage } from "./SkillStudioDeleted";
import { DRAFT_WITH_A_CHAT, NEW_SKILL, SkillStarts, type SkillStart } from "./SkillStarts";
import { askDraft, useSkillDraftsRead } from "./skillDraftAsks";
import { SkillSourcesPage } from "./SkillSources";
import { Select } from "./ui/select/Select";
import { Choice, Field, SettingRow } from "./settings/components";
import { labelOf, levelFrom, levelOf, levelsFor, sameLevel } from "./skillChanges";
import { LevelPicker } from "./SkillLevelPickers";
import { DeleteDialog, MoveDialog, RenameDialog } from "./SkillChangeDialogs";
import {
  SkillPage,
  changed as answered,
  lowerFirst,
  markOf,
  type Answer,
  type Change,
  type Said,
} from "./SkillPage";
import {
  AT_THE_ROOT,
  SKILL_PAGE_TAB_NAMES,
  SKILL_STUDIO_VIEW,
  asPersona,
  STUDIO_PAGE_NAMES,
  STUDIO_START,
  inWorkspace,
  harnessSkillNamed,
  pinnerOf,
  routedChatId,
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
  type SourcesAt,
  type StudioPage,
  type StudioPlace,
  type StudioScope,
} from "./skillStudio";
import { PINNABLE } from "./skillsListing";
import type { ViewRef } from "./tabs";
import type { Offer } from "./actions";
import { PersonaSkillAdd, changePersonaSkill } from "./personaSkills";

/** The light editor, its own chunk (`tools/main-chunk-has-no-codemirror.mjs`). */
const SkillEditor = lazy(() => import("./SkillEditor"));

/** A change in progress: New skill or the editor, drawn in the item pane; or Rename, Move or
 *  Delete, each a dialog (S4, #3685), with what the core refused the last time it was asked. */
type Doing =
  | { what: "new" }
  | { what: "edit"; skill: StudioSkill; level: SkillLevel; was: string }
  | { what: "rename"; skill: StudioSkill; level: SkillLevel; refused?: string }
  | { what: "move"; skill: StudioSkill; from: SkillLevel; refused?: string }
  | { what: "delete"; skill: StudioSkill; level: SkillLevel; refused?: string };

/** **The path inside Sources** (#3687): the source, then the review, as the prototype names
 *  them. Pressing the source goes to its page; the review is where the person is. */
function sourcesPath(at: SourcesAt): Crumb[] {
  const source = { id: "source", label: at.label };
  if (at.at === "source") return [source];
  return [
    source,
    { id: at.at, label: `${at.at === "install" ? "Install" : "Update"} ${at.skill}` },
  ];
}

/** What a change in progress drawn in a pane is called in the path: a dialog is no place. */
const DOING_NAMES: Partial<Record<Doing["what"], string>> = {
  new: NEW_SKILL,
  edit: "Edit",
};

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

/** The row's line under its name: what it does, or why it is left out where it says nothing. */
function lineOf(skill: StudioSkill): string {
  if (skill.description !== "") return skill.description;
  return skill.left_out === null
    ? "It says nothing of what it does."
    : `Left out: ${skill.left_out}`;
}

/** **The Library's built-in skills, folded** (S12): their run's key, whose quiet line is
 *  `built-in:fold` and closing line `built-in:end`, rows the list never chooses. */
const BUILT_IN_RUN = "built-in";
const BUILT_IN_RUN_ROWS = [`${BUILT_IN_RUN}:fold`, `${BUILT_IN_RUN}:end`];

/**
 * **A built-in skill drawn on its own, outside the fold** (S12): one that needs the person (a
 * lint, a folder to accept, or one no chat gets), one a skill of the project's clashes with (a
 * clash the core found, or a row of the same name at another level), one the search matched, or
 * the one chosen, so the row of the page beside the list is always in sight.
 */
function builtInStandsOut(
  skill: StudioSkill,
  studio: Studio | undefined,
  { query, chosen }: { query: string; chosen: string | undefined },
): boolean {
  if (query.trim() !== "" || skillRowId(skill) === chosen) return true;
  if (skill.lint.length > 0 || skill.accept !== null || !skill.on) return true;
  return (
    (studio?.clashes ?? []).some((clash) => clash.name === skill.name) ||
    (studio?.skills ?? []).some((one) => one.level !== "built-in" && one.name === skill.name)
  );
}

/** The scope whose Library lists a skill at `level`: a workspace's or a persona's own, else the
 *  scope shown. */
function scopeOfLevel(level: SkillLevel, shown: StudioScope): StudioScope {
  if (level.at === "workspace") return { at: "workspace", name: level.name };
  if (level.at === "persona") return asPersona(level.name);
  return shown;
}

/** Whether `skill`, a row of the studio showing `scope`, is at `level`. */
function atLevel(skill: StudioSkill, scope: StudioScope, level: SkillLevel): boolean {
  const its = levelOf(skill, scope);
  return its !== null && sameLevel(its, level);
}

/** A change's result notice (S4): what was done, then what the core noted on it. */
function resultOf(label: string, text: string, notes: readonly string[]): Said {
  return { tone: "news", label, text: [text, ...notes].join(" ") };
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
 *   skill chosen opens its page beside the list (`SkillPage.tsx`, #3682), or alone with a way
 *   back where the tab is narrow.
 * - **Needs you** and **Drafts** are in the navigation only while they hold something.
 * - **Sources** is the person's sources and the ones offered, each source a page of its own and
 *   each install or update a review page (#3687, `SkillSources.tsx`), each a step of the history.
 * - **Deleted**, **Harness skills** and **Routing** hold what the old page's sections held, each
 *   scrolling on its own, until their own tickets build them.
 *
 * Each change is one commit (the core's `skills::verbs`). **What it offers when it opens**
 * (SK-22, #2785; decision 42) is the core's (`skills::first_open`).
 */
export function SkillStudioTab({
  plane,
  tab = "",
  changed,
  narrow,
  offerFor,
  onPress,
  onOpenView,
}: {
  plane: string;
  /** The studio tab's own key: `""` for the project's studio, `#2` for one opened beside it. */
  tab?: string;
  changed: number;
  /** The catalogue, for the ways out a skill's Runs offer (a new chat where it has none). */
  offerFor?: (id: string) => Offer | undefined;
  onPress?: (offer: Offer) => void;
  /** Opens a tab from a skill's page: its file, a grade's scorecard. */
  onOpenView?: (view: ViewRef, title: string) => void;
  /** Narrow or wide whatever the width: for a story or a test. Measured when left out. */
  narrow?: boolean;
}) {
  const history = useViewHistory<StudioPlace>(
    studioHistoryOf(plane, { ...SKILL_STUDIO_VIEW, key: tab }),
    STUDIO_START,
  );
  const at = history.at;
  // Where the tab is now, for an answer that comes back after the person may have moved on.
  const placeNow = useRef(at);
  useEffect(() => {
    placeNow.current = at;
  });
  const scope = at.scope;
  const [read, setRead] = useState<{ key: string; studio?: Studio; refused?: string }>();
  const [asked, setAsked] = useState(0);
  const [said, setSaid] = useState<Said>();
  const [done, setDone] = useState(0);
  // **A change in progress belongs to the place it was started at**: back, forward or an entry
  // point leaves it behind, so it never stands under another skill's name. One started now binds
  // to the place the tab is drawn at next, which a step taken with it has already reached.
  const [inHand, setInHand] = useState<{ doing: Doing; at: StudioPlace | "next" }>();
  // The change in hand as last drawn, for an answer that comes back after it may have closed.
  const inHandNow = useRef(inHand);
  useEffect(() => {
    inHandNow.current = inHand;
  });
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
  // The item of Needs you chosen, by its id: on a wide tab the first is shown until one is.
  const [need, setNeed] = useState<string>();
  const [query, setQuery] = useState("");
  const [chips, setChips] = useState<ReadonlySet<Chip>>(new Set());
  // Make my own copy (#3686): of the built-in `from`, or of one the person picks; and why the
  // core did not make the last one asked for.
  const [copying, setCopying] = useState<{ from?: string; refused?: string }>();
  const { workspace, persona } = whoseChats(scope);
  const key = scopeKey(scope);
  // What was read for another scope is not this one's: placeholders until this one's is.
  const studio = read?.key === key ? read.studio : undefined;
  const refused = read?.key === key ? read.refused : undefined;
  const on = studio?.on === true;
  // Read once the studio has said it is on: a scope picked later reads its skills, not these.
  const [opened, setOpened] = useState(false);
  if (on && !opened) setOpened(true);
  const draftsRead = useSkillDraftsRead(plane, changed + done);
  const drafts = draftsRead ?? [];
  // A start the Skills view asked this studio to open on (#3266): the New skill form.
  // Make my own copy, asked from the Skills view's folded built-in skills (#3693).
  useStudioStart(tab, on, (start) => {
    if (start === "copy") setCopying({});
    else setDoing({ what: "new" });
  });
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

  /** Goes to `place`, a step of the tab's history; a change in progress is left, as is the item
   *  of Needs you chosen. */
  const go = (place: StudioPlace) => {
    setDoing(undefined);
    setNeed(undefined);
    history.go(place);
  };

  const change: Change = (label, run, then, refusedIn) => {
    setBusy(true);
    void run()
      .then(async (answer) => {
        if (answer.status === "error") {
          if (refusedIn !== undefined) refusedIn(answer.error);
          else setSaid({ tone: "trouble", label, text: answer.error });
          return;
        }
        setDoing(undefined);
        const notes = answer.data.notes;
        const next = then === undefined ? undefined : await then(notes);
        if (next !== undefined) setSaid(next);
        else if (notes.length > 0)
          setSaid({ tone: "news", label: "Skill changed", text: notes.join(" ") });
        else setSaid(undefined);
      })
      .catch((err: unknown) =>
        refusedIn !== undefined
          ? refusedIn(String(err))
          : setSaid({ tone: "trouble", label, text: String(err) }),
      )
      .finally(() => {
        setBusy(false);
        setDone((n) => n + 1);
      });
  };
  /** Why the core refused the dialog `asked`'s change: said in that dialog, which stays open;
   *  or in the notice where the person closed it meanwhile, or another is open now. */
  const refusedInDialog =
    (asked: Doing, label: string) =>
    (why: string): void => {
      if (inHandNow.current?.doing === asked)
        setInHand((was) =>
          was === undefined ||
          was.doing !== asked ||
          was.doing.what === "new" ||
          was.doing.what === "edit"
            ? was
            : { ...was, doing: { ...was.doing, refused: why } },
        );
      else setSaid({ tone: "trouble", label, text: why });
    };
  /** A field of the dialog in hand changed: the core's last refusal no longer stands. */
  const editedInDialog = () =>
    setInHand((was) =>
      was === undefined ||
      was.doing.what === "new" ||
      was.doing.what === "edit" ||
      was.doing.refused === undefined
        ? was
        : { ...was, doing: { ...was.doing, refused: undefined } },
    );
  const open = (skill: StudioSkill, level: SkillLevel) => {
    const asked = at;
    void commands.skillText(plane, level, skill.name).then((answer) => {
      if (answer.status === "error")
        setSaid({ tone: "trouble", label: "The skill could not be opened", text: answer.error });
      // The person went elsewhere while the text was read: the editor is not opened there.
      else if (samePlace(placeNow.current, asked)) {
        setDoing({ what: "edit", skill, level, was: answer.data });
        // The editor is the Text tab's, in place (#3682; Skills design 04).
        history.replace({ ...placeNow.current, tab: "text" });
      }
    });
  };
  /** **Rename** (S4, #3685): done, its place follows the new name, and Undo renames it back. */
  const rename = (asked: Doing, skill: StudioSkill, level: SkillLevel, to: string) => {
    const from = skill.name;
    const back = () =>
      change(
        "The rename was not undone",
        () => commands.skillRename(plane, level, to, from),
        async (notes) => {
          history.replace({ ...placeNow.current, skill: skillRowNamed(from) });
          return resultOf("Rename undone", `${to} is ${from} again.`, notes);
        },
      );
    change(
      "The skill was not renamed",
      () => commands.skillRename(plane, level, from, to),
      async (notes) => {
        history.replace({ ...placeNow.current, skill: skillRowNamed(to) });
        // What followed it, and what was left as it was, is the core's to say: its notes.
        return { ...resultOf("Skill renamed", `${from} is ${to} now.`, notes), undo: back };
      },
      refusedInDialog(asked, "The skill was not renamed"),
    );
  };
  /** **Move** (S4, #3685): done, and Undo moves it back, with the grant it had where it was a
   *  project skill. */
  const move = (
    asked: Doing,
    skill: StudioSkill,
    from: SkillLevel,
    to: SkillLevel,
    grant: SkillGrant | null,
  ) => {
    const name = skill.name;
    const back = () =>
      change(
        "The move was not undone",
        async () =>
          commands.skillMove(
            plane,
            to,
            name,
            from,
            (await commands.skillMoveNeedsGrant(to, from))
              ? (skill.granted ?? { to: "root" })
              : null,
          ),
        async (notes) =>
          resultOf("Move undone", `${name} is a ${labelOf(levels, from)} skill again.`, notes),
      );
    change(
      "The skill was not moved",
      () => commands.skillMove(plane, from, name, to, grant),
      async (notes) => ({
        ...resultOf("Skill moved", `${name} is a ${labelOf(levels, to)} skill now.`, notes),
        undo: back,
      }),
      refusedInDialog(asked, "The skill was not moved"),
    );
  };
  /** Adds `written` to the persona's skills, or takes it out (SK-5, #2768), as a change. */
  const personaSkill = (who: string, written: string, how: "add" | "remove") =>
    change(how === "add" ? "The skill was not added" : "The skill was not removed", () =>
      changePersonaSkill(plane, who, written, how).then((why): Answer =>
        why === undefined ? { status: "ok", data: { notes: [] } } : { status: "error", error: why },
      ),
    );
  /** Bring back (S2's split of Restore): the deleted skill `id` names, back at its level, `under`
   *  a new name where its own is a built-in skill's now (#3729). */
  const bringBack = (id: string, under: string | null = null) =>
    change("The skill was not brought back", () => commands.skillRestore(plane, id, under));
  /** After a delete: Bring it back on the Notice, for the copy just deleted. */
  const restoreOffered = async (
    name: string,
    level: SkillLevel,
    notes: readonly string[],
  ): Promise<Said | undefined> => {
    const answer = await commands.skillsDeleted(plane);
    const one =
      answer.status === "ok"
        ? answer.data.find((d) => d.name === name && sameLevel(d.level, level))
        : undefined;
    // Where it is not found in Deleted, no days and no Bring it back are promised.
    if (one === undefined) return resultOf("Skill deleted", `${name} is deleted.`, notes);
    const days = counted(one.days_left, "day", "days");
    return {
      ...resultOf("Skill deleted", `${name} is deleted. It stays in Deleted for ${days}.`, notes),
      restore: one.id,
    };
  };

  /** **Make my own copy** (#3686): the copy made, said with its built-in still loading, and its
   *  page opened; or the core's refusal, said in the dialog. */
  const copy = (name: string, level: SkillLevel, to: string, description: string) => {
    setBusy(true);
    const refusedWith = (why: string) =>
      setCopying((was) => (was === undefined ? was : { ...was, refused: why }));
    void commands
      .skillCopy(plane, name, level, to, description)
      .then((answer) => {
        if (answer.status === "error") {
          refusedWith(answer.error);
          return;
        }
        setCopying(undefined);
        go({ page: "library", scope: scopeOfLevel(level, scope), skill: skillRowNamed(to) });
        setSaid({
          tone: "news",
          label: "Copy made",
          text: [
            `${to} is your own copy of ${name}. Chats still get ${name}.`,
            ...answer.data.notes,
          ].join(" "),
        });
      })
      .catch((err: unknown) => refusedWith(String(err)))
      .finally(() => {
        setBusy(false);
        setDone((n) => n + 1);
      });
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

  /** Leaves the old plugin keys as they are: said once, as their Ask's Dismiss was. */
  const leaveOldKeys = () => {
    setBusy(true);
    void commands
      .dismissOldHarnessPlugins(plane)
      .then((answer) => {
        if (answer.status === "error")
          setSaid({ tone: "trouble", label: "The keys were not left", text: answer.error });
      })
      .catch((err: unknown) =>
        setSaid({ tone: "trouble", label: "The keys were not left", text: String(err) }),
      )
      .finally(() => {
        setBusy(false);
        setDone((n) => n + 1);
      });
  };

  /** **Runs an item's one fix** (#3689): a change, or a way to where it is made. */
  const fixNeed = (one: NeedsYou) => {
    const fix = one.fix;
    switch (fix.kind) {
      case "accept":
        change("The change was not accepted", () => commands.skillLevelAccept(plane, fix.folder));
        return;
      case "accept-folders":
        change("The folders were not accepted", () =>
          commands.skillLevelsConfirm(plane, fix.folders),
        );
        return;
      case "rename": {
        const skill = (studio?.skills ?? []).find(
          (row) =>
            row.name === fix.name &&
            row.level === fix.level &&
            row.local === fix.local &&
            row.owner === null,
        );
        if (skill === undefined) return go({ page: "library", scope });
        go({ page: "library", scope, skill: skillRowId(skill) });
        const level = levelOf(skill, scope);
        if (level !== null) setDoing({ what: "rename", skill, level });
        return;
      }
      case "harness": {
        // The program's own skill the item is about, chosen beside the list with its switch.
        const program = (studio?.harnesses ?? []).find((harness) =>
          harness.skills.some((skill) => skill.name === one.title),
        );
        return go({
          page: "harness",
          scope,
          ...(program === undefined
            ? {}
            : { skill: harnessSkillNamed(program.harness, one.title) }),
        });
      }
      case "library":
        return go({ page: "library", scope });
      case "unpin":
        change("The pin was not removed", () => commands.skillPin(plane, fix.by, fix.name, false));
        return;
      case "move-skills":
        moveSkills(fix.fix);
        return;
      case "leave-old-keys":
        leaveOldKeys();
        return;
      case "remove-old-keys":
        // Each file's change is held for the person to apply in its own tab; nothing is
        // written before.
        setBusy(true);
        void proposeOldKeysRemoval(plane)
          .then((answer) => {
            if (answer.status === "error")
              setSaid({ tone: "trouble", label: "Nothing was proposed", text: answer.error });
          })
          .catch((err: unknown) =>
            setSaid({ tone: "trouble", label: "Nothing was proposed", text: String(err) }),
          )
          .finally(() => {
            setBusy(false);
            setDone((n) => n + 1);
          });
        return;
    }
  };
  // --- What each page holds ----------------------------------------------------------------

  // What needs the person, as the core works it out (#3689): the count and the page alike.
  const needs = studio?.needs ?? [];
  const needing = needs.length;
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
    // Drafts stays while it is the page shown, so accepting or discarding its last draft never
    // takes the person to another page (S9).
    ...(drafts.length > 0 || at.page === "drafts"
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
  // What is chosen on Harness skills (#3692): a program's own skill, by its place's id.
  const groups =
    page === "harness" ? harnessGroups(studio?.harnesses ?? [], studio?.not_supported ?? []) : [];
  const harnessRow = page === "harness" ? harnessRowAt(groups, at.skill) : undefined;
  // What is chosen on Routing: a chat's start, by its number.
  const routedChat =
    page === "routing"
      ? studio?.router?.chats.find((one) => routedChatId(one.session) === at.skill)
      : undefined;
  const itemName =
    harnessRow !== undefined
      ? harnessRow.kind === "not-supported"
        ? harnessRow.one.title
        : harnessRow.kind === "none"
          ? harnessRow.title
          : harnessRow.skill.name
      : routedChat === undefined
        ? undefined
        : `Chat ${routedChat.session}`;
  const inSources = page === "sources" ? at.sources : undefined;
  const path: Crumb[] = [
    { id: page, label: STUDIO_PAGE_NAMES[page] },
    ...(chosen === undefined ? [] : [{ id: "skill", label: chosen.name }]),
    ...(itemName === undefined ? [] : [{ id: "item", label: itemName }]),
    ...(chosen !== undefined && at.tab !== undefined && at.tab !== "overview"
      ? [{ id: "tab", label: SKILL_PAGE_TAB_NAMES[at.tab] }]
      : []),
    ...(inSources === undefined ? [] : sourcesPath(inSources)),
    ...(doing === undefined || DOING_NAMES[doing.what] === undefined
      ? []
      : [{ id: "doing", label: DOING_NAMES[doing.what] ?? "" }]),
  ];
  const restoring = said?.restore;
  const undoing = said?.undo;
  const notice = said !== undefined && (
    <Notice
      cause="skill-change"
      tone={said.tone}
      label={said.label}
      fixes={
        restoring !== undefined
          ? [{ label: "Bring it back", onPress: () => bringBack(restoring) }]
          : undoing !== undefined
            ? [{ label: "Undo", onPress: undoing, busy }]
            : undefined
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
          else if (crumb.id === "skill") {
            setDoing(undefined);
            history.replace({
              page,
              scope,
              ...(at.skill === undefined ? {} : { skill: at.skill }),
            });
          } else if (crumb.id === "tab") setDoing(undefined);
          else if (crumb.id === "source" && inSources !== undefined)
            go({
              page,
              scope,
              sources: { at: "source", source: inSources.source, label: inSources.label },
            });
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
  /** An item other than a skill is chosen on its page (a draft): narrow, it is drawn alone. */
  let itemChosen = false;
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
    // a copy that one hides. Counted in the core's order, before the built-ins go to the end.
    const seen = new Map<string, number>();
    const testids = new Map(
      shown.map((skill) => {
        const before = seen.get(skill.name) ?? 0;
        seen.set(skill.name, before + 1);
        const said = before === 0 ? `skill-${skill.name}` : `skill-${skill.name}-hidden-${before}`;
        return [skillRowId(skill), said];
      }),
    );
    // **Built-in skills fold into one quiet line at the end** (S12), unless the Built-in chip
    // asks for them alone; a built-in that needs the person, clashes or is searched for stands
    // out of the fold.
    const own = shown.filter((skill) => skill.level !== "built-in");
    const builtIns = shown.filter((skill) => skill.level === "built-in");
    const foldsBuiltIns = !chips.has("built-in");
    const row = (skill: StudioSkill) => {
      const mark = markOf(skill);
      return (
        <ListRow
          key={skillRowId(skill)}
          id={skillRowId(skill)}
          textValue={skill.name}
          data-testid={testids.get(skillRowId(skill))}
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
                // The fold's rows are never chosen: a press on one folds or unfolds it.
                disabledKeys={BUILT_IN_RUN_ROWS}
                disabledBehavior="selection"
                selectedKeys={chosen === undefined ? new Set() : new Set([skillRowId(chosen)])}
                onSelectionChange={(keys: Selection) => {
                  const [first] = keys === "all" ? [] : [...keys];
                  if (first === undefined) return;
                  go({ page: "library", scope, skill: String(first) });
                }}
              >
                {own.map(row)}
                {foldsBuiltIns ? (
                  <ListRun
                    id={BUILT_IN_RUN}
                    items={builtIns}
                    standsOut={(skill) =>
                      builtInStandsOut(skill, studio, {
                        query,
                        chosen: chosen === undefined ? undefined : skillRowId(chosen),
                      })
                    }
                    says={(count) => counted(count, "built-in skill", "built-in skills")}
                    mark={<Sparkle aria-hidden="true" />}
                    closing={{
                      says: BUILT_IN_FIXED,
                      children: (
                        <Button
                          size="sm"
                          variant="ghost"
                          isDisabled={busy}
                          onPress={() => setCopying({})}
                        >
                          {`${MAKE_MY_OWN_COPY}…`}
                        </Button>
                      ),
                    }}
                  >
                    {row}
                  </ListRun>
                ) : (
                  builtIns.map(row)
                )}
              </List>
            )}
          </>
        ))}
      </>
    );
    const panel =
      (doing?.what === "new" || doing?.what === "edit") && studio !== undefined ? (
        <ChangePanel
          plane={plane}
          scope={scope}
          doing={doing}
          busy={busy}
          levels={levels}
          onCancel={() => setDoing(undefined)}
          change={change}
        />
      ) : undefined;
    item =
      doing?.what === "new" && panel !== undefined ? (
        <div className="skill-studio-item" data-testid="skill-studio-doing">
          {panel}
        </div>
      ) : chosen !== undefined && studio !== undefined ? (
        <SkillPage
          key={skillRowId(chosen)}
          plane={plane}
          studio={studio}
          skill={chosen}
          scope={scope}
          drafts={drafts}
          tab={at.tab ?? "overview"}
          run={at.run}
          busy={busy}
          changed={changed + done}
          change={change}
          doing={
            doing?.what === "edit" && panel !== undefined
              ? { what: "edit", drawn: panel }
              : undefined
          }
          onTab={(tab) => history.replace({ ...at, tab })}
          onGo={(to, item) => go({ page: to, scope, ...(item === undefined ? {} : { item }) })}
          onEdit={(level) => open(chosen, level)}
          onRename={(level) => setDoing({ what: "rename", skill: chosen, level })}
          onMove={(level) => setDoing({ what: "move", skill: chosen, from: level })}
          onDelete={(level) => setDoing({ what: "delete", skill: chosen, level })}
          onPersona={personaSkill}
          onCopy={() => setCopying({ from: chosen.name })}
          offerFor={offerFor}
          onPress={onPress}
          onOpenView={onOpenView}
        />
      ) : (
        <EmptyState
          headline="Pick a skill"
          body="What it does, who gets it and what can be changed open here."
          testid="skill-studio-unpicked"
        />
      );
  } else if (page === "needs") {
    const shown = needShown(needs, need);
    list = pageList(
      NEEDS_YOU_LEAD,
      held(() => (
        <NeedsYouList
          needs={needs}
          chosen={needs.some((one) => one.id === need) ? need : undefined}
          onChoose={(id) => setNeed(id)}
        />
      )),
    );
    item =
      shown === undefined ? undefined : (
        <NeedsYouItem need={shown} busy={busy} onFix={fixNeed} onLeave={leaveOldKeys} />
      );
  } else if (page === "drafts") {
    /* A chat's drafts (S9, #3690; SK-9, #2772): the list, and the draft chosen beside it. */
    const draftLevel = (draft: SkillDraft) => labelOf(studio?.levels ?? [], draft.level);
    const draft = drafts.find((one) => one.id === at.item);
    const none = draftsRead !== undefined && drafts.length === 0;
    list = pageList(
      "Skills a chat drafted for you. Try one, then accept it into the project or discard it.",
      draftsRead === undefined
        ? held(() => <Placeholders />)
        : held(() =>
            none ? (
              <EmptyState
                headline="No drafts"
                body="Ask a chat to draft a skill, and what it writes waits here for you to try."
                testid="skill-drafts-none"
                action={draftAsk}
              />
            ) : (
              <DraftsList
                drafts={drafts}
                labelOf={draftLevel}
                chosen={draft?.id}
                onChoose={(id) => go({ page: "drafts", scope, item: id })}
              />
            ),
          ),
      none ? undefined : draftAsk,
    );
    itemChosen = draft !== undefined;
    item =
      draft !== undefined ? (
        <DraftPage
          key={draft.id}
          plane={plane}
          draft={draft}
          label={draftLevel(draft)}
          local={draft.level.local}
          busy={busy}
          onBack={() => go({ page: "drafts", scope })}
          onAccept={(one) =>
            change(
              "The draft was not accepted",
              () => commands.skillDraftAccept(plane, one.id),
              // The skill it made, or changed, in the Library of chats that get it: accepted,
              // it is no draft now. A workspace's or a persona's is not the root's.
              async () => {
                go({
                  page: "library",
                  scope: scopeOfLevel(one.level, scope),
                  skill: skillRowNamed(one.name),
                });
                return undefined;
              },
            )
          }
          onDiscard={(one) =>
            change(
              "The draft was not discarded",
              () => commands.skillDraftReject(plane, one.id).then(answered),
              async () => {
                go({ page: "drafts", scope });
                return {
                  tone: "news",
                  label: "Draft discarded",
                  text: `${one.name} is discarded.`,
                };
              },
            )
          }
        />
      ) : none ? undefined : (
        <EmptyState
          headline="Pick a draft"
          body="What it writes, Try it, and Accept or Discard open here."
          testid="skill-drafts-unpicked"
        />
      );
  } else if (page === "sources") {
    // Sources and the pages inside it (#3687): each heads itself, as a source's page and the
    // reviews name what they are about, not the page they are on.
    list = held(() => (
      <SkillSourcesPage
        plane={plane}
        at={at.sources}
        onGo={(sources) => go({ page: "sources", scope, ...(sources && { sources }) })}
        onOpenSkill={(name) => go({ page: "library", scope, skill: skillRowNamed(name) })}
        levels={levels}
        start={start}
        busy={busy}
        change={change}
        changed={changed + done}
      />
    ));
  } else if (page === "deleted") {
    /* Deleted (S9, #3691): the skills deleted in the last 30 days, and the one chosen beside. */
    const one = deleted.find((each) => each.id === at.item);
    list = pageList(
      "Skills deleted in the last 30 days. Bring one back and chats started after that get it again.",
      held(() =>
        deleted.length === 0 ? (
          <EmptyState
            headline="Nothing deleted in the last 30 days."
            body="A skill you delete waits here for 30 days, to be brought back."
            testid="skill-deleted-none"
            action={<Button onPress={() => go({ page: "library", scope })}>Go to Library</Button>}
          />
        ) : (
          <DeletedList
            deleted={deleted}
            chosen={one?.id}
            onChoose={(id) => go({ page: "deleted", scope, item: id })}
          />
        ),
      ),
    );
    itemChosen = one !== undefined;
    item =
      one !== undefined ? (
        <DeletedPage
          key={one.id}
          plane={plane}
          deleted={one}
          builtIns={skills.filter((skill) => skill.level === "built-in").map((skill) => skill.name)}
          busy={busy}
          onBringBack={(it, under) => bringBack(it.id, under ?? null)}
        />
      ) : deleted.length === 0 ? undefined : (
        <EmptyState
          headline="Pick a deleted skill"
          body="How long it can be brought back, Bring back and its last text open here."
          testid="skill-deleted-unpicked"
        />
      );
  } else if (page === "harness") {
    const row = harnessRow;
    list = pageList(
      "The skills each program finds itself: your own, a repo's and a plugin's. You can switch your own and a repo's for these chats; a plugin's come and go with the plugin, which its program turns on or off.",
      held(() => (
        <HarnessList
          groups={groups}
          chosen={row}
          onChoose={(id) => go({ page: "harness", scope, skill: id })}
        />
      )),
    );
    item =
      studio === undefined ? undefined : (
        <HarnessItem
          key={row?.id}
          plane={plane}
          workspace={workspace}
          row={row}
          onSwitched={() => setSwitched((n) => n + 1)}
        />
      );
  } else {
    const router = studio?.router ?? null;
    const chats = router?.chats ?? [];
    const chat = routedChat;
    const by: SkillPinner | null = pinnerOf(scope);
    list = pageList(
      "When a chat's list of skills is over its budget, a router decides which it sees. The router is asked once, at a chat's start.",
      held(() =>
        router === null ? null : (
          <>
            <RoutingFacts
              plane={plane}
              router={router}
              by={by}
              workspace={workspace}
              persona={persona}
              busy={busy}
              onChoose={(whose, chosen, text) =>
                change(
                  "The router was not changed",
                  () => commands.skillRouterChoose(plane, whose, chosen),
                  () => Promise.resolve({ tone: "news", label: "Router changed", text }),
                )
              }
              onGraded={() => setDone((was) => was + 1)}
            />
            <ViewListHead title="How it did at each chat's start" />
            <RoutedChats
              chats={chats}
              chosen={chat}
              onChoose={(id) => go({ page: "routing", scope, skill: id })}
            />
          </>
        ),
      ),
    );
    item = studio === undefined || chats.length === 0 ? undefined : <RoutedChatItem chat={chat} />;
  }

  const builtInNames = [
    ...new Set(skills.filter((skill) => skill.level === "built-in").map((skill) => skill.name)),
  ];
  return (
    <div className="skill-studio" data-testid="skill-studio">
      {copying !== undefined && studio !== undefined && (
        <SkillCopy
          builtIns={builtInNames}
          descriptionOf={(name) =>
            skills.find((skill) => skill.level === "built-in" && skill.name === name)
              ?.description ?? ""
          }
          from={copying.from}
          levels={levels}
          start={start}
          busy={busy}
          refused={copying.refused}
          onEdit={() => setCopying((was) => (was === undefined ? was : { from: was.from }))}
          onCopy={copy}
          onCancel={() => setCopying(undefined)}
        />
      )}
      {doing?.what === "rename" && (
        <RenameDialog
          plane={plane}
          skill={doing.skill}
          level={doing.level}
          taken={skills
            .filter((one) => one !== doing.skill && atLevel(one, scope, doing.level))
            .map((one) => one.name)}
          builtIns={builtInNames}
          busy={busy}
          refused={doing.refused}
          onEdit={editedInDialog}
          onRename={(to) => rename(doing, doing.skill, doing.level, to)}
          onClose={() => setDoing(undefined)}
        />
      )}
      {doing?.what === "move" && (
        <MoveDialog
          plane={plane}
          skill={doing.skill}
          from={doing.from}
          levels={levels}
          workspaces={studio?.workspaces ?? []}
          busy={busy}
          refused={doing.refused}
          onEdit={editedInDialog}
          onMove={(to, grant) => move(doing, doing.skill, doing.from, to, grant)}
          onClose={() => setDoing(undefined)}
        />
      )}
      {doing?.what === "delete" && (
        <DeleteDialog
          plane={plane}
          skill={doing.skill}
          level={doing.level}
          busy={busy}
          refused={doing.refused}
          onDelete={() =>
            change(
              "The skill was not deleted",
              () => commands.skillDelete(plane, doing.level, doing.skill.name),
              (notes) => restoreOffered(doing.skill.name, doing.level, notes),
              refusedInDialog(doing, "The skill was not deleted"),
            )
          }
          onClose={() => setDoing(undefined)}
        />
      )}
      <ViewFrame
        label="Skill studio pages"
        pages={pages}
        page={page}
        onPage={(id) => go({ page: id as StudioPage, scope })}
        list={list}
        item={item}
        opened={
          chosen !== undefined ||
          doing !== undefined ||
          itemName !== undefined ||
          itemChosen ||
          (page === "needs" && need !== undefined && needs.some((one) => one.id === need))
        }
        onBack={() => go({ page, scope })}
        narrow={narrow}
        bar={bar}
        history={history}
      />
    </div>
  );
}

/** The panel for the change in progress drawn in a pane: New skill, or the editor. */
function ChangePanel({
  plane,
  scope,
  doing,
  busy,
  levels,
  onCancel,
  change,
}: {
  plane: string;
  scope: StudioScope;
  doing: Extract<Doing, { what: "new" | "edit" }>;
  busy: boolean;
  levels: readonly StudioLevel[];
  onCancel: () => void;
  change: Change;
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
  }
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
