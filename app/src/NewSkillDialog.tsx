// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **New skill…** (#3684, spec #3679 S4; decisions 13, 37 and 42): a dialog that asks first how
 * the person wants to make the skill, as a short choice is (S2), and then only what that way
 * needs.
 *
 * - **Write it yourself**: a name, checked as it is typed (its shape, a built-in skill's name,
 *   one its level already holds), what it does, where it lands and a template. The skill is made
 *   in one commit, and its page opens with its Text tab in Edit (#3683), which the studio does.
 * - **Ask a chat to draft it**: what it should do and where it lands; a chat opens with the
 *   core's prompt typed and not sent, and the draft comes back to Drafts.
 * - **Install one from a source**: goes to Sources, where every install is reviewed (S5).
 *
 * It replaces the inline New skill panel and the three starts of an empty studio (decision 42):
 * the studio's top bar and the Skills view's New skill… both open it. What the core refuses is
 * said in the dialog, beside the fields the person is still filling.
 */
import { FilePlus, FolderGit2, MessageSquarePlus } from "lucide-react";
import { useEffect, useState } from "react";
import {
  commands,
  type SkillLevel,
  type SkillTemplate,
  type SkillTemplateId,
  type StudioLevel,
} from "./bindings";
import { nameProblem, whoGets } from "./SkillChangeDialogs";
import { NEW_SKILL } from "./SkillStarts";
import { Choice, Field, SettingRow } from "./settings/components";
import { labelOf, levelKey } from "./skillChanges";
import { CardButton } from "./ui/card/CardButton";
import { Dialog } from "./ui/dialog/Dialog";
import { Select } from "./ui/select/Select";

/** The way the person picked, or none yet. */
type Way = "how" | "write" | "draft";

/** **The three ways, in their order** (S4): each one's words and line. */
export const NEW_SKILL_WAYS = [
  {
    way: "write",
    title: "Write it yourself",
    line: "Give it a name and say what it does; its page opens with the text ready to edit.",
    mark: FilePlus,
  },
  {
    way: "draft",
    title: "Ask a chat to draft it",
    line: "Say what you want; a chat drafts it, and it comes back under Drafts to try and accept.",
    mark: MessageSquarePlus,
  },
  {
    way: "sources",
    title: "Install one from a source",
    line: "Pick a reviewed skill from a git repository.",
    mark: FolderGit2,
  },
] as const;

/** What the template picker holds a template by: its kind, or `<extension>/<template>`. */
function templateKey(id: SkillTemplateId): string {
  return id.kind === "extension" ? `${id.extension}/${id.template}` : id.kind;
}

/** The templates a new skill can start from, read once they are `wanted` (Write it yourself):
 *  purlis's own and each extension's (ADR 0101). None until they are read, and none where they
 *  could not be. */
function useTemplates(plane: string, wanted: boolean): readonly SkillTemplate[] {
  const [templates, setTemplates] = useState<readonly SkillTemplate[]>([]);
  useEffect(() => {
    if (!wanted) return;
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
  }, [plane, wanted]);
  return templates;
}

/** **New skill…**: how first, then what that way needs. */
export function NewSkillDialog({
  plane,
  levels,
  start,
  takenAt,
  busy,
  refused,
  onEdit,
  onMake,
  onDraft,
  onSources,
  onClose,
}: {
  plane: string;
  /** The levels it can land at, from the studio's scope, by the core's words. */
  levels: readonly StudioLevel[];
  /** Where it lands unless the person picks another: where they came from (decision 37). */
  start: SkillLevel;
  /** The names `level` already holds, as far as the studio has read them. */
  takenAt: (level: SkillLevel) => readonly string[];
  busy: boolean;
  /** Why the core did not make the last skill asked for: its sentence, unchanged. */
  refused?: string;
  /** A field changed: a refusal of what was asked before no longer stands beside it. */
  onEdit: () => void;
  /** Makes the skill; `template` is purlis's own or an extension's. */
  onMake: (level: SkillLevel, name: string, description: string, template: SkillTemplateId) => void;
  /** Opens a chat with the prompt to draft `what`, at `level`, typed and not sent. */
  onDraft: (level: SkillLevel, what: string) => void;
  /** Goes to Sources. */
  onSources: () => void;
  onClose: () => void;
}) {
  const [way, setWay] = useState<Way>("how");
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [level, setLevel] = useState<SkillLevel>(start);
  const [template, setTemplate] = useState<SkillTemplateId>({ kind: "blank" });
  const templates = useTemplates(plane, way === "write");
  const edited =
    <T,>(set: (value: T) => void) =>
    (value: T) => {
      set(value);
      onEdit();
    };

  if (way === "how")
    return (
      <Dialog
        // A dialog per step: each starts the keyboard where its own step does.
        key="how"
        onClose={onClose}
        title={NEW_SKILL}
        titleId="skill-new"
        description="How do you want to make it?"
        initialFocus={(dialog) => dialog.querySelector("ul button") as HTMLElement | null}
        data-testid="skill-new"
      >
        <ul className="skill-new-ways" aria-label="Ways to make a skill">
          {NEW_SKILL_WAYS.map(({ way: one, title, line, mark: Mark }) => (
            <li key={one}>
              <CardButton
                variant="door"
                title={title}
                line={line}
                mark={<Mark aria-hidden="true" />}
                onPress={() => (one === "sources" ? onSources() : setWay(one))}
              />
            </li>
          ))}
        </ul>
      </Dialog>
    );

  const wanted = name.trim();
  const said = description.trim();
  const writing = way === "write";
  const problem = writing ? nameProblem(wanted, "", takenAt(level)) : undefined;
  const ready = !busy && said !== "" && (!writing || (wanted !== "" && problem === undefined));
  const lands = labelOf(levels, level);
  const back = {
    children: "Back",
    // Not while the core is asked: its answer is said on this step.
    isDisabled: busy,
    onPress: () => {
      setWay("how");
      onEdit();
    },
  };
  return (
    <Dialog
      key={way}
      onClose={onClose}
      title={writing ? "Write a new skill" : "Ask a chat to draft a skill"}
      titleId="skill-new"
      description={
        writing
          ? "Its page opens with the text ready to edit, checked as you type."
          : "A chat opens with the request typed and not sent. Its draft comes back under Drafts, to try and accept."
      }
      initialFocus="field"
      summary={
        writing
          ? wanted === ""
            ? undefined
            : `${wanted} is made in the ${lands} skills, in one commit.`
          : `The draft lands in the ${lands} skills once you accept it.`
      }
      alternative={back}
      main={{
        children: writing ? (busy ? "Making…" : "Make it and open its page") : "Open the chat",
        reserve: writing ? ["Make it and open its page", "Making…"] : ["Open the chat"],
        isDisabled: !ready,
        onPress: () => {
          if (!ready) return;
          if (writing) onMake(level, wanted, said, template);
          else onDraft(level, said);
        },
      }}
      data-testid={writing ? "skill-new-write" : "skill-new-draft"}
    >
      {writing && (
        <>
          <SettingRow
            label="Name"
            help="Lowercase letters, digits and hyphens."
            control={(ids) => (
              <Field
                kind="text"
                ids={ids}
                value={name}
                placeholder="pr-review"
                onChange={edited(setName)}
              />
            )}
          />
          {problem !== undefined && (
            <p className="trouble" role="alert" data-testid="skill-new-problem">
              {problem}
            </p>
          )}
        </>
      )}
      <SettingRow
        label={writing ? "What it does, and when a chat should use it" : "What should it do?"}
        help="One line: a chat reads it to know when the skill applies."
        control={(ids) => (
          <Field
            kind="text"
            ids={ids}
            value={description}
            placeholder="Reviews a pull request against our checklist"
            onChange={edited(setDescription)}
          />
        )}
      />
      <SettingRow
        label="Lands in"
        grouped
        control={(ids) => (
          <Choice
            kind="radio"
            ids={ids}
            options={levels.map((one) => ({
              value: levelKey(one.level),
              label: one.label,
              says: whoGets(one.level),
            }))}
            value={levelKey(level)}
            onValueChange={(key) => {
              const picked = levels.find((one) => levelKey(one.level) === key)?.level;
              if (picked !== undefined) edited(setLevel)(picked);
            }}
            disabled={busy}
          />
        )}
      />
      {writing && (
        <SettingRow
          label="Start from"
          control={(ids) => (
            <Select
              id={ids.id}
              aria-labelledby={ids.labelledBy}
              value={templateKey(template)}
              options={templates.map((one) => ({
                value: templateKey(one.id),
                label: one.label,
                tooltip: one.about,
              }))}
              // One of the offered ids, or the blank template where the list does not name it.
              onValueChange={(to) =>
                edited(setTemplate)(
                  templates.find((one) => templateKey(one.id) === to)?.id ?? { kind: "blank" },
                )
              }
            />
          )}
        />
      )}
      {refused !== undefined && (
        <p className="trouble" role="alert" data-testid="skill-new-refused">
          {refused}
        </p>
      )}
    </Dialog>
  );
}
