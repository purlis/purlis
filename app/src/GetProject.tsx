// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from "react";
import { Channel } from "@tauri-apps/api/core";
import { commands, type GetProgress, type ProjectFolder } from "./bindings";
import { DoorPage } from "./DoorPage";
import { Field, SettingRow } from "./settings/components";
import { YourRepos } from "./YourRepos";
import { Button } from "./ui/button/Button";
import { Form } from "./ui/field/Field";
import { ProgressBar } from "./ui/meter/Meter";

/** Where the folder the person last picked to hold their projects is kept, on this machine. */
export const REMEMBERED_PARENT = "purlis.getProject.parent";

/** The place picked last, or none: `~/Projects` is the core's default (ruling Q7). */
function rememberedParent(): string | undefined {
  try {
    return globalThis.localStorage?.getItem(REMEMBERED_PARENT) ?? undefined;
  } catch {
    return undefined;
  }
}

function rememberParent(parent: string): void {
  try {
    globalThis.localStorage?.setItem(REMEMBERED_PARENT, parent);
  } catch {
    // A window that cannot keep it asks again next time; the clone itself is unaffected.
  }
}

/** What the page is doing. */
type Doing =
  | { kind: "asking" }
  | { kind: "getting"; stage: string; percent: number | null }
  | { kind: "refused"; sentence: string }
  | { kind: "called-off" };

/**
 * **Get a project from your forge** (#1815, spec #1810): a pasted URL, cloned into a folder the
 * person picks, then opened through the open-and-trust question every open goes through.
 *
 * - **The URL** is HTTPS or SSH. A bad one is said before anything runs.
 * - **The folder** is `~/Projects/<name>` until the person picks a place with Choose a folder…; the place
 *   picked is remembered on this machine for the next project.
 * - **Progress** is shown while it clones, with Cancel. Cancel, a failure, or leaving the page
 *   leaves nothing behind: the core removes what the clone made.
 * - **Each failure says why**, in the core's sentence (a bad URL, a missing project, no access,
 *   no network, a folder in use), with Retry beside it.
 *
 * Private projects work through the person's own git sign-in until purlis has its own (#752).
 * Below the box, {@link YourRepos} lists what the person's signed-in accounts reach, projects
 * first (#1818): picking one gets it the same way. A project already on this machine is one
 * Open a folder… away.
 */
export function GetProject({
  onBack,
  onOpen,
  onOpenFolder,
  said,
  withGit = (act) => void act(),
}: {
  /** Comes back to the welcome. */
  onBack: () => void;
  /** Opens a folder through the trust question: the welcome's own open. */
  onOpen: (path: string) => void;
  /** The Open door's own act: the folder dialog, then the open. */
  onOpenFolder: () => void;
  /** Why the last open refused, or the folder dialog could not open (#1291). */
  said?: string;
  /** Runs the clone once git is there (#1813's gate, held by the guided steps, #1817). */
  withGit?: (act: () => unknown) => void;
}) {
  const [url, setUrl] = useState("");
  const [parent, setParent] = useState<string | undefined>(rememberedParent);
  /** Where the URL's project goes, by the URL it was asked for. */
  const [answered, setAnswered] = useState<{ url: string; folder?: ProjectFolder }>();
  const [doing, setDoing] = useState<Doing>({ kind: "asking" });
  /** Why Choose… could not open its dialog, in the core's words. */
  const [unpicked, setUnpicked] = useState<string>();
  const getting = doing.kind === "getting";
  // Only the answer for the URL in the box now: an emptied box shows no folder.
  const folder = answered?.url === url ? answered.folder : undefined;
  const gettingNow = useRef(false);

  // The folder follows the URL as it is typed. A URL that is not one yet shows no folder, and is
  // only said to be wrong when the person presses Get project.
  useEffect(() => {
    let gone = false;
    if (!url.trim()) return;
    void commands
      .projectUrlFolder(url, parent ?? null)
      .then((answer) => {
        if (!gone) setAnswered({ url, folder: answer.status === "ok" ? answer.data : undefined });
      })
      .catch(() => {
        if (!gone) setAnswered({ url });
      });
    return () => {
      gone = true;
    };
  }, [url, parent]);

  // Leaving the page while it clones calls the clone off, so nothing is left half-made.
  useEffect(
    () => () => {
      if (gettingNow.current) void commands.cancelGetProject();
    },
    [],
  );

  /** Clones `from`, the URL in the box or a repo picked from the list, and opens it. */
  const get = useCallback(
    async (from: string = url) => {
      const where = await commands
        .projectUrlFolder(from, parent ?? null)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      if (where.status === "error") {
        setDoing({ kind: "refused", sentence: where.error });
        return;
      }
      setAnswered({ url: from, folder: where.data });
      const progress = new Channel<GetProgress>();
      progress.onmessage = (now) =>
        setDoing((was) =>
          was.kind === "getting"
            ? { kind: "getting", stage: now.stage, percent: now.percent }
            : was,
        );
      gettingNow.current = true;
      setDoing({ kind: "getting", stage: "Connecting", percent: null });
      const answer = await commands
        .getProject(from, where.data.folder, progress)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      gettingNow.current = false;
      if (answer.status === "error") setDoing({ kind: "refused", sentence: answer.error });
      else if (answer.data === null) setDoing({ kind: "called-off" });
      else {
        setDoing({ kind: "asking" });
        onOpen(answer.data);
      }
    },
    [url, parent, onOpen],
  );

  /** A repo picked from the list: shown in the box, then got as a pasted URL is, once git is
   *  there. */
  const pick = useCallback(
    (picked: string) => {
      setUrl(picked);
      withGit(() => get(picked));
    },
    [get, withGit],
  );

  const choose = useCallback(() => {
    setUnpicked(undefined);
    void commands
      .pickProject()
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
      .then((answer) => {
        if (answer.status === "error") setUnpicked(answer.error);
        else if (answer.data) {
          rememberParent(answer.data);
          setParent(answer.data);
        }
      });
  }, []);

  return (
    <DoorPage title="Get a project from your forge" className="get-project" onBack={onBack}>
      <Form
        className="get-project-form"
        aria-busy={getting}
        onSubmit={(event) => {
          event.preventDefault();
          if (url.trim() && !getting) withGit(get);
        }}
      >
        <SettingRow
          label="Project URL"
          setting="get-project-url"
          help="The address you clone it from, over HTTPS or SSH."
          control={(fieldIds) => (
            <Field
              ids={fieldIds}
              kind="text"
              value={url}
              placeholder="https://github.com/owner/project"
              onChange={(to) => {
                setUrl(to);
                if (doing.kind !== "getting") setDoing({ kind: "asking" });
              }}
            />
          )}
        />
        <div className="get-project-end">
          {/* The place, not a box to type in: Choose… picks the folder that holds it, and the
              project's own folder is named after it. */}
          <p className="get-project-folder ui-text-secondary">
            {/* Before a URL names the project, the folder that will hold it, once one was
                picked. */}
            Goes into <code className="where">{folder?.folder ?? parent ?? "—"}</code>
          </p>
          <Button variant="ghost" isDisabled={getting} onPress={choose}>
            Choose a folder…
          </Button>
          <Button
            variant={url.trim() ? "primary" : "secondary"}
            type="submit"
            isDisabled={!url.trim() || getting}
          >
            Get project
          </Button>
        </div>
      </Form>

      {doing.kind === "getting" && (
        <CloneProgress
          stage={doing.stage}
          percent={doing.percent}
          onCancel={() => void commands.cancelGetProject()}
        />
      )}
      <div role="status" className="get-project-said">
        {doing.kind === "called-off" && "Cancelled. Nothing was left behind."}
      </div>

      {doing.kind === "refused" && (
        <p className="trouble" role="alert">
          {doing.sentence} <Button onPress={() => void get()}>Retry</Button>
        </p>
      )}

      <YourRepos onPick={pick} busy={getting} />

      <p className="get-project-local ui-text-secondary">
        Already on this machine?{" "}
        <Button variant="link" onPress={onOpenFolder} className="welcome-link">
          Open a folder…
        </Button>
      </p>

      {(unpicked ?? said) && (
        <p className="trouble" role="alert">
          {unpicked ?? said}
        </p>
      )}
    </DoorPage>
  );
}

/**
 * **How far the clone has got, with Cancel** (#3731): the meter family's bar, git's stage as its
 * name and the percent as its value, going with no end known until git gives one. The bar says
 * the stage and the percent politely as they change, at most once a second, so nothing around it
 * repeats them.
 */
export function CloneProgress({
  stage,
  percent,
  onCancel,
}: {
  /** Git's stage, in its words: "Receiving objects". */
  stage: string;
  /** How far the stage is, or none while git has not said. */
  percent: number | null;
  /** Calls the clone off. */
  onCancel: () => void;
}) {
  return (
    <div className="get-project-progress">
      <ProgressBar
        className="get-project-bar"
        label={stage}
        value={percent ?? 0}
        isIndeterminate={percent === null}
      />
      <Button onPress={onCancel}>Cancel</Button>
    </div>
  );
}
