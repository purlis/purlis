// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { dequal } from "dequal/lite";
import { commands, type HarnessGlance, type PlaneId } from "./bindings";

/**
 * **Every harness card a project has, without an open chat** (HP-19 follow-up, #1134): the
 * palette lists a row per harness, *What Codex can do here*, that opens its card tab.
 *
 * The cards are the core's (`harness_cards`, `purlis_core::harness_card::read`), the same ones
 * the picker and a chat's header draw. Only the declarations are read, never a program, so this
 * is not the picker's `start_options`, which may ask a program its version.
 *
 * Read while the project is in front: again when it comes back in front, when `changes`
 * moves (the project's settings, where its harnesses are declared), and when the window comes
 * back into focus, as the curation rows are: a profile file edited by hand names a harness the
 * off rows' reasons link to, and that link's card row must be there too. A project whose harnesses
 * cannot be read has none to list; the picker says why when it is asked.
 *
 * **Neither a focus nor an equal answer draws the project's view again** (felt speed, round 3,
 * H7): the focus asks where it is heard, never counted in the view's state, and an answer equal
 * to the cards held sets nothing.
 */
export function useHarnessCards(
  plane: PlaneId,
  inFront: boolean,
  changes: number,
): readonly HarnessGlance[] {
  const [cards, setCards] = useState<readonly HarnessGlance[]>([]);
  const held = useRef(cards);
  /** Reads the cards again, outside a render: set by the effect that reads them. */
  const readAgain = useRef<() => void>(() => {});
  useEffect(() => {
    if (!inFront) return;
    let gone = false;
    const read = () =>
      void commands
        .harnessCards(plane)
        .then((answer) => {
          if (gone || answer.status !== "ok" || !Array.isArray(answer.data)) return;
          if (dequal(held.current, answer.data)) return;
          held.current = answer.data;
          setCards(answer.data);
        })
        .catch(() => {
          // No core to ask (a test, a window going away): nothing to list.
        });
    readAgain.current = read;
    read();
    return () => {
      gone = true;
      readAgain.current = () => {};
    };
  }, [plane, inFront, changes]);
  // Coming back in front is read above, as `inFront` moves; a focus is heard here, in front alone.
  useEffect(() => {
    if (!inFront) return;
    const again = () => readAgain.current();
    window.addEventListener("focus", again);
    return () => window.removeEventListener("focus", again);
  }, [inFront]);
  return cards;
}
