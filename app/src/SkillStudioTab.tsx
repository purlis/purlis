// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { lazy, Suspense, useEffect, useRef, useState, type DragEvent } from "react";
import { MessageSquarePlus, Plus, Sparkles } from "lucide-react";
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
  type StudioSkill,
} from "./bindings";
import { counted } from "./dispatches";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { SkillStudioHarnesses } from "./SkillStudioHarnesses";
import { Button, ButtonRow } from "./ui/button/Button";
import { SkillPinSwitch, SkillPinsRefusedLine } from "./SkillPins";
import { Notice } from "./Notice";
import { SkillCompatBadges } from "./SkillCompatBadges";
import { SkillRowActions } from "./SkillRowActions";
import { SkillDrafts } from "./SkillDrafts";
import {
  DRAFT_WITH_A_CHAT,
  NEW_SKILL,
  SkillStarts,
  SkillsNotRead,
  SkillsReading,
  type SkillStart,
} from "./SkillStarts";
import { askDraft, useSkillDrafts } from "./skillDraftAsks";
import { SkillSources } from "./SkillSources";
import { Select } from "./ui/select/Select";
import { Field, SettingRow } from "./settings/components";
import {
  grantChoices,
  grantLabel,
  labelOf,
  levelFrom,
  levelKey,
  levelOf,
  levelsFor,
  picking,
  whoseOf,
  sameGrant,
  sameLevel,
} from "./skillChanges";
import {
  AT_THE_ROOT,
  inWorkspace,
  pinnerOf,
  scopeChat,
  scopeKey,
  scopeOfKey,
  skillStudioTitle,
  skillStudioView,
  useStudioStart,
  whoseChats,
  type StudioScope,
} from "./skillStudio";
import { skillView } from "./skillRuns";
import { BUILT_IN_STAYS, turnOf } from "./skillsListing";
import { PersonaSkillAdd, changePersonaSkill } from "./personaSkills";
import type { ViewRef } from "./tabs";

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

/** A change in progress on one skill: which panel is open under the toolbar. */
type Doing =
  | { what: "new" }
  | { what: "edit"; skill: StudioSkill; level: SkillLevel; was: string }
  | { what: "rename"; skill: StudioSkill; level: SkillLevel }
  | { what: "move"; skill: StudioSkill; from: SkillLevel; to: SkillLevel }
  | { what: "delete"; skill: StudioSkill; level: SkillLevel; users: SkillUsers };

/** The levels a skill can be pinned at (SK-15d): the project's. A built-in skill is always
 *  listed, and a harness's or a repo's is never purlis's to route. */
const PINNABLE: ReadonlySet<StudioSkill["level"]> = new Set(["project", "workspace", "persona"]);

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

/** The type a dragged row carries, so a drop from elsewhere is not read as a skill. */
const DRAGGED = "application/x-purlis-skill";

/**
 * **The Skill studio's tab** (SK-2, #2765; SK-4, #2767; SK-7, #2770): one chat's skill set, at
 * the project's root or in one workspace. Every skill the chat gets, with its level, and every one
 * it does not get with the one line that says why. Each skill a person owns can be edited in the
 * light editor, renamed, moved to another level (by dragging its row onto a level, or with Move
 * to), deleted after the studio says who uses it, and restored. New skill starts one from a
 * template at the level the person came from. A project skill's grant, and in a workspace its
 * turn-off, are set on its row. Each change is one commit (the core's `skills::verbs`).
 *
 * **What it offers when it opens** (SK-22, #2785; decision 42) is the core's
 * (`skills::first_open`): while the project has no skill of its own, three starts above the
 * skills it lists (a chat's draft, browse sources, a new skill from the blank template); the
 * offered skill sources in its sources section; and, where the project's skills are still where
 * only some chat programs read them, the move into the studio, which is the doctor's `project-skills` fix (SK-21).
 */
export function SkillStudioTab({
  plane,
  scope,
  changed,
  onOpenView,
}: {
  plane: string;
  /** Whose chats it shows: a chat at the project's root, or in one workspace. */
  scope: StudioScope;
  changed: number;
  /** Opens the studio on another workspace's chats, or a skill's own tab with its runs
   *  (SK-3), each in its own tab. */
  onOpenView?: (view: ViewRef, title: string) => void;
}) {
  const [studio, setStudio] = useState<Studio>();
  const [refused, setRefused] = useState<string>();
  const [said, setSaid] = useState<Said>();
  const [done, setDone] = useState(0);
  const [doing, setDoing] = useState<Doing>();
  const [busy, setBusy] = useState(false);
  const [deleted, setDeleted] = useState<readonly DeletedSkill[]>([]);
  const [switched, setSwitched] = useState(0);
  const [moveLater, setMoveLater] = useState(false);
  const sourcesHeading = useRef<HTMLHeadingElement>(null);
  const { workspace, persona } = whoseChats(scope);
  const on = studio?.on === true;
  const drafts = useSkillDrafts(plane, changed + done);
  // A start the Skills view asked this studio to open on (#3266), taken once it is drawn: the
  // New skill form, or its skill sources.
  useStudioStart(scope, on, (start) => {
    if (start === "new") setDoing({ what: "new" });
    else showSources(sourcesHeading.current);
  });
  useEffect(() => {
    let live = true;
    void commands.skillStudio(plane, workspace, persona).then((answer) => {
      if (!live) return;
      if (answer.status === "ok") {
        setStudio(answer.data);
        setRefused(undefined);
      } else setRefused(answer.error);
    });
    return () => {
      live = false;
    };
  }, [plane, workspace, persona, changed, done, switched]);
  // A delete a crash or a quit cut short is finished or undone as the studio opens (SK-35),
  // said in one line each, and the lists read again.
  useEffect(() => {
    let live = true;
    if (!on) return;
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
  }, [plane, on]);
  useEffect(() => {
    let live = true;
    if (!on) return;
    void commands
      .skillsDeleted(plane)
      .then((answer) => {
        if (live && answer.status === "ok") setDeleted(answer.data ?? []);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [plane, on, changed, done]);

  if (refused !== undefined)
    return <SkillsNotRead refused={refused} testid="skill-studio-refused" />;
  if (studio === undefined) return <SkillsReading />;
  if (!studio.on)
    return (
      <EmptyState
        headline="The Skill studio is not on yet"
        body="Turn it on in Settings › You › Previews to try it while it is being built."
        mark={Sparkles}
        testid="skill-studio-off"
      />
    );

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
  const unmoved = studio.first.unmoved;
  const move = unmoved !== null && !moveLater && (
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
  );
  /** One of the three starts (decision 42), from the empty studio's buttons. */
  const startWith = (start: SkillStart) => {
    if (start === "draft") askDraft(plane, scope, studio.levels);
    else if (start === "sources") showSources(sourcesHeading.current);
    else setDoing({ what: "new" });
  };
  /** The three starts of a project with no skill of its own (decision 42). */
  const starts = studio.first.starts && doing === undefined && (
    <SkillStarts busy={busy} onStart={startWith} />
  );
  const picker = (
    <div className="vault-tools">
      <Select
        className="search-scope"
        aria-label="Whose chats"
        value={scopeKey(scope)}
        options={[
          AT_THE_ROOT,
          ...studio.workspaces.map(inWorkspace),
          ...(persona === null ? [] : [scope]),
        ].map((one) => ({ value: scopeKey(one), label: scopeChat(one) }))}
        onValueChange={(to) => {
          const next = scopeOfKey(to);
          onOpenView?.(skillStudioView(next), skillStudioTitle(next));
        }}
      />
      {persona !== null && (
        /* The persona's own skills list (#2768): every skill choice is the studio's. A name the
           studio lists has Add on its row; this takes one it does not. */
        <PersonaSkillAdd
          plane={plane}
          persona={persona}
          known={studio.skills.filter((skill) => !skill.named).map((skill) => skill.written)}
          onChanged={() => setDone((n) => n + 1)}
        />
      )}
      <Button
        icon={<Plus aria-hidden="true" />}
        onPress={() => setDoing({ what: "new" })}
        isDisabled={doing?.what === "new"}
      >
        {NEW_SKILL}
      </Button>
      <Button
        icon={<MessageSquarePlus aria-hidden="true" />}
        onPress={() => askDraft(plane, scope, studio.levels)}
      >
        {DRAFT_WITH_A_CHAT}
      </Button>
    </div>
  );
  // The project's, each workspace's and the persona's this studio shows (SK-32).
  const levels = levelsFor(studio.levels, scope);
  const start = levelFrom(scope, levels);
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
  const panel = doing !== undefined && (
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
    />
  );
  /** A chat's drafts (SK-9, #2772): Accept is one commit, Reject deletes the draft. */
  const drafted = (
    <SkillDrafts
      plane={plane}
      drafts={drafts}
      levels={studio.levels}
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
  );
  const restorable = deleted.length > 0 && (
    <section className="skill-deleted" aria-label="Deleted skills">
      <h2>Deleted skills</h2>
      <ul>
        {deleted.map((one) => (
          <li key={one.id} data-testid={`deleted-${one.name}`}>
            <code>{one.name}</code> · {one.label}{" "}
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
    </section>
  );
  /* Skill sources (SK-10): install from a git repository, and each newer commit's review. */
  const sourcesSection = (
    <SkillSources
      plane={plane}
      levels={levels}
      start={start}
      busy={busy}
      change={change}
      changed={changed + done}
      headingRef={sourcesHeading}
    />
  );
  // The first row of a name is the copy the chat gets, or its only copy; each row after it is a
  // copy that one hides.
  const seen = new Map<string, number>();
  const testid = (name: string) => {
    const before = seen.get(name) ?? 0;
    seen.set(name, before + 1);
    return before === 0 ? `skill-${name}` : `skill-${name}-hidden-${before}`;
  };
  // The Pin switch (SK-15d): for the workspace's or the persona's chats the studio shows, on the
  // first row of a name a chat could have pinned (one of the project's levels), where the chat
  // gets it or a pin of it is still written, so a pin can always be undone.
  const pinner = pinnerOf(scope);
  const pinRows = pinRowsOf(studio.skills);
  const refusedPins = pinner === null ? null : (studio.pins_refused ?? null);
  const pinOf = (skill: StudioSkill) => {
    if (pinner === null || pinRows.get(skill.name) !== skill) return null;
    const pinned = skill.pinned_by.some((by) => by.at === pinner.at && by.name === pinner.name);
    return skill.on || pinned ? pinned : null;
  };
  const pinSwitch = (skill: StudioSkill) => {
    const pinned = pinOf(skill);
    if (pinner === null || pinned === null) return null;
    return (
      <>
        {" "}
        <SkillPinSwitch
          name={skill.name}
          by={pinner}
          pinned={pinned}
          refused={refusedPins}
          busy={busy}
          onPin={(on) =>
            change(on ? "The skill was not pinned" : "The skill was not unpinned", () =>
              commands.skillPin(plane, pinner, skill.name, on),
            )
          }
        />
      </>
    );
  };
  const dropOn = (to: SkillLevel) => (event: DragEvent) => {
    const raw = event.dataTransfer.getData(DRAGGED);
    if (raw === "") return;
    event.preventDefault();
    const { name, from } = JSON.parse(raw) as { name: string; from: SkillLevel };
    const skill = studio.skills.find((one) => {
      const at = levelOf(one, scope);
      return one.name === name && at !== null && sameLevel(at, from);
    });
    if (skill !== undefined) moveTo(skill, from, to);
  };
  return (
    <div className="skill-studio" data-testid="skill-studio">
      {picker}
      <p className="honest">
        {`Every skill ${lowerFirst(scopeChat(scope))} gets, and why any other is left out.`}
        {persona !== null && " A persona brings its skills into every workspace."}
      </p>
      {notice}
      {move}
      {panel}
      {drafted}
      {starts}
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
      <div
        className="skill-levels"
        role="group"
        aria-label="Drop a skill on a level to move it there"
      >
        {levels.map(({ level, label }) => (
          <span
            key={label}
            className="skill-level-target"
            data-testid={`level-${levelKey(level)}`}
            onDragOver={(event) => {
              if (event.dataTransfer.types.includes(DRAGGED)) event.preventDefault();
            }}
            onDrop={dropOn(level)}
          >
            {label}
          </span>
        ))}
      </div>
      {studio.skills.length > 0 && (
        <table className="ui-table" aria-label="Skills">
          <thead>
            <tr>
              <th scope="col">Skill</th>
              <th scope="col">Level</th>
              <th scope="col">Granted to</th>
              <th scope="col">In a chat</th>
              <th scope="col">What it does</th>
              <th scope="col">Works on</th>
              <th scope="col">Change</th>
            </tr>
          </thead>
          <tbody>
            {studio.skills.map((skill) => {
              const level = levelOf(skill, scope);
              return (
                <tr
                  key={`${skill.level}\u0000${skill.folder}`}
                  data-testid={testid(skill.name)}
                  draggable={level !== null}
                  onDragStart={(event) => {
                    if (level === null) return;
                    event.dataTransfer.setData(
                      DRAGGED,
                      JSON.stringify({ name: skill.name, from: level }),
                    );
                    event.dataTransfer.effectAllowed = "move";
                  }}
                >
                  <td>
                    {onOpenView === undefined ? (
                      <code>{skill.name}</code>
                    ) : (
                      <Button
                        variant="ghost"
                        size="sm"
                        aria-label={`Open ${skill.name}`}
                        onPress={() => onOpenView(skillView(skill.name), skill.name)}
                      >
                        <code>{skill.name}</code>
                      </Button>
                    )}
                  </td>
                  <td>
                    {skill.label}
                    {skill.local && level !== null && (
                      <>
                        {" "}
                        <Button
                          onPress={() => share(skill, level)}
                          tooltip="Give it to the team: it is committed with the project"
                        >
                          Share
                        </Button>
                      </>
                    )}
                  </td>
                  <td>
                    {!skill.granted ? (
                      ""
                    ) : (
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
                    )}
                  </td>
                  <td>
                    {skill.on
                      ? (skill.unchecked ??
                        (skill.hidden.length > 0
                          ? `On, ${skill.hidden[0].hidden.said} (on ${skill.hidden.map((one) => one.title).join(", ")})`
                          : skill.pinned
                            ? "On, pinned"
                            : "On"))
                      : `Left out: ${skill.left_out ?? ""}`}
                    {pinSwitch(skill)}
                    {skill.on && skill.level === "built-in" && (
                      <span className="honest"> {BUILT_IN_STAYS}</span>
                    )}
                    {studio.clashes
                      .filter(
                        (clash) =>
                          skill.on &&
                          clash.name === skill.name &&
                          clash.level === skill.level &&
                          clash.local === skill.local,
                      )
                      .map((clash) => (
                        <span
                          key={`${clash.harness}\u0000${clash.outcome}`}
                          className="honest"
                          data-testid={`clash-${skill.name}`}
                        >
                          {` Also: ${clash.said}. `}
                          {level !== null && (
                            <Button
                              size="sm"
                              isDisabled={busy}
                              onPress={() => setDoing({ what: "rename", skill, level })}
                            >
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
                  </td>
                  <td>{skill.description}</td>
                  <td>
                    <SkillCompatBadges name={skill.name} compat={skill.compat} lint={skill.lint} />
                  </td>
                  <td>
                    <SkillRowActions
                      actions={[
                        ...(level === null
                          ? []
                          : [
                              {
                                id: "edit",
                                label: "Edit",
                                name: `Edit ${skill.name}`,
                                disabled: busy,
                                onPress: () => open(skill, level),
                              },
                              {
                                id: "rename",
                                label: "Rename",
                                name: `Rename ${skill.name}`,
                                disabled: busy,
                                onPress: () => setDoing({ what: "rename", skill, level }),
                              },
                            ]),
                        // A persona's own list (SK-5, #2768), one more pair of the row's actions. A
                        // persona's own skill is in its skills already, and needs no name (SK-32).
                        ...(persona === null || skill.level === "persona"
                          ? []
                          : [
                              skill.named
                                ? {
                                    id: "persona-remove",
                                    label: "Remove",
                                    name: `Remove ${skill.written} from ${persona}`,
                                    disabled: busy,
                                    onPress: () => personaSkill(persona, skill.written, "remove"),
                                  }
                                : {
                                    id: "persona-add",
                                    label: `Add to ${persona}`,
                                    name: `Add ${skill.written} to ${persona}`,
                                    disabled: busy,
                                    onPress: () => personaSkill(persona, skill.written, "add"),
                                  },
                            ]),
                        ...(level === null
                          ? []
                          : [
                              {
                                id: "delete",
                                label: "Delete",
                                name: `Delete ${skill.name}`,
                                disabled: busy,
                                danger: true,
                                onPress: () => askDelete(skill, level),
                              },
                            ]),
                      ]}
                      extra={
                        level === null ? undefined : (
                          <LevelPicker
                            label={`Move ${skill.name} to`}
                            levels={levels.filter((to) => !sameLevel(to.level, level))}
                            disabled={busy}
                            onPick={(to) => moveTo(skill, level, to)}
                          />
                        )
                      }
                    />
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
      <SkillStudioHarnesses
        plane={plane}
        workspace={workspace}
        harnesses={studio.harnesses}
        onSwitched={() => setSwitched((n) => n + 1)}
      />
      {sourcesSection}
      {restorable}
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
            change("The skill was not renamed", () =>
              commands.skillRename(plane, doing.level, doing.skill.name, to),
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

/** **Browse sources**: the studio's skill sources brought into view with the keyboard on their
 *  heading, after the press is done with focus (WebKit hands it back to a pressed button). */
function showSources(heading: HTMLHeadingElement | null): void {
  window.setTimeout(() => {
    heading?.scrollIntoView?.({ block: "start" });
    heading?.focus();
  }, 0);
}

/** `text` with its first letter lower-cased, to go inside a sentence. */
function lowerFirst(text: string): string {
  return text.charAt(0).toLowerCase() + text.slice(1);
}
