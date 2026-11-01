// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from "react";
import { commands, type ChatsRunOn, type PlaneId, type ProjectChatsRunOn } from "../bindings";
import { startAddingAPool } from "../pools";
import { Button } from "../ui/button/Button";
import { Choice } from "./components";
import type { LiveSetting, SettingsGroup } from "./groups";

/** The value the radio gives this machine: no pool's name is empty. */
const HERE = "";
/** The value the radio gives following the project: `@` starts no pool's name. */
const PROJECT = "@project";

/** What each pool's row says, at either level. */
const poolOption = (pool: string) => ({
  value: pool,
  label: `Pool ${pool}`,
  says: "Chats run on the pool's runner, in its sandbox.",
});
const THIS_MACHINE = {
  value: HERE,
  label: "This machine",
  says: "Chats run here, in this machine's sandbox.",
};

/**
 * **Where a workspace's chats run** (#2046, the person's ruling of 2026-10-20): this machine, or
 * a pool's runner, which gets the workspace's code to work on and sends its branches back. Kept
 * in this machine's `machine.json` alone, never in a committed file. The group is offered only
 * once this machine keeps a pool ({@link runsOnGroup}): nothing about pools shows before one is
 * added. A chat running now stays where it runs; the next one starts where this says.
 */
export function runsOnGroup(
  plane: PlaneId,
  workspace: string,
  poolsKept: boolean,
): SettingsGroup | undefined {
  if (!poolsKept) return undefined;
  return {
    id: "workspace.runs",
    label: "Runs on",
    help: "Where this workspace's chats run. A pool's runner gets this workspace's code, and its branches come back to Changes.",
    kept: "Kept on this machine only, never in a committed file.",
    settings: [runsOnSetting(plane, workspace)],
  };
}

function runsOnSetting(plane: PlaneId, workspace: string): LiveSetting {
  const id = "workspace.runs.on";
  return {
    id,
    label: "Chats run on",
    help: "A chat running now stays where it runs. The next chat starts here.",
    writes: "this machine",
    useControl: function useRunsOn() {
      const [now, setNow] = useState<ChatsRunOn>();
      const [error, setError] = useState<string[]>();
      const [setting, setSetting] = useState(false);
      useEffect(() => {
        let gone = false;
        void commands
          .chatsRunOn(plane, workspace)
          .then((said) => {
            if (gone) return;
            if (said.status === "ok") setNow(said.data);
            else setError([said.error]);
          })
          .catch((err: unknown) => !gone && setError([String(err)]));
        return () => {
          gone = true;
        };
      }, []);
      const choose = useCallback((to: string) => {
        setSetting(true);
        setError(undefined);
        void Promise.resolve()
          .then(() =>
            to === PROJECT
              ? commands.followProjectChatsRunOn(plane, workspace)
              : commands.setChatsRunOn(plane, workspace, to === HERE ? null : to),
          )
          .then((said) => {
            if (said.status === "ok") setNow(said.data);
            else setError([said.error]);
          })
          .catch((err: unknown) => setError([String(err)]))
          .finally(() => setSetting(false));
      }, []);
      const options = [
        {
          value: PROJECT,
          label: "Same as the project",
          says:
            now?.project == null
              ? "The project runs on this machine now."
              : `The project runs on pool ${now.project} now.`,
        },
        THIS_MACHINE,
        ...(now?.pools ?? []).map(poolOption),
      ];
      return {
        grouped: true,
        error,
        control: (ids) => (
          <Choice
            kind="radio"
            ids={ids}
            options={options}
            value={now === undefined ? undefined : (now.pool ?? (now.here ? HERE : PROJECT))}
            disabled={now === undefined || setting}
            onValueChange={choose}
          />
        ),
      };
    },
  };
}

/**
 * **Where a project's workspaces run their chats by default** (#4536, ruled 2026-11-01): a
 * workspace that says nothing follows this, on this machine alone, so it never changes where a
 * teammate's chats run. Shown before the first pool, with the way to add one.
 */
export function projectRunsOnGroup(plane: PlaneId): SettingsGroup {
  return {
    id: "project.runs",
    label: "Runs on",
    help: "Where this project's chats run, unless a workspace or a start chooses otherwise. A chat at the project's root runs on this machine.",
    kept: "Kept on this machine only, never in a committed file.",
    settings: [projectRunsOnSetting(plane)],
  };
}

function projectRunsOnSetting(plane: PlaneId): LiveSetting {
  const id = "project.runs.on";
  return {
    id,
    label: "Chats run on",
    help: "A chat running now stays where it runs. The next chat follows this.",
    writes: "this machine",
    useControl: function useProjectRunsOn() {
      const [now, setNow] = useState<ProjectChatsRunOn>();
      const [error, setError] = useState<string[]>();
      const [setting, setSetting] = useState(false);
      useEffect(() => {
        let gone = false;
        void commands
          .projectChatsRunOn(plane)
          .then((said) => {
            if (gone) return;
            if (said.status === "ok") setNow(said.data);
            else setError([said.error]);
          })
          .catch((err: unknown) => !gone && setError([String(err)]));
        return () => {
          gone = true;
        };
      }, []);
      const choose = useCallback((to: string) => {
        setSetting(true);
        setError(undefined);
        void commands
          .setProjectChatsRunOn(plane, to === HERE ? null : to)
          .then((said) => {
            if (said.status === "ok") setNow(said.data);
            else setError([said.error]);
          })
          .catch((err: unknown) => setError([String(err)]))
          .finally(() => setSetting(false));
      }, []);
      const pools = now?.pools ?? [];
      return {
        grouped: true,
        error,
        control: (ids) => (
          <>
            <Choice
              kind="radio"
              ids={ids}
              options={[THIS_MACHINE, ...pools.map(poolOption)]}
              value={now === undefined ? undefined : (now.pool ?? HERE)}
              disabled={now === undefined || setting}
              onValueChange={choose}
            />
            {now !== undefined && pools.length === 0 && (
              <Button onPress={() => startAddingAPool()}>Add a pool…</Button>
            )}
          </>
        ),
      };
    },
  };
}
