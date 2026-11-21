// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from "react";
import { commands, type HeldActShown, type PlaneId } from "./bindings";
import { ASKS_CHANGED } from "./permissionAsks";
import { asksMoved, onAsksMoved } from "./asks";
import { listen } from "./here";
import { Button, ButtonRow } from "./ui/button/Button";
import { Card, CardTitle } from "./ui/card/Card";
import { Fact, Facts } from "./ui/card/Facts";
import { CodeBlock } from "./ui/code/CodeBlock";

/**
 * **Held forge acts in Asks** (one login F2; ADR 0128 D8): a tag, a release, a push that changes
 * CI files or a fork that a chat asked for. A chat holds no forge credential, so it did not do
 * it. It asked, and **the person's press makes it, with their own sign-in**.
 *
 * Each ask shows **exactly what purlis read of the forge and the clone** (`HeldActShown`): every
 * row the digest covers and the long text under them (a release's notes, a CI file's changed
 * lines), as plain text a chat may have written. Nothing is drawn that the press does not make,
 * and a press names the digest of what was drawn, so it performs only that.
 *
 * - **Two presses.** The first reads the forge again and asks once more, in a sentence of
 *   what will be made; the second, naming the nonce the first returned, makes it.
 * - **Don't** puts the ask away and makes nothing, at either step.
 * - **Never a rule.** Nothing here is remembered: the next ask is asked afresh.
 */

/** The verb of a kind's press, and what the sentence asked once more says it makes. */
const VERBS: Record<string, { press: string; making: string }> = {
  tag: { press: "Make the tag", making: "make this tag" },
  release: { press: "Make the release", making: "make this release" },
  "CI-file push": { press: "Push the branch", making: "push this branch with the second sign-in" },
  fork: { press: "Fork it", making: "make this fork in your own account" },
};

/** The verbs of a kind this window does not know yet: said plainly, never blank. */
const verbsOf = (kind: string) => VERBS[kind] ?? { press: "Do it", making: `do this ${kind}` };

/** What an ask's title says: who asked, and for what. */
export const askedSaid = (act: HeldActShown) =>
  `Chat ${act.session} asked for a ${act.kind} in ${act.repo}`;

/** The question the first press asks once more. */
export const onceMoreSaid = (act: HeldActShown) =>
  `Once more: ${verbsOf(act.kind).making}? It is made with your sign-in, as shown.`;

/** Where the first press has been made: the nonce the second names. */
export type FirstPress = { nonce: string; digest: string };

/**
 * **One held act, as its row draws it.** `first` is set once the first press was answered, so
 * the row asks once more; `refused` is the app's sentence where a press did nothing.
 */
export function HeldActRow({
  act,
  first,
  busy = false,
  refused,
  onPress,
  onCancel,
  onDont,
}: {
  act: HeldActShown;
  first?: FirstPress;
  busy?: boolean;
  refused?: string;
  onPress: () => void;
  onCancel: () => void;
  onDont: () => void;
}) {
  const verb = verbsOf(act.kind);
  const asking = first !== undefined;
  return (
    <Card as="li" variant="ask" aria-label={askedSaid(act)} data-ask={act.ask}>
      <CardTitle>{askedSaid(act)}</CardTitle>
      <Facts aria-label={`What the ${act.kind} is`}>
        {act.rows.map((row) => (
          <Fact key={row.label} label={row.label}>
            {row.value}
          </Fact>
        ))}
      </Facts>
      {act.body !== null && <CodeBlock label={act.body.label} text={act.body.text} />}
      {asking && <p role="status">{onceMoreSaid(act)}</p>}
      {refused !== undefined && <p role="status">{refused}</p>}
      <ButtonRow size="sm">
        <Button variant="primary" isDisabled={busy} onPress={onPress}>
          {asking ? `Yes, ${verb.press.toLowerCase()}` : verb.press}
        </Button>
        {asking && (
          <Button isDisabled={busy} onPress={onCancel}>
            Cancel
          </Button>
        )}
        <Button variant="danger" isDisabled={busy} onPress={onDont}>
          Don&apos;t
        </Button>
      </ButtonRow>
    </Card>
  );
}

/** `record` without `key`. */
function without<T>(record: Record<number, T>, key: number): Record<number, T> {
  return Object.fromEntries(Object.entries(record).filter(([one]) => Number(one) !== key));
}

/** The asks of one project, as Asks draws them, each with its own two presses. */
export function HeldActAsks({ plane }: { plane: PlaneId }) {
  const [acts, setActs] = useState<readonly HeldActShown[]>([]);
  const [firsts, setFirsts] = useState<Record<number, FirstPress>>({});
  const [refusals, setRefusals] = useState<Record<number, string>>({});
  const [busy, setBusy] = useState<number>();
  /** What the last act made, said where its row was: the ask is gone once it is made. */
  const [made, setMade] = useState<string>();

  const read = useCallback(() => {
    void commands
      .heldActsWaiting(plane)
      .then((listed) => {
        // A core that lists nothing (an older one, a test's stand-in) holds none.
        if (listed?.status === "ok" && Array.isArray(listed.data)) setActs(listed.data);
      })
      .catch(() => undefined);
  }, [plane]);
  useEffect(() => {
    read();
    const stop = onAsksMoved((moved) => {
      if (moved === plane) read();
    });
    const heard = listen<{ plane: string }>(ASKS_CHANGED, (said) => {
      if (said.payload?.plane === plane) read();
    }).catch(() => undefined);
    return () => {
      stop();
      void heard.then((off) => off?.()).catch(() => undefined);
    };
  }, [plane, read]);

  const press = (act: HeldActShown) => {
    if (busy !== undefined) return;
    setBusy(act.ask);
    setRefusals((was) => without(was, act.ask));
    // A first press of a row that is gone (the ask now shows other facts) is no first press.
    const first = firsts[act.ask]?.digest === act.digest ? firsts[act.ask] : undefined;
    void commands
      .pressHeldAct(plane, act.ask, act.digest, first?.nonce ?? null)
      .then((answer) => {
        if (answer.status === "error") {
          setRefusals((was) => ({ ...was, [act.ask]: answer.error }));
          return;
        }
        if (answer.data.step === "asks-once-more") {
          const again = answer.data;
          setFirsts((was) => ({
            ...was,
            [act.ask]: { nonce: again.nonce, digest: again.shown.digest },
          }));
          setActs((was) => was.map((one) => (one.ask === act.ask ? again.shown : one)));
        } else {
          const said = answer.data.said;
          setFirsts((was) => without(was, act.ask));
          setMade(said);
        }
      })
      .catch((err: unknown) =>
        setRefusals((was) => ({ ...was, [act.ask]: `purlis could not send it: ${String(err)}` })),
      )
      .finally(() => {
        setBusy(undefined);
        asksMoved(plane);
      });
  };

  const dont = (act: HeldActShown) => {
    if (busy !== undefined) return;
    setBusy(act.ask);
    void commands
      .dontHeldAct(plane, act.ask)
      .catch(() => undefined)
      .finally(() => {
        setBusy(undefined);
        setFirsts((was) => without(was, act.ask));
        asksMoved(plane);
      });
  };

  if (acts.length === 0 && made === undefined) return null;
  return (
    <>
      {made !== undefined && <p role="status">{made}</p>}
      <ul aria-label="Held forge acts" className="held-acts">
        {acts.map((act) => (
          <HeldActRow
            key={act.ask}
            act={act}
            first={firsts[act.ask]?.digest === act.digest ? firsts[act.ask] : undefined}
            busy={busy === act.ask}
            refused={refusals[act.ask]}
            onPress={() => press(act)}
            onCancel={() => setFirsts((was) => without(was, act.ask))}
            onDont={() => dont(act)}
          />
        ))}
      </ul>
    </>
  );
}
