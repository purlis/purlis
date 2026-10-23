// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import * as AlertDialog from "./overlays/alert-dialog";
import { CircleAlert, CircleCheck, CircleDot, LoaderCircle, Save } from "lucide-react";
import {
  commands,
  type PlaneId,
  type PlaneSaving,
  type ProjectUpdate,
  type RepoSaving,
  type SaveEntry,
} from "./bindings";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { StatusItem } from "./ui/status/StatusItem";
import {
  askWayOut,
  behindness,
  capitalised,
  repoSavable,
  repoSaveGoesTo,
  repoStageText,
  requestIsRed,
  requestOf,
  requestWords,
  saveAll,
  takeUpdateAsk,
  tellSaved,
  UPDATE_ASKED,
  usePlaneSaving,
  useRepoSaving,
} from "./saving";
import { Choice, Field, SettingActions, SettingRow, type Option } from "./settings/components";
import { useReadOnlySaid } from "./ReadOnlyLine";
import { askSettingsLink, type SettingsLink } from "./settings/links";
import { Notice } from "./Notice";
import { AnswerBar } from "./AnswerBar";
import { RemoteQuestionNotice } from "./RemoteQuestion";
import { SaveBlockedBySecret } from "./SaveBlockedBySecret";
import { updateSaid, UpdateFromRemote, UpdateLeftNotice } from "./SavingUpdate";
import { RequestMerge } from "./SavingMerge";
import { SaveConflict } from "./SavingResolve";
import { Button, IconButton } from "./ui/button/Button";
import { Tooltip } from "./ui/tooltip/Tooltip";

/** Settings › Saving › Mode: where a project's save mode is changed (SE-22's address). */
const SAVING_MODE: SettingsLink = { group: "project.saving", setting: "project.saving.plane.mode" };

/**
 * **The Saving view** (charter-app#294, ADR 0051): where this plane's unsaved work sits, what
 * the next save takes, how far a save goes, a save button, and the last saves.
 *
 * Everything on it is the core's answer: the stage is `planegit::standing`, read from git and the
 * push record; the save is `planegit::save_as` — the function `charter save` runs — so the button
 * and the command cannot disagree about what a save does. After a save the view reads the plane
 * again rather than guessing what changed.
 */
export function SavingView({
  plane,
  workspace,
  onSaved,
}: {
  plane: PlaneId;
  /** The workspace whose repos get a row each (charter-app#299); none outside every one. */
  workspace?: string;
  onSaved?: () => void;
}) {
  // The title bar's reader: fresh on focus, on plane changes, on a timer and after any save.
  const { saving } = usePlaneSaving(plane);
  const repos = useRepoSaving(plane, workspace);
  /** Why the last save was refused — kept across the reads that follow it. */
  const [refused, setRefused] = useState<string | null>(null);
  const [said, setSaid] = useState<string[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  /** Why the project is read-only, when it is (#826): its own Save is off with the sentence,
   *  and Save all saves the repos only. A repo is a clone, and a clone is still written. */
  const readOnly = useReadOnlySaid();
  /** Whether the project's own save could do anything here. */
  const planeSavable = (now: PlaneSaving) => readOnly === undefined && savable(now);

  const save = async () => {
    setBusy(true);
    setSaid(null);
    setRefused(null);
    const typed = message.trim();
    try {
      const got = await commands.savePlane(plane, typed === "" ? null : typed);
      if (got.status === "ok") {
        setSaid(got.data);
        setMessage("");
        onSaved?.();
      } else {
        setRefused(got.error);
      }
    } catch (err: unknown) {
      setRefused(String(err));
    } finally {
      setBusy(false);
      tellSaved();
    }
  };

  /** The last Update from remote that left the project as it was (qw348): its Notice, until it
   *  is dismissed or the next save or update is pressed. */
  const [left, setLeft] = useState<ProjectUpdate | null>(null);

  /** Bring the incoming commits in, after `first` where it is given (the save of *Save, then
   *  update*): what `first` refuses is said, and nothing is then updated. The update's own
   *  answer is the core's, which leaves the project at the new commit or exactly as it was. */
  const update = async (first?: () => Promise<string | null>) => {
    setBusy(true);
    setSaid(null);
    setRefused(null);
    setLeft(null);
    try {
      const refusal = first === undefined ? null : await first();
      if (refusal !== null) {
        setRefused(refusal);
        return;
      }
      const got = await commands.updateProject(plane);
      if (got.status === "error") setRefused(got.error);
      else if (got.data.outcome === "left") setLeft(got.data);
      else {
        setSaid([...got.data.lines, got.data.said]);
        onSaved?.();
      }
    } catch (err: unknown) {
      setRefused(String(err));
    } finally {
      setBusy(false);
      tellSaved();
    }
  };
  /** *Save, then update*: the project's own save with the message typed, then the update. */
  const saveThenUpdate = () =>
    update(async () => {
      const typed = message.trim();
      const got = await commands.savePlane(plane, typed === "" ? null : typed);
      if (got.status === "error") return got.error;
      setMessage("");
      return null;
    });

  /** The title bar's `↓N` asked for an update (`askUpdate`): run here, once, where its
   *  progress and result are said. An ask that arrives while something runs is dropped. */
  const asked = useRef<() => void>(() => undefined);
  useEffect(() => {
    asked.current = () => {
      if (!takeUpdateAsk(plane) || busy) return;
      void update();
    };
  });
  useEffect(() => {
    const hear = (event: Event) => {
      if ((event as CustomEvent<{ plane: string }>).detail.plane === plane) asked.current();
    };
    window.addEventListener(UPDATE_ASKED, hear);
    asked.current();
    return () => window.removeEventListener(UPDATE_ASKED, hear);
  }, [plane]);

  /** Save all's confirmation, while it is open: every tree it would save, as read when Save all
   *  was pressed. Held still, so what is saved is exactly what the operator read — the rereads
   *  that keep the view fresh never add a repo to a list already on screen. */
  const [confirming, setConfirming] = useState<{
    plane: PlaneSaving | null;
    repos: RepoSaving[];
  }>();

  /** Save all: the plane and every repo with something to take, one after another. Only ever
   *  run from its confirmation (ADR 0051, amended 2026-09-25). */
  const saveEverything = async (what: { plane: PlaneSaving | null; repos: RepoSaving[] }) => {
    setConfirming(undefined);
    setBusy(true);
    setSaid(null);
    setRefused(null);
    const typed = message.trim();
    try {
      const got = await saveAll(
        plane,
        what.plane !== null,
        typed === "" ? null : typed,
        workspace,
        what.repos,
      );
      if (got.said.length > 0) setSaid(got.said);
      if (got.refused.length > 0) setRefused(got.refused.join("\n"));
      else {
        setMessage("");
        onSaved?.();
      }
    } finally {
      setBusy(false);
      tellSaved();
    }
  };

  /** One repo's own Save. The message box is the plane's; a repo's commit says its files. */
  const saveOne = async (name: string) => {
    if (workspace === undefined) return;
    setBusy(true);
    setSaid(null);
    setRefused(null);
    try {
      const got = await commands.saveRepo(plane, workspace, name, null);
      if (got.status === "ok") setSaid(got.data);
      else setRefused(got.error);
    } catch (err: unknown) {
      setRefused(String(err));
    } finally {
      setBusy(false);
      tellSaved();
    }
  };

  /** The person's answer to the question about the project's remote, or a repo's (#1848,
   *  #1852): record `shown`, the remote the Notice named, or keep the recorded one (`null`);
   *  then save as Save does. */
  const answerRemote = async (repo: string | null, shown: string | null) => {
    setBusy(true);
    setSaid(null);
    setRefused(null);
    const from = repo === null ? null : (workspace ?? null);
    try {
      const got = await (shown === null
        ? commands.keepRecordedRemote(plane, from, repo)
        : commands.confirmRemote(plane, from, repo, shown));
      if (got.status === "ok") {
        setSaid(got.data);
        onSaved?.();
      } else setRefused(got.error);
    } catch (err: unknown) {
      setRefused(String(err));
    } finally {
      setBusy(false);
      tellSaved();
    }
  };

  const anyToSave =
    saving !== undefined && (planeSavable(saving) || (repos ?? []).some(repoSavable));

  return (
    <div className="saving" data-testid="saving-view">
      {saving === undefined ? (
        <p className="pending" aria-busy="true">
          <LoaderCircle className="node-icon spinning" aria-hidden="true" />
          Reading the project
        </p>
      ) : (
        <>
          <p className={`saving-stage saving-${saving.stage}`}>{stageText(saving)}</p>
          <p className="saving-note">{modeText(saving)}</p>
          {saving.live.length > 0 && (
            <p className="saving-note">
              {`Live workspaces, published by every save: ${saving.live.join(", ")}`}
            </p>
          )}
          {/* A save blocked by what looks like a credential says where and why, and opens the
              file there: a block is never a line with nothing to press. */}
          {saving.stage === "blocked" && (saving.secrets ?? []).length > 0 && (
            <SaveBlockedBySecret
              plane={plane}
              secrets={saving.secrets ?? []}
              busy={busy}
              onSaveAgain={() => void save()}
            />
          )}
          {(saving.behind ?? 0) > 0 && (
            <UpdateFromRemote
              incoming={saving.behind ?? 0}
              busy={busy}
              saveFirst={saving.changed.length > 0 && planeSavable(saving)}
              onUpdate={() => void update()}
              onSaveThenUpdate={() => void saveThenUpdate()}
            />
          )}
          {left !== null && (
            <UpdateLeftNotice
              left={left}
              busy={busy}
              canSave={planeSavable(saving)}
              onUpdate={() => void update()}
              onSaveThenUpdate={() => void saveThenUpdate()}
              onTerminal={() => askWayOut(plane, "terminal")}
              onDismiss={() => setLeft(null)}
            />
          )}
          {saving.stage === "blocked" && (
            /* A save's replay that conflicted, or the window's own replay stopped on one
               (qw382): Resolve, Continue and Abort are the window's, and a chat only edits. */
            <SaveConflict plane={plane} saving={saving} onSaved={onSaved} />
          )}
          {saving.stage === "blocked" && saving.byHand == null && saving.stoppedBy !== "rebase" && (
            <div className="saving-question" role="group" aria-label="Ways out">
              {saving.conflicts.length > 0 && (
                <ul className="saving-files" aria-label="Where it conflicts">
                  {saving.conflicts.map((path) => (
                    <li key={path}>{path}</li>
                  ))}
                </ul>
              )}
              <SettingActions>
                {/* A rebase, `git am`, cherry-pick, revert or bisect git stopped part-way is
                    finished with git, which a chat may not run in the project's root (qw382). */}
                {chatMaySettle(saving) && (
                  <Button size="sm" onPress={() => askWayOut(plane, "chat")} className="panel-view">
                    Resolve in a chat
                  </Button>
                )}
                <Button
                  size="sm"
                  onPress={() => askWayOut(plane, "terminal")}
                  className="panel-view"
                >
                  Open terminal here
                </Button>
              </SettingActions>
            </div>
          )}
          {saving.notice !== null && (
            /* A request mode on an origin no forge charter knows (NO-7, #1232): the mode it
               complains about is changed in Settings › Saving, so that is where it links. */
            <Notice
              cause="saving-no-forge"
              link={{
                label: "Change the mode",
                onPress: () => askSettingsLink(plane, SAVING_MODE),
              }}
            >
              {saving.notice}
            </Notice>
          )}
          {saving.remote != null && (
            <RemoteQuestionNotice
              question={saving.remote}
              name="this project"
              busy={busy}
              onConfirm={(shown) => void answerRemote(null, shown)}
              onKeep={() => void answerRemote(null, null)}
            />
          )}
          {(repos ?? []).map(
            (repo) =>
              repo.remote != null && (
                <RemoteQuestionNotice
                  key={repo.name}
                  question={repo.remote}
                  name={repo.name}
                  busy={busy}
                  onConfirm={(shown) => void answerRemote(repo.name, shown)}
                  onKeep={() => void answerRemote(repo.name, null)}
                />
              ),
          )}
          {saving.pushFailed !== null && (
            <p className="saving-note">{`The last push did not land: ${saving.pushFailed}`}</p>
          )}
          {saving.mode === null && <ModeQuestion plane={plane} branch={saving.branch} />}
          {saving.pr !== null && (
            <p className="saving-note">
              {`${capitalised(saving.request)}: `}
              <a href={saving.pr} target="_blank" rel="noreferrer">
                {saving.pr}
              </a>
            </p>
          )}
          {saving.pr !== null && saving.merge != null && (
            <RequestMerge
              plane={plane}
              workspace={null}
              repo={null}
              name="this project"
              request={saving.request}
              pr={saving.pr}
              target={saving.branch}
              merge={saving.merge}
            />
          )}
          {/* A repo's row says where its request stands; what is wrong with one, or what its
              wait needs of the person, is said in full here, as the project's is. */}
          {(repos ?? []).map(
            (repo) =>
              workspace !== undefined &&
              repo.pr !== null &&
              repo.merge != null &&
              (repo.merge.state === "red" ||
                repo.merge.state === "cannot-merge" ||
                repo.merge.needs != null) && (
                <RequestMerge
                  key={repo.name}
                  plane={plane}
                  workspace={workspace}
                  repo={repo.name}
                  name={repo.name}
                  request={repo.request}
                  pr={repo.pr}
                  target={repo.target}
                  merge={repo.merge}
                  needsSave={!repo.autosave}
                />
              ),
          )}
          {saving.changed.length > 0 && (
            <ul className="saving-files" aria-label="What the next save takes">
              {saving.changed.map((path) => (
                <li key={path}>{path}</li>
              ))}
            </ul>
          )}
          <SettingRow
            label="Message"
            help="Leave empty for one that says what changed"
            control={(ids) => (
              <Field ids={ids} kind="text" value={message} onChange={setMessage} disabled={busy} />
            )}
          />
          <SettingActions>
            <Button
              size="sm"
              onPress={() => void save()}
              isDisabled={busy || !planeSavable(saving)}
              tooltip={readOnly}
              className="panel-view"
            >
              {busy ? (
                <LoaderCircle className="node-icon spinning" aria-hidden="true" />
              ) : (
                <Save className="node-icon" aria-hidden="true" />
              )}
              Save
            </Button>
            {workspace !== undefined && (
              <Button
                size="sm"
                onPress={() =>
                  setConfirming({
                    plane: planeSavable(saving) ? saving : null,
                    repos: (repos ?? []).filter(repoSavable),
                  })
                }
                isDisabled={busy || !anyToSave}
                icon={<Save className="node-icon" aria-hidden="true" />}
                className="panel-view"
              >
                Save all
              </Button>
            )}
          </SettingActions>
          {confirming !== undefined && workspace !== undefined && (
            <ConfirmSaveAll
              plane={confirming.plane}
              workspace={workspace}
              repos={confirming.repos}
              onSave={() => void saveEverything(confirming)}
              onCancel={() => setConfirming(undefined)}
            />
          )}
          {workspace !== undefined && repos !== undefined && repos.length > 0 && (
            <RepoRows
              workspace={workspace}
              repos={repos}
              busy={busy}
              onSave={(name) => void saveOne(name)}
            />
          )}
          {refused !== null && (
            <p className="trouble" role="alert">
              {refused}
            </p>
          )}
          {said !== null && (
            <pre className="saving-said" role="status">
              {said.join("\n")}
            </pre>
          )}
          <h3 className="saving-recent">Recent saves</h3>
          {saving.journal.length === 0 ? (
            <EmptyState
              variant="panel"
              headline="No saves recorded yet"
              body="Each save is listed here, the newest first, with what started it and how it ended."
              testid="saving-empty"
            />
          ) : (
            <ul className="saving-journal" aria-label="Recent saves">
              {saving.journal.map((one, n) => (
                <li key={`${one.at}-${n}`}>{entryText(one)}</li>
              ))}
            </ul>
          )}
        </>
      )}
    </div>
  );
}

/**
 * **One row per repo in the workspace** (charter-app#299): its stage, the branch it is on, the
 * request it waits on, and its own Save. A table for the reason the bottom bar's is one:
 * "which of these is unsaved" is a column question.
 */
function RepoRows({
  workspace,
  repos,
  busy,
  onSave,
}: {
  workspace: string;
  repos: readonly RepoSaving[];
  busy: boolean;
  onSave: (name: string) => void;
}) {
  return (
    <table className="ui-table saving-repos" aria-label={`Repos in ${workspace}`}>
      <thead>
        <tr>
          <th scope="col">Repo</th>
          <th scope="col">Branch</th>
          <th scope="col">Stage</th>
          <th scope="col">{capitalised(requestOf(repos))}</th>
          <th scope="col">Mode</th>
          <th scope="col">Save goes to</th>
          <th scope="col">
            <span className="sr-only">Save</span>
          </th>
        </tr>
      </thead>
      <tbody>
        {repos.map((repo) => (
          <tr key={repo.name} data-repo={repo.name} data-stage={repo.stage}>
            <td>{repo.name}</td>
            <td>{repo.branch ?? "no branch"}</td>
            <td>{repoStageText(repo)}</td>
            <td>
              {repo.pr === null ? (
                "—"
              ) : (
                <a href={repo.pr} target="_blank" rel="noreferrer">
                  {repo.pr}
                </a>
              )}
            </td>
            <td>{`${repo.mode} (${repo.modeFrom})${repo.autosave ? " · auto-save" : ""}`}</td>
            <td data-testid="save-goes-to">{repoSaveGoesTo(repo, workspace)}</td>
            <td>
              <IconButton
                size="sm"
                aria-label={`Save ${repo.name}`}
                tooltip={`Save ${repo.name}: ${repoSaveGoesTo(repo, workspace)}`}
                isDisabled={busy || !repoSavable(repo)}
                onPress={() => onSave(repo.name)}
                icon={<Save className="node-icon" aria-hidden="true" />}
                className="panel-view"
              />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/**
 * **Save all's confirmation** (ADR 0051, amended 2026-09-25): every tree it would save, each
 * repo with the branch it is on, what it would take, and where its save goes — because a repo
 * is a developer's, and a save commits every file changed in it and may push it. Nothing is
 * saved until the operator says so here.
 *
 * Radix's `AlertDialog`, as `DeleteWorkspace` is: an action that reaches past this machine is
 * one the operator confirms, and focus starts on Cancel.
 */
function ConfirmSaveAll({
  plane,
  workspace,
  repos,
  onSave,
  onCancel,
}: {
  /** The project's standing, when it has something to save; else `null`. */
  plane: PlaneSaving | null;
  workspace: string;
  repos: readonly RepoSaving[];
  onSave: () => void;
  onCancel: () => void;
}) {
  const cancel = useRef<HTMLButtonElement>(null);
  const count = (plane === null ? 0 : 1) + repos.length;
  const reposSaid = `${repos.length} ${repos.length === 1 ? "repo" : "repos"}`;
  const title =
    plane === null
      ? `Save ${reposSaid}?`
      : repos.length === 0
        ? "Save the project?"
        : `Save the project and ${reposSaid}?`;
  return (
    <AlertDialog.Root
      open
      onOpenChange={(open) => {
        if (!open) onCancel();
      }}
    >
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="ui-scrim" />
        <AlertDialog.Content
          className="ui-dialog warning"
          onOpenAutoFocus={(event) => {
            event.preventDefault();
            cancel.current?.focus();
          }}
        >
          <AlertDialog.Title>{title}</AlertDialog.Title>
          <AlertDialog.Description className="came-back">
            A repo&apos;s save commits every file changed in it on the branch it is on, then goes as
            far as its mode says.
          </AlertDialog.Description>
          <ul className="saving-files" aria-label="What Save all saves">
            {plane !== null && <li>{`The project — ${planeTakes(plane)}`}</li>}
            {repos.map((repo) => (
              <li key={repo.name}>
                {`${repo.name} on ${repo.branch ?? "no branch"} — ${repoTakes(repo)} — ${repoSaveGoesTo(repo, workspace)}`}
              </li>
            ))}
          </ul>
          <AnswerBar>
            <AlertDialog.Cancel asChild>
              <Button ref={cancel}>Cancel</Button>
            </AlertDialog.Cancel>
            {/* `ends-it`: a save commits, and goes as far as each repo's mode says, a push
                included, and purlis has no Undo for it (D-1719-3, `docs/ui-primitives.md`). */}
            <Button variant="danger" onPress={onSave}>
              {`Save all ${count}`}
            </Button>
          </AnswerBar>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}

/** "1 file changed", "3 files changed". */
function filesSaid(n: number): string {
  return `${n} ${n === 1 ? "file" : "files"} changed`;
}

/** What the project's save would take: its changed files, else the commits a push would carry,
 *  else the blocked save it tries again. */
function planeTakes(plane: PlaneSaving): string {
  if (plane.changed.length > 0) return filesSaid(plane.changed.length);
  if (plane.stage === "blocked") return "trying its blocked save again";
  if (plane.ahead === null) return "a branch never pushed";
  return `${plane.ahead} ${plane.ahead === 1 ? "commit" : "commits"} not pushed`;
}

/** What a repo's save would take: its changed files, else the commits a push would carry. */
function repoTakes(repo: RepoSaving): string {
  if (repo.changed > 0) return filesSaid(repo.changed);
  if (repo.stage === "blocked") return "trying its blocked save again";
  if (repo.ahead === null) return "a branch never pushed";
  return `${repo.ahead} ${repo.ahead === 1 ? "commit" : "commits"} not pushed`;
}

/**
 * **The question a project with no mode is asked once** (ADR 0051): how far its saves go. The
 * answer is written as `[plane] mode` in `charter.toml` by the core's own writer, and until
 * there is one, nothing saves the project by itself. The pull request modes are set in Project
 * settings; this asks the three a person can answer without knowing the repository's rules.
 *
 * Drawn as a setting is (DS-3b, #1174): a row whose control is a radio choice, each answer with
 * the line on what it does. The answer is written only when **Use this** confirms it: a radio's
 * pick follows the arrow keys, and a mode — `push` sends every save to the team — is not one to
 * set by moving through the answers.
 */
function ModeQuestion({ plane, branch }: { plane: PlaneId; branch: string }) {
  const [refused, setRefused] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  /** The answer picked and not yet written. Radix moves the pick with the arrow keys, so a
   *  pick is only ever held here: the mode is written by **Use this**, never by the pick. */
  const [picked, setPicked] = useState<string>();
  const choose = async (mode: string) => {
    setBusy(true);
    setRefused(null);
    try {
      const got = await commands.choosePlaneMode(plane, mode);
      if (got.status === "error") setRefused(got.error);
    } catch (err: unknown) {
      setRefused(String(err));
    } finally {
      setBusy(false);
      tellSaved();
    }
  };
  const options: Option[] = [
    {
      value: "push",
      label: `Push to ${branch || "the remote"}`,
      says: "Every save is committed and pushed, so the team has it.",
    },
    {
      value: "commit",
      label: "Commit only",
      says: "Saves stay on this machine until you push them yourself.",
    },
    { value: "off", label: "Off", says: "purlis commits nothing here; you use git yourself." },
  ];
  return (
    <>
      <SettingRow
        label="How should this project be saved?"
        grouped
        control={(ids) => (
          <Choice
            ids={ids}
            kind="radio"
            options={options}
            value={picked}
            onValueChange={setPicked}
          />
        )}
      />
      <SettingActions>
        <Button
          variant="primary"
          isDisabled={busy || picked === undefined}
          onPress={() => {
            if (picked !== undefined) void choose(picked);
          }}
        >
          Use this
        </Button>
      </SettingActions>
      {refused !== null && (
        <p className="trouble" role="alert">
          {refused}
        </p>
      )}
    </>
  );
}

/**
 * **The title bar's save indicator** (charter-app#294): the stage of the project in front, as
 * a button that opens its Saving tab, and — while there is anything to take — a save button
 * beside it. Both are `<button tabIndex={0}>`: the tag is what the drag handler stops at, and
 * the tab index is what WebKit's sequence needs (`TitleBar.tsx`).
 */
export function SaveIndicator({
  saving,
  repos,
  busy,
  onOpen,
  onSave,
  onUpdate,
  off,
}: {
  saving: PlaneSaving;
  /** The active workspace's repos (charter-app#299): the indicator shows the furthest-back
   *  stage across them and the plane. Its save is the project's only — a repo is saved from
   *  the Saving tab, where it says where the save goes (ADR 0051, amended 2026-09-25). */
  repos?: readonly RepoSaving[];
  busy: boolean;
  onOpen: () => void;
  onSave: () => void;
  /** Update from remote (qw348): what pressing `↓N` does. With it, the count is a button of
   *  its own beside the stage; without it, the count is part of the stage's button, as before. */
  onUpdate?: () => void;
  /** Why the save is off, when it is: a read-only project's sentence (#826), since a commit
   *  changes the committed tree and the core refuses it. Said as the button's tooltip. */
  off?: string;
}) {
  const { stage, said, words, incoming, red } = furthestBack(saving, repos ?? []);
  // A red request is not a block, and it is still something wrong the bar must not draw as
  // "waiting": the mark is the alert, in the failed colour (ADR 0051, amended 2026-10-13).
  const Mark =
    stage === "blocked" || red ? CircleAlert : stage === "saved" ? CircleCheck : CircleDot;
  // Words stay the pill's; only the mark takes the stage's colour.
  const tone = stage === "blocked" || red ? "failed" : stage === "saved" ? "success" : "waiting";
  return (
    <span className="save-indicator" data-stage={stage} data-checks={red ? "failed" : undefined}>
      <StatusItem
        variant="pill"
        tone={tone}
        aria-label={`Saving: ${said}`}
        tooltip={said}
        onPress={onOpen}
        data-testid="save-indicator-where"
        icon={<Mark aria-hidden="true" />}
      >
        <span className="save-indicator-words">{words}</span>
        {/* What came in, as its own element the words' cap never cuts (charter#403). Its
            meaning is in the button's name and title, which say "N incoming" in full. */}
        {incoming > 0 && onUpdate === undefined && (
          <span className="save-indicator-incoming" aria-hidden="true">
            ↓{incoming}
          </span>
        )}
      </StatusItem>
      {/* The count as the Update button (qw348): pressing it brings the incoming commits in,
          from the Saving tab, where the progress and the result are said. */}
      {incoming > 0 && onUpdate !== undefined && (
        <StatusItem
          variant="pill"
          aria-label={updateSaid(incoming)}
          tooltip={updateSaid(incoming)}
          isDisabled={busy}
          onPress={onUpdate}
          className="save-indicator-incoming"
        >
          ↓{incoming}
        </StatusItem>
      )}
      {savable(saving) && (
        <StatusItem
          variant="pill"
          aria-label="Save the project"
          tooltip={off ?? "Save the project"}
          isDisabled={busy || off !== undefined}
          onPress={onSave}
          data-testid="save-indicator-save"
          icon={
            busy ? (
              <LoaderCircle className="spinning" aria-hidden="true" />
            ) : (
              <Save aria-hidden="true" />
            )
          }
        />
      )}
    </span>
  );
}

/**
 * The furthest-back stage across the plane and its repos (ADR 0051), and how the title bar
 * says it: the plane's own words when the plane is that far back, else how many repos are.
 * A repo charter never saves (`off`) is not counted.
 *
 * `words` is the stage alone and `incoming` how many commits came in and were not pulled, so
 * the bar can draw the count as its own `↓N` beside words it caps (charter#403, ADR 0054).
 * `said` is both together, for the button's name and title.
 *
 * `red` is whether the stage shown is a pushed save whose request is red: a check failed on
 * it, and purlis did not merge it (ADR 0051, amended 2026-10-13).
 */
export function furthestBack(
  saving: PlaneSaving,
  repos: readonly RepoSaving[],
): { stage: string; said: string; words: string; incoming: number; red: boolean } {
  const incoming = saving.behind ?? 0;
  const counted = repos.filter((repo) => repo.stage !== "off");
  const stage = counted.reduce(
    (far, repo) => (behindness(repo.stage) < behindness(far) ? repo.stage : far),
    saving.stage,
  );
  if (behindness(saving.stage) <= behindness(stage)) {
    const words = stageWords(saving);
    const red = saving.stage === "pr-open" && requestIsRed(saving);
    return { stage, said: withIncoming(words, incoming), words, incoming, red };
  }
  const waiting = counted.filter((repo) => repo.stage === stage);
  const reds = stage === "pr-open" ? waiting.filter(requestIsRed) : [];
  if (reds.length > 0) {
    const noun = requestOf(reds);
    const words =
      reds.length === 1 ? `1 repo's ${noun} is red` : `${reds.length} repos' ${noun}s are red`;
    return { stage, said: withIncoming(words, incoming), words, incoming, red: true };
  }
  const n = waiting.length;
  const phrases: Record<string, [string, string]> = {
    blocked: ["blocked", "blocked"],
    changed: ["changed", "changed"],
    committed: ["committed, not pushed", "committed, not pushed"],
    "pr-open": [`waiting on its ${requestOf(waiting)}`, `waiting on their ${requestOf(waiting)}s`],
  };
  const [one, many] = phrases[stage] ?? [stage, stage];
  const words = n === 1 ? `1 repo ${one}` : `${n} repos ${many}`;
  return { stage, said: withIncoming(words, incoming), words, incoming, red: false };
}

/** What git stopped part-way that a chat may not finish in the project's root: continuing
 *  any of these rewrites the tree, which its guard refuses there (A3c). A merge's
 *  `--continue`, and unmerged files with no operation, are a chat's to finish. */
const ONLY_GIT_FINISHES = new Set(["rebase", "am", "cherry-pick", "revert", "bisect"]);

/** Whether a blocked save offers *Resolve in a chat*: never for a save's own replay that
 *  conflicted, nor for a stop only git in a terminal can finish (qw382). */
export function chatMaySettle(saving: PlaneSaving): boolean {
  return saving.byHand == null && !ONLY_GIT_FINISHES.has(saving.stoppedBy ?? "");
}

/** Whether pressing Save could do anything: files to commit, a blocked save to try again, or
 *  commits a push would carry. Commits on a plane whose save stops at the commit are as far as
 *  a save goes, and a button that could only say "nothing to save" is not offered. */
export function savable(saving: PlaneSaving): boolean {
  return (
    saving.changed.length > 0 ||
    saving.stage === "blocked" ||
    (saving.stage === "committed" && saving.pushes)
  );
}

/** The stage, as the view and the title bar say it, and what came in and was not pulled. */
export function stageText(saving: PlaneSaving): string {
  return withIncoming(stageWords(saving), saving.behind ?? 0);
}

/** A stage's words and what came in, as one sentence: the one place "N incoming" is phrased. */
function withIncoming(words: string, incoming: number): string {
  return incoming > 0 ? `${words} · ${incoming} incoming` : words;
}

function stageWords(saving: PlaneSaving): string {
  switch (saving.stage) {
    case "blocked":
      return `Blocked: ${saving.blocked ?? "the last save could not finish"}`;
    case "changed":
      return `${saving.changed.length} changed`;
    case "committed":
      return saving.ahead === null
        ? "Committed, not pushed"
        : `${saving.ahead} committed, not pushed`;
    case "pr-open":
      return requestWords(saving);
    default:
      return "Saved";
  }
}

function modeText(saving: PlaneSaving): string {
  const mode = saving.mode === null ? "Mode: not set" : `Mode: ${saving.mode} (${saving.modeFrom})`;
  const reach = !saving.pushes
    ? "a save commits and goes no further"
    : saving.mode === "pr" || saving.mode === "pr-merge"
      ? `a save pushes to this machine's save branch and keeps a ${saving.request} open into ${saving.branch}${
          saving.mode === "pr-merge"
            ? ", which purlis merges once every check on it has passed"
            : ""
        }`
      : `a save pushes to ${saving.branch}`;
  return `${mode} — ${reach}`;
}

function entryText(one: SaveEntry): string {
  const when = new Date((one.at ?? 0) * 1000).toLocaleString();
  const commit = one.commit === null ? "" : ` · ${one.commit.slice(0, 7)}`;
  const detail = one.detail === "" ? "" : ` — ${one.detail}`;
  const files = one.files === 1 ? "1 file" : `${one.files} files`;
  return `${when} · ${one.trigger} · ${one.outcome} · ${files}${commit}${detail}`;
}

/**
 * **A project's unsaved mark** (charter-app#302), on its tab in the project strip: a dot when
 * it has work a save would take — files, a blocked save, commits a push would carry — and
 * nothing when a save has nothing left to do. Named for a screen reader; the Saving tab says
 * what exactly.
 */
export function UnsavedMark({ saving, name }: { saving: PlaneSaving | undefined; name: string }) {
  if (saving === undefined) return null;
  const blocked = saving.stage === "blocked";
  if (!blocked && !savable(saving)) return null;
  return (
    <Tooltip tip={stageText(saving)}>
      <span
        className="project-unsaved"
        data-stage={saving.stage}
        role="img"
        aria-label={blocked ? `saving is blocked in ${name}` : `unsaved work in ${name}`}
      />
    </Tooltip>
  );
}
