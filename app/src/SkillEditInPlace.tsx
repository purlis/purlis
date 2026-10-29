// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Editing a skill in place** (#3683; spec #3679, the amendment's S4, `prototype/skill-changes`):
 * Edit on a skill's page turns its Text tab into the editor, checked as the person types, with
 * Save and Discard, and an "Unsaved changes" mark in the header while there is something to save.
 *
 * **Leaving asks first.** While the editor holds text not saved, every way out of the skill's page
 * asks Keep editing, Discard and leave, or Save and leave: a step in the studio (a row, a page,
 * the scope, the path, back and forward), Stop editing, another change on the skill (Rename, Move,
 * Delete, New skill), and the close of the studio's tab (`skillEdit.ts`). A place reached from
 * outside the tab (a run's chip) is asked about once it is reached: Keep editing goes back.
 *
 * Text with nothing changed is left without a word.
 */
import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { commands, type SkillLevel, type SkillLintFinding, type StudioSkill } from "./bindings";
import { AlertDialog } from "./ui/dialog/Dialog";
import type { ViewHistory } from "./ui/panel/viewHistory";
import { samePlace, type StudioPlace } from "./skillStudio";
import {
  SAVED,
  closeAnswered,
  keepSkillEdit,
  skillEditIn,
  unsaved,
  useCloseAsked,
  useSkillEdit,
  type SkillEdit,
} from "./skillEdit";
import type { Said } from "./SkillPage";

/** The light editor, its own chunk (`tools/main-chunk-has-no-codemirror.mjs`). */
const SkillEditor = lazy(() => import("./SkillEditor"));

/** **The core's lint of `text`** (`skill_lint`), asked a moment after each edit while `edit`
 *  is open. */
function useSkillLint(plane: string, edit: SkillEdit | undefined): readonly SkillLintFinding[] {
  const [lint, setLint] = useState<readonly SkillLintFinding[]>([]);
  const level = edit?.level;
  const name = edit?.skill.name;
  const text = edit?.now;
  useEffect(() => {
    if (level === undefined || name === undefined || text === undefined) return;
    let live = true;
    const asked = setTimeout(() => {
      void commands
        .skillLint(plane, level, name, text)
        .then((answer) => {
          if (live && answer.status === "ok") setLint(answer.data);
        })
        .catch(() => undefined);
    }, 200);
    return () => {
      live = false;
      clearTimeout(asked);
    };
    // Each edit keeps the level it opened with: a keystroke never asks again for it alone.
  }, [plane, level, name, text]);
  return lint;
}

/** What is left, once the person has answered: a step, a close, or nothing more. */
type Leaving = { then: () => void; back?: StudioPlace; closing?: boolean };

/**
 * **The studio's editor in place, and its guard** for the studio tab whose history is `tab`
 * (`studioHistoryOf`), walked by `walked`. Its `history` is `walked` with every step that leaves
 * the editor's page asked about first; `leave` does the same for anything else that leaves it.
 */
export function useSkillEditInPlace({
  plane,
  tab,
  walked,
  busy,
  setBusy,
  say,
  onSaved,
}: {
  plane: string;
  tab: string;
  walked: ViewHistory<StudioPlace>;
  busy: boolean;
  setBusy: (busy: boolean) => void;
  say: (said: Said | undefined) => void;
  /** A save was made: what the studio reads is read again. */
  onSaved: () => void;
}) {
  const edit = useSkillEdit(tab);
  const closeAsked = useCloseAsked(tab);
  const lint = useSkillLint(plane, edit);
  const blocked = lint.find((finding) => finding.level === "blocks");
  const [leaving, setLeaving] = useState<Leaving>();
  const at = walked.at;
  // Where the tab is now, for the text that comes back after the person may have moved on.
  const placeNow = useRef(at);
  useEffect(() => {
    placeNow.current = at;
  });
  // The editor is drawn only on the page it was opened at.
  const here = edit !== undefined && samePlace(edit.at, at) ? edit : undefined;

  /** Leaves the editor, then `then`: at once where nothing would be lost, else once asked. */
  const leave = (then: () => void) => {
    const now = skillEditIn(tab);
    if (now === undefined) return then();
    if (!unsaved(now)) {
      keepSkillEdit(tab, undefined);
      return then();
    }
    setLeaving({ then });
  };
  /** Goes on with `then` where `to` is still the editor's page, else leaves it first. */
  const stepTo = (to: StudioPlace, then: () => void) => {
    const now = skillEditIn(tab);
    if (now !== undefined && samePlace(now.at, to)) then();
    else leave(then);
  };
  const history: ViewHistory<StudioPlace> = {
    ...walked,
    go: (to) => stepTo(to, () => walked.go(to)),
    replace: (to) => stepTo(to, () => walked.replace(to)),
    back: () => leave(walked.back),
    forward: () => leave(walked.forward),
  };

  // A place reached from outside the tab (an entry point) while the editor held unsaved text:
  // asked about here, and Keep editing goes back to the editor's page.
  useEffect(() => {
    if (edit === undefined || samePlace(edit.at, at)) return;
    if (!unsaved(edit)) keepSkillEdit(tab, undefined);
    else setLeaving((was) => was ?? { then: () => undefined, back: edit.at });
  }, [edit, at, tab]);

  /** Saves the text typed, says so, and goes on with `then`; or says why it was not saved and
   *  keeps the editor as it is. */
  const save = (then?: () => void) => {
    const now = skillEditIn(tab);
    if (now === undefined) return;
    setBusy(true);
    void commands
      .skillSave(plane, now.level, now.skill.name, now.was, now.now)
      .then((answer) => {
        if (answer.status === "error") {
          say({ tone: "trouble", label: "The skill was not saved", text: answer.error });
          return;
        }
        keepSkillEdit(tab, undefined);
        say({ tone: "news", label: "Skill saved", text: [SAVED, ...answer.data.notes].join(" ") });
        then?.();
      })
      .catch((err: unknown) =>
        say({ tone: "trouble", label: "The skill was not saved", text: String(err) }),
      )
      .finally(() => {
        setBusy(false);
        onSaved();
      });
  };

  /** **Edit**: the skill's text, read now, in the editor on its page's Text tab. */
  const open = (skill: StudioSkill, level: SkillLevel) => {
    const asked = placeNow.current;
    void commands.skillText(plane, level, skill.name).then((answer) => {
      if (answer.status === "error")
        say({ tone: "trouble", label: "The skill could not be opened", text: answer.error });
      // The person went elsewhere while the text was read: the editor is not opened there.
      else if (samePlace(placeNow.current, asked)) {
        const place = { ...placeNow.current, tab: "text" as const };
        keepSkillEdit(tab, { at: place, skill, level, was: answer.data, now: answer.data });
        walked.replace(place);
      }
    });
  };
  /** **Stop editing**: the editor closes, once asked where its text is unsaved. */
  const stop = () => leave(() => undefined);

  const editor =
    here === undefined ? undefined : (
      <Suspense
        fallback={
          <p className="pending" aria-busy="true">
            Opening the editor…
          </p>
        }
      >
        <SkillEditor
          name={here.skill.name}
          was={here.was}
          text={here.now}
          lint={lint}
          saving={busy}
          onChange={(now) => {
            const was = skillEditIn(tab);
            if (was !== undefined) keepSkillEdit(tab, { ...was, now });
          }}
          onSave={() => save()}
          onDiscard={() => keepSkillEdit(tab, undefined)}
        />
      </Suspense>
    );

  // A close of the tab asked while the editor held unsaved text (`askBeforeClosing`).
  const asking: Leaving | undefined =
    leaving ?? (closeAsked === undefined ? undefined : { then: closeAsked, closing: true });
  const answered = () => {
    setLeaving(undefined);
    if (asking?.closing === true) closeAnswered(tab);
  };
  const question =
    asking === undefined || edit === undefined || !unsaved(edit) ? undefined : (
      <LeaveQuestion
        name={edit.skill.name}
        blocked={blocked !== undefined}
        busy={busy}
        onKeep={() => {
          // Back first: a step drawn before the answer is cleared still finds it asked.
          if (asking.back !== undefined) walked.go(asking.back);
          answered();
        }}
        onDiscard={() => {
          answered();
          keepSkillEdit(tab, undefined);
          asking.then();
        }}
        onSave={() => {
          answered();
          save(asking.then);
        }}
      />
    );

  return {
    /** `walked`, with each step that leaves the editor's page asked about first. */
    history,
    leave,
    /** Goes on with `then` where `to` is the editor's page still, else leaves the editor first. */
    leaveFor: stepTo,
    /** The editor open on the page the tab is at. */
    editing: here,
    open,
    stop,
    editor,
    /** The question a leave asks, while it is asked. */
    question,
  };
}

/**
 * **The question a leave asks** while the editor holds unsaved text (S4): Keep editing, Discard
 * and leave, or Save and leave, which waits while what the lint found blocks the save.
 */
export function LeaveQuestion({
  name,
  blocked,
  busy,
  onKeep,
  onDiscard,
  onSave,
}: {
  name: string;
  /** Whether the lint finds what keeps the text from being saved. */
  blocked: boolean;
  busy: boolean;
  onKeep: () => void;
  onDiscard: () => void;
  onSave: () => void;
}) {
  return (
    <AlertDialog
      onClose={onKeep}
      title={`Save your changes to ${name}?`}
      titleId="skill-leave"
      description={
        blocked
          ? "You changed its text and haven't saved it, and it can't be saved as it is: the check found what stops it from loading. If you leave now, the changes are lost."
          : "You changed its text and haven't saved it. If you leave now, the changes are lost."
      }
      cancel={{ children: "Keep editing" }}
      alternative={{
        children: "Discard and leave",
        variant: "danger",
        isDisabled: busy,
        onPress: onDiscard,
      }}
      main={{ children: "Save and leave", isDisabled: busy || blocked, onPress: onSave }}
    />
  );
}
