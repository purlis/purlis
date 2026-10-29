// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Faded words, measured as drawn** (#3790, #3637).
 *
 * The theme engine holds each pair of tokens to its floor solid (`purlis_core::theme::check`),
 * and axe in the stories holds what it can resolve. Neither sees words a stylesheet fades with
 * `opacity`: a rule in one file can fade a box whose words another file colours, or fade an
 * ancestor of them, and only the cascade knows. So this reads what the browser computed. For
 * each element with words of its own it walks up the ancestors, laying the words and, beside
 * them, the ground they sit on through each box's background and each box's `opacity` and
 * `filter: opacity()`, down to the page, and measures the two pixels it ends with against WCAG
 * 1.4.3's floor: 4.5:1, or 3:1 for large text.
 *
 * What it reads is the element's ancestors, not what the page stacks under it: a word placed
 * over a sibling's fill by `position` is measured on its ancestors' ground. Only a background
 * colour is a ground; an image or gradient is not read. Words faded by nothing are left to axe
 * and the engine; words in a control that is disabled (WCAG 1.4.3 exempts an inactive control),
 * words hidden from assistive technology (a separator drawn as a glyph), words at opacity 0
 * (nothing is drawn) and words whose opacity is moving (an animation or a transition, which has
 * no one value to measure) are not measured.
 *
 * `.storybook/faded.setup.ts` runs it after every story, in Chromium.
 */

/** A colour as the browser draws it: 0 to 255 for each channel, 0 to 1 for alpha. */
export type Rgba = { r: number; g: number; b: number; a: number };

/** One box from the words up: its background, and how much it fades all it draws. */
export type Layer = { background: Rgba; opacity: number };

/** The page under everything: white, as a browser's canvas is. */
const PAGE: Rgba = { r: 255, g: 255, b: 255, a: 1 };

/** `top` laid over `under`, as a browser composites one over the other. */
function over(top: Rgba, under: Rgba): Rgba {
  const a = top.a + under.a * (1 - top.a);
  if (a === 0) return { r: 0, g: 0, b: 0, a: 0 };
  const mix = (t: number, u: number) => (t * top.a + u * under.a * (1 - top.a)) / a;
  return { r: mix(top.r, under.r), g: mix(top.g, under.g), b: mix(top.b, under.b), a };
}

/** WCAG 2.2's relative luminance of an opaque colour. */
function luminance({ r, g, b }: Rgba): number {
  const linear = (channel: number) => {
    const c = channel / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b);
}

/** The contrast ratio of two opaque colours, 1 to 21. */
function ratioOf(one: Rgba, two: Rgba): number {
  const [hi, lo] = [luminance(one), luminance(two)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

/**
 * **Words of colour `words` as drawn through `layers`**, the words' own box first and the root
 * last: their contrast with the ground beside them, and the opacity all the boxes multiply to.
 */
export function asDrawn(words: Rgba, layers: readonly Layer[]): { ratio: number; opacity: number } {
  let word = words;
  let ground: Rgba = { r: 0, g: 0, b: 0, a: 0 };
  let opacity = 1;
  for (const layer of layers) {
    word = over(word, layer.background);
    ground = over(ground, layer.background);
    word = { ...word, a: word.a * layer.opacity };
    ground = { ...ground, a: ground.a * layer.opacity };
    opacity *= layer.opacity;
  }
  return { ratio: ratioOf(over(word, PAGE), over(ground, PAGE)), opacity };
}

/** WCAG 1.4.3's floor for words of `px` pixels at weight `weight`: large text is 3:1. */
export function floorOf(px: number, weight: number): number {
  const large = px >= 24 || (px >= 18.66 && weight >= 700);
  return large ? 3 : 4.5;
}

/** A faded word under its floor, as the guard reports it. */
export type FadedWord = {
  /** The element, as `tag.class.class`, and its ancestors' up to the nearest with an id or a
   *  scope: enough to find the rule. */
  where: string;
  /** Its first words. */
  words: string;
  ratio: number;
  floor: number;
  opacity: number;
};

/** Any CSS colour the browser computed, as the 8-bit sRGB it draws. */
function colourReader(): (css: string) => Rgba {
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = 1;
  const pen = canvas.getContext("2d", { willReadFrequently: true });
  if (pen === null) throw new Error("the faded-words guard has no 2D canvas to read colours on");
  const known = new Map<string, Rgba>();
  return (css) => {
    const hit = known.get(css);
    if (hit) return hit;
    pen.clearRect(0, 0, 1, 1);
    pen.fillStyle = css;
    pen.fillRect(0, 0, 1, 1);
    const [r = 0, g = 0, b = 0, a = 0] = pen.getImageData(0, 0, 1, 1).data;
    const read = { r, g, b, a: a / 255 };
    known.set(css, read);
    return read;
  };
}

/** How much `filter` fades: each `opacity()` in it, multiplied. */
function filterOpacity(filter: string): number {
  let out = 1;
  for (const [, n, percent] of filter.matchAll(/opacity\(\s*([\d.]+)(%?)\s*\)/g))
    out *= Number(n) / (percent ? 100 : 1);
  return out;
}

/** Whether a box's opacity is moving now: an animation or a transition on it. */
function moving(element: Element): boolean {
  return element.getAnimations().some((one) => one.playState === "running");
}

/** `tag.class` of an element, the way a person finds its rule. */
function nameOf(element: Element): string {
  const scope = element.getAttribute("data-scope");
  const part = element.getAttribute("data-part");
  const attrs = scope ? `[data-scope="${scope}"]${part ? `[data-part="${part}"]` : ""}` : "";
  const classes = [...element.classList].map((one) => `.${one}`).join("");
  return `${element.tagName.toLowerCase()}${attrs}${classes}`;
}

/**
 * **Every word under `root` that a fade takes under its floor.** Each element with words of its
 * own is measured once, as drawn now.
 */
export function fadedWordsUnderFloor(root: Element): FadedWord[] {
  const colour = colourReader();
  const found: FadedWord[] = [];
  const seen = new Set<Element>();
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
    const element = node.parentElement;
    if (element === null || seen.has(element) || !node.textContent?.trim()) continue;
    seen.add(element);
    if (element.closest(':disabled, [aria-disabled="true"], [data-disabled], [aria-hidden="true"]'))
      continue;
    const style = getComputedStyle(element);
    if (style.visibility !== "visible" || element.getClientRects().length === 0) continue;
    const layers: Layer[] = [];
    let still = true;
    for (let box: Element | null = element; box !== null; box = box.parentElement) {
      const boxStyle = getComputedStyle(box);
      const opacity = Number(boxStyle.opacity) * filterOpacity(boxStyle.filter);
      if (opacity < 1 && moving(box)) still = false;
      layers.push({ background: colour(boxStyle.backgroundColor), opacity });
    }
    if (!still) continue;
    const drawn = asDrawn(colour(style.color), layers);
    if (drawn.opacity >= 1 || drawn.opacity === 0) continue;
    const floor = floorOf(Number.parseFloat(style.fontSize), Number(style.fontWeight));
    if (drawn.ratio >= floor) continue;
    const path: string[] = [];
    for (let box: Element | null = element; box !== null && box !== root; box = box.parentElement) {
      path.unshift(nameOf(box));
      if (box.id || box.hasAttribute("data-scope") || path.length >= 4) break;
    }
    found.push({
      where: path.join(" > "),
      words: (element.textContent ?? "").trim().slice(0, 40),
      ratio: Math.round(drawn.ratio * 100) / 100,
      floor,
      opacity: Math.round(drawn.opacity * 1000) / 1000,
    });
  }
  return found;
}
