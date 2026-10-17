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
 * - **A token takes a whole multiple or a whole share** (`var(--space-5) * 3`, `* -1`, `/ 2`):
 *   an evenly stepped series (an indent per level) is a multiple of one step, so the steps stay
 *   even; any other factor of a token is a literal.
 * - **A length written out takes no factor but ±1** (`1px * 7` and `1em * 3` are literals): a
 *   hairline or the text's own unit is not a literal alone, and a multiple of it is a size
 *   written by hand.
 * - A share of the box or the window (`%`, `vh`) takes any whole factor.
 */

type Kind = "number" | "token" | "length" | "share" | "other";
interface Term {
  kind: Kind;
  /** A number's value, when it can be known; `NaN` when it cannot (`sign()`). */
  value?: number;
}

type Token =
  | { t: "num"; v: number }
  | { t: "dim"; unit: string }
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
        found.push({ t: "dim", unit: unit[0].toLowerCase() });
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

/** Whether a value's arithmetic writes a literal (the rule above). */
export function arithmeticLiteral(value: string): boolean {
  const list = tokens(value);
  let at = 0;
  let literal = false;
  const peek = () => list[at];

  const join = (terms: Term[]): Term => {
    if (terms.every((term) => term.kind === "number")) return { kind: "number", value: NaN };
    for (const kind of ["token", "length", "share"] as const) {
      if (terms.some((term) => term.kind === kind)) return { kind };
    }
    return { kind: "other" };
  };

  const factor = (of: Term, by: number) => {
    if (of.kind === "token" && !Number.isInteger(by)) literal = true;
    if (of.kind === "length" && Math.abs(by) !== 1) literal = true;
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
    if (token === undefined) return { kind: "other" };
    switch (token.t) {
      case "num":
        if (!Number.isInteger(token.v)) literal = true;
        return { kind: "number", value: token.v };
      case "dim":
        if (LENGTH_UNITS.has(token.unit)) return { kind: "length" };
        return { kind: "share" };
      case "ident":
        if (token.name === "pi" || token.name === "e") {
          literal = true;
          return { kind: "number", value: token.name === "pi" ? Math.PI : Math.E };
        }
        return { kind: "other" };
      case "(": {
        const inner = sum();
        if (peek()?.t === ")") at++;
        return inner;
      }
      case "fn": {
        const inner = args();
        if (token.name === "var") return { kind: "token" };
        if (!MATH.has(token.name)) return { kind: "other" };
        if (NUMERIC.has(token.name)) return { kind: "number", value: NaN };
        const numbers = inner.every((term) => term.kind === "number");
        if (numbers && token.name === "calc" && inner.length === 1) return inner[0];
        if (numbers && token.name === "abs" && inner.length === 1) {
          return { kind: "number", value: Math.abs(inner[0].value ?? NaN) };
        }
        if (numbers && (token.name === "min" || token.name === "max")) {
          const values = inner.map((term) => term.value ?? NaN);
          return { kind: "number", value: Math[token.name](...values) };
        }
        return join(inner);
      }
      default:
        return { kind: "other" };
    }
  };

  const product = (): Term => {
    let left = unary();
    while (peek()?.t === "op" && "*/".includes((peek() as { op: string }).op)) {
      const op = (list[at++] as { op: string }).op;
      const right = unary();
      if (left.kind === "number" && right.kind === "number") {
        const [a, b] = [left.value ?? NaN, right.value ?? NaN];
        left = { kind: "number", value: op === "*" ? a * b : a / b };
      } else if (right.kind === "number") {
        factor(left, right.value ?? NaN);
      } else if (left.kind === "number" && op === "*") {
        factor(right, left.value ?? NaN);
        left = right;
      } else if (left.kind === "number") {
        literal = true;
        left = right;
      } else {
        left = join([left, right]);
      }
    }
    return left;
  };

  function sum(): Term {
    let left = product();
    while (peek()?.t === "op" && "+-".includes((peek() as { op: string }).op)) {
      const op = (list[at++] as { op: string }).op;
      const right = product();
      if (left.kind === "number" && right.kind === "number") {
        const [a, b] = [left.value ?? NaN, right.value ?? NaN];
        left = { kind: "number", value: op === "+" ? a + b : a - b };
      } else left = join([left, right]);
    }
    return left;
  }

  while (at < list.length) {
    const token = list[at];
    if (token.t === "fn" && MATH.has(token.name)) unary();
    else at++;
  }
  return literal;
}
