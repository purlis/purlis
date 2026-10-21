// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What the core hands a window, put where `theme.ts` reads it** (#2901). Imported by
 * `preview.tsx` before anything that imports `theme.ts`, as `test-setup.ts` imports
 * `handed.testkit.ts` first.
 */
import handed from "virtual:purlis-handed";

const AT_CREATION = "__CHARTER_AT_CREATION__";
(globalThis as Record<string, unknown>)[AT_CREATION] ??= handed;
