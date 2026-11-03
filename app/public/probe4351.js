// [DEBUG-4351] Probe, do not merge: built from a throwaway probe4351.ts; see #4351.
"use strict";
(() => {
  const PERFORMED_WORK = 1;
  function nameOf(type) {
    if (typeof type === "function") {
      const f = type;
      return f.displayName ?? (f.name || "anon");
    }
    if (typeof type === "object" && type !== null) {
      const o = type;
      if (o.displayName) return o.displayName;
      if (o.render) return `fwd(${o.render.name || "anon"})`;
      if (o.type) return `memo(${nameOf(o.type) ?? "?"})`;
    }
    return void 0;
  }
  function onCommit(root) {
    const at = performance.now();
    const total = root.current.actualDuration ?? 0;
    const self = /* @__PURE__ */ new Map();
    let rendered = 0;
    const stack = [root.current];
    while (stack.length > 0) {
      const fiber = stack.pop();
      if ((fiber.flags & PERFORMED_WORK) !== 0) {
        const name = nameOf(fiber.type);
        if (name !== void 0) {
          rendered++;
          const was = self.get(name) ?? { ms: 0, n: 0 };
          was.ms += fiber.selfBaseDuration ?? 0;
          was.n++;
          self.set(name, was);
        }
      }
      if (fiber.sibling) stack.push(fiber.sibling);
      if (fiber.child) stack.push(fiber.child);
    }
    const top = [...self.entries()].sort((a, b) => b[1].ms - a[1].ms).slice(0, 12).map(([name, { ms, n }]) => `${name} ${ms.toFixed(1)}${n > 1 ? `x${n}` : ""}`).join(", ");
    const line = `commit ${total.toFixed(0)} ms, ${rendered} rendered, walk ${(performance.now() - at).toFixed(0)} ms: ${top}`;
    const holder = globalThis;
    if (holder.__tl4351 !== void 0 && holder.__t04351 !== void 0)
      holder.__tl4351.push(`${(at - holder.__t04351).toFixed(0)} ${line}`);
    else if (total > 30) console.info(`[DEBUG-4351] ${line}`);
  }
  let id = 0;
  globalThis.__REACT_DEVTOOLS_GLOBAL_HOOK__ = {
    supportsFiber: true,
    isDisabled: false,
    renderers: /* @__PURE__ */ new Map(),
    inject: () => ++id,
    checkDCE: () => void 0,
    onScheduleFiberRoot: () => void 0,
    onCommitFiberUnmount: () => void 0,
    onPostCommitFiberRoot: () => void 0,
    onCommitFiberRoot: (_id, root) => {
      try {
        onCommit(root);
      } catch (e) {
        console.info(`[DEBUG-4351] walk failed ${String(e)}`);
      }
    }
  };
})();
