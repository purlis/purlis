// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from "react";
import {
  commands,
  type PlaneId,
  type PoolSetupSee,
  type PoolSetupSizes,
  type PoolSetupStep,
} from "../bindings";
import {
  addProfileToPool,
  askProfiles,
  moveRunnerToThisVersion,
  profilesHeld,
  removeProfileFromPool,
  saveLimits,
  usePools,
} from "../pools";
import { PoolLimits, ReplaceRunner } from "../settings/PoolLimits";
import { DRIVEN, PoolProfiles } from "../settings/PoolProfiles";
import { PoolSignIn } from "../settings/PoolSignIn";
import { Badge } from "../ui/badge/Badge";
import { Button } from "../ui/button/Button";
import { Card } from "../ui/card/Card";
import { Checkbox } from "../ui/choice/Checkbox";
import { Meter } from "../ui/meter/Meter";
import { Action, Busy, Options, Say, SeeWhat, StepHead } from "./parts";

/** What a cluster pool's sign-in is, in the person's words of 2026-11-01 (#3709, on #4482). */
export const STAYS_ON_THE_RUNNER =
  "On this pool, your sign-in stays on your runner's own storage. If you remove your runner, purlis asks you to sign in again in the same place.";

/** The core's words as a sentence of their own. */
const sentence = (said: string) =>
  `${said.charAt(0).toUpperCase()}${said.slice(1)}${said.endsWith(".") ? "" : "."}`;

/** What helps where a runner's older purlis holds a profile and the window can't move it (#5175). */
const OLDER_RUNNER_HELP: Record<"own-image" | "cluster", string> = {
  "own-image":
    "purlis never moves your team's image: build it from this version of purlis, remove the pool, then add it again.",
  cluster:
    "Ask whoever looks after this pool in the cluster to put its runner on the current image.",
};

/** "1 profile", "2 profiles". */
const profiles = (n: number) => `${n} profile${n === 1 ? "" : "s"}`;

/** One of this machine's profiles, as the step lists it. */
type Row = {
  name: string;
  kind: string;
  harness: string;
  /** The pool keeps it already. */
  kept: boolean;
  signedIn: boolean;
};

type Walk = { names: { name: string; kind: string; harness: string }[]; now: number };

/**
 * **The Sign-ins step** (#4482, Q41, stories 28 to 31): this machine's profiles, the ones in use
 * ticked, each one's size against what the pool lets a profile send, an offer to raise that in
 * place, and each sign-in inside the step through the harness's own flow ({@link PoolSignIn}).
 * It can be skipped and come back to.
 */
export function SignInsStep({
  step,
  pool,
  plane,
  changed,
}: {
  step: PoolSetupStep;
  pool: string;
  plane: PlaneId;
  changed: () => void;
}) {
  const pools = usePools();
  const asked = pools.profiles[pool];
  const limits = pools.limits[pool];
  const [sizes, setSizes] = useState<PoolSetupSizes>();
  const [unmeasured, setUnmeasured] = useState<string>();
  const [chose, setTicked] = useState<ReadonlySet<string>>();
  const [walk, setWalk] = useState<Walk>();
  const [recording, setRecording] = useState(false);
  const [said, setSaid] = useState<string>();
  /** Recorded, with a profile left to sign in on its own line: its lines are shown. */
  const [lines, setLines] = useState(false);
  /** The rows drawn as a save began, held while it asks the runner again, so the step stays put. */
  const [heldRows, setHeldRows] = useState<Row[]>();
  /** The dialog before the runner is moved to the current image, and the move itself (#5175). */
  const [movingAsked, setMovingAsked] = useState(false);
  const [moving, setMoving] = useState(false);

  const measure = useCallback(() => {
    void commands.poolSetupSizes(plane, pool).then((read) => {
      if (read.status === "ok") {
        setSizes(read.data);
        setUnmeasured(undefined);
      } else setUnmeasured(read.error);
    });
  }, [plane, pool]);
  useEffect(() => {
    void askProfiles(pool, plane);
  }, [pool, plane]);
  // Measured again whenever the pool's limits are read again: a limit raised under Options.
  useEffect(() => {
    measure();
  }, [measure, limits]);

  const read: Row[] | undefined =
    asked !== undefined && asked !== "asking" && "kept" in asked
      ? [
          ...asked.kept.map((row) => ({
            name: row.name,
            kind: row.kind,
            harness: row.harness,
            kept: true,
            signedIn: row.standing === "signed-in",
          })),
          ...asked.addable.map((one) => ({
            name: one.name,
            kind: "",
            harness: one.harness,
            kept: false,
            signedIn: false,
          })),
        ]
      : undefined;
  const rows = read ?? (recording ? heldRows : undefined);

  // Ticked until the person changes it: what the pool keeps, and what is in use (story 28).
  const used = new Set(sizes?.profiles.filter((one) => one.in_use).map((one) => one.name));
  const ticked =
    chose ??
    new Set((rows ?? []).filter((row) => row.kept || used.has(row.name)).map((row) => row.name));
  // What the button would run, by hand, as the core spells it for what is ticked (#4718, Q30).
  const adding = (rows ?? []).filter((row) => ticked.has(row.name) && !row.kept);
  const takingOff = (rows ?? []).filter((row) => !ticked.has(row.name) && row.kept);
  const signing = (rows ?? []).filter((row) => ticked.has(row.name) && !row.signedIn);
  const handKey = JSON.stringify([
    adding.map((row) => row.name),
    takingOff.map((row) => row.name),
    signing.map((row) => row.name),
  ]);
  /** Not asked yet, or what the core spelled: `null` where the button would run nothing. */
  const [byHand, setByHand] = useState<PoolSetupSee | null | undefined>(undefined);
  useEffect(() => {
    const [add, off, sign] = JSON.parse(handKey) as [string[], string[], string[]];
    let current = true;
    void commands.poolSetupSignInsByHand(pool, add, off, sign).then((said) => {
      if (current) setByHand(said.status === "ok" ? said.data : undefined);
    });
    return () => {
      current = false;
    };
  }, [pool, handKey]);

  // Work done by hand is picked up as the core reads the step anew (#4718): a profile recorded
  // with `purlis pool add-profile` shows ticked once the runner is asked again.
  const stepRead = JSON.stringify([step.state, step.line, step.findings, step.next?.id]);
  const lastRead = useRef(stepRead);
  useEffect(() => {
    if (lastRead.current === stepRead) return;
    lastRead.current = stepRead;
    if (!recording) void askProfiles(pool, plane, { quietly: true });
  }, [stepRead, recording, pool, plane]);
  const skip = (skipped: boolean) => {
    setSaid(undefined);
    setLines(false);
    void commands.poolSetupSkip(pool, "signins", skipped).then((done) => {
      if (done.status === "error") setSaid(done.error);
      else changed();
    });
  };
  const told = said && (
    <p className="pool-setup-does" role="status">
      {said}
    </p>
  );
  const skipButton = step.skippable && (
    <Button variant="ghost" onPress={() => skip(true)}>
      Skip for now
    </Button>
  );
  const head = (
    <StepHead
      title="Choose the profiles for this pool"
      lead="A profile holds your Claude, Codex or other account. A chat on the pool can use the ones you tick. The ones you use are ticked for you."
    />
  );

  if (step.state === "skipped")
    return (
      <>
        {head}
        <Say tone="attention" title="You skipped this for now">
          Chats on {pool} start on the profiles its runner has of its own until you choose yours.
        </Say>
        {told}
        <Action main="Choose profiles now" onMain={() => skip(false)} />
      </>
    );

  // What the core found that this list can't help with: a runner asleep, no project, and so on.
  const blocked =
    step.next !== null && step.next.id !== "choose-profiles" && step.next.id !== "sign-in";
  if (blocked && step.state !== "done")
    return (
      <>
        <StepHead title="Sign-ins" />
        {step.findings.map((finding) => (
          <p key={finding} className="pool-setup-p">
            {finding}
          </p>
        ))}
        {told}
        {step.next?.id === "check-again" ? (
          <Action does={step.next.does} main={step.next.label} onMain={changed}>
            {skipButton}
          </Action>
        ) : step.next?.id === "wake-runner" ? (
          <Action
            does={step.next.does}
            main={step.next.label}
            busy={recording}
            onMain={() => {
              setRecording(true);
              setSaid(undefined);
              void commands.poolSetupWake(pool).then((woke) => {
                setRecording(false);
                if (woke.status === "error") setSaid(woke.error);
                else changed();
              });
            }}
          >
            {skipButton}
          </Action>
        ) : (
          <Action does={step.next?.does}>{skipButton}</Action>
        )}
      </>
    );

  if (walk !== undefined) {
    const one = walk.names[walk.now];
    const next = () => {
      if (walk.now + 1 < walk.names.length) setWalk({ ...walk, now: walk.now + 1 });
      else {
        setWalk(undefined);
        void askProfiles(pool, plane).then(() => {
          if (!lines) changed();
        });
      }
    };
    return (
      <>
        {head}
        <Card variant="ask">
          <h2 className="pool-setup-h2">{`Sign in ${one.name}`}</h2>
          <p className="pool-setup-does">{`Profile ${walk.now + 1} of ${walk.names.length}`}</p>
          <PoolSignIn
            key={one.name}
            pool={pool}
            profile={one.name}
            kind={one.kind}
            harness={one.harness}
            onEnd={next}
          />
        </Card>
      </>
    );
  }

  if (rows === undefined || sizes === undefined)
    return (
      <>
        {head}
        {asked !== undefined && asked !== "asking" && !("kept" in asked) ? (
          <Say
            tone="trouble"
            title="purlis couldn't ask the runner"
            actions={<Button onPress={() => void askProfiles(pool, plane)}>Ask again</Button>}
          >
            {asked.trouble}
          </Say>
        ) : unmeasured ? (
          <Say
            tone="trouble"
            title="purlis couldn't measure your profiles"
            actions={<Button onPress={measure}>Measure again</Button>}
          >
            {unmeasured}
          </Say>
        ) : (
          <Busy>Asking the runner about your profiles. This only reads.</Busy>
        )}
      </>
    );

  const sizeOf = (name: string) => sizes.profiles.find((one) => one.name === name);
  const overOf = (name: string) => (ticked.has(name) ? (sizeOf(name)?.over ?? []) : []);
  const anyOver = rows.some((row) => overOf(row.name).length > 0);
  const tick = (name: string, on: boolean) => {
    const now = new Set(ticked);
    if (on) now.add(name);
    else now.delete(name);
    setTicked(now);
  };
  const needs = rows.filter((row) => ticked.has(row.name) && !row.signedIn);
  const changes = rows.some((row) => row.kept !== ticked.has(row.name));
  const raise = (key: string, to: number) =>
    void saveLimits(pool, [[key, to]]).then((refused) => {
      // Saving reads the pool's limits again, which measures again.
      setSaid(refused);
    });

  const go = async () => {
    setHeldRows(rows);
    setRecording(true);
    const notes: string[] = [];
    for (const row of rows.filter((one) => ticked.has(one.name) && !one.kept)) {
      const why = await addProfileToPool(pool, row.name, plane);
      if (why !== undefined) notes.push(why);
    }
    for (const row of rows.filter((one) => !ticked.has(one.name) && one.kept)) {
      const why = await removeProfileFromPool(pool, row.name, plane);
      if (why !== undefined) notes.push(why);
    }
    setRecording(false);
    setSaid(notes.length > 0 ? notes.join(" ") : undefined);
    // The pool's profiles as the core keeps them now: only those it signs in in place are walked.
    const now = profilesHeld(pool);
    const walked =
      now !== undefined && now !== "asking" && "kept" in now
        ? now.kept
            .filter(
              (row) =>
                ticked.has(row.name) &&
                row.not_kept === null &&
                row.standing !== "signed-in" &&
                DRIVEN.has(row.kind),
            )
            .map((row) => ({ name: row.name, kind: row.kind, harness: row.harness }))
        : [];
    // Those whose sign-in purlis doesn't drive keep their own line's Sign in.
    const own =
      now !== undefined && now !== "asking" && "kept" in now
        ? now.kept.some(
            (row) =>
              ticked.has(row.name) &&
              row.not_kept === null &&
              row.standing !== "signed-in" &&
              !DRIVEN.has(row.kind),
          )
        : false;
    if (own) setLines(true);
    if (walked.length > 0) setWalk({ names: walked, now: 0 });
    else if (!own) changed();
  };

  const moveRunner = async () => {
    setMovingAsked(false);
    setMoving(true);
    setSaid(undefined);
    const why = await moveRunnerToThisVersion(pool, plane);
    setMoving(false);
    if (why !== undefined) {
      setSaid(why);
      return;
    }
    measure();
    changed();
  };

  const most = sizes.most_mib;
  if (lines)
    return (
      <>
        {head}
        <p className="pool-setup-does">
          purlis signs these in from their own line: press Sign in beside each one.
        </p>
        <PoolProfiles pool={pool} plane={plane} />
        {told}
        <Action main="Continue" onMain={changed}>
          <Button variant="ghost" onPress={() => setLines(false)}>
            Change the profiles
          </Button>
          {skipButton}
        </Action>
      </>
    );
  const none = ticked.size === 0;
  return (
    <>
      {head}
      {asked !== undefined && asked !== "asking" && "kept" in asked && asked.fresh_claim && (
        <Say tone="attention" title="Where your sign-in is kept">
          {STAYS_ON_THE_RUNNER}
        </Say>
      )}
      <ul className="pool-setup-profiles" aria-label="Your profiles">
        {rows.map((row) => {
          const size = sizeOf(row.name);
          const over = overOf(row.name);
          const mib = size?.mib ?? null;
          return (
            <li key={row.name} className="pool-setup-profile">
              <div className="pool-setup-profile-top">
                <Checkbox
                  isSelected={ticked.has(row.name)}
                  isDisabled={recording}
                  onChange={(on) => tick(row.name, on)}
                  description={
                    size?.in_use ? `${row.harness}. In use in this project.` : row.harness
                  }
                >
                  {row.name}
                </Checkbox>
                {row.signedIn ? (
                  <Badge variant="ok">Signed in</Badge>
                ) : (
                  <Badge>Not signed in</Badge>
                )}
              </div>
              {mib !== null && most !== undefined ? (
                <Meter
                  label="Size against this pool's limit"
                  value={mib}
                  minValue={0}
                  maxValue={Math.max(most, mib, 1)}
                  valueLabel={`${mib} of ${most} MiB${over.length > 0 ? ", over the limit" : ""}`}
                />
              ) : (
                size?.unread && (
                  <p className="pool-setup-does">{`purlis couldn't measure it: ${size.unread}`}</p>
                )
              )}
              {over.map((hit) => (
                <Say
                  key={hit.key}
                  tone="attention"
                  title={`${row.name} is too big to send`}
                  actions={
                    <>
                      {hit.raise_to !== null && (
                        <Button variant="primary" onPress={() => raise(hit.key, hit.raise_to ?? 0)}>
                          {`Raise the limit to ${hit.raise_to}${hit.key === "profileSize" ? " MiB" : " files"}`}
                        </Button>
                      )}
                      {hit.older_runner === "move" && (
                        <Button
                          variant="primary"
                          isDisabled={moving}
                          onPress={() => setMovingAsked(true)}
                        >
                          {moving ? "Replacing…" : "Replace the runner…"}
                        </Button>
                      )}
                      <Button variant="ghost" onPress={() => tick(row.name, false)}>
                        Leave this profile out
                      </Button>
                    </>
                  }
                >
                  {hit.older_runner === null
                    ? `${sentence(hit.said)} Raising the limit only changes this pool, and applies from the next send.`
                    : hit.older_runner === "move"
                      ? sentence(hit.said)
                      : `${sentence(hit.said)} ${OLDER_RUNNER_HELP[hit.older_runner]}`}
                </Say>
              ))}
            </li>
          );
        })}
      </ul>
      {told}
      {movingAsked && (
        <ReplaceRunner
          pool={pool}
          onCurrentImage
          onCancel={() => setMovingAsked(false)}
          onReplace={() => void moveRunner()}
        />
      )}
      {none && (
        <p className="pool-setup-does">Tick a profile for chats on {pool}, or skip this for now.</p>
      )}
      <Options>
        <PoolLimits pool={pool} plane={plane} />
      </Options>
      {byHand ? (
        <SeeWhat text={byHand.text} note={byHand.note} plane={plane} />
      ) : (
        // Only until the core has spelled what is ticked: its own read doesn't know the ticks.
        byHand === undefined &&
        step.see && <SeeWhat text={step.see.text} note={step.see.note} plane={plane} />
      )}
      <Action
        does={
          anyOver || none
            ? undefined
            : needs.length > 0
              ? "Sends the ticked profiles' settings to your runner, then opens each sign-in that's needed. Nothing of this machine's sign-in goes there."
              : "Sends the ticked profiles' settings to your runner."
        }
        reserve={["Sign in 9 profiles", "Save and continue", "Continue"]}
        main={
          anyOver || none
            ? undefined
            : needs.length > 0
              ? `Sign in ${profiles(needs.length)}`
              : changes
                ? "Save and continue"
                : "Continue"
        }
        busy={recording}
        onMain={() => void go()}
      >
        {skipButton}
      </Action>
    </>
  );
}
