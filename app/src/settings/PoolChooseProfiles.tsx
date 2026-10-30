// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import {
  addProfileToPool,
  askProfiles,
  profilesHeld,
  removeProfileFromPool,
  usePools,
} from "../pools";
import { Button } from "../ui/button/Button";
import { Card } from "../ui/card/Card";
import { Checkbox } from "../ui/choice/Checkbox";
import { DRIVEN, PoolProfiles } from "./PoolProfiles";
import { PoolSignIn } from "./PoolSignIn";

/** Where choosing a new pool's profiles is: choosing, signing the chosen in, or done. */
type At =
  | { at: "choose" }
  | { at: "recording" }
  | { at: "walk"; names: { name: string; harness: string }[]; now: number }
  | { at: "lines" };

/** "1 profile", "2 profiles". */
const profiles = (n: number) => `${n} profile${n === 1 ? "" : "s"}`;

/**
 * **What adding pool `pool` ends with: the profiles that sign in on it** (#4048, spec #4023 Q4,
 * Q12, prototype Variant A). A strip lists this machine's profiles, the one the pool keeps
 * already ticked: on a pool just added, that is the project's default, the profile a new chat in
 * the open project `plane` gets. **Sign in N profiles** records the ticked ones on the pool (the
 * core's Add a profile… and Remove, so its runner keeps exactly them) and then signs each in, one
 * after another, in the same strip ({@link PoolSignIn}); each one's end, signed in or not, goes on
 * to the next. A profile whose harness's sign-in the window does not drive yet is recorded and
 * left to its line's own Sign in. **Not now** records nothing. Either ends on the pool's lines
 * ({@link PoolProfiles}), each not signed in offering Sign in.
 */
export function ChooseProfiles({ pool, plane }: { pool: string; plane: string | undefined }) {
  const asked = usePools().profiles[pool];
  const [at, setAt] = useState<At>({ at: "choose" });
  const [ticked, setTicked] = useState<ReadonlySet<string>>();
  const [trouble, setTrouble] = useState<string>();
  useEffect(() => {
    if (asked === undefined) void askProfiles(pool, plane);
  }, [pool, plane, asked]);
  const rows = asked !== undefined && asked !== "asking" && "kept" in asked ? asked : undefined;
  // What the pool keeps as it is first read is what is ticked: its default.
  useEffect(() => {
    if (rows !== undefined && ticked === undefined)
      setTicked(new Set(rows.kept.map((row) => row.name)));
  }, [rows, ticked]);

  const told = trouble !== undefined && (
    <p className="ui-setting-help" role="alert">
      {trouble}
    </p>
  );
  if (at.at === "lines")
    return (
      <>
        {told}
        <PoolProfiles pool={pool} plane={plane} />
      </>
    );
  if (at.at === "walk") {
    const one = at.names[at.now];
    const next = () => {
      if (at.now + 1 < at.names.length) setAt({ ...at, now: at.now + 1 });
      else {
        setAt({ at: "lines" });
        void askProfiles(pool, plane);
      }
    };
    return (
      <>
        {told}
        <p className="ui-setting-help">{`Profile ${at.now + 1} of ${at.names.length}: ${one.name}`}</p>
        <PoolSignIn
          key={one.name}
          pool={pool}
          profile={one.name}
          harness={one.harness}
          onEnd={next}
        />
      </>
    );
  }
  if (asked !== undefined && asked !== "asking" && !("kept" in asked)) {
    // The pool's lines say why, with Ask again.
    return <PoolProfiles pool={pool} plane={plane} />;
  }
  if (rows === undefined || ticked === undefined) {
    return <p className="ui-setting-help">Asking the runner about this machine&apos;s profiles…</p>;
  }
  const kept = new Set(rows.kept.map((row) => row.name));
  const all = [
    ...rows.kept.map((row) => ({ name: row.name, harness: row.harness, kept: true })),
    ...rows.addable.map((one) => ({ name: one.name, harness: one.harness, kept: false })),
  ];
  const tick = (name: string, on: boolean) => {
    const now = new Set(ticked);
    if (on) now.add(name);
    else now.delete(name);
    setTicked(now);
  };
  const signIn = async () => {
    setAt({ at: "recording" });
    const refused: string[] = [];
    for (const one of all.filter((one) => ticked.has(one.name) && !one.kept)) {
      const why = await addProfileToPool(pool, one.name, plane);
      if (why !== undefined) refused.push(why);
    }
    for (const one of all.filter((one) => !ticked.has(one.name) && one.kept)) {
      const why = await removeProfileFromPool(pool, one.name, plane);
      if (why !== undefined) refused.push(why);
    }
    if (refused.length > 0) setTrouble(refused.join(" "));
    // The lines as the core keeps them now: only those it keeps and could sign in are walked.
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
            .map((row) => ({ name: row.name, harness: row.harness }))
        : [];
    setAt(walked.length === 0 ? { at: "lines" } : { at: "walk", names: walked, now: 0 });
  };
  const n = ticked.size;
  return (
    <Card className="pool-sign-in" role="group" aria-label={`Profiles that sign in on pool ${pool}`}>
      <p className="ui-setting-help">
        Which of your profiles should chats on {pool} use? Each one you pick signs in on its
        runner once, there; nothing of this machine&apos;s sign-in goes there.
      </p>
      {all.map((one) => (
        <Checkbox
          key={one.name}
          isSelected={ticked.has(one.name)}
          onChange={(on) => tick(one.name, on)}
          isDisabled={at.at === "recording"}
          description={
            one.kept && kept.size > 0
              ? `${one.harness}, the project's default: the profile a new chat in it gets`
              : one.harness
          }
        >
          {one.name}
        </Checkbox>
      ))}
      <div className="pool-sign-in-foot">
        <Button
          variant="primary"
          isDisabled={n === 0 || at.at === "recording"}
          onPress={() => void signIn()}
        >
          {`Sign in ${profiles(n)}`}
        </Button>
        <Button
          variant="ghost"
          isDisabled={at.at === "recording"}
          onPress={() => setAt({ at: "lines" })}
        >
          Not now
        </Button>
      </div>
    </Card>
  );
}
