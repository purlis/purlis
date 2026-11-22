// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// A small GraphQL reader, for two jobs only: the fake forge answers the queries `gh` sends by
// walking the selection against a plain object, and the stand-in proxy reads which repositories
// a query names and which root fields and mutations it uses. It is not a GraphQL server: no
// schema, no validation, no directives (`@include` and `@skip` are read and ignored).

const PUNCT = new Set(['{', '}', '(', ')', '[', ']', ':', '=', '!', '$', '@', '|', '&']);

function tokens(source) {
  const out = [];
  let i = 0;
  while (i < source.length) {
    const c = source[i];
    if (/[\s,﻿]/.test(c)) i += 1;
    else if (c === '#') while (i < source.length && source[i] !== '\n') i += 1;
    else if (source.startsWith('...', i)) {
      out.push({ t: '...' });
      i += 3;
    } else if (PUNCT.has(c)) {
      out.push({ t: c });
      i += 1;
    } else if (source.startsWith('"""', i)) {
      const end = source.indexOf('"""', i + 3);
      out.push({ t: 'string', v: source.slice(i + 3, end).trim() });
      i = end + 3;
    } else if (c === '"') {
      let j = i + 1;
      while (source[j] !== '"') j += source[j] === '\\' ? 2 : 1;
      out.push({ t: 'string', v: JSON.parse(source.slice(i, j + 1)) });
      i = j + 1;
    } else if (/[-0-9]/.test(c)) {
      const m = /^-?\d+(\.\d+)?([eE][-+]?\d+)?/.exec(source.slice(i));
      out.push({ t: 'number', v: Number(m[0]) });
      i += m[0].length;
    } else if (/[_A-Za-z]/.test(c)) {
      const m = /^[_A-Za-z][_0-9A-Za-z]*/.exec(source.slice(i));
      out.push({ t: 'name', v: m[0] });
      i += m[0].length;
    } else throw new Error(`graphql: unexpected ${JSON.stringify(c)} at ${i}`);
  }
  return out;
}

/** Parse a document into `{ operation, fragments, selection }`. */
export function parse(source) {
  const toks = tokens(source);
  let p = 0;
  const peek = () => toks[p];
  const next = () => toks[p++];
  const is = (t, v) => peek()?.t === t && (v === undefined || peek().v === v);
  const eat = (t, v) => (is(t, v) ? next() : null);
  const need = (t, v) => {
    const tok = eat(t, v);
    if (!tok) throw new Error(`graphql: expected ${v ?? t}, got ${JSON.stringify(peek())}`);
    return tok;
  };

  function value() {
    if (eat('$')) return { variable: need('name').v };
    if (is('number') || is('string')) return next().v;
    if (eat('[')) {
      const list = [];
      while (!eat(']')) list.push(value());
      return list;
    }
    if (eat('{')) {
      const obj = {};
      while (!eat('}')) {
        const k = need('name').v;
        need(':');
        obj[k] = value();
      }
      return obj;
    }
    const n = need('name').v;
    if (n === 'true') return true;
    if (n === 'false') return false;
    if (n === 'null') return null;
    return n; // an enum value
  }

  const skipDirectives = () => {
    while (eat('@')) {
      need('name');
      if (eat('(')) while (!eat(')')) { need('name'); need(':'); value(); }
    }
  };

  function skipType() {
    if (eat('[')) {
      skipType();
      need(']');
    } else need('name');
    eat('!');
  }

  function selectionSet() {
    need('{');
    const list = [];
    while (!eat('}')) {
      if (eat('...')) {
        if (is('name') && peek().v !== 'on') {
          const name = next().v;
          skipDirectives();
          list.push({ kind: 'spread', name });
        } else {
          let on = null;
          if (eat('name', 'on')) on = need('name').v;
          skipDirectives();
          list.push({ kind: 'inline', on, selection: selectionSet() });
        }
        continue;
      }
      let name = need('name').v;
      let alias = null;
      if (eat(':')) {
        alias = name;
        name = need('name').v;
      }
      const args = {};
      if (eat('(')) {
        while (!eat(')')) {
          const k = need('name').v;
          need(':');
          args[k] = value();
        }
      }
      skipDirectives();
      list.push({ kind: 'field', alias, name, args, selection: is('{') ? selectionSet() : null });
    }
    return list;
  }

  const doc = { operation: 'query', fragments: {}, selection: null };
  while (p < toks.length) {
    if (is('{')) {
      doc.selection = selectionSet();
    } else if (eat('name', 'fragment')) {
      const name = need('name').v;
      need('name', 'on');
      const on = need('name').v;
      skipDirectives();
      doc.fragments[name] = { on, selection: selectionSet() };
    } else {
      doc.operation = need('name').v;
      eat('name');
      if (eat('(')) {
        while (!eat(')')) {
          need('$');
          need('name');
          need(':');
          skipType();
          if (eat('=')) value();
          skipDirectives();
        }
      }
      skipDirectives();
      doc.selection = selectionSet();
    }
  }
  return doc;
}

/** Replace `{variable}` placeholders by the request's variables. */
export function resolveValue(v, variables) {
  if (Array.isArray(v)) return v.map((x) => resolveValue(x, variables));
  if (v && typeof v === 'object') {
    if ('variable' in v && Object.keys(v).length === 1) return variables?.[v.variable] ?? null;
    return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, resolveValue(x, variables)]));
  }
  return v;
}

/** The fields of a selection with fragments expanded; `typename` picks the inline fragments that apply. */
function expand(selection, doc, typename) {
  const out = [];
  for (const s of selection) {
    if (s.kind === 'field') out.push(s);
    else if (s.kind === 'inline') {
      if (!s.on || !typename || s.on === typename) out.push(...expand(s.selection, doc, typename));
    } else {
      const f = doc.fragments[s.name];
      if (f && (!typename || f.on === typename)) out.push(...expand(f.selection, doc, typename));
    }
  }
  return out;
}

function complete(field, value, doc, variables) {
  if (value === undefined || value === null) return null;
  if (Array.isArray(value)) return value.map((v) => complete(field, v, doc, variables));
  if (!field.selection) return value;
  return select(field.selection, value, doc, variables);
}

function select(selection, obj, doc, variables) {
  const result = {};
  for (const f of expand(selection, doc, obj.__typename)) {
    const key = f.alias ?? f.name;
    if (f.name === '__typename') {
      result[key] = obj.__typename ?? null;
      continue;
    }
    let v = obj[f.name];
    if (typeof v === 'function') v = v(resolveValue(f.args, variables));
    result[key] = complete(f, v, doc, variables);
  }
  return result;
}

/**
 * Run a document against plain objects. `roots` is `{ query: {...}, mutation: {...} }`; a root
 * field that is a function is called with its resolved arguments. A field the object lacks
 * answers `null`, which is what a client that asked for more than the fake knows can live with.
 */
export function execute(source, variables, roots) {
  const doc = parse(source);
  const root = roots[doc.operation];
  if (!root) return { errors: [{ message: `no ${doc.operation} root` }] };
  const unknown = expand(doc.selection, doc, undefined).filter((f) => !(f.name in root) && f.name !== '__typename');
  if (unknown.length) return { errors: [{ message: `fake forge: no root field ${unknown.map((f) => f.name).join(', ')}` }] };
  return { data: select(doc.selection, { ...root, __typename: doc.operation }, doc, variables) };
}

/**
 * What a document asks, for the proxy to judge: its operation, its root field names, and every
 * `owner`/`name` pair or `fullPath` it names, after variables are filled in.
 */
export function describe(source, variables) {
  const doc = parse(source);
  const roots = expand(doc.selection, doc, undefined).map((f) => f.name);
  const repos = [];
  const mutationInputs = [];
  const visit = (selection, depth) => {
    for (const f of expand(selection, doc, undefined)) {
      const args = resolveValue(f.args, variables);
      if (typeof args.owner === 'string' && typeof args.name === 'string') repos.push(`${args.owner}/${args.name}`);
      if (typeof args.fullPath === 'string') repos.push(args.fullPath);
      if (depth === 0 && doc.operation === 'mutation') mutationInputs.push({ field: f.name, input: args.input ?? null });
      if (f.selection) visit(f.selection, depth + 1);
    }
  };
  visit(doc.selection, 0);
  return { operation: doc.operation, roots, repos, mutationInputs };
}
