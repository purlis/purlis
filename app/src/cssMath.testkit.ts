// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **How the geometry guard reads CSS arithmetic** (#2450): the math functions in a value
 * (`calc()`, `min()`, `max()`, `clamp()`, `round()`, `mod()`, `rem()`, `abs()`, `sign()` and
 * the rest), parsed rather than matched, so a factor is read whatever it is spelled as (`3.5`,
 * `+0.5`, `(0.5)`, `calc(3.5)`, `35e-1`, `pi`).
 *
 * The rule it holds, in what the guard reads:
 * - **A number in the arithmetic is whole.** Any other number is a literal.
 * - **A product's factors fold into one net factor** (`* 7 / 2` is `* 3.5`), and it is a whole
 *   number or a whole share (`1/n`): an evenly stepped series (an indent per level) is a whole
 *   number of one step, so its steps stay even, and a share halves or thirds a step.
 * - **A term that holds a written length takes no multiple** (`calc(1px * 7)`,
 *   `calc((var(--space-0) + 1px) * 7)`, `max(1px, var(--space-0)) * 7` are literals), only a
 *   whole share (`(var(--size-control) - 1lh) / 2`). A hairline or the text's own unit is not a
 *   literal alone; a multiple of it is a size written by hand.
 * - **A sum writes each length once**: two written lengths of the same unit and sign
 *   (`1px + 1px`, `1em + 1em`) are a literal written in parts.
 * - A custom property whose value is a bare written length (`--hair: 1px`) is that length where
 *   it is read (`lengths`).
 */

/** What a term of the arithmetic is. */
interface Term {
  /** A plain number; its value, when it can be known (`NaN` when not: `sign()`). */
  number: boolean;
  value?: number;
  /** It holds a token (`var(--space-4)`). */
  token: boolean;
  /** It holds a length written out (`1px`, `1lh`, or a custom property that is one). */
  length: boolean;
  /** It is one written length and nothing else: its sign and unit (`+px`). */
  dim?: string;
}

type Token =
  | { t: "num"; v: number }
  | { t: "dim"; unit: string; negative: boolean }
  | { t: "fn"; name: string }
  | { t: "ident"; name: string }
  | { t: "op"; op: string }
  | { t: "(" | ")" | "," | "other" };

const MATH = new Set([
  "calc",
  "min",
  "max",
  "clamp",
  "round",
  "mod",
  "rem",
  "abs",
  "sign",
  "pow",
  "sqrt",
  "hypot",
  "log",
  "exp",
  "sin",
  "cos",
  "tan",
  "asin",
  "acos",
  "atan",
  "atan2",
]);
/** The math functions whose result is a number whatever their arguments are. */
const NUMERIC = new Set([
  "sign",
  "pow",
  "sqrt",
  "log",
  "exp",
  "sin",
  "cos",
  "tan",
  "asin",
  "acos",
  "atan",
  "atan2",
]);
const LENGTH_UNITS = new Set([
  "px",
  "rem",
  "em",
  "ch",
  "ex",
  "lh",
  "pt",
  "pc",
  "cm",
  "mm",
  "in",
  "q",
]);

const NUMBER = /[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?/iy;
const UNIT = /%|[a-z]+/iy;
const NAME = /-{0,2}[a-z_][\w-]*/iy;

function tokens(value: string): Token[] {
  const found: Token[] = [];
  const signs = () => {
    const last = found[found.length - 1];
    return !last || ["op", "(", ",", "fn"].includes(last.t);
  };
  for (let i = 0; i < value.length;) {
    const char = value[i];
    if (/\s/.test(char)) {
      i++;
      continue;
    }
    NUMBER.lastIndex = i;
    const number = /[\d.]/.test(char) || (/[+-]/.test(char) && signs()) ? NUMBER.exec(value) : null;
    if (number) {
      i += number[0].length;
      UNIT.lastIndex = i;
      const unit = UNIT.exec(value);
      if (unit) {
        i += unit[0].length;
        found.push({ t: "dim", unit: unit[0].toLowerCase(), negative: number[0].startsWith("-") });
      } else found.push({ t: "num", v: Number(number[0]) });
      continue;
    }
    NAME.lastIndex = i;
    const name = NAME.exec(value);
    if (name) {
      i += name[0].length;
      if (value[i] === "(") {
        i++;
        found.push({ t: "fn", name: name[0].toLowerCase() });
      } else found.push({ t: "ident", name: name[0].toLowerCase() });
      continue;
    }
    if ("+-*/".includes(char)) found.push({ t: "op", op: char });
    else if (char === "(" || char === ")" || char === ",") found.push({ t: char });
    else found.push({ t: "other" });
    i++;
  }
  return found;
}

const number = (value: number): Term => ({ number: true, value, token: false, length: false });
const plain: Term = { number: false, token: false, length: false };

/** Several terms read as one: a number when every one is, and holding what any one holds. */
function join(terms: readonly Term[]): Term {
  if (terms.every((term) => term.number)) return number(NaN);
  return {
    number: false,
    token: terms.some((term) => term.token),
    length: terms.some((term) => term.length),
  };
}

const near = (value: number) => Math.abs(value - Math.round(value)) < 1e-9;
/** A whole number, or a whole share of one (`1/n`). */
const wholeOrShare = (net: number) => near(net) || (net !== 0 && near(1 / net));
/** A whole share of one, or ±1. */
const share = (net: number) => net !== 0 && Math.abs(net) <= 1 && near(1 / net);

/** Whether a value's arithmetic writes a literal (the rule above). `lengths` holds the custom
 *  properties whose value is a bare written length, by name, with their unit. */
export function arithmeticLiteral(
  value: string,
  lengths: ReadonlyMap<string, string> = new Map(),
): boolean {
  const list = tokens(value);
  let at = 0;
  let literal = false;
  const peek = () => list[at];
  const isOp = (ops: string) => {
    const next = peek();
    return next?.t === "op" && ops.includes(next.op);
  };

  /** The arguments of a function whose `(` was just read, up to its `)`. */
  const args = (): Term[] => {
    const terms: Term[] = [];
    while (at < list.length && peek().t !== ")") {
      if (peek().t === ",") at++;
      else terms.push(sum());
    }
    at++;
    return terms;
  };

  const unary = (): Term => {
    const token = list[at++];
    if (token === undefined) return plain;
    switch (token.t) {
      case "num":
        if (!Number.isInteger(token.v)) literal = true;
        return number(token.v);
      case "dim":
        if (!LENGTH_UNITS.has(token.unit)) return plain;
        return { ...plain, length: true, dim: `${token.negative ? "-" : "+"}${token.unit}` };
      case "ident":
        if (token.name === "pi" || token.name === "e") {
          literal = true;
          return number(token.name === "pi" ? Math.PI : Math.E);
        }
        return plain;
      case "(": {
        const inner = sum();
        if (peek()?.t === ")") at++;
        return inner;
      }
      case "fn": {
        const named = peek();
        const custom = named?.t === "ident" ? named.name : undefined;
        const inner = args();
        if (token.name === "var") {
          const unit = custom === undefined ? undefined : lengths.get(custom);
          if (unit !== undefined) return { ...plain, length: true, dim: `+${unit}` };
          return { ...plain, token: true };
        }
        if (!MATH.has(token.name)) return plain;
        if (NUMERIC.has(token.name)) return number(NaN);
        const numbers = inner.every((term) => term.number);
        if (token.name === "calc" && inner.length === 1) return inner[0];
        if (numbers && token.name === "abs" && inner.length === 1) {
          return number(Math.abs(inner[0].value ?? NaN));
        }
        if (numbers && (token.name === "min" || token.name === "max")) {
          return number(Math[token.name](...inner.map((term) => term.value ?? NaN)));
        }
        return join(inner);
      }
      default:
        return plain;
    }
  };

  /** A product, its numbers folded into one net factor of what it multiplies. */
  const product = (): Term => {
    const first = unary();
    const parts: Term[] = first.number ? [] : [first];
    let net = first.number ? (first.value ?? NaN) : 1;
    while (isOp("*/")) {
      const op = (list[at++] as { op: string }).op;
      const right = unary();
      if (right.number) net = op === "*" ? net * (right.value ?? NaN) : net / (right.value ?? NaN);
      else if (op === "*") parts.push(right);
      else literal = true;
    }
    if (parts.length === 0) return number(net);
    if (parts.length === 1 && net === 1) return parts[0];
    const term = join(parts);
    if (net !== 1 && !(term.length ? share(net) : wholeOrShare(net))) literal = true;
    return term;
  };

  function sum(): Term {
    const first = product();
    if (!isOp("+-")) return first;
    const terms = [first];
    const written = new Set<string>();
    const add = (term: Term, minus: boolean) => {
      if (term.dim === undefined) return;
      const sign = (term.dim[0] === "-") !== minus ? "-" : "+";
      const key = sign + term.dim.slice(1);
      if (written.has(key)) literal = true;
      written.add(key);
    };
    add(first, false);
    let total = first.value ?? NaN;
    while (isOp("+-")) {
      const op = (list[at++] as { op: string }).op;
      const right = product();
      add(right, op === "-");
      total = op === "+" ? total + (right.value ?? NaN) : total - (right.value ?? NaN);
      terms.push(right);
    }
    return terms.every((term) => term.number) ? number(total) : join(terms);
  }

  while (at < list.length) {
    const token = list[at];
    if (token.t === "fn" && MATH.has(token.name)) unary();
    else at++;
  }
  return literal;
}
