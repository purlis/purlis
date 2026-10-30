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
 * Delete, New skill). A place reached from outside the tab (a run's chip) is asked about once it
 * is reached: Keep editing goes back. Every close that would lose the text (the tab's, the
 * project's, a quit) asks the same question through the window's registry (`unsavedWork.ts`).
 *
 * Text with nothing changed is left without a word.
 */
import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { commands, type SkillLevel, type SkillLintFinding, type StudioSkill } from "./bindings";
import { LeaveQuestion } from "./UnsavedWorkQuestion";
import type { ViewHistory } from "./ui/panel/viewHistory";
import { samePlace, type StudioPlace } from "./skillStudio";
import {
  SAVED,
  keepSkillEdit,
  saveSkillEdit,
  skillEditIn,
  unsaved,
  useSkillEdit,
  useSkillSaved,
  type SkillEdit,
} from "./skillEdit";
import type { ViewRef } from "./tabs";
import type { Said } from "./SkillPage";

/** The light editor, its own chunk (`tools/main-chunk-has-no-codemirror.mjs`). */
const SkillEditor = lazy(() => import("./SkillEditor"));

/**
 * **The core's lint of the text typed** (`skill_lint`), asked a moment after each edit while
 * `edit` is open: what it found, and whether that was found in the text as it is now. Until it
 * has, nothing may be saved on its word, so a text drawn afresh is never saved unchecked.
 */
function useSkillLint(
  plane: string,
  edit: SkillEdit | undefined,
): { findings: readonly SkillLintFinding[]; checked: boolean } {
  const [lint, setLint] = useState<{ of: string; findings: readonly SkillLintFinding[] }>({
    of: "",
    findings: [],
  });
  const level = edit?.level;
  const name = edit?.skill.name;
  const text = edit?.now;
  const of = `${name ?? ""}\u0000${text ?? ""}`;
  useEffect(() => {
    if (level === undefined || name === undefined || text === undefined) return;
    let live = true;
    const asked = setTimeout(() => {
      void commands
        .skillLint(plane, level, name, text)
        .then((answer) => {
          if (live && answer.status === "ok") setLint({ of, findings: answer.data });
        })
        .catch(() => undefined);
    }, 200);
    return () => {
      live = false;
      clearTimeout(asked);
    };
    // Each edit keeps the level it opened with: a keystroke never asks again for it alone.
  }, [plane, level, name, text, of]);
  // Another skill's findings are never shown as this one's.
  const findings = lint.of.startsWith(`${name ?? ""}\u0000`) ? lint.findings : [];
  return { findings, checked: lint.of === of };
}

/** What is left, once the person has answered: a step, or nothing more. */
type Leaving = { then: () => void; back?: StudioPlace };

/**
 * **The studio's editor in place, and its guard** for the studio tab whose history is `tab`
 * (`studioHistoryOf`) and whose view is `view`, walked by `walked`. Its `history` is `walked`
 * with every step that leaves the editor's page asked about first; `leave` does the same for
 * anything else that leaves it.
 */
export function useSkillEditInPlace({
  plane,
  tab,
  view,
  walked,
  busy,
  setBusy,
  say,
  onSaved,
}: {
  plane: string;
  tab: string;
  view: ViewRef;
  walked: ViewHistory<StudioPlace>;
  busy: boolean;
  setBusy: (busy: boolean) => void;
  say: (said: Said | undefined) => void;
  /** A save was made: what the studio reads is read again. */
  onSaved: () => void;
}) {
  const edit = useSkillEdit(tab);
  const lint = useSkillLint(plane, edit);
  const blocked = lint.findings.find((finding) => finding.level === "blocks");
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

  // A place reached from outside the tab (an entry point): an editor with nothing unsaved is
  // left behind; one with unsaved text is asked about below, and Keep editing goes back to it.
  const away = edit !== undefined && !samePlace(edit.at, at);
  useEffect(() => {
    if (away && !unsaved(edit)) keepSkillEdit(tab, undefined);
  }, [away, edit, tab]);
  // The edit whose leave from afar was answered with Save and leave, while it saves.
  const [savingAway, setSavingAway] = useState<SkillEdit>();

  /** Saves the text typed, says so, and goes on with `then`; or says why it was not saved and
   *  keeps the editor as it is. */
  const save = (then?: () => void) => {
    if (skillEditIn(tab) === undefined) return;
    setBusy(true);
    void saveSkillEdit(tab).then((refused) => {
      setBusy(false);
      if (refused === undefined) return then?.();
      // Not saved: a leave from afar is asked again, so the person can go back to it.
      setSavingAway(undefined);
      say({ tone: "trouble", label: "The skill was not saved", text: refused });
      onSaved();
    });
  };
  // **A save, whichever way it was asked** (the studio's Save, or the window's question about a
  // close, #3929): said here, and what the studio reads is read again. A save made while the
  // studio was not drawn is not said: the studio drawn afresh reads it anyway.
  const saved = useSkillSaved(tab);
  const heard = useRef(saved);
  useEffect(() => {
    if (saved === undefined || heard.current === saved) return;
    heard.current = saved;
    say({ tone: "news", label: "Skill saved", text: [SAVED, ...saved.notes].join(" ") });
    onSaved();
  }, [saved, say, onSaved]);

  /** **Edit**: the skill's text, read now, in the editor on its page's Text tab, if the tab is
   *  still at `asked` when it comes back: where it is now, or where a step just taken goes. */
  const open = (skill: StudioSkill, level: SkillLevel, asked: StudioPlace = placeNow.current) => {
    void commands.skillText(plane, level, skill.name).then((answer) => {
      if (answer.status === "error")
        say({ tone: "trouble", label: "The skill could not be opened", text: answer.error });
      // The person went elsewhere while the text was read: the editor is not opened there.
      else if (samePlace(placeNow.current, asked)) {
        const place = { ...placeNow.current, tab: "text" as const };
        keepSkillEdit(tab, {
          plane,
          view,
          at: place,
          skill,
          level,
          was: answer.data,
          now: answer.data,
        });
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
          lint={lint.findings}
          checking={!lint.checked}
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

  const asking: Leaving | undefined =
    leaving ?? (away && edit !== savingAway ? { then: () => undefined, back: edit.at } : undefined);
  const answered = () => setLeaving(undefined);
  const question =
    asking === undefined || edit === undefined || !unsaved(edit) ? undefined : (
      <LeaveQuestion
        names={[edit.skill.name]}
        blocked={blocked !== undefined}
        busy={busy}
        checking={!lint.checked}
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
          if (asking.back !== undefined) setSavingAway(edit);
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
    /** Whether that editor holds text not saved yet. */
    unsaved: unsaved(here),
    open,
    stop,
    editor,
    /** The question a leave asks, while it is asked. */
    question,
    /** Whether the tab is at `place` now, for an answer that comes back after the person may have
     *  moved on: the tab's one place ref (#3973). */
    isAt: (place: StudioPlace) => samePlace(placeNow.current, place),
  };
}
