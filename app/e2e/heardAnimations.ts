// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** What {@link listenForAnimations} keeps on the page's window. */
export type HeardAnimations = {
  /** The names heard starting, by the selector each was heard on. */
  heard: Record<string, string[]>;
  /** Takes the listener off. */
  stop: () => void;
};

export type WithHeardAnimations = { heardAnimations?: HeardAnimations };

/**
 * **Runs in the page: hears each animation that starts on what `selectors` name** (#3101), for
 * `hearAnimations` (`animations.ts`). It stands apart from the driver so a unit test can run it,
 * and it closes over nothing, because the driver sends its source to the page.
 *
 * **An animation with no duration moves nothing, and is not heard.** Reduced motion and a theme's
 * 0 both leave the animation there at `0s`, and the engine still fires `animationstart` for it.
 * Hearing it would report motion where nothing moved.
 */
export function listenForAnimations(selectors: string[]): void {
  const kept = window as unknown as WithHeardAnimations;
  kept.heardAnimations?.stop();
  const heard: Record<string, string[]> = {};
  for (const selector of selectors) heard[selector] = [];
  const listen = (event: AnimationEvent) => {
    const on = event.target;
    if (!(on instanceof Element)) return;
    const style = getComputedStyle(on, event.pseudoElement || undefined);
    const names = style.animationName.split(",").map((one) => one.trim());
    const durations = style.animationDuration.split(",").map((one) => one.trim());
    const at = names.indexOf(event.animationName);
    // A list of durations shorter than its names repeats. A name gone already says nothing, and
    // an animation that started did start.
    if (at >= 0 && !(Number.parseFloat(durations[at % durations.length] ?? "0") > 0)) return;
    for (const selector of selectors)
      if (on.matches(selector)) heard[selector]?.push(event.animationName);
  };
  document.addEventListener("animationstart", listen, true);
  kept.heardAnimations = {
    heard,
    stop: () => document.removeEventListener("animationstart", listen, true),
  };
}
