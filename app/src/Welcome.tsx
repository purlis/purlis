// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { CloudDownload, FolderOpen, FolderPlus, LogIn } from "lucide-react";
import { commands, type ForgeRow, type Recents, type TemplateChoice } from "./bindings";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { CreateOnForge, type Where, WhereItLives } from "./CreateOnForge";
import { ForgeSignIn } from "./ForgeSignIn";
import type { ForgeAsk } from "./ForgeQuestion";
import { FirstRun } from "./FirstRun";
import { GetProject } from "./GetProject";
import { MakeItAProject } from "./MakeItAProject";
import { Notice } from "./Notice";
import { dismissGone, GoneProjectNotice, settleGone, useGoneDismissed } from "./GoneProjectNotice";
import { goneCause, keepGoneDismissed, letGoneGo, settleGoneKept } from "./dismissals";
import { useNewerTrouble } from "./pickTrouble";
import { Field, SettingActions, SettingRow } from "./settings/components";
import { Button } from "./ui/button/Button";
import { CardButton } from "./ui/card/CardButton";
import { Form } from "./ui/field/Field";
import { List, ListRow } from "./ui/list/List";
import { RowDescription, RowLabel, RowTrail } from "./ui/list/Row";

/** The pages behind the welcome's doors. The welcome itself is the absence of one. */
export type Door = "create" | "get" | "sign-in";

/** What a press on the welcome asks of the guided steps that hold it (#1817): a door's page,
 *  the Open door's folder dialog, the welcome again, or (#1819) the sign-in an act needs first,
 *  coming back to the page that asked once it is done. */
export type Chose = Door | "open" | "sign-in-first" | undefined;

/**
 * **The welcome: what a window with no project open draws** (#1812, spec #1810).
 *
 * It is always this, whatever the machine store remembers, wherever the app was launched from
 * and however many projects the window already holds. It replaced two screens: a bare page whose
 * only act was a folder dialog, and a first run (FR-4) that took its place only on a machine that
 * remembered nothing at all. One row in the store — a project from an old Mac, a launch from a
 * terminal — was enough to hide the first run, and a person setting up a new machine met the
 * bare page (#1810's problem statement).
 *
 * **Three doors, a sign-in, and the recent projects** (ruling Q1, 2026-10-12):
 *
 * - **Create a project** — the first run's page, behind its door: a repo on this machine, opened
 *   in a workspace of its own in the project purlis keeps here, with a template for its stack
 *   (FR-17). New project… is one press from it, for a project in a folder of its own.
 * - **Get a project from your forge** — a pasted HTTPS or SSH URL, cloned into a folder under
 *   `~/Projects` (or the place picked last) and opened through the trust question (#1815).
 *   {@link GetProject} is its page; the list of the person's repos comes with sign-in (#1818).
 * - **Open a folder on this machine…** — the folder dialog, at once; and the path box under the
 *   doors, which is the same act typed (a native dialog cannot be driven by the scenario tests).
 *   A folder that is in no project is not refused: {@link MakeItAProject} offers to make it one,
 *   with the template choice, and Not now leaves it untouched (#1822).
 * - **Sign in to GitHub or GitLab** — a quieter entry under the doors, because it is skippable
 *   and asked again inline when an act needs it (Q2). Its page says that signing in from purlis
 *   comes later (#1814) and lists the `gh` and `glab` sign-ins that work today.
 *   {@link ForgeSignIn} is the seam.
 *
 * A door opens its page in place of the welcome, and Back comes home to the door that was
 * pressed. The keyboard goes with the person: onto the page's heading, then back onto the door.
 *
 * **What the old page kept, it still keeps**: the recents and their Notices — a dropped row, a
 * gone project with Locate… and Forget (NO-5), a machine that keeps no store (ADR 0031) — and the
 * open refusal, said where the open was asked.
 *
 * **The heading still says which no-project state this is**, because it is the one line that
 * answers the question the launch asked:
 * - **a window that already holds projects** (`adding`): the `+` was pressed on a window full of
 *   them, so nothing is missing;
 * - **a directory that is in no project** (`here`): `purlis` run somewhere ordinary, and the
 *   resolver's own words say where it looked;
 * - **anything else**: a dock launch, or a window whose last project was closed.
 */
export function Welcome({
  here,
  reason,
  adding,
  onOpen,
  onGoneListed,
  onGoneSettled,
  goneChanged,
  trouble,
  onOpenRepo,
  onNewProject,
  onSignInToForge,
  openingRepo,
  repoTrouble,
  repoForgeAsk,
  door: doorAsked,
  onChose,
  withGit = run,
  onSignedIn,
}: {
  /** Whether the launch had a directory to go on at all. */
  here: boolean;
  /** Why no project was opened, in the resolver's own words. */
  reason: string;
  /** Whether this window already holds projects, so this is a ninth rather than a first. */
  adding?: boolean;
  /** Asks the core to open this path. It answers, or asks the operator first. */
  onOpen: (path: string) => void;
  /** The gone projects this welcome draws, told whenever they change (NO-5): while it is drawn,
   *  the window draws only the ones this list does not hold. `"unread"` until the list has
   *  answered and again when the welcome goes, so the window holds its own back rather than
   *  drawing a line that the welcome's would replace under the pointer. */
  onGoneListed?: (paths: readonly string[] | "unread") => void;
  /** A gone project was located, forgotten or dismissed here. */
  onGoneSettled?: (path: string) => void;
  /** Changes when the window located or forgot a gone project, so the list is read again. */
  goneChanged?: number;
  /** Why the last attempt opened nothing. */
  trouble?: string;
  /** Opens a repo into this machine's local project, laid out from `template`: Create a
   *  project's act. */
  onOpenRepo: (path: string, template: TemplateChoice) => void;
  /** Opens the New project dialog, for a project in a folder of its own. */
  onNewProject: () => void;
  /** Opens the local project with `<cli> auth login` in a shell tab: today's forge sign-in. */
  onSignInToForge: (row: ForgeRow) => void;
  /** Whether a repo is being opened right now. */
  openingRepo?: boolean;
  /** Why the last repo opened nothing. */
  repoTrouble?: string;
  /** The question about the forge, when the repo's remote did not say (#839). */
  repoForgeAsk?: ForgeAsk;
  /** **Held by the guided steps** (#1817): the page they are at. With `onChose` given, the
   *  steps decide which page is drawn and every press is told to them; without it, the welcome
   *  keeps its own. */
  door?: Door;
  /** A door, Sign in, Back, or the Open door's dialog answering nothing: told to the steps. */
  onChose?: (chose: Chose) => void;
  /** Runs an act that needs git once git is there (#1813's gate). Runs it at once without one. */
  withGit?: (act: () => unknown) => void;
  /** A token signed in on the sign-in page: told to the guided steps. */
  onSignedIn?: () => void;
}) {
  const [recents, setRecents] = useState<Recents>();
  /** The Notices dismissed here, by cause, for this run (NO-2 keeps them across a relaunch). */
  const [dismissed, setDismissed] = useState<ReadonlySet<string>>(() => new Set());
  /** The gone projects dismissed in this window, here or by its own line, this run or at an
   *  earlier launch (#1291). */
  const goneDismissed = useGoneDismissed();
  /** Whether the recent list has answered at all, so "no recent projects" is never said about
   *  a list that simply has not arrived yet. */
  const [heard, setHeard] = useState(false);
  /** Why the recent list could not be read, in the core's words; said, never swallowed. */
  const [unread, setUnread] = useState<string>();
  const [typed, setTyped] = useState("");
  /** Bumped when a gone recent was forgotten or re-pointed, so the list is read again. */
  const [changed, setChanged] = useState(0);
  /** The page a door opened, or none for the welcome itself: the welcome's own, unless the
   *  guided steps hold it. */
  const [ownDoor, setOwnDoor] = useState<Door>();
  const door = onChose ? doorAsked : ownDoor;
  /** Where a project made behind Create lives (#1819): kept while the person signs in. */
  const [where, setWhere] = useState<Where>("mac");
  /** The page a sign-in an act needed comes back to, when the welcome holds its own pages. */
  const [afterSignIn, setAfterSignIn] = useState<Door>();
  const setDoor = useCallback(
    (to: Door | undefined) => (onChose ? onChose(to) : setOwnDoor(to)),
    [onChose],
  );
  /** The door the person came back from, so the keyboard lands on it again. */
  const cameBackFrom = useRef<Door>(undefined);
  const doors = useRef<Partial<Record<Door, HTMLButtonElement | null>>>({});
  /** The folder offered to be made a project, because it is in none (#1822). */
  const [makeAt, setMakeAt] = useState<string>();
  /** The Open door, which the keyboard comes back to from that offer. */
  const openDoorButton = useRef<HTMLButtonElement | null>(null);
  /** Whether the person just came back from that offer, so the keyboard lands on the door. */
  const cameBackFromMaking = useRef(false);
  const ids = useId();

  // The list is read when the welcome appears and re-read whenever an attempt did not open
  // anything, because a refused open is exactly when a row may have gone. It is a command of
  // its own and it stats each row off the thread that draws, so nothing here waits on a
  // network mount that is not coming back.
  useEffect(() => {
    let gone = false;
    void commands
      .recentPlanes()
      .then((answer) => {
        if (!gone && answer.status === "ok") {
          setRecents(answer.data ?? undefined);
          // The core's answer: a dismissed project no longer gone in it has changed (#1291).
          const stillGone = (answer.data?.gone ?? []).map((one) => one.path);
          settleGone(stillGone);
          settleGoneKept(stillGone);
        }
        if (!gone) setUnread(answer.status === "error" ? answer.error : undefined);
        if (!gone) setHeard(true);
      })
      // A window that cannot ask offers no list, and says so (#1719): the doors and the path box
      // still work, which is the whole of what this screen has to do.
      .catch((err: unknown) => {
        if (!gone) {
          setUnread(String(err));
          setHeard(true);
        }
      });
    return () => {
      gone = true;
    };
  }, [trouble, changed, goneChanged]);

  // The gone projects drawn here, said to the window, which then leaves them to this list. Behind
  // a door none are drawn here, so the window draws its own.
  const listed =
    door !== undefined || makeAt !== undefined
      ? ""
      : heard
        ? (recents?.gone ?? []).map((gone) => gone.path).join("\n")
        : undefined;
  useEffect(() => {
    onGoneListed?.(listed === undefined ? "unread" : listed === "" ? [] : listed.split("\n"));
    return () => onGoneListed?.("unread");
  }, [listed, onGoneListed]);

  // Back on the welcome from a door: the keyboard is put on that door again, before paint, so
  // Tab goes on from where the person was rather than from the top of the page.
  useLayoutEffect(() => {
    if (door !== undefined || cameBackFrom.current === undefined) return;
    doors.current[cameBackFrom.current]?.focus();
    cameBackFrom.current = undefined;
  }, [door]);
  // Back from the offer to make a folder a project, on the welcome itself: onto the Open door.
  useLayoutEffect(() => {
    if (makeAt !== undefined || !cameBackFromMaking.current) return;
    cameBackFromMaking.current = false;
    if (door === undefined) openDoorButton.current?.focus();
  }, [makeAt, door]);

  // Why the folder dialog could not open (#1291): said where a refused open is, until any open
  // starts or a newer refusal comes. A cancelled dialog is null and is not a failure: nothing is
  // said and nothing moves.
  const { said, pickFailed, started } = useNewerTrouble(trouble);
  const open = useCallback(
    (path: string) => {
      started();
      onOpen(path);
    },
    [onOpen, started],
  );
  /**
   * A folder the person named, by the Open door, the path box or a clone (#1822): opened when it
   * is in a project, and otherwise offered to be made one. A folder that cannot be looked at goes
   * to the open all the same, whose refusal says what is wrong with it.
   */
  const openAFolderAt = useCallback(
    (path: string) => {
      started();
      void commands
        .folderIsAProject(path)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
        .then((answer) => {
          if (answer?.status === "ok" && answer.data === false) setMakeAt(path);
          else open(path);
        });
    },
    [open, started],
  );
  /** The folder dialog, then the open. `nothing` is what happens when it opens nothing. */
  const pickAFolder = useCallback(
    (nothing?: () => void) => {
      started();
      void commands
        .pickProject()
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
        .then((answer) => {
          if (answer.status === "error") pickFailed(answer.error);
          if (answer.status === "ok" && answer.data) openAFolderAt(answer.data);
          else nothing?.();
        });
    },
    [openAFolderAt, pickFailed, started],
  );
  /** Get's page's way in today: the dialog, once git is there. */
  const openAFolder = useCallback(() => withGit(() => pickAFolder()), [pickAFolder, withGit]);
  /** The Open door: the steps are told, then the dialog, once git is there. A dialog that
   *  opened nothing is the welcome again. */
  const openDoor = useCallback(() => {
    onChose?.("open");
    withGit(() => pickAFolder(() => onChose?.(undefined)));
  }, [onChose, pickAFolder, withGit]);
  /** Not now, on the offer: back where the person was, with nothing written or opened. On the
   *  welcome itself, the guided steps are told so too, as a dialog that opened nothing is. */
  const notMakingIt = useCallback(() => {
    cameBackFromMaking.current = true;
    setMakeAt(undefined);
    if (door === undefined) onChose?.(undefined);
  }, [door, onChose]);

  /** Back from a door's page to the welcome, with the keyboard on that door; from a sign-in an
   *  act asked for, back to that act's page. */
  const back = useCallback(() => {
    if (!onChose && door === "sign-in" && afterSignIn) {
      setAfterSignIn(undefined);
      setOwnDoor(afterSignIn);
      return;
    }
    cameBackFrom.current = door;
    setDoor(undefined);
  }, [afterSignIn, door, onChose, setDoor]);
  /** Create on a forge with no sign-in to it (#1819): the sign-in, then Create again. */
  const signInFirst = useCallback(() => {
    if (onChose) onChose("sign-in-first");
    else {
      setAfterSignIn("create");
      setOwnDoor("sign-in");
    }
  }, [onChose]);
  /** A token signed in: told to the steps, or back to the page that asked for it. */
  const signedIn = useCallback(() => {
    onSignedIn?.();
    if (!onChose && afterSignIn) {
      setAfterSignIn(undefined);
      setOwnDoor(afterSignIn);
    }
  }, [afterSignIn, onChose, onSignedIn]);

  // The three doors' handlers, by the names the tickets that fill them in will look for.
  /** Create a project: the first run's page (FR-4, FR-17), behind its door. */
  const createAProject = useCallback(() => setDoor("create"), [setDoor]);
  /** Get a project from your forge: a pasted URL cloned into a folder, then opened (#1815). */
  const getFromYourForge = useCallback(() => setDoor("get"), [setDoor]);
  /** Sign in to GitHub or GitLab: the page #1814 builds; until then it says so. */
  const signInToAForge = useCallback(() => setDoor("sign-in"), [setDoor]);

  if (makeAt !== undefined)
    return (
      <MakeItAProject
        path={makeAt}
        onMake={onOpenRepo}
        onBack={notMakingIt}
        making={openingRepo ?? false}
        trouble={repoTrouble}
        forgeAsk={repoForgeAsk}
      />
    );
  if (door === "create")
    return (
      <FirstRun
        onOpenRepo={onOpenRepo}
        onBack={back}
        onNewProject={onNewProject}
        opening={openingRepo ?? false}
        trouble={repoTrouble}
        forgeAsk={repoForgeAsk}
        where={<WhereItLives value={where} onChange={setWhere} />}
        elsewhere={
          where === "mac"
            ? undefined
            : (templates) => (
                <CreateOnForge
                  forge={where}
                  templates={templates}
                  onOpen={open}
                  onNeedsSignIn={signInFirst}
                  withGit={withGit}
                />
              )
        }
      />
    );
  if (door === "get")
    return (
      <GetProject
        onBack={back}
        onOpen={openAFolderAt}
        onOpenFolder={openAFolder}
        said={said}
        withGit={withGit}
      />
    );
  if (door === "sign-in")
    return (
      <ForgeSignIn
        onBack={back}
        onSignInToForge={onSignInToForge}
        trouble={repoTrouble}
        onSignedIn={signedIn}
      />
    );

  const planes = recents?.planes ?? [];
  const goneRows = recents?.gone ?? [];
  // "No recent projects yet" is said only about a list that answered, on a machine that keeps
  // one: a list that could not be read says that instead, and a machine with no store says so in
  // its own Notice.
  const nothingRecent =
    heard &&
    unread === undefined &&
    !recents?.forgetful &&
    planes.length === 0 &&
    goneRows.length === 0;

  // Each door's line says what it does today, so nobody presses one expecting something else.
  const doorRows: {
    door: Door | "open";
    title: string;
    says: string;
    mark: typeof FolderPlus;
    press: () => void;
  }[] = [
    {
      door: "create",
      title: "Create a project",
      says: "On GitHub, on GitLab or only on this machine, with a template for its stack.",
      mark: FolderPlus,
      press: createAProject,
    },
    {
      door: "get",
      title: "Get a project from your forge",
      says: "Clone your team's project from GitHub or GitLab, from your repos or by its URL.",
      mark: CloudDownload,
      press: getFromYourForge,
    },
    {
      door: "open",
      title: "Open a folder on this machine…",
      says: "Open a project you already have.",
      mark: FolderOpen,
      press: openDoor,
    },
  ];

  return (
    <section className="welcome" aria-labelledby={`${ids}-heading`}>
      <header className="welcome-head">
        <h1 id={`${ids}-heading`}>
          {adding
            ? "Add another project"
            : here
              ? "purlis found no project here"
              : "Welcome to purlis"}
        </h1>
        {adding ? (
          <p className="welcome-lede">
            It opens as another tab in this window, beside the ones already here. Every project
            keeps its own chats, and nothing running in them stops.
          </p>
        ) : here ? (
          // The resolver's own words. Somebody who ran `purlis` in a directory asked a question,
          // and "a project is the nearest directory at or above this one with a purlis.toml" is
          // the answer to it.
          <p className="welcome-lede">{reason}</p>
        ) : (
          <p className="welcome-lede">
            A project is where purlis keeps your workspaces, personas and memory. Create one, get
            your team&apos;s from GitHub or GitLab, or open one you already have.
          </p>
        )}
      </header>

      {/* Each door is a card pressed whole (`ui/card/CardButton`), named by its title alone and
          described by its line, so a screen reader reads the act first and the explanation after
          it. */}
      <ul className="welcome-doors" aria-label="Ways to start">
        {doorRows.map(({ door: which, title, says, mark: Mark, press }) => (
          <li key={which}>
            <CardButton
              title={title}
              line={says}
              mark={<Mark aria-hidden="true" />}
              ref={(element) => {
                if (which === "open") openDoorButton.current = element;
                else doors.current[which] = element;
              }}
              onPress={press}
            />
          </li>
        ))}
      </ul>

      <p className="welcome-sign-in">
        <Button
          variant="link"
          ref={(element) => {
            doors.current["sign-in"] = element;
          }}
          onPress={signInToAForge}
          icon={<LogIn />}
          className="welcome-link"
        >
          Sign in to GitHub or GitLab
        </Button>
      </p>

      {/* The path box is not a lesser picker. A native folder dialog cannot be driven by the
          scenario tests, and somebody who already knows the path types faster than they click —
          so the two are one command with two ways in, and neither resolves anything itself. */}
      <Form
        className="by-path"
        onSubmit={(event) => {
          event.preventDefault();
          if (typed.trim()) openAFolderAt(typed.trim());
        }}
      >
        {/* The settings set's row (#1719). `setting` is the handle the scenario tests find it
            by. */}
        <SettingRow
          label="Or type a path"
          setting="open-by-path"
          control={(fieldIds) => (
            <Field
              ids={fieldIds}
              kind="text"
              value={typed}
              placeholder="/path/to/project"
              onChange={setTyped}
            />
          )}
        />
        <SettingActions>
          <Button type="submit" isDisabled={!typed.trim()}>
            Open project
          </Button>
        </SettingActions>
      </Form>

      {said && (
        <p className="trouble" role="alert">
          {said}
        </p>
      )}

      {/* Said, not swallowed (#1719): a welcome with no list and no word reads as a machine
          that remembers nothing. A status of its own, as the eye sees it arrive. */}
      {unread !== undefined && (
        <p className="came-back" role="status">
          purlis could not read the recent projects: {unread}
        </p>
      )}

      {/* `?.` on the list as well as on the answer: a core that answered oddly — the shape
          changed, a command stubbed out — must cost this screen its recent list and not its
          doors. The welcome is the one screen somebody with no project can reach. */}
      {planes.length > 0 && (
        <section className="welcome-recents" aria-labelledby={`${ids}-recents`}>
          <h2 id={`${ids}-recents`}>Recent projects</h2>
          {/* Rows that act (`ui/list`, #2911): a press or Enter opens the project. */}
          <List
            className="recents"
            aria-labelledby={`${ids}-recents`}
            onAction={(key) => open(String(key))}
          >
            {planes.map((plane) => (
              <ListRow key={plane.path} id={plane.path} textValue={plane.name}>
                <RowLabel className="tab-name">{plane.name}</RowLabel>
                <RowDescription>
                  <code className="where">{plane.path}</code>
                </RowDescription>
                {/* What the store holds, and nothing more: whether this project still
                    contributes what was approved is asked when it is opened. */}
                {!plane.approved && (
                  <RowTrail className="value">purlis will ask about this one</RowTrail>
                )}
              </ListRow>
            ))}
          </List>
        </section>
      )}
      {nothingRecent && (
        <EmptyState
          variant="panel"
          headline="No recent projects yet"
          body="Each project you open is listed here, one press from opening again."
        />
      )}

      {/* An entry the store itself would not take back is dropped with a line saying so, never
          an error dialog (ADR 0034): the record is a convenience and the project is the truth. */}
      {recents?.dropped
        ?.filter((line) => !dismissed.has(`not-remembered:${line}`))
        .map((line) => (
          <Notice
            key={line}
            cause={`not-remembered:${line}`}
            onDismiss={() => setDismissed((was) => new Set(was).add(`not-remembered:${line}`))}
          >
            {line}
          </Notice>
        ))}

      {/* A project that has moved or gone offers Locate… and Forget (NO-5): never done
          without a press, because a disk that is unplugged may come back. */}
      {goneRows
        .filter((gone) => !goneDismissed.has(gone.path))
        .map((gone) => (
          <GoneProjectNotice
            key={gone.path}
            gone={gone}
            cause={goneCause(gone.path)}
            onLocated={(found) => {
              onGoneSettled?.(gone.path);
              letGoneGo(gone.path);
              setChanged((n) => n + 1);
              open(found);
            }}
            onForgotten={() => {
              onGoneSettled?.(gone.path);
              letGoneGo(gone.path);
              setChanged((n) => n + 1);
            }}
            onDismiss={() => {
              dismissGone(gone.path);
              keepGoneDismissed(gone.path);
              onGoneSettled?.(gone.path);
            }}
          />
        ))}

      {/* A machine with no store — Windows, where `0600` has no expression, so purlis's guard
          refuses rather than degrades (ADR 0031). The app works; it just cannot remember. */}
      {recents?.forgetful && !dismissed.has("no-machine-store") && (
        <Notice
          cause="no-machine-store"
          onDismiss={() => setDismissed((was) => new Set(was).add("no-machine-store"))}
        >
          purlis cannot remember projects on this machine ({recents.forgetful}), so there is no
          recent list and it will ask about every project you open.
        </Notice>
      )}
    </section>
  );
}

/** An act run at once: the welcome with no git gate around it. */
function run(act: () => unknown): void {
  void act();
}
