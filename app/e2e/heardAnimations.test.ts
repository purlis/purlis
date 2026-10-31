// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { listenForAnimations, type WithHeardAnimations } from "./heardAnimations.js";

const kept = () => (window as unknown as WithHeardAnimations).heardAnimations;
const heardOn = (css: string) => kept()?.heard[css];

/** A part wearing `animation`, in the page. */
function part(className: string, animation: { name: string; duration: string }): HTMLElement {
  const it = document.createElement("div");
  it.className = className;
  it.style.animationName = animation.name;
  it.style.animationDuration = animation.duration;
  document.body.append(it);
  return it;
}

/** The engine saying `name` started on `on`. */
function starts(on: Element, name: string): void {
  on.dispatchEvent(
    Object.assign(new Event("animationstart", { bubbles: true }), { animationName: name }),
  );
}

afterEach(() => {
  kept()?.stop();
  document.body.replaceChildren();
});

describe("hearing the animations that start (#3101)", () => {
  it("hears a name on what its selector names, and on nothing else", () => {
    const right = part("right", { name: "purlis-open", duration: "0.2s" });
    const left = part("left", { name: "purlis-reveal", duration: "0.2s" });
    listenForAnimations([".right", ".elsewhere"]);
    starts(right, "purlis-open");
    starts(left, "purlis-reveal");
    expect(heardOn(".right")).toEqual(["purlis-open"]);
    expect(heardOn(".elsewhere")).toEqual([]);
  });

  it("hears each selector apart", () => {
    const right = part("half right", { name: "purlis-reveal", duration: "0.2s" });
    listenForAnimations([".right", ".left"]);
    starts(right, "purlis-reveal");
    expect(heardOn(".right")).toEqual(["purlis-reveal"]);
    expect(heardOn(".left")).toEqual([]);
  });

  it("keeps what it heard after the part's mark has come off", () => {
    const right = part("right arrived", { name: "purlis-open", duration: "0.2s" });
    listenForAnimations([".arrived"]);
    starts(right, "purlis-open");
    right.classList.remove("arrived");
    expect(heardOn(".arrived")).toEqual(["purlis-open"]);
  });

  it("hears nothing of an animation with no duration, as reduced motion leaves it", () => {
    const right = part("right", { name: "purlis-open", duration: "0s" });
    listenForAnimations([".right"]);
    starts(right, "purlis-open");
    expect(heardOn(".right")).toEqual([]);
  });

  it("reads the duration of the name that started, among several", () => {
    const right = part("right", { name: "still, moving", duration: "0s, 0.3s" });
    listenForAnimations([".right"]);
    starts(right, "still");
    starts(right, "moving");
    expect(heardOn(".right")).toEqual(["moving"]);
  });

  it("starts afresh on a second call, and the first listener stops hearing", () => {
    const right = part("right", { name: "purlis-open", duration: "0.2s" });
    listenForAnimations([".right"]);
    starts(right, "purlis-open");
    listenForAnimations([".right"]);
    expect(heardOn(".right")).toEqual([]);
    starts(right, "purlis-open");
    expect(heardOn(".right")).toEqual(["purlis-open"]);
  });
});
