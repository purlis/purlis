// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A part is styled through its classes, and named by its attributes** (#4351; ADR 0103, amended
 * 2026-11-04). A part's element carries `data-scope` and `data-part`, as ADR 0103 has it, and
 * beside them `ds-<scope>` and `dp-<part>` as classes. Every stylesheet is written against the
 * attributes, and is keyed on the classes once it is built: the window's own CSS by the build's
 * PostCSS step (`vite.config.ts`), and the theme engine's parts by {@link classKeyed} as the
 * window draws them (`theme/theme.ts`).
 *
 * **Why** (#4351): WebKit files a rule keyed on an attribute under the attribute's name alone, so
 * every element carrying `data-scope` was tested against every rule keyed on any scope, on every
 * whole-window restyle. A rule keyed on a class is tried only on an element with that class. Run
 * by run on the outcome bars, style time in an opening of Skill studio fell from 279 to 150 ms.
 *
 * A class and an attribute selector weigh the same (0,1,0), and each selector is rewritten where
 * it stands, so neither the cascade's order nor any rule's specificity moves; the forced-colours
 * sheets are rewritten as the rest are.
 *
 * Nobody writes these classes by hand: the JSX runtime (`ui/jsx-runtime.ts`) adds them from the
 * attributes, and {@link parted} does for a part made with `createElement`.
 */

/** A value a class can be named from as it stands: anything else stays keyed on its attribute. */
const NAMEABLE = /^[A-Za-z0-9_-]+$/;

/** The class an attribute's value is drawn as, by the attribute. */
const PREFIX = { "data-scope": "ds-", "data-part": "dp-" } as const;

/** The class of `value` for `attribute`, when it has one. */
function classOf(attribute: keyof typeof PREFIX, value: unknown): string | undefined {
  return typeof value === "string" && NAMEABLE.test(value)
    ? `${PREFIX[attribute]}${value}`
    : undefined;
}

/** `[data-scope=…]` or `[data-part=…]` matched exactly, quoted either way or not at all. */
const KEYED = /\[(data-scope|data-part)\s*=\s*(?:"([^"\\]*)"|'([^'\\]*)'|([A-Za-z0-9_-]+))\s*\]/g;

/**
 * `css` with every exact match on a part's scope or part keyed on its class instead. A presence
 * test (`[data-part]`), a prefix match, a case-blind match and a value no class can be named from
 * are left as they are, keyed on the attribute, which the element still carries.
 */
export function classKeyed(css: string): string {
  return css.replace(KEYED, (whole, attribute: keyof typeof PREFIX, double, single, bare) => {
    const named = classOf(attribute, double ?? single ?? bare);
    return named === undefined ? whole : `.${named}`;
  });
}

/** The classes a part's `data-scope` and `data-part` draw, space-separated: "" for neither. */
export function classesOf(attributes: Readonly<Record<string, unknown>>): string {
  return [
    classOf("data-scope", attributes["data-scope"]),
    classOf("data-part", attributes["data-part"]),
  ]
    .filter((one) => one !== undefined)
    .join(" ");
}

/**
 * **The build's PostCSS step** (`vite.config.ts`): each rule's selector keyed on its parts'
 * classes by {@link classKeyed}, where it stands, so the cascade's order and every rule's weight
 * stay as written.
 */
export function keyedOnClasses() {
  return {
    postcssPlugin: "purlis-parts-keyed-on-classes",
    Rule(rule: { selector: string }) {
      const keyed = classKeyed(rule.selector);
      if (keyed !== rule.selector) rule.selector = keyed;
    },
  };
}

/** Whether a class is one a part's attribute draws. */
const isPartClass = (one: string) => one.startsWith("ds-") || one.startsWith("dp-");

/** `given`'s own classes, without any a part's attributes drew, then `drawn`. */
function joined(given: unknown, drawn: string): string {
  const own = typeof given === "string" ? given.split(/\s+/).filter((one) => one !== "") : [];
  return [...own.filter((one) => !isPartClass(one)), drawn].join(" ");
}

/**
 * `props` with the classes of the `data-scope` and `data-part` they carry: added to the
 * `className` they were given, which may be a function of a React Aria component's state. Any
 * part class among those given is dropped first, so an element carries the classes of the
 * attributes it ends with: a family that names its own scope over its caller's is the family's
 * part. Props with neither attribute come back as they are.
 */
export function parted<P>(props: P): P {
  if (typeof props !== "object" || props === null) return props;
  // Every element the window draws passes here: most carry no part, and cost one look.
  if (!("data-scope" in props) && !("data-part" in props)) return props;
  const named = props as Record<string, unknown>;
  const drawn = classesOf(named);
  if (drawn === "") return props;
  const given = named.className ?? undefined;
  if (given !== undefined && typeof given !== "string" && typeof given !== "function") return props;
  const className =
    typeof given === "function"
      ? (...state: unknown[]) =>
          joined((given as (...state: unknown[]) => unknown)(...state), drawn)
      : joined(given, drawn);
  return { ...props, className };
}

/** The part classes `element` should carry, from its attributes. */
function owed(element: Element): string {
  return classesOf({
    "data-scope": element.getAttribute("data-scope"),
    "data-part": element.getAttribute("data-part"),
  });
}

/**
 * **Each element under `root` whose part classes are not exactly its attributes'**, as
 * `scope/part: class "…" should be "…"`: a part drawn without its classes is left unstyled
 * without a word, and a class left over from another part is styled as that part.
 */
export function partsWithoutTheirClass(root: ParentNode): string[] {
  const found: string[] = [];
  for (const element of root.querySelectorAll("[data-scope], [data-part], [class]")) {
    const carried = [...element.classList].filter(isPartClass).join(" ");
    const should = owed(element);
    if (carried === should) continue;
    const scope = element.getAttribute("data-scope") ?? "-";
    const part = element.getAttribute("data-part") ?? "-";
    found.push(
      `${scope}/${part}: class "${element.getAttribute("class") ?? ""}" should be "${should}"`,
    );
  }
  return found;
}
