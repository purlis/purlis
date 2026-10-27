// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Which Rust tests and test binaries took longest, read from nextest's JUnit report. Run with
// `node --test tools/`.
import assert from "node:assert/strict";
import { test } from "node:test";

import {
  annotations,
  cargoTestAnnotations,
  failureAnnotations,
  flakyAnnotations,
  readCargoFailures,
  readCases,
  readFailures,
  readFlaky,
  slowest,
} from "./slowest-tests.mjs";

/** A report as nextest writes it: one testsuite per test binary, a testcase per test. */
const report = `<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="4" failures="0" errors="0" time="12.5">
    <testsuite name="purlis-cli::cli" tests="2" disabled="0" errors="0" failures="0">
        <testcase name="a_quick_one" classname="purlis-cli::cli" timestamp="2026-10-10T10:00:00Z" time="0.010">
        </testcase>
        <testcase name="waits &amp; waits" classname="purlis-cli::cli" timestamp="2026-10-10T10:00:00Z" time="9.500">
            <system-out>long output</system-out>
        </testcase>
    </testsuite>
    <testsuite name="fake-harness::unread_answers" tests="2" disabled="0" errors="0" failures="0">
        <testcase time="3.000" name="floods" classname="fake-harness::unread_answers"/>
        <testcase name="floods_again" classname="fake-harness::unread_answers" time="2.250"></testcase>
    </testsuite>
</testsuites>`;

test("every testcase is read, whatever order its attributes come in", () => {
  assert.deepEqual(readCases(report), [
    { binary: "purlis-cli::cli", name: "a_quick_one", seconds: 0.01 },
    { binary: "purlis-cli::cli", name: "waits & waits", seconds: 9.5 },
    { binary: "fake-harness::unread_answers", name: "floods", seconds: 3 },
    { binary: "fake-harness::unread_answers", name: "floods_again", seconds: 2.25 },
  ]);
});

test("a report with no tests reads as none", () => {
  assert.deepEqual(readCases(""), []);
});

test("tests come slowest first, and binaries by the time their tests took in all", () => {
  const { tests, binaries } = slowest(readCases(report), { limit: 2 });
  assert.deepEqual(
    tests.map((one) => one.name),
    ["waits & waits", "floods"],
  );
  assert.deepEqual(binaries, [
    { binary: "purlis-cli::cli", seconds: 9.51, tests: 2 },
    { binary: "fake-harness::unread_answers", seconds: 5.25, tests: 2 },
  ]);
});

test("two notices: the slowest tests and the slowest binaries, one per line", () => {
  const lines = annotations(readCases(report));
  assert.equal(lines.length, 2);
  assert.match(lines[0], /^::notice title=Slowest Rust tests::/);
  assert.match(lines[0], /9\.5 s {2}purlis-cli::cli {2}waits & waits%0A/);
  assert.match(lines[1], /^::notice title=Slowest Rust test binaries \(their tests' time summed\)::/);
  assert.match(lines[1], /9\.5 s {2}purlis-cli::cli \(2 tests\)/);
});

test("a name that holds a workflow command's own characters is escaped", () => {
  const [tests] = annotations([{ binary: "x::y", name: "50% done\nnext", seconds: 1 }]);
  assert.match(tests, /x::y {2}50%25 done%0Anext$/);
});

test("a run that wrote no report says so, and does not fail", () => {
  assert.deepEqual(annotations([]), ["::notice title=Slowest Rust tests::No JUnit report with tests was found."]);
});

test("each failed test is an error, with where it panicked and what it said", () => {
  const failing = `<testsuites>
    <testsuite name="purlis-core">
        <testcase name="passes" classname="purlis-core" time="0.1"/>
        <testcase name="sandbox::x::fails" classname="purlis-core" time="0.2">
            <failure type="test failure with exit code 101" message="thread panicked"/>
            <system-out>running 1 test</system-out>
            <system-err>thread &apos;sandbox::x::fails&apos; panicked at src/x.rs:3:5:
assertion failed: a &lt; b
note: run with RUST_BACKTRACE=1</system-err>
        </testcase>
        <testcase name="no_panic" classname="purlis-core" time="0.3"><failure message="timed out"/></testcase>
    </testsuite>
</testsuites>`;
  const failed = readFailures(failing);
  assert.deepEqual(
    failed.map((one) => [one.name, one.said[0], one.said[1]]),
    [
      ["sandbox::x::fails", "thread 'sandbox::x::fails' panicked at src/x.rs:3:5:", "assertion failed: a < b"],
      ["no_panic", "timed out", undefined],
    ],
  );
  const [first] = failureAnnotations(failed);
  assert.match(first, /^::error title=Failed%3A purlis-core sandbox%3A%3Ax%3A%3Afails::thread /);
  assert.match(first, /a < b%0Anote/);
  assert.deepEqual(readFailures(report), []);
});

/** `cargo test --no-fail-fast` as it writes to a pipe with `CARGO_TERM_COLOR=always`: cargo's
 *  own lines coloured, the test binaries' lines plain. */
const cargoRun = `\u001b[1m\u001b[32m    Finished\u001b[0m \`mutants\` profile [unoptimized] target(s) in 4m 02s
\u001b[1m\u001b[32m     Running\u001b[0m unittests src/lib.rs (target/mutants/deps/purlis_core-0f3a)

running 3 tests
test adopt::tests::passes ... ok
test sandbox::x::fails ... FAILED
test sandbox::x::says_nothing ... FAILED

failures:

---- sandbox::x::fails stdout ----

thread 'sandbox::x::fails' (4242) panicked at crates/purlis-core/src/sandbox/x.rs:3:5:
assertion \`left == right\` failed: 50% done
  left: 1
 right: 2
note: run with \`RUST_BACKTRACE=1\` environment variable to display a backtrace

---- sandbox::x::says_nothing stdout ----
Error: "it refused"


failures:
    sandbox::x::fails
    sandbox::x::says_nothing

test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.52s

\u001b[1m\u001b[32m     Running\u001b[0m tests/a_branch_marks_what_it_changed.rs (target/mutants/deps/a_branch_marks_what_it_changed-77aa)

running 1 test
test a_branch_is_marked ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

\u001b[1m\u001b[32m   Doc-tests\u001b[0m purlis_core

running 1 test
test crates/purlis-core/src/fits.rs - fits::capacity (line 12) ... FAILED

failures:

---- crates/purlis-core/src/fits.rs - fits::capacity (line 12) stdout ----
error[E0425]: cannot find function \`capacity\` in this scope

failures:
    crates/purlis-core/src/fits.rs - fits::capacity (line 12)

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s

\u001b[1m\u001b[31merror\u001b[0m: 2 targets failed:
    \`-p purlis-core --lib\`
    \`-p purlis-core --doc\`
`;

test("cargo test's own output names each failed test, its binary and what it said", () => {
  assert.deepEqual(readCargoFailures(cargoRun), [
    {
      binary: "src/lib.rs",
      name: "sandbox::x::fails",
      said: [
        "thread 'sandbox::x::fails' (4242) panicked at crates/purlis-core/src/sandbox/x.rs:3:5:",
        "assertion `left == right` failed: 50% done",
        "  left: 1",
        " right: 2",
        "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace",
      ],
    },
    // One that never panicked says what it wrote.
    { binary: "src/lib.rs", name: "sandbox::x::says_nothing", said: ['Error: "it refused"'] },
    {
      binary: "doc-tests purlis_core",
      name: "crates/purlis-core/src/fits.rs - fits::capacity (line 12)",
      said: ["error[E0425]: cannot find function `capacity` in this scope"],
    },
  ]);
});

test("cargo test's failures are annotated as nextest's are, under the same title", () => {
  const lines = cargoTestAnnotations(cargoRun);
  assert.equal(lines.length, 3);
  assert.match(lines[0], /^::error title=Failed%3A src\/lib.rs sandbox%3A%3Ax%3A%3Afails::thread /);
  assert.match(lines[0], /failed: 50%25 done%0A {2}left: 1%0A/);
  assert.match(lines[2], /^::error title=Failed%3A doc-tests purlis_core crates\/purlis-core\/src\/fits.rs - /);
});

test("a test binary that ended before its result is named, with the last it wrote", () => {
  const died = `     Running tests/it_dies.rs (target/mutants/deps/it_dies-1b2c)

running 2 tests
test the_first ... ok
thread 'the_second' has overflowed its stack
fatal runtime error: stack overflow
error: test failed, to rerun pass \`-p purlis-core --test it_dies\`

Caused by:
  process didn't exit successfully: \`target/mutants/deps/it_dies-1b2c\` (signal: 6, SIGABRT: process abort signal)
`;
  const failed = readCargoFailures(died);
  assert.equal(failed.length, 1);
  assert.equal(failed[0].binary, "tests/it_dies.rs");
  assert.equal(failed[0].name, "(the binary ended before its tests' result)");
  assert.ok(failed[0].said.includes("fatal runtime error: stack overflow"));
  assert.ok(failed[0].said.some((line) => line.includes("SIGABRT")));
});

test("a run that failed before any test ran says what cargo said", () => {
  const unbuilt = `   Compiling purlis-core v0.4.2 (/w/crates/purlis-core)
error[E0432]: unresolved import \`crate::gone\`
 --> crates/purlis-core/src/lib.rs:9:5
error: could not compile \`purlis-core\` (lib test) due to 1 previous error
`;
  assert.deepEqual(readCargoFailures(unbuilt), []);
  const lines = cargoTestAnnotations(unbuilt);
  assert.equal(lines.length, 1);
  assert.match(lines[0], /^::error title=cargo test failed with no failed test in its output::/);
  assert.match(lines[0], /error\[E0432\]%3A|error\[E0432\]: unresolved import/);
  assert.match(lines[0], /could not compile/);
});

test("a run with nothing failed is annotated with nothing", () => {
  const green = `     Running unittests src/lib.rs (target/mutants/deps/purlis_core-0f3a)

running 1 test
test passes ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
`;
  assert.deepEqual(readCargoFailures(green), []);
  assert.deepEqual(cargoTestAnnotations(green), []);
});

/** A report in which one test failed once and passed on its retry, as nextest writes it. */
const flakyReport = `<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="2" failures="0" errors="0" time="5.0">
    <testsuite name="purlis-browser-engine::frames" tests="2" disabled="0" errors="0" failures="0">
        <testcase name="keys_reach_a_field" classname="purlis-browser-engine::frames" time="2.000">
            <flakyFailure message="test failed" type="test failure">thread 'keys_reach_a_field' panicked at crates/browser-engine/tests/frames.rs:46:39:
the field read &quot;ab&quot;, not &quot;abc&quot;</flakyFailure>
        </testcase>
        <testcase name="steady" classname="purlis-browser-engine::frames" time="1.000">
        </testcase>
    </testsuite>
</testsuites>`;

test("a test that passed only on its retry is read as flaky, and not as failed", () => {
  assert.deepEqual(readFailures(flakyReport), []);
  const flaky = readFlaky(flakyReport);
  assert.equal(flaky.length, 1);
  assert.equal(flaky[0].binary, "purlis-browser-engine::frames");
  assert.equal(flaky[0].name, "keys_reach_a_field");
  assert.equal(flaky[0].retries, 1);
  assert.match(flaky[0].said.join("\n"), /panicked at crates\/browser-engine\/tests\/frames\.rs:46:39/);
});

test("each flaky test is a warning on the run, so a retry never hides it", () => {
  const lines = flakyAnnotations(readFlaky(flakyReport));
  assert.equal(lines.length, 1);
  assert.match(lines[0], /^::warning title=Flaky \(passed on retry 1\)%3A purlis-browser-engine%3A%3Aframes keys_reach_a_field::/);
  assert.match(lines[0], /frames\.rs%3A46%3A39|frames\.rs:46:39/);
  assert.deepEqual(flakyAnnotations(readFlaky(report)), []);
});
