// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useId, useState, type ReactNode } from "react";
import { useNow } from "../stateClock";
import type { ClusterPoolRow, PoolLink, PoolRow, PoolVersion, ProjectNotHeld } from "../bindings";
import { Notice } from "../Notice";
import { useProjectsNotHeld } from "../notheld";
import {
  anotherEngineSaid,
  askLink,
  askVersion,
  markPoolOwned,
  makeRunnerAgain,
  removedSaid,
  removePool,
  openSetupOf,
  startAddingAPool,
  useHeldRunners,
  useNamedBy,
  usePools,
  type HeldRunners,
  type NamedBy,
} from "../pools";
import type { LiveSetting, SettingsGroup } from "./groups";
import { Button, ButtonRow } from "../ui/button/Button";
import { Tooltip } from "../ui/tooltip/Tooltip";
import { AlertDialog } from "../ui/dialog/Dialog";
import { Badge } from "../ui/badge/Badge";
import { Switch } from "../ui/choice/Switch";
import { PoolLimits } from "./PoolLimits";
import { PoolProfiles } from "./PoolProfiles";
import { notCheckedPoolSaid } from "../notChecked";

/**
 * **You › This machine › Pools** (#2679, the person's rulings of 2026-10-20): the Docker pools
 * added on this machine, each with its image, its limits, how long its runner is left without a
 * link before it suspends itself (#3471; as its runner was made, {@link suspendSaid}, #3717) and
 * Remove, and Add a pool… (Pool setup for a Docker pool, #4484) and Add a cluster pool….
 * **Always listed** (the person's ruling of 2026-10-30, amending #2679's "nothing about pools
 * shows before the first is added" for Settings): with no pool kept, it says in plain words what a
 * pool is, and its Add a pool… goes where the palette's does.
 *
 * **Each pool's runner is asked once whether it answers with the key pinned for it**, as the
 * group comes on screen. One whose key changed is refused on every link, so its row has a
 * Notice with Remove, which removes its runner whole, the pool with it. A runner that does not
 * answer says why, in Docker's or purlis's words.
 *
 * **A runner of another version than this machine's says so** ({@link versionSaid}, #2749):
 * a pool of purlis's own image moves its new chats to a runner of this version on its own, and
 * its older runner keeps its chats until they end; a pool of your own image does not, so its
 * row says what to do.
 *
 * **An older runner that waits on a project says which, and what lets it go** ({@link owedSaid},
 * #5461): it stays until each project its chats ran for has brought back what they did there,
 * which only a project open in a window does.
 *
 * **A project a runner would not hold again is marked here** (#2846): a Notice on the row of
 * each pool its chats run on, in the runner's own words (`<bdi>`, so they cannot turn the
 * sentence around them), until a hold of it is taken. One whose pool this machine cannot name
 * is marked under the list. Every command that needs the hold asks again and says why it was
 * refused; Dismiss puts the Notice away until the runner says otherwise.
 *
 * **Every Remove asks first** ({@link RemovePool}): it says what goes and that there is no undo,
 * and Remove pool removes it.
 *
 * **A cluster pool has a row of its own** ({@link ClusterPoolEntry}, #3292, #4071): This is my own,
 * off until the person turns it on, and its cluster's pool,
 * context and namespace, the cluster operator key pinned as it was added, and Remove, which
 * deletes its claim. A changed runner key and a changed cluster operator key get the same Notice
 * with Remove ({@link KeyChangedNotice}): either refuses every link to its runner.
 *
 * **Each pool's limits fold under its row** ({@link PoolLimits}, #4285).
 *
 * **A runner removed outside purlis says so** ({@link RemovedNotice}, #4149): Docker has no
 * container of its id, so purlis no longer links to it, and its row offers Make the runner
 * again, saying whether its home, with its sign-ins and kept work, is still there. A runner only
 * stopped or suspended is not removed: it resumes as before.
 */
export function poolsGroup(plane?: string): SettingsGroup {
  return {
    id: POOLS,
    label: "Pools",
    help: "The pools on this machine. Each runs a runner paired with this machine: a container of its image on your Docker, held to its limits, or a runner claimed in your team's cluster.",
    sub: true,
    settings: [poolsList(plane)],
  };
}

/** The group's address. */
export const POOLS = "you.machine.pools";

/** What a pool's Remove does, said under the list while there is a pool. */
const REMOVE_SAID =
  "Remove takes this machine out of the runner's peers while it answers, then removes a Docker pool's container and home, or releases a cluster pool's claim in its cluster. There is no undo.";

/** What the group says while this machine keeps no pool: what a pool is. */
const NO_POOL_YET =
  "No pool yet. A pool is a runner on your Docker, or one claimed in your team's cluster, that chats and browsers can move to. Add a pool… adds the first.";

/** The limits a row says, in a line. */
function limitsSaid(pool: PoolRow): string {
  const { cpus, memory_mib, processes, tmp_mib } = pool.limits;
  const cpusSaid = cpus === "1" ? "1 CPU" : `${cpus} CPUs`;
  return `${cpusSaid}, ${memory_mib} MiB of memory, ${processes} processes, ${tmp_mib} MiB of /tmp`;
}

/** What a row says of its runner, beside its name. */
function linkSaid(link: PoolLink | "asking" | undefined): string | undefined {
  if (link === undefined || link === "asking") return "asking its runner…";
  if (link.state === "linked") return "linked";
  if (link.state === "key_changed" || link.state === "operator_key_changed") return "key changed";
  if (link.state === "suspended") return "suspended";
  if (link.state === "removed") return "removed outside purlis";
  if (link.state === "on_another_engine") return "on another Docker engine";
  return "not linked";
}

/**
 * **How long a row says its runner is left without a link before it suspends itself** (#3717):
 * the pool's time, unless its runner was made otherwise. A runner made before purlis suspended
 * runners never does, and one made with another time keeps that time.
 */
export function suspendSaid(pool: PoolRow, version: PoolVersion | undefined): string {
  const made = version?.suspends;
  if (made?.made === "never") {
    return "Never suspends: its runner was made before purlis suspended runners";
  }
  if (made?.made === "after" && made.after !== pool.suspend_after) {
    return `Suspends after ${made.after} without a link from this machine, as its runner was made (the pool says ${pool.suspend_after})`;
  }
  return `Suspends after ${pool.suspend_after} without a link from this machine`;
}

/** What a row says of its runner's version, where it is not this machine's; nothing where it
 *  is. */
export function versionSaid(version: PoolVersion | undefined): string | undefined {
  if (version === undefined) return undefined;
  const lines: string[] = [];
  if (version.runner === null) {
    lines.push(
      `purlis could not read which version its runner runs, so it may not be ${version.desktop}: ${version.why ?? "it gave no reason"}`,
    );
  } else if (version.runner !== version.desktop) {
    lines.push(
      !version.follows
        ? `Its runner runs purlis ${version.runner}, and this machine runs ${version.desktop}. Build your image from purlis ${version.desktop}, then add the pool again.`
        : version.named === false
          ? // The release came out without its runner image (#2880): the pool stays put.
            `Its runner runs purlis ${version.runner}, and this machine runs ${version.desktop}. The release of ${version.desktop} has no runner image, so the pool does not move to ${version.desktop} and stays on its runner.`
          : `Its runner runs purlis ${version.runner}, and this machine runs ${version.desktop}. New chats start on a runner of ${version.desktop} once that version's runner image is published.`,
    );
  }
  if (version.held > 0) {
    lines.push(
      version.held === 1
        ? "1 older runner stopped with what its chats left in its home. purlis keeps it until you bring their conversations back or discard them in Devices."
        : `${version.held} older runners stopped with what their chats left in their homes. purlis keeps them until you bring their conversations back or discard them in Devices.`,
    );
  }
  if (version.draining > 0) {
    lines.push(
      version.draining === 1
        ? "1 older runner keeps its chats until they end, then purlis removes it once their work is brought back."
        : `${version.draining} older runners keep their chats until they end, then purlis removes them once their work is brought back.`,
    );
    // What they still wait on, as purlis last found it (#2865).
    if (version.waiting > 0) {
      lines.push(
        version.waiting === 1
          ? "1 chat still runs there: end it to let purlis remove the older runner."
          : `${version.waiting} chats still run there: end them to let purlis remove the older runners.`,
      );
    }
  }
  return lines.length === 0 ? undefined : lines.join(" ");
}

/**
 * **What a row says of the projects its older runner waits on** (#5461): a draining runner stays
 * until each project its chats ran for has brought back the work they saved on it, and only a project
 * open in a window does. Each is named, with what lets the runner go: an open one brings it back
 * on its own; a closed one says when it was last opened and that opening it brings the work back.
 * Removing the pool lets the runner go too, and the row says what that costs. Nothing where no
 * project is owed; why, where purlis could not read which are. `now` is in milliseconds.
 */
export function owedSaid(version: PoolVersion | undefined, now: number): string | undefined {
  if (version === undefined) return undefined;
  if (version.owed_why !== null) {
    return `purlis could not read which projects still have work on an older runner, so the runner stays: ${version.owed_why}`;
  }
  if (version.owed.length === 0) return undefined;
  // Two projects in folders of the same name are told apart by where they are.
  const owed = version.owed.map((one) => ({
    ...one,
    name: version.owed.some((other) => other.path !== one.path && other.name === one.name)
      ? one.path
      : one.name,
  }));
  const lines = [
    owed.length === 1
      ? `An older runner stays until project ${owed[0].name} brings back the work its chats saved on that runner.`
      : `An older runner stays until ${owed.length} projects bring back the work their chats saved on that runner: ${owed.map((one) => one.name).join(", ")}.`,
  ];
  for (const one of owed) {
    lines.push(
      one.open_now
        ? `${one.name} is open, so purlis brings it back on its own.`
        : `${one.name} is closed, and was last opened ${openedAgo(one.opened, now)}. Open ${one.name} to bring its work back.`,
    );
  }
  if (owed.some((one) => !one.open_now)) {
    lines.push(
      "Removing the pool lets the runner go now too, but anything not brought back is deleted with it.",
    );
  }
  return lines.join(" ");
}

/**
 * **What a row says of work an older runner holds that bringing work back does not reach**
 * (ruled 2026-11-07): a stash, a commit on no branch, a clone only on the runner, or one whose
 * folder here was removed or renamed. The runner stays, the row lists what keeps it, and says
 * what lets it go: Let it go in Devices, which frees that runner alone. Nothing where nothing
 * does.
 */
export function leftSaid(version: PoolVersion | undefined): string | undefined {
  if (version === undefined || version.left.length === 0) return undefined;
  const shown = version.left.slice(0, 5);
  const more = version.left.length - shown.length;
  return [
    `An older runner stays, since it holds work purlis can't bring back on its own: ${shown.join("; ")}${more > 0 ? `; and ${more} more` : ""}.`,
    "To keep it, copy it out of the runner's container with Docker. Let it go in Devices removes only that runner, and deletes that work with it.",
  ].join(" ");
}

/** When a project was last opened, `opened` in seconds since the epoch, in whole days before
 *  `now` in milliseconds, never calendar days: a clock that moved back is under a day. */
function openedAgo(opened: number, now: number): string {
  const days = Math.floor((now / 1000 - opened) / 86_400);
  if (days < 1) return "less than a day ago";
  if (days === 1) return "1 day ago";
  return `${days} days ago`;
}

/** Whether a pool's link says its runner is on a Docker engine that Docker no longer reaches. */
function linkOnAnotherEngine(link: PoolLink | "asking" | undefined): boolean {
  return typeof link === "object" && link.state === "on_another_engine";
}

/**
 * **Remove pool {name}?**, asked before any pool is removed: what goes, and that there is no
 * undo. Cancel changes nothing. It is the one question Pools settings and the Devices view ask.
 *
 * **It names what goes back to this machine** (#4678, the person's ruling of 2026-11-02): each
 * project and workspace that names the pool, by name, which removing it sets to This machine
 * (ADR 0078, D-4536-3). Where none does, it says nothing of them. Remove pool waits for the
 * read, and where purlis could not read them it says what is true of any, and why.
 *
 * **It names the runners that may hold conversations, and then asks again** (#4860, D-4860-1):
 * each older runner kept with its chats' conversations, or not yet read, whose conversations go
 * with the pool, and a second question, as Discard asks one, before they do. With none, it is one
 * question. `onRemove` is told the runners it named, and a runner that may hold one and was not
 * named removes nothing.
 */
export function RemovePool({
  name,
  cluster = false,
  elsewhere = false,
  onRemove,
  onCancel,
}: {
  name: string;
  /** A cluster pool's (#3292): its claim is deleted, and its runner with it. */
  cluster?: boolean;
  /** Its runner is on a Docker engine that Docker no longer reaches (#4325): nothing there goes. */
  elsewhere?: boolean;
  onRemove: (discarding: readonly string[]) => void;
  onCancel: () => void;
}) {
  const named = useNamedBy(name);
  const held = useHeldRunners(name, cluster);
  const [again, setAgain] = useState(false);
  const discarding = held.state === "read" ? held.held : [];
  // Each ask is a dialog of its own, so the second opens with the keyboard on its Cancel and not
  // on the button the first was answered with: two presses never discard.
  if (again)
    return (
      <AlertDialog
        key="again"
        onClose={onCancel}
        title={
          elsewhere
            ? `Forget the conversations on ${runnersSaid(discarding)}?`
            : `Delete the conversations on ${runnersSaid(discarding)}?`
        }
        description={
          <p className="came-back">
            {elsewhere
              ? `They stay on the Docker engine that Docker doesn't reach now, but purlis forgets them with pool ${name} and can no longer bring them back. To keep them, cancel and keep the pool.`
              : `They are deleted with pool ${name} and cannot be brought back. To keep them, cancel and keep the pool.`}
          </p>
        }
        main={{
          children: elsewhere
            ? "Remove pool and forget conversations"
            : "Remove pool and conversations",
          variant: "danger",
          onPress: () => onRemove(discarding),
        }}
      />
    );
  return (
    <AlertDialog
      key="first"
      onClose={onCancel}
      title={`Remove pool ${name}?`}
      description={
        <>
          <p className="came-back">
            {elsewhere
              ? "Its runner is on another Docker engine, which Docker doesn't reach now, so its container and home stay there, with everything chats left in them. purlis forgets the pool, and says what to remove once Docker points back. There is no undo."
              : cluster
                ? "Its runner's claim is deleted from your team's cluster, and its runner with it, with everything chats left there, and this machine is taken out of the runner's peers. There is no undo."
                : "Its runner's container and home are removed from your Docker, with everything chats left in them, and this machine is taken out of the runner's peers. There is no undo."}
          </p>
          <HeldLine held={held} elsewhere={elsewhere} />
          <NamedByLine named={named} />
        </>
      }
      // Pressed only once the question names what goes back (#4678) and what is discarded (#4860).
      main={{
        children: discarding.length > 0 ? "Remove pool…" : "Remove pool",
        variant: "danger",
        onPress: () => (discarding.length > 0 ? setAgain(true) : onRemove([])),
        isDisabled: named.state === "reading" || held.state === "reading",
      }}
    />
  );
}

/** `runner a`, or `runners a, b and c`. */
function runnersSaid(runners: readonly string[]): string {
  return runners.length < 2
    ? `runner ${runners.join("")}`
    : `runners ${runners.slice(0, -1).join(", ")} and ${runners[runners.length - 1]}`;
}

/**
 * What Remove pool's question says of the older runners that hold, or may hold, their chats'
 * conversations, which go with the pool (#4860).
 */
function HeldLine({ held, elsewhere }: { held: HeldRunners; elsewhere: boolean }) {
  const said = useId();
  if (held.state === "reading") return null;
  if (held.state === "trouble")
    return (
      <p className="came-back">
        {"purlis could not read whether its older runners still hold chats' conversations: "}
        <bdi>{held.why}</bdi>
        {". If one does, nothing is removed, and purlis says so."}
      </p>
    );
  if (held.held.length === 0) return null;
  return (
    <>
      <p className="came-back pool-named-lead" id={said}>
        {elsewhere
          ? "These older runners may still hold conversations from chats that ran on them. Removing the pool means purlis can no longer bring those conversations back:"
          : "These older runners may still hold conversations from chats that ran on them. Removing the pool deletes those conversations:"}
      </p>
      <ul className="came-back pool-named-by" aria-labelledby={said}>
        {held.held.map((runner) => (
          <li key={runner}>
            {"Runner "}
            <bdi>{runner}</bdi>
          </li>
        ))}
      </ul>
    </>
  );
}

/** What Remove pool's question says of the projects and workspaces that name the pool. */
function NamedByLine({ named }: { named: NamedBy }) {
  const said = useId();
  // The one local file is read in a moment: nothing is said before it is.
  if (named.state === "reading") return null;
  if (named.state === "trouble")
    return (
      <p className="came-back">
        {
          "Workspaces and projects set to run on it go back to this machine. purlis could not read which: "
        }
        <bdi>{named.why}</bdi>
      </p>
    );
  if (named.named.length === 0) return null;
  return (
    <>
      <p className="came-back pool-named-lead" id={said}>
        These go back to this machine:
      </p>
      <ul className="came-back pool-named-by" aria-labelledby={said}>
        {named.named.flatMap((one) => [
          ...(one.project
            ? [
                <li key={`${one.path}\0p`}>
                  {"Project "}
                  <bdi>{one.name}</bdi>
                  {", and the workspaces that follow it"}
                </li>,
              ]
            : []),
          ...one.workspaces.map((workspace) => (
            <li key={`${one.path}\0w\0${workspace}`}>
              {"Workspace "}
              <bdi>{workspace}</bdi>
              {" in "}
              <bdi>{one.name}</bdi>
            </li>
          )),
        ])}
      </ul>
    </>
  );
}

/**
 * **Discard runner {name}'s conversations?**, asked twice before a held runner goes with what its
 * chats left (#4633, ruled 2026-11-02): `first`, then `again`, each with Cancel as the way back.
 */
export function DiscardHeldRunner({
  name,
  ask,
  onGoOn,
  onCancel,
}: {
  name: string;
  ask: "first" | "again";
  onGoOn: () => void;
  onCancel: () => void;
}) {
  // Each ask is a dialog of its own, so the second opens with the keyboard on its Cancel and not
  // on the button the first was answered with: two presses never discard.
  return ask === "first" ? (
    <AlertDialog
      key="first"
      onClose={onCancel}
      title={`Discard runner ${name}'s conversations?`}
      description={
        <p className="came-back">
          Its container and home are removed from your Docker, with every conversation and
          everything else its chats left in them.
        </p>
      }
      main={{ children: "Discard…", variant: "danger", onPress: onGoOn }}
    />
  ) : (
    <AlertDialog
      key="again"
      onClose={onCancel}
      title="Discard them for good?"
      description={
        <p className="came-back">
          {`Runner ${name}'s conversations cannot be brought back after this. There is no undo.`}
        </p>
      }
      main={{ children: "Discard conversations", variant: "danger", onPress: onGoOn }}
    />
  );
}

function PoolEntry({
  pool,
  link,
  version,
  removing,
  remaking,
  remade,
  onAsk,
  onMakeAgain,
  notHeld,
  profiles,
  plane,
}: {
  pool: PoolRow;
  link: PoolLink | "asking" | undefined;
  version: PoolVersion | undefined;
  removing: boolean;
  /** Its runner is being made again (#4149). */
  remaking: boolean;
  /** What Make the runner again did, once it did. */
  remade: string | undefined;
  /** Asks whether to remove it. */
  onAsk: () => void;
  /** Makes its runner again, once it was removed outside purlis (#4149). */
  onMakeAgain: () => void;
  /** The marks of projects whose chats run on it, drawn on its row. */
  notHeld: ReactNode;
  /** This machine's profiles on its runner (#2049). */
  profiles: ReactNode;
  plane: string | undefined;
}) {
  const remove = onAsk;
  const owed = owedSaid(version, useNow());
  const left = leftSaid(version);
  return (
    <li className="ui-machine-entry">
      <span className="ui-machine-name">{pool.name}</span>
      <Tooltip tip={pool.image}>
        <span className="ui-machine-path">{limitsSaid(pool)}</span>
      </Tooltip>
      <Badge variant="accent">{linkSaid(link)}</Badge>
      {/* A pool's own setup, each step as it stands (#4491): never reached from Add (#6256). */}
      {plane !== undefined && (
        <Button
          size="sm"
          aria-label={`Open setup of pool ${pool.name}`}
          onPress={() => openSetupOf(pool.name)}
        >
          Open setup
        </Button>
      )}
      <Button
        size="sm"
        aria-label={removing ? `Removing pool ${pool.name}` : `Remove pool ${pool.name}`}
        aria-disabled={removing}
        onPress={removing ? undefined : remove}
      >
        {removing ? "Removing…" : "Remove"}
      </Button>
      <p className="ui-setting-help">{suspendSaid(pool, version)}</p>
      <KeyChangedNotice name={pool.name} link={link} removing={removing} onRemove={remove} />
      {typeof link === "object" && link.state === "removed" && (
        <RemovedNotice
          name={pool.name}
          homeKept={link.home_kept}
          remaking={remaking}
          onMakeAgain={onMakeAgain}
        />
      )}
      {typeof link === "object" && link.state === "on_another_engine" && (
        <p className="ui-setting-help">{anotherEngineSaid(pool.name)}</p>
      )}
      {remade !== undefined && <p className="ui-setting-help">{remade}</p>}
      {typeof link === "object" && link.state === "down" && (
        <p className="ui-setting-help">{link.why}</p>
      )}
      {versionSaid(version) !== undefined && (
        <p className="ui-setting-help">{versionSaid(version)}</p>
      )}
      {owed !== undefined && <p className="ui-setting-help">{owed}</p>}
      {left !== undefined && <p className="ui-setting-help">{left}</p>}
      {version?.not_checked.map((one) => (
        <p key={`not-checked:${one.runner}`} className="ui-setting-help">
          {notCheckedPoolSaid(one.runner, one.left)}
        </p>
      ))}
      {notHeld}
      {profiles}
      <PoolLimits pool={pool.name} plane={plane} />
    </li>
  );
}

/**
 * **A pool's runner key, or its cluster operator key, changed** (#3292): either refuses every
 * link to its runner, so the row says which, with Remove.
 */
function KeyChangedNotice({
  name,
  link,
  removing,
  onRemove,
}: {
  name: string;
  link: PoolLink | "asking" | undefined;
  removing: boolean;
  onRemove: () => void;
}) {
  if (typeof link !== "object") return null;
  if (link.state !== "key_changed" && link.state !== "operator_key_changed") return null;
  return (
    <Notice
      cause={`pool-key-changed:${name}`}
      tone="trouble"
      fixes={[{ label: "Remove", onPress: onRemove, busy: removing }]}
    >
      {link.state === "key_changed"
        ? `Pool ${name}'s runner answered with another key than the one this machine paired with, so purlis no longer links to it. Remove it, then add the pool again.`
        : `Pool ${name}'s cluster now names another cluster operator key than the one this machine pinned, so purlis no longer links to its runner. Check the new key with your cluster's admin, then remove the pool and add it again.`}
    </Notice>
  );
}

/** What marking a cluster pool as the person's own means (#4071): who else can read its runner. */
const MY_OWN_SAID =
  "Your cluster's admins, whoever holds the node or its storage, and anyone else allowed into this runner's pod can read what it holds, the vault values, tokens and logins purlis sends it among them. Turn this on only if you accept that. It stays off until you do, and purlis sends this runner nothing while it is off. Each vault value or token still asks you when it is sent. Your profiles, with their settings, go to it and sign in there while this is on. Either needs the cluster to vouch for the runner at that moment.";

/** What an unmarked cluster pool's row says of profiles (#3819): none goes there until it is marked. */
export const notMarkedSaid = (pool: string) =>
  `Your profiles go to pool ${pool}'s runner, and sign in there, only once you turn on This is my own.`;

/**
 * **A pool's runner was removed outside purlis** (#4149): Docker has no container of its id, so
 * no link to it comes back. The Notice says whether its home went too, with Make the runner
 * again.
 */
export function RemovedNotice({
  name,
  homeKept,
  remaking,
  onMakeAgain,
}: {
  name: string;
  homeKept: boolean;
  remaking: boolean;
  onMakeAgain: () => void;
}) {
  return (
    <Notice
      cause={`pool-runner-removed:${name}`}
      tone="trouble"
      fixes={[{ label: "Make the runner again", onPress: onMakeAgain, busy: remaking }]}
    >
      {removedSaid(name, homeKept)}
    </Notice>
  );
}

/** A cluster pool's row (#3292): its cluster's pool, where it is claimed, and Remove. */
function ClusterPoolEntry({
  pool,
  link,
  removing,
  onAsk,
  notHeld,
  profiles,
}: {
  pool: ClusterPoolRow;
  link: PoolLink | "asking" | undefined;
  removing: boolean;
  /** Asks whether to remove it. */
  onAsk: () => void;
  /** The marks of projects whose chats run on it, drawn on its row. */
  notHeld: ReactNode;
  /** Its profiles (#3819), drawn only while it is marked as the person's own. */
  profiles: ReactNode;
}) {
  const said = useId();
  const [refused, setRefused] = useState<string | undefined>();
  return (
    <li className="ui-machine-entry">
      <span className="ui-machine-name">{pool.name}</span>
      <span className="ui-machine-path">
        Pool {pool.pool} in {pool.context}, namespace {pool.namespace}
      </span>
      <Badge variant="accent">{linkSaid(link)}</Badge>
      <Button
        size="sm"
        aria-label={removing ? `Removing pool ${pool.name}` : `Remove pool ${pool.name}`}
        aria-disabled={removing}
        onPress={removing ? undefined : onAsk}
      >
        {removing ? "Removing…" : "Remove"}
      </Button>
      <p className="ui-setting-help">
        Cluster operator key <code>{pool.operator_key}</code>
      </p>
      <Switch
        isSelected={pool.owned}
        aria-describedby={said}
        onChange={(owned) => {
          void markPoolOwned(pool.name, owned).then(setRefused);
        }}
      >
        This is my own
      </Switch>
      <p className="ui-setting-help" id={said}>
        {MY_OWN_SAID}
      </p>
      {refused !== undefined && (
        <p className="ui-setting-help" role="alert">
          {refused}
        </p>
      )}
      <KeyChangedNotice name={pool.name} link={link} removing={removing} onRemove={onAsk} />
      {typeof link === "object" && link.state === "down" && (
        <p className="ui-setting-help">{link.why}</p>
      )}
      {pool.owned ? profiles : <p className="ui-setting-help">{notMarkedSaid(pool.name)}</p>}
      {notHeld}
      <PoolLimits pool={pool.name} />
    </li>
  );
}

/** The Notice of a project the runner its chats run on would not hold. */
export function NotHeldNotice({
  name,
  root,
  why,
  onDismiss,
}: {
  name: string;
  root: string;
  why: string;
  onDismiss: () => void;
}) {
  return (
    <Notice cause={`project-not-held:${root}`} tone="trouble" onDismiss={onDismiss}>
      The runner that {name}&apos;s chats run on does not hold {name}, so no chat starts or answers
      there. purlis asks it again the next time you start or use a chat in {name}. The runner said:{" "}
      <bdi>{why}</bdi>
    </Notice>
  );
}

const poolsList = (plane: string | undefined): LiveSetting => ({
  id: `${POOLS}.list`,
  label: "Pools on this machine",
  // What Remove does is said under the list, and only while there is a pool to remove: with
  // none, the group says what a pool is instead (the person's ruling of 2026-10-30).
  help: "",
  useControl: function usePoolsList() {
    const now = usePools();
    const removeSaid = useId();
    const notHeld = useProjectsNotHeld();
    // Put away until the runner says something else of the project.
    const [dismissed, setDismissed] = useState<ReadonlySet<string>>(new Set());
    const [asking, setAsking] = useState<string | undefined>();
    // Why the last Make the runner again was refused, in the core's words (#4149).
    const [remakeSaid, setRemakeSaid] = useState<string | undefined>();
    const shown = notHeld.filter((one) => !dismissed.has(`${one.root}\u0000${one.why}`));
    const clusters = now.read.state === "read" ? (now.read.shown.clusters ?? []) : [];
    const pools =
      now.read.state === "read"
        ? [...now.read.shown.pools.map((one) => one.name), ...clusters.map((one) => one.name)]
        : [];
    // Drawn once, on the row of the first pool it runs on that is listed here.
    const rowOf = (one: ProjectNotHeld) => one.pools.find((pool) => pools.includes(pool));
    const marks = (of: readonly ProjectNotHeld[]) =>
      of.map((one) => (
        <NotHeldNotice
          key={one.root}
          name={one.name}
          root={one.root}
          why={one.why}
          onDismiss={() => setDismissed((was) => new Set([...was, `${one.root}\u0000${one.why}`]))}
        />
      ));
    const names = now.read.state === "read" ? now.read.shown.pools.map((one) => one.name) : [];
    const asked = names.join("\u0000");
    const clustersAsked = clusters.map((one) => one.name).join("\u0000");
    // Each runner is asked once, as the group comes on screen or a pool is added.
    useEffect(() => {
      for (const name of asked === "" ? [] : asked.split("\u0000")) {
        void askLink(name);
        void askVersion(name);
      }
    }, [asked]);
    // A cluster pool's runner runs the cluster's version, which purlis does not move (#3292).
    useEffect(() => {
      for (const name of clustersAsked === "" ? [] : clustersAsked.split("\u0000")) {
        void askLink(name);
      }
    }, [clustersAsked]);
    return {
      grouped: true,
      error:
        now.said === undefined && remakeSaid === undefined
          ? undefined
          : [now.said, remakeSaid].filter((said) => said !== undefined),
      control: (ids) => (
        <div
          role="group"
          aria-labelledby={ids.labelledBy}
          aria-describedby={pools.length === 0 ? undefined : removeSaid}
        >
          {now.read.state === "reading" && <p className="ui-setting-help">Reading…</p>}
          {now.read.state === "trouble" && (
            <p className="ui-setting-help" role="alert">
              {now.read.trouble}
            </p>
          )}
          {now.read.state === "read" &&
            (pools.length === 0 ? (
              <p className="ui-setting-help">{NO_POOL_YET}</p>
            ) : (
              <ul className="ui-machine-list">
                {now.read.shown.pools.map((pool) => (
                  <PoolEntry
                    key={pool.name}
                    pool={pool}
                    link={now.links[pool.name]}
                    version={now.versions[pool.name]}
                    removing={now.removing === pool.name}
                    remaking={now.remaking === pool.name}
                    remade={now.remade[pool.name]}
                    onAsk={() => setAsking(pool.name)}
                    onMakeAgain={() => void makeRunnerAgain(pool.name, plane).then(setRemakeSaid)}
                    notHeld={marks(shown.filter((one) => rowOf(one) === pool.name))}
                    profiles={<PoolProfiles pool={pool.name} plane={plane} />}
                    plane={plane}
                  />
                ))}
                {clusters.map((pool) => (
                  <ClusterPoolEntry
                    key={pool.name}
                    pool={pool}
                    link={now.links[pool.name]}
                    removing={now.removing === pool.name}
                    onAsk={() => setAsking(pool.name)}
                    notHeld={marks(shown.filter((one) => rowOf(one) === pool.name))}
                    profiles={<PoolProfiles pool={pool.name} plane={plane} />}
                  />
                ))}
              </ul>
            ))}
          {pools.length > 0 && (
            <p className="ui-setting-help" id={removeSaid}>
              {REMOVE_SAID}
            </p>
          )}
          {asking !== undefined && (
            <RemovePool
              name={asking}
              cluster={clusters.some((one) => one.name === asking)}
              elsewhere={linkOnAnotherEngine(now.links[asking])}
              onCancel={() => setAsking(undefined)}
              onRemove={(discarding) => {
                setAsking(undefined);
                void removePool(asking, linkOnAnotherEngine(now.links[asking]), discarding);
              }}
            />
          )}
          {/* A project whose pool this machine cannot name is marked under the list. */}
          {marks(shown.filter((one) => rowOf(one) === undefined))}
          <ButtonRow>
            <Button onPress={() => startAddingAPool()}>Add a pool…</Button>
            <Button onPress={() => startAddingAPool("cluster")}>Add a cluster pool…</Button>
          </ButtonRow>
        </div>
      ),
    };
  },
});
