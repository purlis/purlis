window.BENCHMARK_DATA = {
  "lastUpdate": 1792074137035,
  "repoUrl": "https://github.com/purlis/purlis",
  "entries": {
    "session layer (ubuntu-24.04)": [
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "23d07b28b6f1fb3c7cd113124ee70b695fa82f05",
          "message": "FD-25: per-device logs are named by the device id, not the hostname (#943)\n\n* Device id replaces hostname keys in per-device logs (FD-25)\n\nThe dispatch and skill logs, the piece claim log, the landing log and\npending landings are named by this device's id from the machine store\n(ADR 0066), with the hostname kept as a label. A writer reads the id and\nnever mints it; before one exists the file keeps the hostname name.\nRenaming files written under a hostname is FR-9's (#607).\n\nRefs #662\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* FD-25: the new device-log tests shed the shell's steering first\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-02T23:15:14+04:00",
          "tree_id": "76f31c359d011ba0d270b8fbc447db8e6f7c02ec",
          "url": "https://github.com/diazoxide/charter/commit/23d07b28b6f1fb3c7cd113124ee70b695fa82f05"
        },
        "date": 1790968660241,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5616239999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 0.538, 0.546, 0.562, 0.569, 0.574 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.9554925,
            "unit": "ms",
            "extra": "median of 5 runs: 16.823, 16.866, 16.955, 16.979, 17.432 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.4088925,
            "unit": "ms",
            "extra": "median of 5 runs: 103.633, 104.332, 104.409, 104.813, 105.135 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0c8dfc58d02c260fe5efd7f577ede05b9b790add",
          "message": "Every view is drawn from tokens, in both themes (DS-1) (#957)\n\n* Every view is drawn from tokens, in both themes (DS-1)\n\nA rendered token test: each of charter's own views and an extension's is\ndrawn in each built-in theme, and every element is checked for a colour\nin an inline style or SVG paint attribute, a var() that is not a token,\na token the theme in force does not set, and an arbitrary-value class.\nIt covers colours built at run time, which the source guard cannot see.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* Token test draws every state of every view, and its loopholes are shut (DS-1 review)\n\nFixtures now reach every state a view tab draws: the changes view's\nPush, Land and a refused landing; bars and columns charts; every tone\nand row mark, with row actions; gone, refused and waiting views; heading\noffers; a revealed secret and an unreadable vault; memory edit and new\nmemory; a picked theme and a custom workspace colour.\n\nBoth guards share one definition of a colour (theme/literal.ts): every\nCSS named colour and the system colours, case-insensitively, and colours\ninside data URLs. The rendered checker reads var() in any case, holds an\nSVG paint's var() to the tokens and the theme, and refuses an arbitrary\nvalue behind a variant, \"!\", a modifier, or as a bare property. Each\nloophole has a probe that was seen red.\n\nOWN_MARKS is exported, and a test fails when a view in it has no state.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-02T23:49:08+04:00",
          "tree_id": "991c7b248fe4fd1e7f774fc3e58e16c9825a26b1",
          "url": "https://github.com/diazoxide/charter/commit/0c8dfc58d02c260fe5efd7f577ede05b9b790add"
        },
        "date": 1790973453199,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5707875,
            "unit": "ms",
            "extra": "median of 5 runs: 0.564, 0.567, 0.571, 0.587, 0.591 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.112492500000002,
            "unit": "ms",
            "extra": "median of 5 runs: 17.046, 17.057, 17.112, 17.174, 17.300 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.62407300000001,
            "unit": "ms",
            "extra": "median of 5 runs: 103.450, 104.200, 104.624, 105.191, 105.457 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4ab28aca7ab92e55366185d172a54c7813cc67a2",
          "message": "First switch into a busy project: the explorer's chat rows paint without layers of their own (#891) (#974)\n\n* A chat's explorer row and state mark paint without layers of their own (#891)\n\nThe first switch of each round into the launch project set L9's p95.\nProfiling on CI (#912) showed the core answers watch_session and\nunwatch_session in under 5 ms on the main thread, with no stall: the\ntime was the WebView's own paint of the project's fifty explorer chat\nrows. Each row had opacity 0.85 and its unknown mark opacity 0.5 inside\nit, two nested transparency layers per row, and the mark a dashed round\nborder. The dimming is now mixed into the colour and the unknown ring is\ntwo solid arcs, still a different shape from done's whole ring.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* paint.test.ts scans every per-chat rule; unreported and the row icon mixed too (#891 review)\n\nThe test read only the first rule with the exact selector, so a later\nor grouped rule, filter: opacity(), a dashed border on another state\nselector or a deleted fallback all passed. It now scans every rule\nwhose selector list names a per-chat element, rejects opacity, filter\nopacity() and dashed or dotted borders in each, and wants a plain\ndeclaration before each color-mix one. .explorer .unreported and a\nrow's icon are dimmed in their colour too.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-03T01:00:35+04:00",
          "tree_id": "33e9f65a14472c61656677d24ec7bde95b6999e5",
          "url": "https://github.com/diazoxide/charter/commit/4ab28aca7ab92e55366185d172a54c7813cc67a2"
        },
        "date": 1790975913298,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.49838499999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 0.492, 0.498, 0.498, 0.506, 0.511 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.276179499999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.238, 16.239, 16.276, 16.283, 16.354 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.7259865,
            "unit": "ms",
            "extra": "median of 5 runs: 101.139, 101.207, 101.726, 101.939, 102.950 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "85d00c8bf37956d42b97f28bd6b1fd213da21697",
          "message": "bench: a throwaway profile still being written to no longer fails the gate (#1001)\n\nAfter the app exits, a WebKit helper can still be writing into the bench's\nthrowaway profile, and a single recursive rmSync then meets ENOTEMPTY. That\nturned main's ubuntu \"app builds\" job red while the cold start itself was\nwithin its limit. tools/cleanup.mjs removes the profile with retries; if it\nstill can't, it reports and leaves the directory behind. Cleanup is\nhousekeeping, not part of the measurement.\n\nRefs #954\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-03T02:02:03+04:00",
          "tree_id": "456a119a62554a7a09c12e296fdca09e93e0b658",
          "url": "https://github.com/diazoxide/charter/commit/85d00c8bf37956d42b97f28bd6b1fd213da21697"
        },
        "date": 1790979479686,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.6255135000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.605, 0.613, 0.626, 0.627, 0.632 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.002759,
            "unit": "ms",
            "extra": "median of 5 runs: 15.928, 15.979, 16.003, 16.045, 16.444 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.099471,
            "unit": "ms",
            "extra": "median of 5 runs: 101.850, 101.853, 103.099, 103.178, 103.471 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "fe47ef5d2195bc2c4768749747ec5efd6e3f8f97",
          "message": "theme guards: a token-only color-mix dims, and the worktree views are drawn (#1020)\n\nMain's web job is red on two tests. Each comes from two PRs that merged separately:\n\n- #974 dims explorer rows with color-mix(in srgb, currentcolor N%,\n  transparent), and #957's colour-literal guard refuses every colour\n  function. A mix of only currentcolor, transparent and var(--token) adds\n  no colour of its own, so both guards (the stylesheet one and the\n  rendered one) now let it through. A mix with a hex, rgb() or named\n  colour inside it is still refused, and probes cover both cases.\n- #950 added the piece-files and piece-file view kinds, and #957's\n  guard requires every view to have a drawn state. Both now have one.\n\nCloses #1017\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-03T02:24:24+04:00",
          "tree_id": "a182c06294779ba39a19b7a258258f7a98272558",
          "url": "https://github.com/diazoxide/charter/commit/fe47ef5d2195bc2c4768749747ec5efd6e3f8f97"
        },
        "date": 1790980235517,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.42856700000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 0.404, 0.406, 0.429, 0.442, 0.444 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.6733015,
            "unit": "ms",
            "extra": "median of 5 runs: 16.473, 16.629, 16.673, 16.686, 16.726 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.13193050000001,
            "unit": "ms",
            "extra": "median of 5 runs: 100.632, 101.826, 102.132, 102.266, 102.397 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "d09addcb16ee0384fc057340f943ea5b459a4f48",
          "message": "Hook spool with sequence numbers: no hook call is lost when the host is down (FD-30)\n\nA hook now delivers its line before it answers its harness. The host says it\ntook the line only once its hearer has recorded the event and fsynced the\nevent log. Otherwise the line goes to the chat's own spool in a project's\n.charter/app/spool/, fsynced, with the next number in that key's sequence and\na MAC under a key derived from the chat's token. The app drains every spool\nwhen it opens a project, before any chat starts, and records each line that\nchecks under the run it ran in, plus each gap and each line that did not\ncheck (hook.spool.gap, hook.spool.rejected, hook.spool.drained).\n\n- The chat sandbox is denied reading and writing every spool and its keys.\n  Nothing spools, and no key is written, where that denial does not reach.\n- A line that is not text, or a keys file that does not read, is rejected and\n  never stops the drain or the next hook.\n- The fsync follows each seal of the event log (FD-24) onto the segment being\n  written.\n- A refused commit is recorded as hook.commit_refused.\n- ADR 0068 §6 is amended by ruling V63, and ADR 0075's registry gains\n  hook.commit_refused.\n\nThis closes the two departures from ADR 0075 §7 that FD-9 recorded: the hook\nanswered before its event was recorded, and the event log was not fsynced.\n\nCloses #667\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-03T07:45:44+04:00",
          "tree_id": "354e0ec590a5befff7bb6f1a0c8688e90b38e3fa",
          "url": "https://github.com/diazoxide/charter/commit/d09addcb16ee0384fc057340f943ea5b459a4f48"
        },
        "date": 1790999209612,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.621307,
            "unit": "ms",
            "extra": "median of 5 runs: 0.603, 0.608, 0.621, 0.630, 0.631 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.0742715,
            "unit": "ms",
            "extra": "median of 5 runs: 15.891, 16.064, 16.074, 16.124, 16.510 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.4633965,
            "unit": "ms",
            "extra": "median of 5 runs: 101.131, 101.259, 101.463, 101.639, 101.723 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "d7d8244356d26b66f1426859cecc6e636e33f704",
          "message": "e2e: Undo of a memory waits for its index line as well as its file\n\nThe memory store's unarchive renames the file back, then appends its\nindex line, under its own lock. The test polled for the file and then\nread the index straight away, so it could land between the two writes.\nIt failed once on train 1 (passed on re-run) and on both platforms on\ntrain 2.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-03T08:13:50+04:00",
          "tree_id": "8daa4763c488e2a4c588673e0940ada30215a949",
          "url": "https://github.com/diazoxide/charter/commit/d7d8244356d26b66f1426859cecc6e636e33f704"
        },
        "date": 1791000891269,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.3439535,
            "unit": "ms",
            "extra": "median of 5 runs: 0.341, 0.343, 0.344, 0.345, 0.359 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.673347999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.653, 16.672, 16.673, 16.708, 16.775 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.08881500000001,
            "unit": "ms",
            "extra": "median of 5 runs: 100.316, 100.334, 101.089, 101.230, 101.267 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "47940265034a153fe1aee01065882e3afa21bb1c",
          "message": "changelog fold: a guessed flag folds nothing, and a stray file is refused\n\nTwo agents ran `node tools/changelog-fold.mjs --help` expecting help. It\nfolded their fragments into CHANGELOG.md and deleted them instead. The\nscript now:\n\n- answers `--help` and `-h` with its usage;\n- refuses any other argument with usage and exit 2, writing nothing;\n- refuses a file in changes/ that isn't README.md or a <slug>.md\n  fragment, because it used to be skipped silently and its entry lost;\n- recognises itself when run through a symlinked path. It used to do\n  nothing and exit 0, so `--check` passed with a fragment waiting.\n\nRefs #1023\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-03T09:46:36+04:00",
          "tree_id": "940b06ffea13112343e0b7d3cdd69af6f69ea332",
          "url": "https://github.com/diazoxide/charter/commit/47940265034a153fe1aee01065882e3afa21bb1c"
        },
        "date": 1791006459617,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.587981,
            "unit": "ms",
            "extra": "median of 5 runs: 0.581, 0.586, 0.588, 0.597, 0.598 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.904024,
            "unit": "ms",
            "extra": "median of 5 runs: 16.375, 16.466, 16.904, 17.328, 17.488 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.4111,
            "unit": "ms",
            "extra": "median of 5 runs: 103.167, 103.278, 103.411, 104.357, 106.405 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "9055f7a4cfd2b366aaf8d17444bb357220374a73",
          "message": "SC-3: a per-chat store, so a chat moving redraws only its own rows\n\nWhat every chat in a project is doing now lives in a store outside React (chatState.ts:\nuseChats, useChatsSelect, ChatsHere), not in a hook's state at the top of PlaneView. Each\nreader subscribes to its own share through useSyncExternalStore:\n\n- a tab's mark and an explorer row's mark read their own chat (ChatRows.tsx: ChatStateMark,\n  memo'd, and TabMarks, moved out of PlaneView);\n- a pane reads its own chat's last move and whether it is running, for its gauge;\n- the status line reads the running count itself;\n- PlaneView reads only the queue, reports, refusals, the quiet list and the show-more orders,\n  each compared so that an unchanged share does not redraw it;\n- the quit list's states and the newest move are reported to the window straight off the\n  store, and PlaneView is memo'd, so the window redrawing for that report does not redraw it.\n\nThe reducer keeps what a move did not change as the same object (the queue, and each chat's\nreports and refusals), so identity checks hold. A move that carries no queue or no report\nlists is still taken, reading them as empty. useChatsSelect is React's own\nuse-sync-external-store/with-selector (MIT, the React team's package), not a hand-written\nselector cache.\n\nA resting tab no longer keeps dnd-kit's one-frame `transition: transform 0ms linear`\n(sortable.tsx). dnd-kit sets it on the render after a strip's items change and relies on a\nnext render to take it off; with the project view no longer redrawn on every move, a strip\nread once never got one, and the inline rule hid the stylesheet's transition (motion.e2e.ts,\ntrain 3). Workspaces.test.tsx guards it.\n\nMeasured with MovedRenders.test.tsx (five chats, one moves; ChatMark and pane renders):\nbefore, 10 marks and 2 pane renders per move; after, 1 mark and 0 panes for a state change,\nand 1 mark and 1 pane when the needs-you queue changes too (#1034).\n\nCloses #681\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-03T11:55:03+04:00",
          "tree_id": "c98a836a08feda0728581c05f7bea69f5012c837",
          "url": "https://github.com/diazoxide/charter/commit/9055f7a4cfd2b366aaf8d17444bb357220374a73"
        },
        "date": 1791014179978,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4331585,
            "unit": "ms",
            "extra": "median of 5 runs: 0.417, 0.427, 0.433, 0.433, 0.443 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.6302965,
            "unit": "ms",
            "extra": "median of 5 runs: 16.565, 16.584, 16.630, 16.638, 16.663 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.4856775,
            "unit": "ms",
            "extra": "median of 5 runs: 100.897, 101.456, 101.486, 102.050, 102.053 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "85b76f2b126a0bfe8e7de24eda40023d74e19cc7",
          "message": "DS-9: the first run counts its prompts and fails above three\n\nW10's interrupt budget: at most three prompts before the first answered\nagent turn. app/src/interruptBudget.ts counts every prompt a page shows:\neach dialog and alertdialog (an open native <dialog> included), and each\ninline question: a group or radiogroup named by its question, or a\nfieldset whose legend is one, so an ask moved onto the page is still\ncounted. Its docstring lists what it cannot see. Every\nscenario in FirstRun.test.tsx now counts its prompts and fails above\nthree, naming each one. A new test walks the longest way to a chat\n(forge question, trust question, picker) and pins it at exactly three,\nso one more prompt on that way fails the build.\n\nThe ForgeQuestion, RepoInstructionsTab and PlaneView comments now describe the\nbudget this way, and docs/design-system.md has a section on it.\n\nCloses #631\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-03T14:10:49+04:00",
          "tree_id": "fa953d4b643dc47b7bebf3050bd399866f57d756",
          "url": "https://github.com/diazoxide/charter/commit/85b76f2b126a0bfe8e7de24eda40023d74e19cc7"
        },
        "date": 1791022433731,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5827484999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 0.571, 0.578, 0.583, 0.583, 0.586 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.004761000000002,
            "unit": "ms",
            "extra": "median of 5 runs: 16.554, 16.999, 17.005, 17.041, 17.223 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.692088,
            "unit": "ms",
            "extra": "median of 5 runs: 103.502, 103.688, 104.692, 105.160, 105.807 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "4d5d9490c64cdc98951f54397c84974abf7a0164",
          "message": "HY-18: ticket and ADR issue forms\n\n.github/ISSUE_TEMPLATE/ticket.yml carries the sections the program map\nfiles a ticket with, and adr.yml an ADR proposal's; both ask for the\nmilestone and each label family HY-18 set up, and adr.yml applies\ntype:adr. The community test holds both forms.\n\nCloses #1097\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-03T16:09:03+04:00",
          "tree_id": "8c5dc2868fd48b7277ef0a00ecdb5c9b5fd88fee",
          "url": "https://github.com/diazoxide/charter/commit/4d5d9490c64cdc98951f54397c84974abf7a0164"
        },
        "date": 1791029980769,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.592132,
            "unit": "ms",
            "extra": "median of 5 runs: 0.580, 0.590, 0.592, 0.597, 0.597 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.956614000000002,
            "unit": "ms",
            "extra": "median of 5 runs: 16.710, 16.882, 16.957, 17.254, 17.264 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.096882,
            "unit": "ms",
            "extra": "median of 5 runs: 102.718, 105.044, 105.097, 105.338, 105.396 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "b6069cea22c6a3efc5621bc933b26e900a0f97c8",
          "message": "Burn-down 1: read-only charter tools pre-allowed, request mode, branch on screen\n\nV79 (#1050): a Claude Code chat the app starts carries an allow for each of\ncharter's five read-only MCP tools (todo_list, memory_search,\nsession_record_list, session_record_read, change_status) beside Smart close's\nallow, in the same per-chat --settings. The list is written out\n(chattools::PRE_ALLOWED), never derived from the read-only mark, so\nask_operator and every write still ask. ADR 0064's SI-8e section and the spec\nare amended, and record what was measured on Claude Code 2.1.288: the\nsession's --mcp-config server wins over a same-named project, local or user\nserver, which is not started at all.\n\nV81 (#1087): prose calls the pr and pr-merge modes \"request mode\" on every\nforge, including prsave's save_branch refusal, the docs and CONTEXT.md, which\ngains a Request mode entry. The config words are unchanged. No recorded\nscenario pinned the old words.\n\n#989: the window calls a piece its branch and its directory the branch's\nfolder. Menu rows name the branch by its own name (Cut carries it) and a\nfolder on no branch as a folder; the bottom bar counts branches. The window's\nmerge, remove, mark-done and list commands send the core's window sentence\n(worktree::Refusal::in_window, the new NotDeclared::in_window), and the\nrefusals charter words itself (seven merge and remove cases) gain one through\nRefusal::Stuck, so no \"worktree\", \"git -C\" or \"--force\" of charter's reaches\nthe window; charter worktree keeps its sentences byte for byte. A merge of a\nmissing piece is refused as missing, and workspace drop says \"branch\nfolders\". CONTEXT.md says so.\n\nCloses #1050\nCloses #1087\nCloses #989\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-03T16:46:13+04:00",
          "tree_id": "59dd233ae04b01cabfeebddb11ea45874bb8dbab",
          "url": "https://github.com/diazoxide/charter/commit/b6069cea22c6a3efc5621bc933b26e900a0f97c8"
        },
        "date": 1791031883314,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4830015,
            "unit": "ms",
            "extra": "median of 5 runs: 0.477, 0.482, 0.483, 0.489, 0.494 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.5059435,
            "unit": "ms",
            "extra": "median of 5 runs: 16.172, 16.214, 16.506, 16.898, 16.994 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.1567185,
            "unit": "ms",
            "extra": "median of 5 runs: 100.704, 101.080, 101.157, 101.210, 102.185 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "21e20045093dc40e68448a288967f8a10d0d3624",
          "message": "FM-1: branches in the explorer expand into their files\n\nEach branch in the explorer, and each repo for its own folder (#948), has a\nFiles row that opens folder by folder into its files, inside the same tree.\n\nPrefactor: charter-core's `piecefiles` becomes `files` (git mv), keeping all\nof the light editor's confinement, and its calls take a `Branch` (a piece, or\nno piece for the repo's own folder). The light editor's commands moved onto\nit; their tests pass on the new module.\n\nNew in core: `files::tree(branch, folder)`, one level per call: name, kind\n(folder, file, link), ignored, and the reason it does not open. Plus\n`files::folder` for the watch.\n\nApp: `branch_tree` and `files_watch` (both off the main thread), a\n`files-changed` event from a new `filewatch.rs`, and the explorer's lazy file\nrows with tree semantics, arrows, Enter and Home/End.\n\nDecided in implementation:\n- D-1 Where the files hang: a \"Files\" row is the first child of each branch\n  and each repo. The branch row itself does not fold. A click on a branch\n  picks where the next chat starts, and its chats stay drawn under it (#238).\n  Rejected: folding the branch row, which hides its chats.\n- D-2 Laziness: one listing plus one `git check-ignore` per opened folder.\n  Not `ls-files`, which walks the whole tree. Paths go to git on its standard\n  input (`--stdin -z -v -n`, through a new `git::run_with_input`) as\n  `./<path>`. The answers are matched by position, never by spelling. A name\n  is also asked in composed form, and it counts as ignored when either\n  spelling is.\n- D-3 Ignored: an ignored folder still expands (listing only). An ignored file\n  is refused. `.git` counts as ignored-and-refused, so it is hidden by default\n  and shown with its reason under \"Show ignored files\". That toggle is\n  per-window state and appears only while some branch's files are open.\n- D-4 A nested repository or submodule is drawn, and nothing inside it\n  opens, which matches what `open` answers. A linked folder never expands,\n  even when it stays inside. A link opens only to another offered file.\n- D-5 The repo's own folder: `Branch { piece: None }`, accepted only when\n  `git rev-parse --show-toplevel` is that folder, so a plain folder inside the\n  project's repository is refused. The view key is `ws/repo/` (an empty piece\n  segment, which no piece name can be), and the commands take\n  `piece: string | null`.\n- D-6 Live updates reuse planewatch's notify and debouncer pattern in a new\n  `filewatch.rs`. It watches exactly the folders a window has opened, each\n  non-recursively, as a per-window set, dropped when the window is destroyed.\n  A newer ask always wins over a set that resolved late. Rejected: widening\n  planewatch, which deliberately stays out of clones, and polling.\n- D-7 Wording (V81, CONTEXT.md): the refusals keep burn-1's \"branch's\n  folder\" sentences, which are now on main, on the new `files` module. The new\n  rows say \"Files\", \"Show ignored files\" and \"N more not shown\".\n- D-8 One folder level answers its first 5,000 entries after sorting, plus a\n  count of the rest, drawn as \"N more not shown\". The folder is listed through\n  a descriptor opened one component at a time with O_NOFOLLOW|O_DIRECTORY, so\n  a folder swapped for a link is refused. Windows lists by path and then\n  checks the folder again. A path under a submodule the index records but\n  that has no `.git` on disk counts as nested too, so it never reaches git,\n  and a link to a target whose name is not UTF-8 is refused.\n- D-9 `files_watch` takes at most 256 folders and resolves each branch once\n  (`files::folders`). planewatch and filewatch share `watchset.rs`\n  (`matters`, `follow`), and the window-side `branch()` is shared too.\n\nTests: crates/charter-core/tests/a_branch_expands_into_its_files.rs (20, on\nreal git fixtures, including names git spells differently, a link to one,\nthe 5,000 cap and several folders of one branch); a unit test that a folder\nwhich is a link by the time it is listed is refused; app unit tests for\nbranch_tree and filewatch; src/Explorer.files.test.tsx (mockIPC: refused\nrows with their reasons, ignored toggle, re-read and removal on\nfiles-changed, the count of the rest, arrows, Enter, Home and End);\nworkspace-explorer.e2e.ts expands the fixture branch, opens README.md in its\nfile tab, sees an agent's file appear and go, sees one appear in a nested\nopen folder, and expands a repo's own folder; a link into an uninitialised\nsubmodule; and a folder whose name holds a newline expands.\n\nLeftover: #1128 (on Windows, list a branch's folder through a handle).\n\nCloses #1104\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-03T17:56:14+04:00",
          "tree_id": "580acd953ae290e1e5dd814d375bfbb0c3255cd9",
          "url": "https://github.com/diazoxide/charter/commit/21e20045093dc40e68448a288967f8a10d0d3624"
        },
        "date": 1791036095712,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.581055,
            "unit": "ms",
            "extra": "median of 5 runs: 0.565, 0.580, 0.581, 0.590, 0.597 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.1382655,
            "unit": "ms",
            "extra": "median of 5 runs: 16.726, 16.745, 17.138, 17.275, 17.408 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.21552550000001,
            "unit": "ms",
            "extra": "median of 5 runs: 102.839, 103.990, 104.216, 105.765, 105.873 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "e396e568fb8747269cb79e6942524cbf7cf7e724",
          "message": "FM-2: the file tab becomes a tree beside a resizable preview\n\n\"Browse the files\" shows the branch as the explorer's own tree (FM-1's\nrows, read a folder at a time) instead of a flat list capped at 500,\nbeside a read-only preview, with a divider between them that is dragged\nor moved with the arrow keys. Where the divider was left is kept for the\ntab across a close and a relaunch. Opening the same branch's files again\nstill brings its one tab forward; the view key is unchanged.\n\nCore: files::open now knows an image by its first bytes (PNG, JPEG, GIF,\nWebP) and answers its type and bytes, and the preview cap is 2 MiB\n(was 5 MiB). reopen::View gains `split`, and a `piece-files` view's key\n(`ws/repo/piece`, or `ws/repo/` for the repo's own folder) is now kept in\nthe record, so the tab comes back at a relaunch at all.\n\nApp: PieceFile gains `image` (base64), ViewTab gains `split`; tabs.ts\nkeeps each view's split (`splits`), kept after its tab closes; a new\neditor/BranchTree.tsx draws the tree; PieceFiles has one fetch path and\none union for a file's states (#988).\n\nShared tree component (kept minimal, for FM-4 and FM-7): Explorer.tsx\nexports its file-row pieces, its folder walk is hoisted out of treeOf\nunchanged (folderNode, walkRows, plus fileTreeRows for a branch-rooted\ntree), and TreeItem may carry aria-selected. branchFolders.ts now tells\nthe core the union of every tree's open folders in the window, since\nfiles_watch replaces one set per window and the tab and the explorer\nwould otherwise unwatch each other.\n\nDecided in implementation:\n- D-1 The split is stored as the tree's share of the tab in whole\n  percent (held 10-70), on the record's view line as `split`, plus a\n  window-held map by view key so a closed tab reopens at its width; a\n  branch never moved starts where the last one was left in this window,\n  else 30%. Rejected: pixels (do not follow the window's size); one\n  global width in the layout file (the brief and the spec put it on the\n  tab); an opaque per-view state blob (nothing else needs one yet).\n- D-2 The divider is react-resizable-panels' Separator, as the window's\n  regions and pane splits are: pointer, arrows, Home/End and\n  role=separator with aria-valuenow come with it. Rejected: a hand-made\n  splitter.\n- D-3 Images are decoded in the window with createImageBitmap into a\n  canvas. The CSP gives images no source but the app's own, and this adds\n  none: no data:, blob: or asset protocol. An animated image shows its\n  first frame; an SVG is text and previews as text. Rejected: widening\n  img-src (a security-boundary change for a preview).\n- D-4 An image is known only by its signature bytes; a name never makes\n  one. Raster kinds the web view decodes: PNG, JPEG, GIF, WebP.\n- D-5 Markdown (.md, .markdown) renders by default with react-markdown,\n  skipHtml, http(s)-only links (ReleaseNotes' ExternalLink), and images\n  drawn as their alt words, so a rendered file fetches nothing. \"Source\"\n  (aria-pressed) shows the text in the light editor. A file's own tab\n  opened at a line (a jump) starts on the source.\n- D-6 2 MiB is 2 * 1024 * 1024 bytes and applies to text and images, in\n  both tabs (V86 F4), replacing RC-5's 5 MiB.\n- D-7 The record keeps a `piece-files` key only when it is exactly three\n  names charter would mint (the piece may be empty), for charter's own\n  view; every other view still takes one name or none. A split outside\n  1-99 is forgotten, never a reason to drop the tab. Single-file tabs\n  (key holds a path) are still not kept, as before.\n- D-8 The flat list's type-to-narrow box goes with it: a lazy tree has no\n  whole list to narrow. Finding a file by name is FM-7's ⌘P and the\n  sidebar's type-to-filter is FM-4's.\n- D-9 PieceFiles.test.tsx's list tests were rewritten for the tree and\n  its too-large wording follows the 2 MiB cap; the single-file and\n  your-editor tests are otherwise unchanged (#988's \"unchanged\" is\n  superseded by the list becoming a tree).\n\nReview round (dispatcher decisions F1-F5):\n- D-10 Decoded size is bounded, not just bytes: core reads an image's\n  declared width and height from its header with the `imagesize` crate\n  (MIT, header-only, no dependencies; cargo deny passes) and answers an\n  image past 40 megapixels as `HugeImage` (type and size, no bytes),\n  which the preview says in a sentence. A signature whose header gives no\n  size is answered as binary, never handed to the window to decode.\n  40 MP is about 320 MB decoded at four bytes a pixel, a phone photo\n  still fits. Rejected: hand-parsing four formats; a cap on one side.\n- D-11 The flat list's `piece_files` command is removed from the IPC\n  allow-list and both generated clients; `files::list` stays in core,\n  where `open` still uses it.\n- D-12 One navigation guard for every webview (new `navguard.rs`): a\n  plugin `on_navigation` allows only the app's own origin\n  (`tauri://localhost`, `http(s)://tauri.localhost`, and in a dev build\n  the configured devUrl's origin), and `on_new_window` denies every\n  new-window request on both windows charter builds (`lib.rs`,\n  `windows.rs`). Links still open through the opener plugin. The\n  predicate is unit-tested; that WebKit/WebView2 route a middle click or\n  `window.open` through these hooks is Tauri's and not measured here.\n- docs/plane-format.md gains the `views[]` rows of `app/reopen.json`,\n  with `key`'s piece-files exception and `split`'s 1-99 range.\n\nTests: three core tests in any_file_in_a_piece_opens_in_the_light_editor.rs\n(an image known by its bytes; an image declaring a huge canvas is said\nby its size; the 2 MiB edge for text and images); navguard unit tests; two\nreopen unit tests (the file tab and its split come back; a bad key or a\nwild split); an app unit test that an image crosses as its type and\nbase64; tabs.test.ts (the split is kept across a close, comes back with a\nlaunch); ViewTabs.test.tsx (a relaunch hands the split back to the\nrecord); PieceFiles.test.tsx (the tree, no cap at 600 entries, arrows,\nOpen in a tab of its own, and the markdown, image, huge-image, binary and\nover-2-MiB previews); workspace-explorer.e2e.ts opens the fixture branch's file\ntab, opens a file, drags the divider and moves it with a key, checks the\nrecord, opens the tab twice and keeps one, and closes and reopens it at\nthe same width. FM-1's e2e now sees README.md rendered.\n\nLeftover: #1132 (FM-2 follow-ups).\n\nCloses #1105\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-03T21:19:16+04:00",
          "tree_id": "c6da2c54cc6b2e80ab36fa8078a13b5a29ecf71e",
          "url": "https://github.com/diazoxide/charter/commit/e396e568fb8747269cb79e6942524cbf7cf7e724"
        },
        "date": 1791048078313,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.597519,
            "unit": "ms",
            "extra": "median of 5 runs: 0.595, 0.597, 0.598, 0.606, 0.610 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.154092999999996,
            "unit": "ms",
            "extra": "median of 5 runs: 16.564, 16.777, 17.154, 17.311, 17.535 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.50796550000001,
            "unit": "ms",
            "extra": "median of 5 runs: 104.396, 105.163, 105.508, 105.576, 106.708 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "16d6d40f77572b56d66bc9cbe8ce971e7056ab53",
          "message": "FM-7: ⌘P finds a file by fuzzy name\n\n⌘P (Ctrl+Shift+P off a Mac) finds files as well as projects. Once something\nis typed, a \"Files\" group follows the projects in the switcher: each file with\nits folder, branch and project, and Enter or a click opens it in its file tab\n(the explorer's `pieceFileView`, so FM-2's file tab picks it up in one place).\nThe scope is said above the group and follows focus: the branch picked in the\nexplorer, else the project in front; Tab widens it to the project and then,\nwhile more than one is open, to every open project, Shift+Tab narrows. The\nwider rungs keep the focus as their order: the picked branch leads the\nproject, and the project in front leads every open project, so of two equal\nhits the nearer comes first.\n\nCore: `files::find(scope, query, most)` and `files::Finder` in their own\nsubmodule, `crates/charter-core/src/files/find.rs`. A scope is a list of\n`Place { plane, branch }`; `files::branches(plane)` is every branch of a project\n(each repo's own folder, then its pieces). Names come from the branch's\noffered list (`ls-files --cached --others --exclude-standard`, the light\neditor's own), so ignored files (a secret file git ignores among them) and\n`.git` are never found; a file git does not ignore is offered, as the light\neditor already offers it. A hit is checked as it is shown: a plain file, or a link to\nanother offered file of the same branch; never a link out, a link to an\nignored file, a submodule, a nested repository or a file gone since the\nlisting. A branch that cannot be listed is answered in the core's sentence\nand the rest still answer. A branch is listed up to `files::LISTED`\n(200,000 files) and the answer says so past it. Each keystroke selects the\nbest `2 x most` matches in linear time and sorts only those.\n\nApp: `find_files(session, scope, query)` and `find_files_end(session)` in\n`findfiles.rs`. `FileScope::Project` carries the focused branch (`near`),\nand `FileScope::OpenProjects` the project in front and that branch; they\nonly order the scope. The window names projects by `PlaneId` (vouched by the\nregistry) and branches by name; \"all open projects\" is the registry's own\n`open_now()`. One `Finder` per window per palette session, dropped when the\npalette closes or the window is destroyed; a find that arrives after its\nsession ended keeps nothing. Session ids are seeded from the load time, so a\nreloaded window never reuses one the core still holds. The project in front reports its\npicked branch (`PlaneReport.branch`); a file in another project brings that\nproject forward and opens its tab there (`fileAsked`, settingsAsk's shape).\n\nDecided in implementation:\n- D-1 Matcher: `nucleo-matcher` 0.3 (helix's; fzf's algorithm with path\n  boundary bonuses via `match_paths`, no allocation per match). MPL-2.0, on\n  deny.toml's allow-list; its deps memchr and unicode-segmentation were\n  already in the lockfile, so one crate is added. `cargo deny check licenses\n  bans sources` passes. Rejected: `fuzzy-matcher` (MIT), skim's older\n  matcher, with no release in years and no path-boundary configuration.\n  Patterns use `Pattern::new(.., AtomKind::Fuzzy)`, so `!`, `^`, `'` and `$`\n  in a typed name are letters, not fzf syntax; words split on spaces.\n- D-2 ⌘P was already the project switcher (FR-27). V86 F10 (operator) gives\n  ⌘P to file search, so the two share the one surface: projects first, files\n  as their own group after them, never ranked together (ADR 0079's rule).\n  This replaces D-FR27a's \"the key does nothing with one project open\": with\n  files to find, the key opens whenever a project is open, and a lone project's\n  own row (which could only say it is in front) is not listed.\n  `Projects.test.tsx`'s one-project test now asserts that. Rejected: a second\n  key for files, which would leave F10 unmet; moving the switcher off ⌘P, which\n  amends FR-27.\n- D-3 Scope ladder: branch (the explorer's picked spot in the focused\n  workspace, a piece or a repo's own folder) → project (every branch of every\n  workspace, the picked branch first) → every open project (the one in front\n  first). A missing rung is skipped, and \"every open project\" is skipped\n  while only one is open. Tab cycles\n  round, Shift+Tab goes back. The cockpit's branch (FM-5) is not built yet;\n  it feeds the same `PlaneReport.branch` when it lands (noted on #1108).\n- D-4 Performance: cached per palette session, not streamed. The palette asks\n  with an empty query as it opens, which lists the scope and finds nothing,\n  so the first keystroke pays only for its match. Link checks run only on the\n  hits shown (at most 50), not on every match. Ties go to the branch nearer\n  the front of the scope, then the shorter path. A branch is listed up to\n  200,000 files (about 18 MB of names while the palette is up), said past it.\n- D-5 Only files are hits; a folder is not (submodule, nested repo).\n- D-6 Files only in the ⌘P surface; ⌘K stays for actions (dispatcher).\n\nMeasured, release build, macOS, on a branch of 100,000 tracked files\n(`measured_on_a_hundred_thousand_files`, ignored, run by hand):\n- The core match per keystroke, the core's half of ADR 0086 L1 (50 ms\n  keystroke to screen): at worst 3.5 / 3.6 / 4.0 ms over three runs at load\n  54-59, with the top-k selection (11.2 ms with a full sort, at load 8-10).\n  The window's half (IPC and draw) is not measured here; it stays open on\n  FM-12 (#1115).\n- The first find of a session (git listing + match): 347 / 353 / 418 ms at\n  load 54-59 (134 / 174 / 487 ms at load 8-10). It is past ADR 0079's 100 ms\n  project-search figure, which is why the palette asks it as it opens and not\n  on the first keystroke.\n\nTests: crates/charter-core/tests/a_file_is_found_by_fuzzy_name.rs (11 on real\ngit fixtures: a fuzzy fragment ranks the file first; ignored files and a\nsecret file git ignores never found; links found only to another file of the\nbranch; nothing inside a submodule or nested repo; several branches say which\nbranch each hit is in, nearest first; a project's branches; a finder lists\neach branch once per session; an unlistable branch is said and the rest\nanswer; a branch past the listing cap is searched up to it and says so; the\nbest hits are kept when far more match than are shown; an empty query finds\nnothing and the most is kept; plus the ignored 100k measurement). App unit\ntests in findfiles.rs (a hit names its project and branch in any scope; the\nfocused branch's copy leads the project scope; the project in front and its\nfocused branch lead every open project; a find after its session ended keeps\nnothing; sessions). src/Palette.files.test.tsx (mockIPC: the two groups and\narrows across them, where each file is, the scope and Tab / Shift+Tab, no\n\"every open project\" rung with one project open, nothing matching and a\nrefused branch, a branch listed in part, a lone project).\nworkspace-explorer.e2e.ts: pick the fixture branch, ⌘P, the scope (found by\nits role and name) says the branch, type a fuzzy fragment, the file's row\nsays its folder and branch, Tab to the project, where the file still leads,\nEnter opens it in its file tab.\n\nCloses #1110\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-03T22:58:59+04:00",
          "tree_id": "65fd339b135d297be43e1f6b3d6bebfbf8bbbb13",
          "url": "https://github.com/diazoxide/charter/commit/16d6d40f77572b56d66bc9cbe8ce971e7056ab53"
        },
        "date": 1791054022915,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.479157,
            "unit": "ms",
            "extra": "median of 5 runs: 0.475, 0.477, 0.479, 0.480, 0.483 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.2933315,
            "unit": "ms",
            "extra": "median of 5 runs: 16.200, 16.204, 16.293, 16.543, 16.818 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 100.9315075,
            "unit": "ms",
            "extra": "median of 5 runs: 100.495, 100.510, 100.932, 101.212, 102.334 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "390ba08b0b63db8f9de4848d698cad11a00e1f27",
          "message": "HP-19: what each harness can do here, on its own card\n\nA harness's capability card, read off the real sources: its declaration\n(FD-14: [levels], [capabilities], [terminal] ready_to_type) and, for a\nbuilt-in, the adapter charter ships for it (whether charter can sandbox\nit). charter_core::harness_card is neutral: no harness is named in it, and\na harness a project declares gets a card by being declared.\n\n- The card is a view tab, `harness/<name>`, answered by the core in the\n  panel vocabulary and labelled \"What Codex can do here\": a row per line,\n  yes / no with the declaration's reason / not declared.\n- A chat's header names the product it runs; pressing it opens the card.\n- The picker shows the picked harness's card: its label and a line for\n  each thing it lacks.\n- A control that is off because the harness lacks something says the\n  card's line for it and the card's label: the Curate rows (palette and\n  menus) and the first task's rows, which used to name Claude Code and\n  Codex by hand.\n\nReview fixes (HP-19 review of 0e9006b):\n- M1: a harness a project declares has no adapter, so every line charter\n  acts on only through one (hooks, what they report, per-chat plugins,\n  typing a prompt in, the sandbox) answers No(\"charter does this only for\n  a harness it ships an adapter for\"), whatever the declaration says. It\n  keeps what charter does from a declaration alone: resuming, its ACP\n  agent. The curation and first-task refusals for a non-built-in now say\n  the card's line too.\n- M2: a declaration's `title` (<= 60), `tested` (<= 80) and a `no:` reason\n  (<= 200) are refused at parse time unless one line with no control, bidi\n  or zero-width character, the file named; a project title may not be a\n  built-in's. The card also clips them with shown::one_line.\n  docs/plane-format.md records the limits.\n- F1: OpenChat.card and ProfileRow.harness are plain nullable fields, so\n  bindings.ts has no _Serialize/_Deserialize split for them.\n- F2: the first-hour word test covers lines() and lacks() for every\n  built-in and two declared harnesses; ready_to_type's reason is plain.\n\nTests: charter-core harness_card (23: one per adapter-only line, every\ncapability charter reads has a line, every line and off-control sentence\nkeeps to the first hour's words, the card clips) and harness_declaration\n(4: title, tested and a reason refused unless one short plain line, a\nbuilt-in's title); charter-app views (the card tab, a harness gone),\ncuration and first task (the off reason is the card's line, built-in and\ndeclared); vitest HarnessCard.test.tsx (picker, header),\nFirstTask.test.tsx (the off row's tooltip), theme/views.test.tsx (the card\ntab's two render states); e2e harness-card.e2e.ts (header to card tab, and\nan off Curate row saying opencode's line).\n\nDecided in implementation:\n- D-HP19-1 Words: ADR 0072 §3 / ADR 0073 §6 rule the card's words, so the\n  label is \"What <product> can do here\", each \"no\" is one plain line, and\n  \"level\", \"harness\" and \"capability\" stay in code. The levels are on the\n  card as two lines (\"Tells charter whether it is working, waiting or\n  done\", \"Works without a terminal, through its ACP agent\"). A core test\n  keeps every line off the first-hour word list. Rejected: showing \"L2\" /\n  \"level 3\", which the ADRs keep off the card.\n- D-HP19-2 The card tab is a built-in view answered by `open_view` in panel\n  blocks, like the persona and changes views, not a bespoke component and\n  not a new command. Rejected: a dialog (operator rule: tabs hold views).\n- D-HP19-3 The window gets the card at a glance on the answers it already\n  has: `ProfileRow.harness` and `OpenChat.card` (`HarnessGlance`: name,\n  title, label, lines, cannot_type), null for a shell or a kind with no\n  declaration. Rejected: a new `harness_cards` command the picker and\n  header would each have to ask.\n- D-HP19-4 Sandbox comes from `sandbox::compiler(harness)`, the adapter's\n  answer, so #1123 (Codex) and SD-2 change the card without touching it.\n- D-HP19-7 (review S2) A project's title equal to a built-in's, ignoring\n  case, is refused: a card and picker naming it would read as that harness.\n- D-HP19-5 Off-control reason = the card's line + \"See What X can do\n  here.\" as text; a link is in the follow-up (#1134).\n- D-HP19-6 The curation and first-task refusals now read the card's line\n  instead of naming Claude Code / Codex by hand (harness-agnostic).\n\n- D-HP19-8 (train 11) The header button is the pane's first Tab stop: it is\n  drawn in the top-left corner, before split and end at the top right, so\n  the order is reading order and the product stands. keyboard-reach's\n  \"controls first\" now means everything the pane's corners draw, left\n  then right, before the terminal; Window.keyboard.test.tsx lists the\n  button before Split right.\n\nFollow-ups: #1134 (one-time notes at a fallback's point of action, the\nlabel as a link, unreported from the card's lines, a header button for a\ndeclared harness's chat, palette rows per harness).\n\nCloses #677\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-04T00:20:02+04:00",
          "tree_id": "1ae5ba27db8e6b10d72b4d8694c30d5e2b41fc22",
          "url": "https://github.com/diazoxide/charter/commit/390ba08b0b63db8f9de4848d698cad11a00e1f27"
        },
        "date": 1791058867756,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5998695,
            "unit": "ms",
            "extra": "median of 5 runs: 0.577, 0.582, 0.600, 0.601, 0.605 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.040411499999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.701, 16.993, 17.040, 17.259, 17.418 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.8035555,
            "unit": "ms",
            "extra": "median of 5 runs: 104.727, 105.176, 105.804, 106.094, 106.410 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "ba656b3531dbef43beaee920d814fc43afa40969",
          "message": "FM-8: ⌘⇧F searches the content of the files in a Search view tab\n\nA content search over a branch, a workspace, a project or every project open in\ncharter (#1103, V86 F9/F10). The core engine is `charter_core::files::search`\n(files/search.rs): the `ignore` crate walks each branch in-process and\n`grep-regex`/`grep-searcher` match line by line, with literal or regex, match\ncase and whole word. `search_files` streams the hits to the window that asked\nas `files-searched` events, tagged with the tab's id and run. A page stops at a\ncap, and \"Show more\" continues the same walk. A new query stops the old one.\n\n⌘⇧F (Ctrl+Shift+F off a Mac, outside a chat) opens a Search view tab keyed by\nscope and query. Hits are grouped project → branch → file, with their matching\nlines and counts. ↓/↑/Home/End step through the hits, and Enter opens one in\nthe branch's file tab at its line, through a new `fileJump.ts` that #984 can\nreuse.\n\nDecided in implementation (operator offline; recommended option taken):\n- D-FM8-1 FM-8 sits on main after FM-7 (train 10) and trains with FM-4. It reuses FM-7's\n  FileScope (which gains a `workspace` variant), `files::{Place, Named,\n  branches}`, the nearest-first ordering (now `findfiles::branches_in`), the\n  registry check (now `findfiles::projects_of`, shared by find_files and\n  search_files), and the App → PlaneView file-ask plumbing (with an optional\n  line).\n- D-FM8-2 git's offered list is the only rule for what is read. The `ignore`\n  walk reads no rule file of its own (git_ignore, git_exclude, git_global and\n  ignore all off). An agent can plant any of those files as a link to a device\n  or a FIFO, and the crate would follow it. The walk descends only into folders\n  that hold an offered path (a range check on the sorted list), so it opens\n  nothing but folders and never walks target/ or node_modules/. The list is\n  one `git ls-files` per branch: operator-initiated git, which stays on the\n  binary (V88a), through the hardened runner (FM-4, in the same train, sets\n  GIT_NO_LAZY_FETCH on every call it makes). The search also never reads:\n  - vault or charter-state paths (the leak guard's one pattern);\n  - credential names (`reposave::secret_name` plus .npmrc/.pypirc, even when\n    committed);\n  - binary files or images (the preview's own sniff);\n  - files past 2 MiB;\n  - links, .git, or nested repositories.\n  A tracked file under an ignore pattern is now searched, as the light editor\n  opens it.\n- D-FM8-3 The branch's folder is opened once from the project's root, one\n  component at a time with O_NOFOLLOW (`files::hold_folder`), and held in the\n  walk. Every file is opened relative to that handle, one component at a time\n  with O_NOFOLLOW, and O_NONBLOCK for the file itself (`files::open_inside`,\n  FM-1's openat walk). A folder swapped for a link mid-search is refused, not\n  followed, whether it is inside the branch or an ancestor of it such as\n  `.worktrees/<repo>` (D-88i). This replaces `contain::open_no_link` here.\n- D-FM8-4 Stop and the page's time are heard inside a file. The matcher reads\n  through `search_reader` from a reader that hands it 16 KiB a fill and\n  checks both before each fill. `heap_limit` is 256 KiB, and a file with a\n  longer line is reported as not searched. A file the page's time runs out in\n  is reported as not searched, and Show more carries on after it. Measured\n  (debug): a 2 MiB file of a pattern that defeats the lazy DFA takes 8 s\n  whole, and a stop raised at 0.5 s now ends the page in well under 1 s.\n- D-FM8-5 A sequential, name-sorted walk, not ignore's parallel walker. It\n  gives a stable order for grouping and a walk Show more can resume. FM-12\n  (#1115) measures whether more is needed.\n- D-FM8-6 Bounds:\n  - 200 matching lines a page, counted by whole files;\n  - 100 lines shown per file, with the full count;\n  - 240 characters of a line, around its first match;\n  - 10 s a page;\n  - a query of at most 1000 characters, on one line;\n  - regex size_limit 1 MiB and dfa_size_limit 8 MiB.\n- D-FM8-7 Hits stream as events sent to the asking window (emit_to), not a\n  Tauri Channel, because channel commands are excluded from the UI RPC client.\n  A new query from a tab is asked without an `end` first: asking stops the\n  tab's last run in the core, so an end and a begin can never arrive out of\n  order. A run that is not the tab's newest is never kept. A window keeps at\n  most 8 searches.\n- D-FM8-8 A hit opens the branch's file tab (FM-2) with the preview on the\n  file at the line. A markdown file shows its source. A hit in another project\n  brings that project forward. ⌘P still opens a file in a tab of its own.\n- D-FM8-9 The key is ⌘⇧F on a Mac. Off a Mac it is Ctrl+Shift+F, except while\n  a chat has the keyboard, where that chord stays the chat's find bar. It takes\n  no byte from a chat (docs/ui-primitives.md).\n- D-FM8-10 A Search tab's key is `kind|workspace|branch|flags|query`. It\n  follows the query as the query settles (250 ms), and it is never written to\n  the reopen record, because the query is the operator's own text.\n\nTests:\n- core: crates/charter-core/tests/content_is_searched_across_branches.rs (23\n  tests): literal, regex (and a refused one), case, whole word; never ignored,\n  link, vault, credential, binary, image, past 2 MiB, .git or a nested repo;\n  a tracked file under an ignore pattern is searched; an ignore file planted\n  as a link to /dev/zero or a FIFO is never opened; a stop raised inside a\n  slow file ends the page in about a second, and so does the page's time;\n  a line past 256 KiB is said; a folder swapped for a link is refused, inside\n  the branch or above it;\n  streaming, the page cap and resuming, per-file and per-line caps; a\n  multi-project scope; a refused branch; bad queries.\n- app: searchfiles.rs unit tests (open projects come from the registry only,\n  the workspace scope, paging, a new run stopping the old, a late run not kept,\n  the window cap, a file not searched said with its branch, a refused query).\n- vitest: SearchTab.test.tsx (render states; a re-ask sends no end first) and theme/views.test.tsx (two\n  Search tab states).\n- e2e: workspace-explorer.e2e.ts, where ⌘⇧F finds a fixture string and opens\n  it at line 5 in the file tab.\n\nLeftovers: #1137 (FM-8 follow-ups).\n\nCloses #1111\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-04T02:36:04+04:00",
          "tree_id": "9f0677baa85857669f5a09d824a294f2f8e8c9e6",
          "url": "https://github.com/diazoxide/charter/commit/ba656b3531dbef43beaee920d814fc43afa40969"
        },
        "date": 1791067055542,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5843700000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.582, 0.583, 0.584, 0.587, 0.594 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.8565885,
            "unit": "ms",
            "extra": "median of 5 runs: 16.686, 16.832, 16.857, 17.236, 17.382 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.433087,
            "unit": "ms",
            "extra": "median of 5 runs: 103.935, 104.237, 105.433, 105.619, 106.473 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "0393f3736f476a866acff3c7ff1712e91f39c663",
          "message": "SD-2: suggest only harnesses this machine can sandbox; portable program tests\n\nA refusal that suggests another harness (\"Start this chat on a … profile\") listed every\nharness with a sandbox compiler, so it still named Codex, which a sandboxed project refuses\nuntil #1123. It now names only the harnesses `never_on` lets start sandboxed on this system.\n\nTwo program-check tests used an opencode profile, which charter wraps on macOS only, so on\nLinux the \"cannot wrap opencode here\" refusal came first and they failed in CI. They now use a\nClaude Code profile and test the same rule on every system.\n\nRefs #695\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-04T03:59:43+04:00",
          "tree_id": "a130d0b543856e86e1d358377503a6b2c26c01b2",
          "url": "https://github.com/diazoxide/charter/commit/0393f3736f476a866acff3c7ff1712e91f39c663"
        },
        "date": 1791072076903,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.607289,
            "unit": "ms",
            "extra": "median of 5 runs: 0.575, 0.582, 0.607, 0.612, 0.621 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.3171395,
            "unit": "ms",
            "extra": "median of 5 runs: 17.161, 17.281, 17.317, 17.358, 17.358 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.86770100000001,
            "unit": "ms",
            "extra": "median of 5 runs: 104.663, 105.631, 105.868, 106.113, 108.858 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "53f0e4e505caf8667f9e8078f55fa04444eb787f",
          "message": "HP-6: answer a Claude Code permission prompt from the needs-you list\n\nA Claude Code chat's `PermissionRequest` hook now runs `charter hook\npermissionrequest`. The hook hands its payload to the project's hook channel\nand waits there. The host raises an ask in the shared HP-5 registry, and the\nwindow's ✋ list shows it with the options Claude Code offered. The operator's\nchoice goes back on that same connection, and the hook prints Claude Code's\nown decision, so the chat carries on without its pane having focus. If nobody\nanswers, or the hook goes away, nothing is printed and the pane's own prompt\ndecides. charter never allows or denies by itself.\n\n- same-user: `peer_process_of`, the shared ancestry walker\n  (`ancestry.rs`) and `admit_host`. The permission hook writes nothing to a\n  listener unless it is its own ancestor and holds the socket.\n- core: `harness::hooked` reads the hook's ask (with V28c applied),\n  renders Claude Code's decision, and holds open hook asks (`HookAsks`, capped\n  at 32 per chat and 64 KiB). `hookwire::permission` is the one hook line\n  that waits.\n- `Asks::answer` now takes the chat. An answer naming another chat is\n  refused as unknown. ACP's `Chat::answer` uses it too.\n- Claude Code adapter: the permission hook is armed in the session's\n  `--settings`, never in the plugin's `hooks.json`.\n- app: `asking.rs` adds `pending_asks`, `answer_ask` (local-ui) and the\n  `asks-changed` event. NeedsYou draws each ask with its options.\n- session protocol: `answer` is refused (`not_allowed`) on every link.\n  `ui::WINDOW_ONLY` keeps `answer_ask` off the UI RPC, and the generated\n  `uiRpc.ts` leaves it out.\n\nDecided in implementation:\n- D-HP6-1 Who answers. Only the window, through the Tauri IPC, admitted as\n  local-ui. Rejected: answering over the link or `charter inbox` (approval).\n  FD-27 (#664) has not built the scope check, and a link's local-ui\n  credential is a file a chat can still read (V16a). So the session\n  protocol's `answer` is refused for every host, and `answer_ask` is in\n  `ui::WINDOW_ONLY`. This tightens a fail-closed default and is re-opened by\n  FD-27.\n- D-HP6-2 Replay and misroute. `Asks::answer(chat, id, ..)` is atomic. A\n  mismatched chat reads as Unknown, open or closed, so it reveals nothing.\n  First-answer-wins makes a replay \"answered elsewhere\". Rejected: checking\n  `chat_of` and then answering in two locks (ACP's old shape).\n- D-HP6-3 Arming. The hook goes in `--settings` (argv), not the bundled\n  plugin, because a hook that can allow must never come from a file a chat\n  can write. `plugin::tests` still holds that.\n- D-HP6-4 Trust. The host parses the payload with its own hook timeout, so\n  the chat never sets the deadline or the options. The hook re-derives the\n  options and prints only one it offers itself. The reply carries an option\n  id, never a decision.\n- D-HP6-5 V28c. \"From now on\" options (a suggestion saved to a settings\n  file) are dropped from hook asks, so they can be neither shown nor\n  answered. Session-scoped rules stay.\n- D-HP6-6 Timeout. 60 s, Claude Code's default for a hook. The deadline sits\n  2 s below it and the hook waits 59 s. Measured on Claude Code 2.1.288: the\n  pane draws its own prompt while the hook waits, so the wait costs the pane\n  nothing, and either place may answer.\n- D-HP6-7 Scope. Claude Code at level 2 only. Codex's hook decision output is\n  unmeasured, so its hook stays unarmed. The level-1 pane snapshot and typed\n  reply, the e2e, and the audit actions are in #1146. ACP and the Codex\n  app-server are HP-16 (#673).\n- D-HP6-9 (D-88n, dispatcher; review must-fix) The hook authenticates the\n  host before writing anything: the listener's uid must be its own and its pid\n  one of the hook's ancestors (`charter_same_user::admit_host`, new\n  `peer_process_of` and `ancestors`). No env/argv secret, since the chat reads\n  both. Otherwise it prints nothing and the pane decides. ADR 0068 gains the\n  HP-6 amendment: the hook socket's owner stays the chats' ancestor under\n  charterd. The sandbox's integrity denial stays as the second layer.\n  Ancestry is read from /proc on Linux and from /bin/ps by absolute path on\n  macOS, since the workspace denies `unsafe`.\n- D-HP6-10 (review must-fix) Allow only on a whole display. `hooked::ask`\n  drops every allowing option unless the summary is the action word for word\n  (`shown_in_full`): no newline collapsed, no cut at 200, no mask. An edit,\n  which shows only its path, is never allowed from the window. The row then\n  offers Deny and \"Open in its pane\". It is enforced in the core, and the hook\n  re-derives the same options, so an allow cannot be sent for such an ask.\n- D-HP6-11 (rounds 2 and 3, R2-M1/R3-M1; dispatcher tightening of D-88n)\n  The host is the ancestor that holds the socket. A peer pid is a stale\n  number on both kernels: Linux SO_PEERCRED keeps the pid of a listener that\n  exited while a child kept the socket (measured), and macOS LOCAL_PEERPID is a\n  sample. The hook admits the peer only if its pid is a same-uid ancestor AND\n  that process holds a socket bound at the hook path now, with paths compared\n  once resolved (/tmp == /private/tmp):\n  - Linux: a listening /proc/net/unix entry whose inode is in /proc/<pid>/fd.\n  - macOS: /usr/sbin/lsof -nP -a -p <pid> -U -F n, by absolute path with no\n    environment. Chosen over the `libproc` crate, which only moves the\n    `unsafe` into a new dependency.\n  No timestamps: round 2's birth-time guard was forgeable (a symlink,\n  setattrlist, a swap) and is removed, along with the self shortcut and\n  PEER_PID_IS_PINNED. Any error fails closed. ADR 0068's HP-6 amendment says\n  so.\n- D-HP6-12 (round 2, R2-M2) Allow also needs a plain shell input (only\n  command, description and timeout, so `dangerouslyDisableSandbox` and\n  `run_in_background` lose it) and no character drawn otherwise. That is a\n  control, general category Cf, or another Default_Ignorable_Code_Point. Cf\n  comes from the `unicode-properties` crate (MIT/Apache-2.0, unicode-rs,\n  general-category only), not a hand list. A suggestion whose own label (rule\n  text, directory path) holds such a character is not offered.\n- D-HP6-13 (D-88o, dispatcher) The window never offers a permission-mode\n  switch (any `setMode` suggestion). It is dropped in the core like V28c's\n  \"from now on\", so the hook cannot print one either.\n- D-HP6-14 One ancestry walker: `charter_same_user::{Parents, walk}`.\n  `charter_core::process::descends_from` uses it, under the core's fork lock.\n- D-HP6-15 `ask_permission_of_an_admitted_host` (the in-process test seam)\n  is built only under cfg(test) or the new `test-support` feature, which only\n  dev-dependencies turn on.\n- D-HP6-16 The permission row stacks its line above its options and wraps\n  anywhere, so a long token can't push the command off the menu.\n- D-HP6-8 No e2e. A scenario harness would need to run the permission hook.\n  The CLI test drives the real binary over a real socket, the app test\n  drives `Hooks` end to end, and vitest drives the list.\n\nCloses #672\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-04T05:39:50+04:00",
          "tree_id": "67fd4492d95c4568550a3148f9d82c0dfec92705",
          "url": "https://github.com/diazoxide/charter/commit/53f0e4e505caf8667f9e8078f55fa04444eb787f"
        },
        "date": 1791078071735,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.383393,
            "unit": "ms",
            "extra": "median of 5 runs: 0.379, 0.381, 0.383, 0.389, 0.396 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.4647565,
            "unit": "ms",
            "extra": "median of 5 runs: 16.358, 16.413, 16.465, 16.488, 16.539 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.213261,
            "unit": "ms",
            "extra": "median of 5 runs: 100.520, 100.617, 101.213, 101.249, 101.599 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "de0610e550a425ceccf93329302e8c59ce17be58",
          "message": "FM-9: files into chats, typed as each harness's reference and never sent\n\nA file, a folder, a range of lines or a search hit reaches a chat without anyone typing a path.\n\n- Neutral model, `charter_core::reference`. A reference is a path, an optional line range and\n  whether it is a folder. It is made only by `reference::of`, from a path that `files::place`\n  confined: inside the branch, no `..`, no link, not `.git`. Its text is built in core.\n- `HarnessAdapter::reference` renders it in each harness's own syntax:\n  - Claude Code: `@src/main.rs`, `@src/`, `@src/main.rs#L10-20`, and `@\"…\"` for a name with\n    white space.\n  - Codex: `src/main.rs:10-20`, with double quotes for white space.\n  - opencode: `@src/main.rs#10-20` and `@src/`.\n- Typing goes through the curation path (ADR 0061): one bracketed paste with nothing after it,\n  sent as the operator's input. A running chat is typed into only when it is at its prompt\n  (`reference::may_type_into`). Otherwise, and always on opencode, the reference goes on the\n  clipboard and the window says why.\n- Three ways in:\n  - drag a file or folder row, a search hit, or the preview's selection handle onto a chat's tab\n    or its pane;\n  - \"Ask a chat about this\" and \"Add to a chat's context\" in the preview;\n  - \"Start a chat here\" on every file and folder row (`start_chat_here`), which opens a chat on\n    the project's default profile in the branch's folder, with the reference held as its first\n    prompt.\n\nMeasured (D-FM9-1):\n- Claude Code 2.1.288: read from the mention parser in its own bundle. A mention is `@` at the\n  start or after white space, then `\"…\"` or non-space ending on a word character. `#L<a>` or\n  `#L<a>-<b>` after the name names lines.\n- codex-cli 0.147.0: drove its `@` file search in a pty, with a scratch CODEX_HOME and a\n  stand-in provider. It inserts the relative path, a folder without a trailing `/`, and quotes a\n  path with white space. It attaches nothing, so lines use its own `path:line` vocabulary.\n- opencode 1.18.33: drove its `@` completion in a pty with scratch XDG directories, and read its\n  bundle's completion code. It writes `@path`, `@dir/` and `@path#a-b`.\n\nDecided in implementation:\n- D-FM9-1: each harness's syntax, as measured above. A path that a harness's mention cannot\n  hold is handed over in plain words, which the agent reads with its own tools: a `#` for all\n  three, a `\"` that needs quoting for Claude Code, and white space for opencode. Rejected:\n  inventing an escape that no harness parses.\n- D-FM9-2: a reference is relative to the chat's own folder when the file is inside it, and\n  absolute otherwise, so it resolves for a chat on another branch too. Rejected: always\n  relative to the branch, which breaks for a chat elsewhere.\n- D-FM9-3, the idle check, is `reference::may_type_into`, decided from hooks and the kernel only.\n  A reference is typed only after a hook has said `Waiting`, the chat is not asking, and its\n  terminal is raw. It is never typed:\n  - mid-turn, while the chat asks something, or after it has ended;\n  - into a chat no hook has spoken for (`Unknown`, on any harness: a Codex chat with untrusted\n    hooks stays silent through an approval dialog that is raw and quiet like an input). This\n    tightens it, per review M2;\n  - into a chat whose curation or first-task prompt is still held;\n  - where the paste would be drawn as a placeholder.\n  Rejected: a quiet period (ADR 0061's Codex rule, kept for curation only), and reading the\n  screen.\n- D-FM9-4: a chat that is not at its prompt gets the reference on the clipboard with the reason,\n  not a refusal, so the gesture is never lost. Nothing is queued to type later, since late\n  typing lands in whatever the operator began.\n- D-FM9-5: what is typed is the reference and a trailing space inside one paste, so the\n  operator's words follow it. That holds for \"Start a chat here\" too: the held paste keeps its\n  space (`ChatTyped::then_a_space`, `Typed::hold_paste`). Control characters are stripped again\n  at the paste, and a path holding one (a newline is a legal file-name byte) is refused in core,\n  so nothing can submit. The paste markers and the bracketing are defined once, in\n  `charter_core::curation` (`PASTE_BEGINS`/`PASTE_ENDS`, `bracketed`,\n  `bracketed_then_a_space`), and curation, the first task and references all use them.\n- D-FM9-6: a reference goes through `Held::operator_input`. It is the operator's act, so it\n  cancels a smart close as typing does.\n- D-FM9-7: \"Start a chat here\" uses the project's default profile, as a curation chat does\n  (`curation::default_profile` was extracted for it). It runs as no persona, in the branch's\n  folder, labelled `About <name>`. On a harness charter cannot type into, the chat still opens\n  and the reference is copied (`curation::start_typed_where_it_can_be`).\n- D-FM9-8: \"Ask\" types and brings the chat to the front; \"Add\" types and stays. Both are typed,\n  unsent.\n- D-FM9-9: a drag carries charter's own MIME type with the project in it. A drop from another\n  project is refused in the window, and the core places the path again whatever the window\n  sent.\n- D-FM9-10: \"Start a chat here\" joins FILE_VERBS. It changes nothing in the branch, so V86 F8\n  holds.\n- D-FM9-11: an ignored file can be referenced, since `files::place` places it. Dragging it is an\n  explicit act, and the agent can read its folder anyway.\n- D-FM9-13 (review M1, security): no reference begins with a character a harness reads as a\n  mode or a command. Measured: Claude Code 2.1.288 enters bash mode on a pasted leading `!`, and\n  codex-cli 0.147.0 runs a submitted `!…`. `reference::of` writes a relative path that does not\n  begin with a letter, a digit, `_` or `.` as `./<path>`. An absolute path is quoted (or written\n  `.//…` when it holds a quote) wherever an adapter would hand it over bare, so it is never read\n  as a `/` command. `reference::starts_safely` is the rule; each adapter's test and the core test\n  hold every rendering to it.\n- D-FM9-14 (review F1): a name holding a format, bidirectional, zero-width or other\n  default-ignorable character, or U+2028/U+2029, is refused in `of` and stripped in `pasted`.\n  `reference::not_typeable` reuses HP-6's `hooked::drawn_otherwise` and adds the two\n  separators.\n- D-FM9-12: the e2e uses a fake harness with a new `--raw` mode, which echoes bytes and answers\n  only on Enter. It runs on a Claude-kind profile that reports SessionStart through the real\n  `charter hook`.\n\nLeftovers: #1151 (keyboard path for rows and hits, opencode typing, multi-file drags,\nStart-here with lines, local macOS e2e, and the accepted residual that the check before typing\nis not atomic with the harness).\n\nCloses #1112\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-04T06:32:20+04:00",
          "tree_id": "02209006e7edb8c56ff64fae7dcd76ccec4c0f5d",
          "url": "https://github.com/diazoxide/charter/commit/de0610e550a425ceccf93329302e8c59ce17be58"
        },
        "date": 1791081213648,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.612085,
            "unit": "ms",
            "extra": "median of 5 runs: 0.599, 0.604, 0.612, 0.616, 0.618 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.98643,
            "unit": "ms",
            "extra": "median of 5 runs: 16.652, 16.659, 16.986, 17.124, 17.389 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.1376075,
            "unit": "ms",
            "extra": "median of 5 runs: 104.246, 104.682, 105.138, 105.485, 107.709 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "ffba151e07b37cdf447cc954dcb5071e2f1a5066",
          "message": "FM-12: measure ⌘P, ⌘⇧F and status on 100,000- and 300,000-file repos\n\nAn ignored test, search_is_measured_on_large_repos.rs, generates a\nrealistic repo: nested packages, mixed kinds and sizes, about 6%\nbinaries, and ignored build output and dependencies. It times the core\ncalls the app makes:\n- ⌘P: a palette session's first find, and each keystroke;\n- ⌘⇧F: the first hit and a 200-line page, for common, rare and absent\n  queries, and a whole scan;\n- status: a read through the bounded reader child.\n\nIt holds the results to new budget rows G2–G4 in docs/spec.md:\n- a ⌘P keystroke within L1's 50 ms;\n- a first find within 3 s;\n- a common query's first hit within 1 s;\n- status within the reader's 30 s deadline.\n\nstress.yml's new `search at scale` job runs it at both sizes on\nmacOS and Ubuntu, as evidence on main and nightly only (V70).\n\nNumbers (macOS, loaded) are on #1115.\n- ⌘P meets L1 at both sizes: 25 ms at worst.\n- A common ⌘⇧F query's first hit takes 0.19 s at 100,000 files and\n  0.57 s at 300,000, warm.\n- A rare query, or one found nowhere, fills no 10 s page: the whole\n  scan takes 61–131 s, the speed this machine reads the files at all.\n\nNo index is built. That one case goes grill → ADR in M52 (#1153),\ntogether with a parallel walk, status at scale (about 4× git status\nat 300,000), and the Linux numbers, which the new job will post.\n\nRefs #1115\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-04T07:25:30+04:00",
          "tree_id": "cb3cafae6a9a6285036adb467894998602c79583",
          "url": "https://github.com/diazoxide/charter/commit/ffba151e07b37cdf447cc954dcb5071e2f1a5066"
        },
        "date": 1791084456691,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5901860000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.580, 0.587, 0.590, 0.597, 0.604 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.741025999999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.558, 16.711, 16.741, 17.198, 17.373 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.73469449999999,
            "unit": "ms",
            "extra": "median of 5 runs: 102.264, 103.300, 103.735, 105.377, 106.214 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "4d007619f81b4566d6279a34df454ed3b18dc4d5",
          "message": "sandbox: let a killed probe's child be reaped before reading its group\n\nThe hang test read the probe's process group once, right after killing it.\nOn Linux a killed child stays a zombie in the group until init reaps it, so\nthe read could still find it and fail (train 17, ubuntu). Poll for up to\nfive seconds; a child that really survived the kill still fails the test.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-04T08:25:57+04:00",
          "tree_id": "6d27fe6701186631d22dba53a5d988da8dbf2f61",
          "url": "https://github.com/diazoxide/charter/commit/4d007619f81b4566d6279a34df454ed3b18dc4d5"
        },
        "date": 1791088087003,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.589099,
            "unit": "ms",
            "extra": "median of 5 runs: 0.570, 0.576, 0.589, 0.596, 0.601 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.5034435,
            "unit": "ms",
            "extra": "median of 5 runs: 17.195, 17.432, 17.503, 17.664, 17.873 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.78711799999999,
            "unit": "ms",
            "extra": "median of 5 runs: 103.622, 105.619, 105.787, 106.020, 106.142 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "d56da346bd376a99860c4be9f081d2280e8ef4b2",
          "message": "browser install: read a sandbox refusal off the failure, write nothing\n\nThe hint that a sandboxed chat may not write a project's skills was\nfound by a write probe: it made the skills folder and a file in it, and\ncould follow a link or leave .claude/ behind, in the very run the\noperator starts outside any sandbox.\n\nThe probe is gone. The hint is now read off the failure alone: the\ngenerator's output, or why it could not run, saying permission denied,\noperation not permitted, EPERM or EACCES. Any other failure gets no\nhint, and nothing is written to find out.\n\nCloses #1057\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-04T09:45:28+04:00",
          "tree_id": "fa0d13c971c7c9b63af5c378be6ba788daf21ff9",
          "url": "https://github.com/diazoxide/charter/commit/d56da346bd376a99860c4be9f081d2280e8ef4b2"
        },
        "date": 1791092850449,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.575094,
            "unit": "ms",
            "extra": "median of 5 runs: 0.570, 0.571, 0.575, 0.576, 0.589 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.3248925,
            "unit": "ms",
            "extra": "median of 5 runs: 16.986, 17.218, 17.325, 17.484, 17.539 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 106.17664500000001,
            "unit": "ms",
            "extra": "median of 5 runs: 105.155, 105.258, 106.177, 106.300, 106.928 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "5de2ddaef221f35838614f4780bd1c13d945b77b",
          "message": "FG-4 re-review: catch the whole secrets context, pin the record's path\n\nFold-ins from the re-review of 8537772 (approved).\n\n- The guard test flags any use of the secrets context inside a `${{ }}` expression other than\n  exactly `secrets.FORGE_LIVE_TOKEN`: a bare `secrets` (`toJSON(secrets)` and the like sends the\n  whole context to the runner), another name, or an index. `toJSON(secrets)` in the test step's\n  env passed the guard before this commit and fails it now.\n- The ungated upload of what each forge tested is pinned to `path: ${{ runner.temp }}/outcome`,\n  so it can never upload the workspace. `path: .` passed before this commit and fails now.\n- docs/forges.md: create both environments and set their main-only deployment branch policy\n  before adding `FORGE_LIVE_TOKEN` (GitHub creates a missing environment unprotected the first\n  time a job names it), and add no required reviewers, which would hold every nightly run.\n\nRefs #712\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-04T10:36:05+04:00",
          "tree_id": "13a5bf2963ee2f31c91662ffe872e5428f58de96",
          "url": "https://github.com/diazoxide/charter/commit/5de2ddaef221f35838614f4780bd1c13d945b77b"
        },
        "date": 1791095907465,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.325017,
            "unit": "ms",
            "extra": "median of 5 runs: 0.318, 0.324, 0.325, 0.330, 0.331 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.581789,
            "unit": "ms",
            "extra": "median of 5 runs: 16.514, 16.529, 16.582, 16.585, 16.724 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.14287999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 100.482, 100.926, 101.143, 101.646, 102.024 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "65536fa8d6d99828c92d0809f9fba4c765e5b990",
          "message": "FD-10 review: a partial watch is no watch; #935's render count stays open\n\nMust-fix from the review of f660a58:\n\n- A watch registration the platform refused was dropped, and Held trusted\n  any running watch, so a store left unwatched served stale panels, and\n  the sidebar, for ever (the sidebar's hole dated from FD-10b).\n  watchset::follow now answers the wanted folders the platform would not\n  watch that are still there. The plane watch keeps them and says its\n  Standing: Partial, WholeAgain (once, after a gap) or Whole. Held serves\n  the model only when the watch is Whole. On Partial it reads fresh, and\n  on WholeAgain it rebuilds the model first. A root the platform will\n  not watch fails Watch::start. A test-only refusal list in watchset\n  plays the platform; the tests failed before the fix.\n- The held-plane watch test is robust on macOS: in this crate's tests\n  the plane watch runs on notify's poller (planewatch::Platform), as\n  planewatch's own tests do.\n\nFold-ins:\n- A workspace with a store that could not be read is read again on\n  every ask, not held.\n- The memory commands tell the model inside their blocking task. The\n  todo commands tell it whether or not the write succeeded, as the\n  persona commands do.\n- The property test also writes workspace.json and removes clones.\n\nDecided in review:\n- D-FD10j #935's added acceptance line (count PlaneView renders) is not\n  met for a write in the workspace in front, whose panels state lives in\n  PlaneView. This branch refers to #935 and #650 instead of closing them,\n  and the render-count half is tracked in #1160. Supersedes the closing\n  lines for #935 and #650 in f660a58: the train closes #934 and refers\n  to #935 and #650.\n- D-FD10k A folder that vanished between the listing and its watch is\n  not a failure (the next burst lists again); one still on disk that the\n  platform refused is.\n\nCloses #934\nRefs #935\nRefs #650\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-04T11:41:38+04:00",
          "tree_id": "18944feab7f68e5a4ba272cf3edb16b1d90f60f7",
          "url": "https://github.com/diazoxide/charter/commit/65536fa8d6d99828c92d0809f9fba4c765e5b990"
        },
        "date": 1791099803255,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5650584999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 0.553, 0.558, 0.565, 0.583, 0.585 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.1597315,
            "unit": "ms",
            "extra": "median of 5 runs: 16.555, 17.076, 17.160, 17.220, 17.516 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.064515,
            "unit": "ms",
            "extra": "median of 5 runs: 104.538, 104.891, 105.065, 106.747, 106.814 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "e92d89bed7eac847e5c309c0b42c1504c1299e16",
          "message": "FD-11 review: fail-closed coverage, no status takes index.lock\n\nFixes the review of 35bd5c0.\n\n- Every change of a repo's coverage, either way, has its standing read\n  again (`Shared::cover`). An answer read before a watch began is not kept\n  for the covered backstop.\n- D-FD11h (delegated, fail closed): only a watch of the whole tree\n  (`How::Whole`: FSEvents, ReadDirectoryChangesW) covers a standing.\n  - inotify clones stay on the uncovered clock.\n  - Folders that could not be listed are their own state (`How::Unread`),\n    never `vec![root]`.\n  - A burst from a rescan, a watcher error or a pathless event\n    (`Burst::lost`) uncovers every kept clone and re-watches its tree. Only\n    a watch that is made covers it again, and that re-cover re-reads the\n    standing.\n- No `git status` charter runs takes `index.lock`. The runner sets\n  `GIT_OPTIONAL_LOCKS=0` for every `status`, so a new reader can't forget\n  it. The briefing's status also passes `--no-optional-locks`.\n  `no_automatic_read_takes_the_index_lock` now also covers the briefing\n  and the status line.\n- An unborn branch's staged files count as tracked dirt in the shared\n  standing.\n- `Root::matters`: a file in the index matters even under an ignore\n  pattern.\n- branchwatch:\n  - A clone found after its plane was let go of is not kept.\n  - A clone the reader refuses is not asked for again for 60 s.\n  - The repeated listing is one `listened()` helper.\n  - The leftover block in `start` is gone.\n- Docs: a covered idle monorepo costs about one read in twenty minutes,\n  not \"no status\".\n\nTests (each seen red against the code it guards):\n- cover on both edges;\n- a lost watch re-watched and re-covered, and a refused re-watch staying\n  uncovered;\n- folder-by-folder and unread clones never covered;\n- the refusal backoff;\n- the let-go race;\n- the runner's status leaving a stat-stale index alone, with a control;\n- the unborn branch;\n- a tracked file under an ignore pattern.\n\nFollow-ups are on #874: inotify coverage, and the reader-child spawn cost\nduring builds.\n\nCloses #651\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-04T12:32:34+04:00",
          "tree_id": "f8046fbb5719e35f0cb329b9ef1e4e7809ba501b",
          "url": "https://github.com/diazoxide/charter/commit/e92d89bed7eac847e5c309c0b42c1504c1299e16"
        },
        "date": 1791102893785,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.33716650000000004,
            "unit": "ms",
            "extra": "median of 5 runs: 0.327, 0.331, 0.337, 0.337, 0.344 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.6712695,
            "unit": "ms",
            "extra": "median of 5 runs: 16.589, 16.641, 16.671, 16.708, 16.714 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.3700155,
            "unit": "ms",
            "extra": "median of 5 runs: 100.854, 101.215, 101.370, 101.920, 102.154 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "d15c62961c544de087cc9b1ce74189a05e954e23",
          "message": "mutants: install the sandbox's programs and fetch the whole lock\n\nThe nightly's baseline (charter-core's own tests) went red on main e92d89b\nwith six failures that are its runner, not the code: the sandboxed-start\ntests refuse without bubblewrap and socat, which ci.yml installs and\nmutants.yml did not, and the gitoxide guard reads `cargo metadata\n--offline` over the whole workspace, which a `--package charter-core`\nbuild never downloads. The shards had the same gap, where it is worse:\nevery mutant those tests cover read as caught. Both jobs now install the\ntwo programs and run `cargo fetch --locked`.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-04T16:04:06+04:00",
          "tree_id": "119164f88e2573d7c24715bb6507e9d7a9e0b091",
          "url": "https://github.com/diazoxide/charter/commit/d15c62961c544de087cc9b1ce74189a05e954e23"
        },
        "date": 1791115520256,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5799735,
            "unit": "ms",
            "extra": "median of 5 runs: 0.577, 0.579, 0.580, 0.580, 0.589 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.9627695,
            "unit": "ms",
            "extra": "median of 5 runs: 16.395, 16.729, 16.963, 17.227, 17.243 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.44758,
            "unit": "ms",
            "extra": "median of 5 runs: 103.376, 103.789, 105.448, 105.873, 106.176 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "d58ad21b8751b43a8ee679ed75d7b023933415ae",
          "message": "keyhold: the Keychain's not-the-owner code is macOS's alone\n\n`NOT_THE_OWNER` is read only by the macOS `owned_by_another`, so on Linux it\nwas dead code and `-D warnings` stopped every Linux job (train 24, #1181).\nIt is now gated to macOS like the function that reads it.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-04T19:33:20+04:00",
          "tree_id": "c42841d2993b1e9f56de94c8e5be3cb48112ef61",
          "url": "https://github.com/diazoxide/charter/commit/d58ad21b8751b43a8ee679ed75d7b023933415ae"
        },
        "date": 1791128132905,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.425822,
            "unit": "ms",
            "extra": "median of 5 runs: 0.418, 0.419, 0.426, 0.429, 0.430 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.518321999999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.462, 16.518, 16.518, 16.598, 16.628 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.4335515,
            "unit": "ms",
            "extra": "median of 5 runs: 100.840, 101.417, 101.434, 101.554, 102.059 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "3be8db4afd7b14e6971ce777ab88fee263b3f361",
          "message": "Bump taiki-e/install-action\n\nBumps the actions group with 1 update in the / directory: [taiki-e/install-action](https://github.com/taiki-e/install-action).\n\n\nUpdates `taiki-e/install-action` from 2.87.21 to 2.87.22\n- [Release notes](https://github.com/taiki-e/install-action/releases)\n- [Changelog](https://github.com/taiki-e/install-action/blob/main/CHANGELOG.md)\n- [Commits](https://github.com/taiki-e/install-action/compare/4cef1412cce204788f482e778a0b9187f9626a29...83ac0ad63c0167e6f06796fab0fce28db1bf3db0)\n\n---\nupdated-dependencies:\n- dependency-name: taiki-e/install-action\n  dependency-version: 2.87.22\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: actions\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-10-04T20:26:16+04:00",
          "tree_id": "0d3b5bb0fdca12fe228ec3d4a2ea010eb5044008",
          "url": "https://github.com/diazoxide/charter/commit/3be8db4afd7b14e6971ce777ab88fee263b3f361"
        },
        "date": 1791131262365,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.3134925,
            "unit": "ms",
            "extra": "median of 5 runs: 0.311, 0.313, 0.313, 0.323, 0.328 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.571075999999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.529, 16.561, 16.571, 16.577, 16.590 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.35528249999999,
            "unit": "ms",
            "extra": "median of 5 runs: 101.135, 101.137, 101.355, 101.535, 101.736 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "9bbadf17cae2520195a2230531bc2ddc7731b3a5",
          "message": "Bump the rust group across 1 directory with 2 updates\n\nBumps the rust group with 2 updates in the / directory: [imagesize](https://github.com/Roughsketch/imagesize) and [nix](https://github.com/nix-rust/nix).\n\n\nUpdates `imagesize` from 0.14.0 to 0.15.0\n- [Release notes](https://github.com/Roughsketch/imagesize/releases)\n- [Commits](https://github.com/Roughsketch/imagesize/compare/v0.14.0...v0.15.0)\n\nUpdates `nix` from 0.28.0 to 0.31.3\n- [Changelog](https://github.com/nix-rust/nix/blob/master/CHANGELOG.md)\n- [Commits](https://github.com/nix-rust/nix/compare/v0.28.0...v0.31.3)\n\n---\nupdated-dependencies:\n- dependency-name: imagesize\n  dependency-version: 0.15.0\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: rust\n- dependency-name: nix\n  dependency-version: 0.31.3\n  dependency-type: direct:production\n  update-type: version-update:semver-minor\n  dependency-group: rust\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-10-04T20:26:57+04:00",
          "tree_id": "552e2080e0cdf7e54bc3998b06f2d76b109598bb",
          "url": "https://github.com/diazoxide/charter/commit/9bbadf17cae2520195a2230531bc2ddc7731b3a5"
        },
        "date": 1791131966431,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5877129999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 0.582, 0.584, 0.588, 0.597, 0.598 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.073181499999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.687, 17.071, 17.073, 17.340, 17.499 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.548031,
            "unit": "ms",
            "extra": "median of 5 runs: 103.113, 104.062, 104.548, 104.559, 106.733 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "d63df068a20ecf9892dd39257e05887afa39c0ab",
          "message": "stress: search at scale reports its budgets in CI instead of failing\n\n`search at scale` on macOS failed on every main run since FM-12 added it\n(ten runs on 2026-10-04), and passed on every Ubuntu run with the same\ncode. Run 37216804748, job 111483533718, 100,000 files:\n- ⌘P first find 8082 ms (budget 3 s); the same find in a second session\n  of the same run took 434 ms;\n- ⌘P keystroke worst 54.4 ms (budget 50 ms; median 6.8 ms);\n- status 11.8-14.5 s (budget 30 s, not missed).\n\nDiagnosis. The runner, not charter's code:\n- writing the repo took 146 s on macOS against 29 s on Ubuntu (718 s\n  against 96 s at 300,000), and the whole-repo scan 14 s against 4 s\n  (119 s against 9 s). Neither touches the reader child;\n- the reader child is spawned once per status, never per search; ⌘P and\n  ⌘⇧F run in process. perf-1 measured the child at 6-8 ms;\n- locally (M4 Pro, load 6-9) the same 100,000-file repo reads its status\n  in 0.7-1.0 s, with gix's compare trusting all 100,000 stats: 0 racily\n  clean, 0 to update, 0 files read.\nOne real cost class was confirmed locally. A stat that stops matching\nthe index (here every file's ctime changed by an xattr) makes the reader\nhash all 826 MB on every read, because it never writes the index:\n2.3-3.2 s per status. It is not proven to be the runner's cause. The run\nnow prints gix's counters and `git status`'s time next to the reader's,\nso the next main run says which one it was. The follow-up goes on #1153.\n\nWhat changes (ADR 0086, amended 2026-10-04):\n- `CHARTER_MEASURE_BUDGETS=report` prints each miss as\n  `FM-12 | budget missed | ...` and passes. By hand, unset, the budgets\n  still fail the run on the operator's machine;\n- stress.yml runs it that way, turns each miss into a warning, writes\n  every FM-12 line to the job summary and uploads the log. A wrong\n  answer or a panic still fails the job;\n- docs/spec.md G2-G4: release absolute by hand, evidence only in CI.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-04T23:30:46+04:00",
          "tree_id": "928f696fad931e6e304f624d9dcf3a7ca60d1485",
          "url": "https://github.com/diazoxide/charter/commit/d63df068a20ecf9892dd39257e05887afa39c0ab"
        },
        "date": 1791142397566,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5607205,
            "unit": "ms",
            "extra": "median of 5 runs: 0.540, 0.553, 0.561, 0.561, 0.563 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.996339,
            "unit": "ms",
            "extra": "median of 5 runs: 16.270, 16.762, 16.996, 17.022, 17.141 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.8087725,
            "unit": "ms",
            "extra": "median of 5 runs: 103.415, 103.652, 103.809, 104.497, 105.174 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "d908b3ca32f318f349f6305d9eaca93704f46f1b",
          "message": "stress: a run that met every budget doesn't fail its own summary\n\nTrain 25's summary step greps the log for missed budgets and pipes the\nresult into a loop. On a run that met every budget, grep finds nothing and\nexits 1, and under the step's pipefail that failed the step, so every\nsearch-at-scale job went red on main d63df06 (all ubuntu ones included).\nThe grep now tolerates finding nothing.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T00:30:17+04:00",
          "tree_id": "41d87aca13a53a0cd22c61f87db8ffee7e1852a8",
          "url": "https://github.com/diazoxide/charter/commit/d908b3ca32f318f349f6305d9eaca93704f46f1b"
        },
        "date": 1791145924446,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4230125,
            "unit": "ms",
            "extra": "median of 5 runs: 0.412, 0.413, 0.423, 0.425, 0.436 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.666341,
            "unit": "ms",
            "extra": "median of 5 runs: 16.387, 16.590, 16.666, 16.673, 16.792 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.75712200000001,
            "unit": "ms",
            "extra": "median of 5 runs: 101.314, 101.554, 101.757, 101.927, 102.121 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "a37080d39c8a6ae7e2ffea3f322a57aa8a5585e2",
          "message": "DS-3d: Rename workspace, Start chat, First run and Updates on the settings set\n\nThe four remaining hand-built forms draw their form parts from the settings\nset (SettingGroup, SettingRow, Field, Choice) instead of the dialogs'\nasks/choices/choice classes. Button rows stay on their old classes for DS-3e\n(#1177).\n\n- Rename workspace: the New name box is a row; the dialog finds the box to\n  focus and select on opening, since a Field takes no ref.\n- Start chat: harness and persona are radio Choices (a harness's ProfileMeta\n  is its option's line), the footer, branch and sandbox boxes are toggles,\n  Name and the sandbox reason are text Fields, and the sandbox block sits in\n  a \"Sandbox\" SettingGroup in every state.\n- First run: the path is a Field row (placeholder an example of the answer)\n  with Open below it; the template a radio Choice disabled while opening.\n- Updates: the channel is a radio Choice marking the current one; the pick\n  is held and written only by \"Use this channel\" (busy-guarded). Success\n  says the new channel and returns focus to it; a refusal shows the channel\n  it is still on.\n- e2e/opening.ts finds the harness rows through the radiogroup labelled\n  \"Harness\" instead of the gone `pick-harness` id.\n- docs/ui-primitives.md: the arrow-pick repair lives in Choice, and the rule\n  for radios that write with no Undo or start something.\n\nDecided in implementation:\n- D-DS3d1 (accepted globally): Choice's radio picks the option a single\n  arrow key moves to. Radix's arrow flag comes from a document listener that\n  hears the key after the roving focus has set its move going, so only a\n  held key picked; StartChat had worked around it with onFocus per row.\n  Choice hears the arrow in the capture phase, picks on the focus that\n  follows, and forgets it on key-up or when focus leaves the group. Guarded\n  by components.test.tsx. Rejected: an opt-in prop, a hand-built group in\n  StartChat. Visible effect: Settings > Your editor follows one arrow.\n- D-DS3d2: Updates' channel pick is held and written by \"Use this channel\",\n  shown only while the pick differs from the channel, forgotten on close.\n  Rejected: writing on pick (the radio trap), an always-shown disabled\n  button. The existing \"puts the machine on the channel the operator picks\"\n  test now presses it.\n- D-DS3d3, D-DS3d4: duplicates of DS-3c's toggle tabIndex={0} and text\n  input autoComplete=\"off\"; DS-3c's side was taken on rebase.\n- D-DS3d5: Option.says widens from string to ReactNode, so a harness's\n  kind, command and source keep their own marks; ProfileMeta's id becomes\n  optional.\n- D-DS3d6: Choice's radio gains Radix's own `disabled` (First run holds the\n  template while opening; Updates holds the channel while writing).\n- D-DS3d7: moot after rebase onto DS-3c: Rename's help keeps\n  <code>workspaces/</code>, First run keeps its /path/to/repo placeholder.\n  The checkbox labels keep their exact wording, which tests name.\n\nLeftover for DS-3e: Start chat's refused list still names a profile with\nthe `who` class, and `.choices-name` now has no user.\n\nCloses #1176\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T00:34:46+04:00",
          "tree_id": "b5e9e0805631ddc46b481b9efe203d5a176b730c",
          "url": "https://github.com/diazoxide/charter/commit/a37080d39c8a6ae7e2ffea3f322a57aa8a5585e2"
        },
        "date": 1791146674585,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4282525,
            "unit": "ms",
            "extra": "median of 5 runs: 0.409, 0.419, 0.428, 0.428, 0.431 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.574596999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.409, 16.475, 16.575, 16.615, 16.740 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.78090599999999,
            "unit": "ms",
            "extra": "median of 5 runs: 100.751, 101.756, 101.781, 101.916, 102.326 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "0cefd0236a739fc65dd572a6f4f1a1eda0f9e397",
          "message": "search at scale: a status the reader cannot finish is a budget missed\n\nOn the macOS runner at 300,000 files, a status runs past the reader child's\nown deadline, and the evidence job (main d908b3c) panicked on\n`.expect(\"a status\")` instead of saying so. A read that comes back with no\nanswer is now pushed as a missed budget, \"status read: no answer (…)\", and\nthe measurement ends there: in report mode it is a warning, by hand it still\nfails the run (ADR 0086 as amended in train 25).\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T01:14:14+04:00",
          "tree_id": "b20e0b80d2e0a1ee4a63c867a53b55adcc89d4a6",
          "url": "https://github.com/diazoxide/charter/commit/0cefd0236a739fc65dd572a6f4f1a1eda0f9e397"
        },
        "date": 1791148536530,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5961495,
            "unit": "ms",
            "extra": "median of 5 runs: 0.584, 0.592, 0.596, 0.599, 0.621 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.757609,
            "unit": "ms",
            "extra": "median of 5 runs: 16.590, 16.614, 16.758, 17.305, 17.424 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.236635,
            "unit": "ms",
            "extra": "median of 5 runs: 103.286, 103.601, 104.237, 104.803, 106.278 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "8a4cf9719a15fe72fea40edaa76f2df68d1406b8",
          "message": "ADR 0090: agents work together as chats that are listed, observable and stoppable, and no agent's word is consent (AC-1)\n\nProposed, for the operator's acceptance. Restates \"visibility is the\ncontrol\" as Q11 and allows headless chats once AC-12 lists them.\n\n- One primitive, dispatch, in two modes: handoff (as today, at most\n  one report) and task (AC-2, over the MCP Tasks extension).\n- Every act between chats rides the chat scope; no new client scope and\n  no new power; answer stays a human scope's (V16, V75).\n- A dispatch needs a person's yes: charter's own ask per dispatch, or a\n  dispatch grant made on a human scope (coordinator, workflows,\n  triggered chats).\n- A closed list of what a chat may ask of another: dispatch, report,\n  message along its lineage or over a person-made message link, stop\n  its own subtree, leases and claim links through the host.\n- Approvals never travel along a lineage; a dispatcher never answers\n  its task's input request; AC-11's answerer must exclude the asking\n  chat's lineage.\n- Caps on depth, live chats per lineage and message rate; audit\n  actions for every act, with no brief or message text in an entry.\n\nAmends ADR 0066 (handed_from covers both modes) and ADR 0076 (a\ndispatcher cause). CONTEXT.md gains Dispatch, Handoff, Task, Report,\nLineage, Headless chat, Peer message, Mailbox, Dispatch grant and\nMessage link.\n\nRefs #713\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T01:25:48+04:00",
          "tree_id": "f72505c4b0b3e7e9827be9e10f523892a08517bd",
          "url": "https://github.com/diazoxide/charter/commit/8a4cf9719a15fe72fea40edaa76f2df68d1406b8"
        },
        "date": 1791149336425,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.269614,
            "unit": "ms",
            "extra": "median of 5 runs: 0.254, 0.266, 0.270, 0.287, 0.287 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.425747,
            "unit": "ms",
            "extra": "median of 5 runs: 16.390, 16.410, 16.426, 16.432, 16.458 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.486448,
            "unit": "ms",
            "extra": "median of 5 runs: 101.042, 101.326, 101.486, 101.718, 101.989 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "08fb7c1276b471e709462ba4262f841cfc93afca",
          "message": "SE-21 review: sr-only count, refusals stay under a no-match filter\n\n- The live count uses Tailwind's sr-only, and the hand-written hidden rule is gone.\n- A filter that matches nothing says so above the level's standing refusals instead of in\n  their place; a Project-level test holds a refusal on screen under such a filter.\n- The arrow-key test filters by a word that narrows both groups to one setting each, and\n  says so correctly.\n\nCloses #1171\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T01:47:20+04:00",
          "tree_id": "38ed3871209d22c781c051aaf60d3d0872b821df",
          "url": "https://github.com/diazoxide/charter/commit/08fb7c1276b471e709462ba4262f841cfc93afca"
        },
        "date": 1791150513981,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5147295000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.500, 0.504, 0.515, 0.518, 0.528 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.966683,
            "unit": "ms",
            "extra": "median of 5 runs: 16.483, 16.599, 16.967, 17.086, 17.485 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.262693,
            "unit": "ms",
            "extra": "median of 5 runs: 103.357, 103.905, 104.263, 106.796, 107.851 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "3c4f46aee0f3b388744cbbfa75615b886e6150a2",
          "message": "FM-11 review: a theme state, the no-line sentences, honest comments, your editor\n\n- The `piece-diff` view has a state in the theme test (`theme/views.test.tsx`): a text diff\n  whose drawn line is checked, so every view in `OWN_MARKS` is drawn in every theme.\n- A change with no changed line says what changed, by its mark: a pure rename \"Only its name\n  changed: moved from X\", a new empty file \"X was added, and it is empty\", a deleted empty one\n  \"X was deleted, and it was empty\", and only a changed file keeps the mode sentence.\n- The comparison tab has \"Open in your editor\" (the light editor's, at the change's first\n  line), and the binary and too-large sentences point to it.\n- The comments in `piecefiles.rs` and on `files::what_changed` no longer say no git process\n  starts: confining the path finds the branch's folder in the app process (`git worktree list`\n  or `git rev-parse`). Moving that into the reader, announcing the tab's states and naming the\n  merge view are on #1189.\n\nCloses #1114\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T04:05:23+04:00",
          "tree_id": "e6968cb56696b48bbce9e46321abaddd740b8e7b",
          "url": "https://github.com/diazoxide/charter/commit/3c4f46aee0f3b388744cbbfa75615b886e6150a2"
        },
        "date": 1791158830005,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.465265,
            "unit": "ms",
            "extra": "median of 5 runs: 0.435, 0.461, 0.465, 0.485, 0.488 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.429012,
            "unit": "ms",
            "extra": "median of 5 runs: 16.335, 16.362, 16.429, 16.506, 16.576 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.942263,
            "unit": "ms",
            "extra": "median of 5 runs: 100.617, 101.529, 101.942, 102.052, 102.207 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "152e2dda66582b76f612ee03e0d4f47413f4af84",
          "message": "updates: the weekly count's review fold-ins (OB-17)\n\n- Guard: `weekly_endpoint` is a way of naming Charter's address in\n  only_the_updater_and_report_name_charters_own_address. A temporary caller in clipath.rs\n  failed the guard with the new entry and passed it without. The guard also stopped\n  scanning a file at a mid-file `#[cfg(test)] mod name;` declaration, so a caller after\n  line 138 of charter-core's lib.rs went unseen. It now stops only at an inline test module.\n- offer_from gives the week back when the weekly request was never sent, and reads the\n  manifest. A test covers this.\n- docs/updating.md:\n  - the macOS opt-out route (`launchctl setenv DO_NOT_TRACK 1`, or start from a shell) and\n    #1185;\n  - that GitHub can join one IP's weekly fetch to its usual fetches, and that the fallback\n    makes two requests back to back;\n  - two more biases: anyone can inflate the count, and a request counted and then failed\n    is counted again;\n  - the fixed headers named: Host, Accept, Accept-Encoding and the user agent;\n  - ragged lines rewrapped.\n- \"Nothing is published\" now reads \"charter publishes no estimate; the raw count is public,\n  as every asset's is\", in the ADR and in updating.md. main.rs's module comment is rewrapped.\n\nCloses #688\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T04:40:56+04:00",
          "tree_id": "320d88b65d48a48822e4797eab1fece4937a3800",
          "url": "https://github.com/diazoxide/charter/commit/152e2dda66582b76f612ee03e0d4f47413f4af84"
        },
        "date": 1791161024485,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.441404,
            "unit": "ms",
            "extra": "median of 5 runs: 0.411, 0.436, 0.441, 0.446, 0.446 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.337739,
            "unit": "ms",
            "extra": "median of 5 runs: 16.076, 16.173, 16.338, 16.400, 16.791 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.3306105,
            "unit": "ms",
            "extra": "median of 5 runs: 101.007, 101.093, 101.331, 101.633, 101.685 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "e6f44ed5e10f62fdb556131047567048a7aeed51",
          "message": "KN-4 review: a verb on the archive's button, focus after Restore\n\n- The archive's catalogue row, and the heading button that draws it as a\n  glyph, now reads as a verb: \"Open alpha's archive\", \"Open steward's\n  archive\", \"Open the shared archive\". The tab it opens keeps its name\n  (`archiveTitle`).\n- After a Restore, focus moves to the status line that says what it did\n  (tabIndex -1), so a keyboard user is not left on a button that has gone.\n  A test asserts document.activeElement.\n- Hardening follow-up filed as #1194 (D-90c: fail closed) and added to #1191.\n\nRefs #716\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T04:46:13+04:00",
          "tree_id": "8ac77cfc76762471c11cb2b0ddac55d14ab34320",
          "url": "https://github.com/diazoxide/charter/commit/e6f44ed5e10f62fdb556131047567048a7aeed51"
        },
        "date": 1791161882708,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.46993150000000006,
            "unit": "ms",
            "extra": "median of 5 runs: 0.455, 0.463, 0.470, 0.472, 0.491 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.3554645,
            "unit": "ms",
            "extra": "median of 5 runs: 16.165, 16.307, 16.355, 16.399, 16.562 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.0739225,
            "unit": "ms",
            "extra": "median of 5 runs: 100.580, 101.041, 102.074, 102.101, 102.231 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "7692ba246cb842293e14e8ce0c656b6d7c8ec9dc",
          "message": "KN-3 fold-ins: workspace move-memory in two comments, rewrapped lines\n\n- memscope.rs and the core test's header name `charter workspace move-memory`,\n  not `charter workspace move`.\n- Rewrapped to the usual width: memory_move.rs's first doc line, the doc\n  comment above `move_memory`, the changes fragment, and the plane-format\n  line ending \"and the window's Move\".\n\nRefs #715\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T05:36:36+04:00",
          "tree_id": "c0fc57270d0db47859cfc8a3095015ab7103f769",
          "url": "https://github.com/diazoxide/charter/commit/7692ba246cb842293e14e8ce0c656b6d7c8ec9dc"
        },
        "date": 1791164337665,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4598055,
            "unit": "ms",
            "extra": "median of 5 runs: 0.450, 0.457, 0.460, 0.460, 0.469 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.293289,
            "unit": "ms",
            "extra": "median of 5 runs: 16.241, 16.248, 16.293, 16.452, 16.479 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.63660949999999,
            "unit": "ms",
            "extra": "median of 5 runs: 100.433, 101.388, 101.637, 101.749, 103.041 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "86abd28970f2115e97f44f2b5365e7de431f4533",
          "message": "HY-7 review: hangs are judged per shard; trust_gates is tested, not excluded\n\nThe review found two must-fixes and two fold-ins.\n\n- A TIMEOUT is now a hang only when its own shard's slowest whole suite fits in two thirds\n  of the timeout. A shard with no MISSED mutant has no measurement, so its TIMEOUTs stay\n  survivors. The first version took the largest suite time over all shards. Replaying\n  2026-09-25 (run 36115292771), that rule made 373 suites cut short on slow runners into\n  \"hangs\" and took survivors from 481 to 128, which could have closed #480 on a red night.\n  The per-shard rule gives 480 survivors and 1 hang there. 2026-10-02 and 10-03 are\n  unchanged: 7 and 13 hangs.\n- `trust_gates([claude, claude])` must name \"hooks\" once. That test kills the `&&`→`||`\n  mutant, so `.cargo/mutants.toml` no longer excludes it as equivalent, and the site no\n  longer claims it is.\n- The notice job's checkout sets `persist-credentials: false`.\n- AGENTS.md and the script say \"the same shard\", and that the per-shard rule is what keeps\n  a night like 2026-09-25 red.\n\nAmended:\n- D-HY7c: a TIMEOUT is a hang only when `--timeout` is at least 1.5x the slowest whole\n  suite on its own shard. With no MISSED mutant on that shard, it stays red. Rejected: one\n  suite time for the whole run, because shards run on different runners.\n- D-HY7g: equivalent mutants are excluded with the proof at their site, for `probe` `-`,\n  `default_branch` `&&` and `git_dir_of` `||`. `trust_gates` is a plain function, so a\n  test can give it a gate twice, and its `&&` is killed rather than excluded.\n\nRefs #480\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T06:30:20+04:00",
          "tree_id": "04385952b8624de51f2a20bd18a13486e960f758",
          "url": "https://github.com/diazoxide/charter/commit/86abd28970f2115e97f44f2b5365e7de431f4533"
        },
        "date": 1791167536878,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4634115,
            "unit": "ms",
            "extra": "median of 5 runs: 0.448, 0.459, 0.463, 0.473, 0.486 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.271413,
            "unit": "ms",
            "extra": "median of 5 runs: 16.230, 16.238, 16.271, 16.362, 16.418 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.47392450000001,
            "unit": "ms",
            "extra": "median of 5 runs: 100.764, 100.931, 101.474, 101.858, 101.874 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "89ee5972ba7549349fce4382f87ef4acb5c5f07d",
          "message": "FW-6a review: per-board iterations, archived items left out, every page checked\n\nFold-ins from the FW-6a review:\n\n- F1: `Iteration.title` is now an `Option<String>`, and `of_forge` and `lasting` take one,\n  because a GitLab cadence's iterations have no title.\n- F2: the `WorkItem` query asks `projectItems(includeArchived: false)`, since GitHub's default is\n  to include them. It also reads `isArchived` and drops an archived item it is given anyway, so an\n  archived board item is never a placement.\n- F3: `fieldValues(first: 50)` reads its `pageInfo`. A second page of board fields is an error,\n  as D-FW6a-5 says for every other connection.\n- F4: `Placement` keeps its own `iteration`. The item-level `iteration` is the first board's.\n- F5: the `Placement` doc says its status is per board, and that FW-6b (#734) adds an item-level\n  status for GitLab, Jira and Linear.\n- F6: GitLab's `read` refuses an answer with no title as malformed, as GitHub does; `create`\n  still falls back to the title it sent. A GitHub `read` of a pull request's number says\n  \"#n is a pull request\". The overflow test now covers sub-issues, closing pull requests, boards\n  and board fields.\n- F7: ADR 0070 §2's `Capability` sketch lists `CloseReasons`, with a note that the enum grows by\n  ticket.\n\nAmends D-FW6a-3: each board keeps its own iteration in `Placement::iteration`, and the\nitem-level `iteration` is the first board's in GitHub's order. FW-9 no longer has to choose an\niteration across boards: it reads the board it groups by.\n\nRefs #733\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T06:30:29+04:00",
          "tree_id": "c912cef8cf1622c3c8418fe4fe179088584db729",
          "url": "https://github.com/diazoxide/charter/commit/89ee5972ba7549349fce4382f87ef4acb5c5f07d"
        },
        "date": 1791168319871,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.473178,
            "unit": "ms",
            "extra": "median of 5 runs: 0.465, 0.467, 0.473, 0.474, 0.482 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.7056295,
            "unit": "ms",
            "extra": "median of 5 runs: 16.442, 16.482, 16.706, 16.715, 16.912 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.560234,
            "unit": "ms",
            "extra": "median of 5 runs: 100.850, 101.499, 101.560, 102.030, 102.610 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "dc620c4046d1ac5a10de9053665525eaa7c17f43",
          "message": "stress: a comparison the capped reader cannot answer is a budget miss, not a panic\n\nThe 300,000-file evidence-only run (37252093589) panicked on the uncommitted\ncomparison's `.expect` when the capped reader stopped without an answer. ADR 0086,\nas amended, says an evidence-only run reports each budget it missed and never\nfails for one.\n\nA new `Held::answered` turns only \"no answer from the capped reader\" (its\ndeadline, its memory cap, its child failing: every refusal that says \"charter\ncould not read the branch\") into a named miss. Any other refusal is still a\nfailure, and so is a wrong answer. `measure_compare` uses it for the file list,\none file's hunks and the uncommitted comparison. It prints and holds each part as\nsoon as that part is measured, so a later miss doesn't drop earlier timings.\n`measure_status` uses it too, and keeps its clean timings when the six-change\nreads miss. The find, search and scan steps don't go through the capped reader.\n\nRefs #1209\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T07:17:27+04:00",
          "tree_id": "bdf3d52a9bb2ffb5efded39a0f5968203d661539",
          "url": "https://github.com/diazoxide/charter/commit/dc620c4046d1ac5a10de9053665525eaa7c17f43"
        },
        "date": 1791170441603,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4552035,
            "unit": "ms",
            "extra": "median of 5 runs: 0.435, 0.442, 0.455, 0.469, 0.477 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.650356000000002,
            "unit": "ms",
            "extra": "median of 5 runs: 16.232, 16.277, 16.650, 16.702, 16.821 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.254574,
            "unit": "ms",
            "extra": "median of 5 runs: 101.212, 101.277, 102.255, 102.486, 103.160 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "2a02e598adfb3101d158a6e950501f425034a750",
          "message": "HY-8 review fold-ins: every #464 item accounted for, permission tests restore their mode\n\nRe-measuring the item-1 functions the first commit did not name turned\nup survivors that an earlier pass had wrongly read as caught (a broken\nselection in the local helper). They are fixed here.\n\nNew tests (tests/a_pr_mode_save_keeps_one_pr_open_from_the_save_branch.rs\nunless named):\n- still_holds `&&`→`||` ×2: a_block_clears_once_the_plane_is_moved_off_\n  the_commit_it_is_about\n- known_pr_url → None / delete `!`: the known PR stays on the standing\n  through a failed save (in a_pr_whose_state_the_forge_cannot_give_…)\n- knows_a_pr → true: a_clean_request_mode_plane_with_no_pr_known_has_\n  nothing_to_save_or_ask\n- tip_is_in_head → false: a_save_branch_tip_this_clone_pushed_but_lost_\n  the_note_of_is_pushed_over\n- describe `>`→`<`: a_request_carrying_more_saves_than_it_lists_says_\n  how_many_earlier_ones_it_left_out\n- push's kept-branch `!=`→`==` and `delete field branch`: a_save_branch_\n  changed_by_hand_is_pushed_and_given_its_own_pr\n- settle's merged-commit `&&`→`||`: a_merge_the_forge_names_at_a_commit_\n  not_on_the_target_blocks_the_plane\n- plane::place delete `!` (CHARTER_ROOT): a `place` assertion added to\n  HY-7's a_pinned_plane_is_the_one_acted_on_and_an_empty_home_is_no_home\n  in the steering test (one CHARTER_ROOT child, not two)\n- push_save_branch's stale-lease `&&`→`||`: a_push_the_remote_refuses_for_\n  its_own_reason_asks_it_nothing_more and a_push_the_remote_refuses_keeps_\n  the_remotes_reason_when_the_remote_then_goes_away (planegit/prsave/\n  tests.rs, from the reviewer's proof tests). Under the mutant a refused\n  push runs an extra ls-remote, and when that fails the remote's own reason\n  is replaced by \"could not read from remote\".\n- push_save_branch's gone-branch case: the `!expected.is_empty()` guard is\n  gone (D-HY8g, amended), so its mutant no longer exists. The arm that\n  replaced it, `None => Some(String::new())`, is pinned by a_save_branch_\n  deleted_on_the_remote_while_a_first_push_was_on_its_way_is_pushed_afresh:\n  the first push's absent lease is refused, the listing finds the branch\n  gone (the listing's own upload-pack deletes it first), and the second\n  push lands. That test asserts 2 pushes, 1 ls-remote and Ok, and it fails\n  under `delete match arm None` and both `delete !` mutants.\n\nRecorded as equivalent in .cargo/mutants.toml, with the proof at the site:\n- describe's `[one] if total == 1` → true: one subject of the newest 50\n  is a range of one, and `total` falls back to the subjects' count.\n\nItem-1 functions re-measured and caught, with the test that catches them:\n- Claim::within: a_claim_asked_for_within_a_bound_is_had_once_its_holder_\n  lets_go_inside_it. Its `+`→`*` and `Some(Default)` do not compile.\n- planegit summary: a_message_naming_every_group_it_has_counts_none_as_more\n- signer_said: the_signers_words_are_kept_and_gits_progress_hints_and_\n  bare_prefixes_are_not\n- Stage::word: a_stage_is_named_by_the_word_the_window_reads\n- unpushed: a_commit_left_unpushed_is_pushed_by_the_next_save_even_with_\n  nothing_new_to_commit. Its two let-chain `||` mutants do not compile.\n- still_holds → true/false, `!=`: every_pr_mode_save_is_run_against_a_\n  stand_in_forge, and alerts::tests::a_memory_commit_never_pushed_is_red_\n  and_one_awaiting_a_pull_request_is_not\n- tip_is_in_head → true, describe → constants, `==`, `>=`: the PR-mode suite\n- reposave summary: a_generated_message_lists_the_files_and_counts_the_rest\n  and a_generated_message_naming_exactly_as_many_files_as_it_shows_counts_\n  none_as_more\n- nobody_working (its non-empty mutants): autosave::tests::quitting_commits_\n  what_a_repo_with_auto_save_on_had_changed. `vec![]` was already excluded\n  as equivalent.\n- Standing::worth_saving: the autosave::tests quit and quiet-period tests,\n  and a_repo_is_worth_saving_for_changed_files_a_block_or_an_unpushed_\n  commit_and_never_when_off\n- Standing::fingerprint: the_fingerprint_moves_with_the_head_the_changed_\n  count_and_the_unpushed_count_only\n\nWhat the remaining TIMEOUTs came to:\n- plane resolve → Ok(Default): caught, by doctor::tests::a_plane_without_\n  the_report_rule_is_flagged_and_fix_adds_it.\n- plane place delete `!`: a survivor only a steered child can see, now\n  caught (above).\n- personaverbs/stats hh_mm_ss_ff `+=`→`*=` names four sites:\n  - three are caught: recorded::an_hour_alone_is_two_digits_as_\n    fromisoformat_requires, recorded::an_offset_of_hours_minutes_or_\n    seconds_moves_the_instant_and_not_the_printed, and tests::a_time_is_\n    refused_exactly_where_fromisoformat_refuses_it;\n  - the fourth, the digit-skip loop after six fraction digits, was a\n    GENUINE HANG: `p *= 1` never advanced, and only a test's timeout could\n    catch it (50 minutes locally before I stopped it). The loop is now one\n    count of the remaining digits, which reads the same input the same way;\n    its `+=` mutants fail tests::a_time_is_refused_exactly_where_\n    fromisoformat_refuses_it instead of hanging, so no exclusion is needed.\n- workspaces Workspace::manifest_text NotFound → true: a survivor, now\n  caught (first commit).\n\nThe flaky index test #464's comment named\n(reading_where_unsaved_work_sits_never_writes_the_index_a_save_needs) was\nsettled by #527 (1f864a0), which made reading a plane never write its\nindex.\n\nThe two permission tests restore their directory's mode through a drop\nguard, so a failing assertion cannot leave a mode-000/555 directory behind,\nand return early as root (rustix geteuid), which ignores the mode:\n- doctor git_auth_names_the_workspaces_folder_itself_when_it_cannot_be_listed\n- a_merged_pr_the_plane_cannot_move_onto_yet_is_said_in_the_saves_journal,\n  now `#[cfg(unix)]`\n\nDecided in implementation:\n- D-HY8a: Base the work on the last full nightly that finished its shards\n  (run 36308723145, 2026-09-27) plus local runs. The 2026-10-04 nightly\n  tested nothing (red baseline, fixed in d15c629), and the diff nightlies\n  do not cover these files.\n- D-HY8b: A mutant counts as caught when a test related to it fails under\n  it, since that implies the full suite fails. A run whose only failure is\n  an unrelated test does not count as caught (that is what misled the\n  earlier pass). A mutant that survives the targeted tests is re-run\n  against the lib tests and the relevant integration tests before a test\n  is written.\n- D-HY8c: Write behaviour tests rather than equivalence entries for every\n  live survivor. The prsave field deletions are pinned through\n  plane-push.json and the journal, which doctor and the next save read.\n  Exclusions are only for proven equivalents, with the proof at the site.\n- D-HY8d: Keep #489's charter-core steering integration test for the\n  cli-only mutants. Rejected: adding charter-cli as a mutants test package\n  (it runs the CLI suite for every mutant); and recording them as known\n  (that leaves real gaps).\n- D-HY8e: Settle's waiting path is reached with a read-only directory\n  blocking the move, because `reset --keep` overwrites ignored files. This\n  test and doctor's unreadable-workspaces test are unix-only and skip\n  themselves as root. Both restore the mode on drop.\n- D-HY8f: Survivors next to #464's lists but outside them go to #480\n  (HY-7) as one checklist comment, not a new issue.\n- D-HY8g (dispatcher), amended after review: a save branch that is gone\n  from the remote when charter lists it, after a push with nothing expected\n  was refused as stale, IS pushed again with an absent lease, as every gone\n  branch is (the contract on push_save_branch). The first ruling (block\n  instead) was reversed for these reasons:\n  - that path runs only when this clone never pushed the branch, so it has\n    no request of its own, and settle answers Nothing; the next save would\n    recreate the branch anyway;\n  - recording Blocked stops auto-save over a race that has passed;\n  - it contradicted the documented contract.\n  The block message added for it is removed.\n- D-HY8h: the hh_mm_ss_ff hang is removed at its source (a count instead of\n  a step loop) rather than excluded. An exclusion names all four `+=`\n  sites, three of them caught by tests.\n\nCloses #464\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T07:57:17+04:00",
          "tree_id": "9bef8dec55f7c05140fbe4b525c9e6953af45ba2",
          "url": "https://github.com/diazoxide/charter/commit/2a02e598adfb3101d158a6e950501f425034a750"
        },
        "date": 1791172842940,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.530035,
            "unit": "ms",
            "extra": "median of 5 runs: 0.509, 0.519, 0.530, 0.540, 0.551 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.558281,
            "unit": "ms",
            "extra": "median of 5 runs: 16.529, 16.546, 16.558, 16.686, 16.737 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.62584799999999,
            "unit": "ms",
            "extra": "median of 5 runs: 102.598, 102.861, 103.626, 104.104, 105.994 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "e28d6aa9041758c1b0ec5be5afd27e3cdba919ea",
          "message": "mutants: an unreadable alert record holds the whole crate (HY-7b review)\n\nReview fixes to 9cc724c.\n\nMust-fix: `notice` read a malformed record of dirty files as clean. With a clean slice\nover a.rs of {a.rs, b.rs}, the records {}, \"xyz\", [1] and [\"*\"] each produced `close`,\nand null and 5 crashed with a TypeError. `recorded` now accepts only two shapes: a JSON\nlist of path strings, none of which is \"*\", or exactly the string \"*\". Anything else,\nincluding a decode error, counts as the whole crate. A hand-written [] still means no\nfile is left. New tests cover each malformed record (plus \"[\" and a list holding a\nnumber); each one must produce `edit`, never `close`, and never crash.\n\nD-HY7b-6, amended: `notice` runs on `schedule` only, as in HY-7, so a dispatched run,\nfull or not, never touches the alert. Only a full cycle of clean Sunday slices shuts it.\nFor the record-less alert that is open today, that means 13 weeks of clean slices; a\ndispatched full run does not count. The trigger is unchanged. The wording is corrected\nin the workflow's header and `notice` job comments, in AGENTS.md, in the issue body text\nand in mutants-report.py's docstrings.\n\nFold-in: a missing space in `add_parser(\"notice\", help=...)`.\n\nChecks: node --test tools/*.test.mjs (66/66), actionlint mutants.yml (clean).\n\nRefs #1200\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T08:41:42+04:00",
          "tree_id": "c3f38247541ccf13001003e058a57a9bff3b9a11",
          "url": "https://github.com/diazoxide/charter/commit/e28d6aa9041758c1b0ec5be5afd27e3cdba919ea"
        },
        "date": 1791175456923,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.422231,
            "unit": "ms",
            "extra": "median of 5 runs: 0.393, 0.421, 0.422, 0.431, 0.451 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.398059500000002,
            "unit": "ms",
            "extra": "median of 5 runs: 16.296, 16.315, 16.398, 16.546, 16.905 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.69546,
            "unit": "ms",
            "extra": "median of 5 runs: 101.368, 101.582, 101.695, 101.865, 103.198 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "74f2399a8a44a21ec5577842de06973306bab015",
          "message": "FW-6b fold-ins: every recording names a case, overrides are checked\n\nThe two optional points from the FW-6b review, in the contract harness.\n\nA. `every_recording_on_both_forges_names_a_case` walks each forge's\nrecordings and requires every file's stem, with or without the\n`.self_managed` suffix, to name a case in CASES. A self-managed override\nleft behind by a renamed case would otherwise go unread while the\nself-managed run fell back to the main recording. The override loader\n(`own_recording`) now tells \"not there\" from a read that failed: only\nNotFound means no override; any other error fails the case.\n\nB. The network-log check also reads every `{case}.self_managed.json`, so a\nREST path that only an override asks is checked for scene names too.\nGitLab's expected count goes from 22 to 24 (the read override's issue and\nboards calls).\n\nRefs #734\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T08:42:30+04:00",
          "tree_id": "8c87a34dd20ab4c7b854cd5b83a9dbcf4a3be2e2",
          "url": "https://github.com/diazoxide/charter/commit/74f2399a8a44a21ec5577842de06973306bab015"
        },
        "date": 1791176232550,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5357555,
            "unit": "ms",
            "extra": "median of 5 runs: 0.506, 0.525, 0.536, 0.551, 0.570 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.905222,
            "unit": "ms",
            "extra": "median of 5 runs: 16.468, 16.650, 16.905, 16.974, 17.154 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.10152099999999,
            "unit": "ms",
            "extra": "median of 5 runs: 102.869, 105.020, 105.102, 105.238, 106.275 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "340c80a910098059b5264b211afabb8d36fbe036",
          "message": "Docs site: http-cache-semantics 4.2.0 -> 4.3.0; sharp alerts are the stand-in\n\nhttp-cache-semantics (Dependabot alert 12, GHSA-ch52-4w7c-c8xp, range <= 4.2.0):\n- 4.3.0 was published 2026-10-04, after the advisory. astro@7.3.5 asks for\n  ^4.2.0, so this is a lockfile-only bump: the one entry changes, nothing else.\n- 4.3.0 does not change max-stale handling; its diff is Vary matching (wildcard,\n  inherited properties) and a status() accessor. The upstream maintainer calls\n  the report bogus (kornelski/http-cache-semantics issue 56: RFC 9111 7.3 lets a\n  shared cache reuse a Set-Cookie response; Cache-Control: private is the guard),\n  and github/advisory-database issue 10139 disputes it. The alert should still\n  go away, since 4.3.0 is outside the vulnerable range.\n- Reachability, either way: the only user is astro/dist/assets/build/remote.js,\n  which builds a CachePolicy for a remote image astro fetches itself during\n  `astro build`, to decide how long to keep it in the local build cache. Its\n  requests carry no max-stale, the cache serves no other user, and the site has\n  no remote images (astro.config.mjs uses passthroughImageService, no\n  remotePatterns or domains). The advisory's path is not reachable here.\n\nsharp (alerts 7, 8, 9, 10): no change, and none is possible by a bump. The site\ninstalls no sharp: package.json maps sharp to the local stand-in no-sharp\n(MIT, 0.0.0-not-installed) and overrides every sharp to it, because sharp's\nprebuilt libvips is LGPL-3.0. astro already asks for sharp ^0.35.4 as an\noptional dependency, so a real sharp would be a patched one, but bringing it\nback would bring the LGPL binary with it. osv-scanner.toml already skips sharp\nfor the same reason, and test/no-sharp.test.mjs pins the two together.\nDependabot reads the stand-in as a vulnerable sharp. These four alerts are\nfalse positives, to dismiss as \"vulnerable code is not used\" with that reason.\n\nChecks in site/: npm ci, npm test (27 pass), npm run build (79 pages, links\nvalid). node ../tools/npm-licences.mjs from app/ passes; it covers\napp/package-lock.json only, and the licence here is unchanged (BSD-2-Clause).\n\nRefs #1217\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T08:58:51+04:00",
          "tree_id": "4313eb917f128f4c1f60ca39c2aa1ee3015280bf",
          "url": "https://github.com/diazoxide/charter/commit/340c80a910098059b5264b211afabb8d36fbe036"
        },
        "date": 1791177090676,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.38640399999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 0.359, 0.363, 0.386, 0.392, 0.418 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.6901535,
            "unit": "ms",
            "extra": "median of 5 runs: 16.670, 16.681, 16.690, 16.703, 16.793 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.7695425,
            "unit": "ms",
            "extra": "median of 5 runs: 100.964, 101.732, 101.770, 102.491, 102.732 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "ac9ba5ced5cfcbb8851301e4196d742f9cde2541",
          "message": "SE-23 e2e: press the gear the driver cannot show, and leave the window as found\n\nsettings-gears.e2e.ts went red on both scenario jobs on its first run\n(run 37260967190), and every spec after it with it.\n\n- The gear: a WebDriver pointer move does not make WebKit match\n  `:hover`, a WebDriver Tab is synthesised in the page and moves no\n  focus, and whether a scripted focus matches `:focus-visible` depends\n  on what earlier specs did with the pointer. The gear is pressed from\n  the page; its quiet stays SettingsGears.test.tsx's to hold.\n- Sign commits: found by its `<label for>`, which wdio's `aria/`\n  selector does not resolve, and chosen by value plus a `change`,\n  because `selectByAttribute` leaves the value unset in this driver.\n- The leak: `close_plane` lets the core go of a project but leaves its\n  tab on the strip, in front, with its Settings tab, and one app\n  process serves the whole run. The after-hook now closes the project\n  from its own tab's close button, as saving.e2e.ts does, and puts the\n  copy's charter.toml back.\n\nRefs #1173\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T11:24:07+04:00",
          "tree_id": "97af29d756c7745416bb6fbf0219d7066477775f",
          "url": "https://github.com/diazoxide/charter/commit/ac9ba5ced5cfcbb8851301e4196d742f9cde2541"
        },
        "date": 1791185176542,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.445917,
            "unit": "ms",
            "extra": "median of 5 runs: 0.434, 0.444, 0.446, 0.453, 0.456 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.5211865,
            "unit": "ms",
            "extra": "median of 5 runs: 16.357, 16.481, 16.521, 16.605, 16.652 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.318011,
            "unit": "ms",
            "extra": "median of 5 runs: 101.042, 101.194, 101.318, 101.999, 102.512 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "ea9c202023f6dd65074ef3e5c9e4908dc49b210a",
          "message": "secret shapes: recognise more token formats\n\nEvery check built on `secret_kind` now also asks the token-prefix rule\nthat `found` and `token_kind` already used, after the existing rules, so\neach kind those rules named stays the one named. The prefix list gains\nthe remaining documented GitLab token prefixes and Slack's app-level and\nworkflow token prefixes, each cited from its provider's documentation.\n\nTests build made-up token bodies at runtime and cover every documented\nformat plus names that only start like a token.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T13:15:07+04:00",
          "tree_id": "cea96b623a4caba9075bf5abfcc5346e003787e0",
          "url": "https://github.com/diazoxide/charter/commit/ea9c202023f6dd65074ef3e5c9e4908dc49b210a"
        },
        "date": 1791191840410,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.6108515000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.595, 0.600, 0.611, 0.616, 0.617 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.138594,
            "unit": "ms",
            "extra": "median of 5 runs: 15.880, 16.068, 16.139, 16.226, 16.337 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.578577,
            "unit": "ms",
            "extra": "median of 5 runs: 100.599, 101.286, 101.579, 102.227, 103.890 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "18291f234aa8a0e79c2e50cd30a6da55695aa94d",
          "message": "train 44: settings-forges e2e finds the Forges button inside the nav\n\nWebdriverIO's `button=Forges` text selector cannot be joined to a CSS\nselector, so `nav[aria-label=\"Groups\"] button=Forges` threw \"is not a\nvalid selector\" on both scenario runners. The spec now finds the nav,\nthen the button inside it, as settings-gears does.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T13:41:32+04:00",
          "tree_id": "1d561608fb6e37844a5fd806253846daa8244805",
          "url": "https://github.com/diazoxide/charter/commit/18291f234aa8a0e79c2e50cd30a6da55695aa94d"
        },
        "date": 1791193483636,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.491734,
            "unit": "ms",
            "extra": "median of 5 runs: 0.480, 0.491, 0.492, 0.501, 0.526 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.797514,
            "unit": "ms",
            "extra": "median of 5 runs: 16.280, 16.286, 16.798, 16.975, 17.095 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.35280800000001,
            "unit": "ms",
            "extra": "median of 5 runs: 103.336, 103.826, 104.353, 104.627, 105.698 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "dc37c4d0ff80e00998358505e5664f5157343ed1",
          "message": "train 45: the updater tests' manifest server counts a request before it answers\n\n`the_real_check_lists_its_read_in_the_network_log` failed twice in a row on this train's CI\nwith 0 served, expected 1, and `a_release_without_a_weekly_manifest_still_answers_the_check`\nfailed the same way on the first run. Both pass on their own. The stub server bumped its count\nonly after writing the reply, so a client that had read the reply could assert before the\ncount moved. It now counts each request before it writes the reply. Test code only; refs\nissue 1222.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T18:09:16+04:00",
          "tree_id": "98d5eded6e53b80fa3bbacc46a2c8e771c9df6f1",
          "url": "https://github.com/purlis/purlis/commit/dc37c4d0ff80e00998358505e5664f5157343ed1"
        },
        "date": 1791209635192,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.410767,
            "unit": "ms",
            "extra": "median of 5 runs: 0.386, 0.398, 0.411, 0.421, 0.425 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.4801145,
            "unit": "ms",
            "extra": "median of 5 runs: 16.184, 16.435, 16.480, 16.602, 16.866 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.824378,
            "unit": "ms",
            "extra": "median of 5 runs: 100.878, 101.220, 101.824, 101.891, 102.641 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "79d4fd6ad30b7f2d86456176e1e5b1fc19a4e7b2",
          "message": "rename train 1: pin the keychain and 1Password old names RN-4 moved\n\nRN-1's review round pinned identity::SERVICE_BASE and onepassword::TAG\nto their entries' old names. RN-4 moved both to the purlis spelling, so\nin the train they are pinned to `write`, and the old names each entry\nstill reads are pinned through the constants that read them:\nkeyring::OWN_PREFIXES[1], identity::READ_BASES[1] and onepassword's\nOLD_TAG. The pin count matches the old names held again.\n\nRefs #1254\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T19:04:59+04:00",
          "tree_id": "24354a48dc7dae5836f9b5fd5e786f9833b47d4e",
          "url": "https://github.com/purlis/purlis/commit/79d4fd6ad30b7f2d86456176e1e5b1fc19a4e7b2"
        },
        "date": 1791212862223,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.47235150000000004,
            "unit": "ms",
            "extra": "median of 5 runs: 0.469, 0.470, 0.472, 0.487, 0.493 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.347838,
            "unit": "ms",
            "extra": "median of 5 runs: 16.205, 16.332, 16.348, 16.393, 16.408 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.487864,
            "unit": "ms",
            "extra": "median of 5 runs: 100.552, 100.905, 101.488, 101.562, 101.838 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "76c36affc058bc70df299afda43cc188475da39c",
          "message": "rename train 2: the gauge e2e expects hooks and the statusline through purlis\n\nRN-3 makes the app run its hooks through the purlis binary beside it, so the bundled plugin's hook binary and the armed statusLine command are target/debug/purlis, not the charter alias.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-05T22:22:35+04:00",
          "tree_id": "37bacf0f4d21c743af33fcb0d7d09d220f8783ae",
          "url": "https://github.com/purlis/purlis/commit/76c36affc058bc70df299afda43cc188475da39c"
        },
        "date": 1791224660997,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5167824999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 0.488, 0.508, 0.517, 0.524, 0.531 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.9201925,
            "unit": "ms",
            "extra": "median of 5 runs: 16.458, 16.763, 16.920, 17.222, 17.480 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.00195550000001,
            "unit": "ms",
            "extra": "median of 5 runs: 102.795, 103.146, 105.002, 105.835, 107.490 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "d32a38be70791e7522daa5377dfd0fe3500a24fa",
          "message": "Bump the web group in /app with 3 updates\n\nBumps the web group in /app with 3 updates: [@types/node](https://github.com/DefinitelyTyped/DefinitelyTyped/tree/HEAD/types/node), [globals](https://github.com/sindresorhus/globals) and [vite](https://github.com/vitejs/vite/tree/HEAD/packages/vite).\n\n\nUpdates `@types/node` from 22.20.4 to 22.20.5\n- [Release notes](https://github.com/DefinitelyTyped/DefinitelyTyped/releases)\n- [Commits](https://github.com/DefinitelyTyped/DefinitelyTyped/commits/HEAD/types/node)\n\nUpdates `globals` from 17.12.0 to 17.13.0\n- [Release notes](https://github.com/sindresorhus/globals/releases)\n- [Commits](https://github.com/sindresorhus/globals/compare/v17.12.0...v17.13.0)\n\nUpdates `vite` from 8.3.1 to 8.3.2\n- [Release notes](https://github.com/vitejs/vite/releases)\n- [Changelog](https://github.com/vitejs/vite/blob/main/packages/vite/CHANGELOG.md)\n- [Commits](https://github.com/vitejs/vite/commits/v8.3.2/packages/vite)\n\n---\nupdated-dependencies:\n- dependency-name: \"@types/node\"\n  dependency-version: 22.20.5\n  dependency-type: direct:development\n  update-type: version-update:semver-patch\n  dependency-group: web\n- dependency-name: globals\n  dependency-version: 17.13.0\n  dependency-type: direct:development\n  update-type: version-update:semver-minor\n  dependency-group: web\n- dependency-name: vite\n  dependency-version: 8.3.2\n  dependency-type: direct:development\n  update-type: version-update:semver-patch\n  dependency-group: web\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-10-05T22:49:35+04:00",
          "tree_id": "1f4ff64121679ad74df52f359e692bfeed6f8a0e",
          "url": "https://github.com/purlis/purlis/commit/d32a38be70791e7522daa5377dfd0fe3500a24fa"
        },
        "date": 1791226269116,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.46841900000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 0.449, 0.463, 0.468, 0.473, 0.476 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.3165385,
            "unit": "ms",
            "extra": "median of 5 runs: 16.142, 16.273, 16.317, 16.779, 16.960 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.196714,
            "unit": "ms",
            "extra": "median of 5 runs: 101.090, 101.095, 101.197, 101.802, 101.938 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "697dc80cbcbc0d0daee72d3cb112c9eb54f85b5c",
          "message": "rename-plane review fold-in: the consent guard asks the session's folder too (RN-7)\n\nThe host loads its settings for the folder the session started in, and a\n`cd` in the shell does not move that. `settings::twin_in_force` used to\nanchor its layer lookup on the payload's `cwd` alone. It now takes the\ndirectories the host's settings may come from: `$CLAUDE_PROJECT_DIR`, when\nthe hook is given it, and the call's `cwd`. It requires the purlis twin in\nthe layer at or above each of them, as well as in the project's settings\nand opencode's file. With neither directory present it keeps refusing the\npurlis spelling.\n\n`toolgate::Plane` carries `session_dir`, which the CLI guard reads from\n`CLAUDE_PROJECT_DIR`. A new test covers a session started in a workspace\nwhose layer lacks the twin, with the call's cwd at the project root: it is\nrefused, and so is a call with no folder at all.\n\nRefs #1265\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-06T01:32:03+04:00",
          "tree_id": "ee9e6a35b6ce0fd226962ddd03f66016301299b5",
          "url": "https://github.com/purlis/purlis/commit/697dc80cbcbc0d0daee72d3cb112c9eb54f85b5c"
        },
        "date": 1791235999473,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.371544,
            "unit": "ms",
            "extra": "median of 5 runs: 0.353, 0.355, 0.372, 0.375, 0.384 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.474576,
            "unit": "ms",
            "extra": "median of 5 runs: 16.434, 16.459, 16.475, 16.491, 16.551 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.060102,
            "unit": "ms",
            "extra": "median of 5 runs: 101.351, 101.689, 102.060, 102.121, 102.620 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "18e02c208a7bc0b60ddb6b3050f8f74467976683",
          "message": "rename train 5: the old-name guard allows rename-local's refusal prose\n\nrenamelocal/busy.rs spells the old name once, in QUIT_FIRST (\"quit every\ncharter/purlis window, chat and terminal first\"). That text is what the\nrefusal tells the operator, so it is counted as visible prose (1) and\nwaits for RN-11a with the rest of the prose entries.\n\nRefs #1263\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-06T02:08:21+04:00",
          "tree_id": "97fad8b560deafd3fa6d0aba082b0226b4b000d2",
          "url": "https://github.com/purlis/purlis/commit/18e02c208a7bc0b60ddb6b3050f8f74467976683"
        },
        "date": 1791238180620,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.42861399999999994,
            "unit": "ms",
            "extra": "median of 5 runs: 0.397, 0.426, 0.429, 0.434, 0.455 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.292042000000002,
            "unit": "ms",
            "extra": "median of 5 runs: 16.245, 16.253, 16.292, 16.330, 16.392 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.4131735,
            "unit": "ms",
            "extra": "median of 5 runs: 101.284, 101.346, 101.413, 101.854, 103.443 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "b0bbea88b94684d149114f4ad63dd529181816fe",
          "message": "ST-4: A page per harness profile: Add, Remove, and rename when unused\n\nEach [harness.<name>] profile in the local settings file is an entry of a\nnew Settings collection, charter_core::settings::harness_profiles, with\nlisted/add/remove/rename on the ST-3 collection write seam (drawn base,\nopaque ids, exact Undo of a remove, inverse op for an add or a rename).\n\n- Field refusals come from the loader's own rules: profiles::refusals_of\n  and launch_refusals now return every broken rule with the profile key it\n  is about; the loader still takes the first, in the same words.\n- A value shaped like a secret in the name, the kind, the command or a new\n  name is refused under that field by its kind, pointing to the vault, and\n  is never said back (V91m).\n- Remove and rename are refused while a [harness] default in either file\n  names the profile and nothing else would answer to the name; each user is\n  named with a link to Harness & profiles (V91g, V91k).\n- A rename is exact: the profile keeps its place among [harness]'s keys\n  (an inline profile too) and its name keeps its quoting, so the rename\n  back gives the text it started from.\n- Writes go to whichever local settings file the project has (RN-2a).\n- The window: one nav page per profile, set in under Harness & profiles,\n  with its rows, Rename and Remove; Add (name, kind, command) and an Open\n  per profile on Harness & profiles. A Remove or an Undo on a page lands\n  on Harness & profiles. New profile... in the default pickers opens the\n  Add form and picks the new profile.\n- The approval before a profile's first run is unchanged: an added or\n  renamed profile asks, as any profile nothing approved does.\n\nDecided in implementation:\n- D-ST4-1: the core module is settings::harness_profiles, not\n  settings::profiles, since settings already imports crate::profiles.\n- D-ST4-2: one rule set. The loader's rules answer every failure with its\n  field; the collection adds only what a writer can break (a name the file\n  already has, a secret). Rejected: a second copy of the rules.\n- D-ST4-3: the users of a profile are [harness] default in either file,\n  only when the name would then name nothing (a local table shadowing a\n  built-in can go). Chat records are not users: a reopened chat whose\n  profile is gone is skipped by name (ADR 0022).\n- D-ST4-4: Add asks for name, kind and command; the environment is set on\n  the profile's page. No profile field holds a secret, so there is no vault\n  picker; a secret-shaped value is refused under its field, by kind.\n- D-ST4-5: a page per profile (project.profile.<name>, `sub` in the nav)\n  holds that entry alone with Rename and Remove, no Add; its writes are\n  kept under project.harness (Collection.home), so a Remove or an Undo\n  there lands where the Undo is said. Row labels stay \"<name>: kind\" so\n  the filter finds them by the profile's name.\n- D-ST4-6: rename is an EntryOp; its Undo is the rename back through the\n  core, with its reference check. The wire answers the renamed id in\n  EntryWritten.added rather than a new field.\n- D-ST4-7: a renamed profile asks for approval again (the record is keyed\n  by name). Fail-closed; no record is moved.\n- D-ST4-8: New profile... opens the Add form and picks what it added,\n  superseding D-ST1-4's Edit as TOML (kept only when the local file is not\n  TOML). A new table goes after the last profile, after a blank line.\n\nCloses #1236\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-06T02:18:37+04:00",
          "tree_id": "9755b1716a8fc566c4b83e125acd898850cd4812",
          "url": "https://github.com/purlis/purlis/commit/b0bbea88b94684d149114f4ad63dd529181816fe"
        },
        "date": 1791239432804,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4519425,
            "unit": "ms",
            "extra": "median of 5 runs: 0.446, 0.451, 0.452, 0.454, 0.468 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.319095500000003,
            "unit": "ms",
            "extra": "median of 5 runs: 16.263, 16.264, 16.319, 16.467, 16.948 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.399489,
            "unit": "ms",
            "extra": "median of 5 runs: 100.807, 100.971, 101.399, 101.788, 102.516 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "51996375944338391ef5d1c219dfe34f12f0937d",
          "message": "Rename the rest of the tracker text: comments, PRs, labels, repo descriptions\n\ntools/tracker-rename-rest.mjs renames, from charter to purlis, what\ntools/tracker-rename.mjs (RN-12) leaves: issue and PR comments, PR review\ncomments, PR reviews' own text, PR titles and bodies, label names and\ndescriptions, and the\nrepos' descriptions. It imports RN-12's renameText, so the rules and the\nkeep-rules are RN-12's own (D-RN12-1..9), and it works the same way: a\nread-only dry run by default that prints a diff per item, counts by kind\nand by rule, and accounts for every occurrence; and `--apply --from\n<plan>`, which re-reads everything and refuses the whole run if any\nplanned item changed or is gone, re-reads each item just before its\nwrite, skips what already reads as done (so a rerun resumes), PATCHes\nonly the changed fields, waits --delay-ms (7.5s) between writes and\nhonours Retry-After.\n\nDecided in implementation:\n- D-RN12b-1 / D-RN12b-9: in the shared rules, every HTML comment stays\n  byte for byte, for every kind, RN-12's issues and milestones included.\n  That covers <!-- charter-save --> (what forge::pr::is_ours reads), the\n  BEGIN/END charter change markers that `change push` finds, the\n  <!-- mutants-report dirty: [...] --> record tools/mutants-report.py\n  parses (issue 480), and quoted fixtures (issue 712).\n- D-RN12b-2: labels the code finds by name keep their names\n  (CODE_LABELS: bug, enhancement, type:adr from the issue templates,\n  dependencies and github_actions for Dependabot, ws:* and\n  charter::ws::* from `ws todo promote`); their descriptions are\n  renamed. None of them says charter today. No code or workflow in\n  either repo reads via-charter-report or gap, so both are renamed.\n  --keep-label adds one.\n- D-RN12b-3: a repo description names the moved repos by their new\n  slugs (diazoxide/charter becomes purlis/purlis), as #1274 asks. The\n  old slug stays as history everywhere else.\n- D-RN12b-4: in the shared rules, `-p charter-app` / `--package\n  charter-app` is the app crate and becomes purlis-app (RN-13, V93c),\n  not the repo's name. This also changes RN-12's own plan for a\n  handful of issues, so its dry run should be run again before apply.\n- D-RN12b-5: comments on M60 issues and PRs in M60 stay as written,\n  as RN-12 leaves M60's issues.\n- D-RN12b-6: in the shared rules, a line where two different old names\n  would read the same afterwards (\"called charter, not charter-app\" as\n  \"called purlis, not purlis\") stays as written, counted as `collapse`\n  and listed in both dry runs for a person to word by hand.\n- D-RN12b-7: in the shared rules, old-plugin-id also keeps `charter-app`\n  said as a plugin's name (\"the plugin is called ..., not charter-app\",\n  \"the bundled `charter-app` plugin\"), short of a sentence's end.\n- D-RN12b-10: the collapse check leaves `cli` edits out (a command\n  names no product), and \"the Python charter\" / \"the old charter\" are\n  kept as retired-python, so the rest of such a line is still renamed.\n- D-RN12b-11: RN-12 rule misses, now kept: the persona's charter as\n  \"the charter body/prose/concatenation\", \"charter-format\" and \"the\n  charter (or personas/<name>/...)\"; `op-item: charter-<persona>`\n  (1Password titles still say charter, issue 1275); and the GitLab label\n  `charter::ws::<name>` the code still writes.\n- D-RN12b-8: a PR review's own text is covered (GET/PUT\n  pulls/{n}/reviews/{id}). One paginated GraphQL read finds the PRs\n  that have reviews, and only those are listed through the paginated\n  reviews endpoint.\n\nTested in node:test against a stand-in gh, per item kind. The real\napply waits for the operator's approval of the dry run.\n\nRefs #1274\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-06T02:45:48+04:00",
          "tree_id": "c0bad4d2fe962e2c49864673f0e11955a0c8e15e",
          "url": "https://github.com/purlis/purlis/commit/51996375944338391ef5d1c219dfe34f12f0937d"
        },
        "date": 1791240422743,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.496493,
            "unit": "ms",
            "extra": "median of 5 runs: 0.469, 0.492, 0.496, 0.510, 0.537 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.103396500000002,
            "unit": "ms",
            "extra": "median of 5 runs: 16.511, 16.809, 17.103, 17.198, 17.222 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.227304,
            "unit": "ms",
            "extra": "median of 5 runs: 103.225, 103.849, 104.227, 104.372, 104.723 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "272fd673a632c304772e6c95d3822f7de5f3fc9e",
          "message": "train 47: the status line's alert e2e expects the drawer's Reinit button\n\nNO-6 draws an Alerts drawer row as a Notice whose button is its way out,\nso the workspace-behind-the-layout row offers Reinit in place of the\n`charter ws reinit --all` text the spec looked for. The spec now asks\nfor the row's words and its Reinit button.\n\nRefs #1238\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-06T02:59:05+04:00",
          "tree_id": "3bc3a5289adad8e6eaca2688f78c123b978e1d28",
          "url": "https://github.com/purlis/purlis/commit/272fd673a632c304772e6c95d3822f7de5f3fc9e"
        },
        "date": 1791241770994,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5978684999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 0.590, 0.594, 0.598, 0.599, 0.622 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.003626999999998,
            "unit": "ms",
            "extra": "median of 5 runs: 15.894, 15.971, 16.004, 16.008, 16.577 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.62571,
            "unit": "ms",
            "extra": "median of 5 runs: 101.249, 101.458, 101.626, 101.913, 102.570 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "e691c31272ce7cc01aa7470c18deac99afc1dbfc",
          "message": "save: the secret check also asks each decoded key and string on its own\n\nThe JSON and TOML writers keep an escaped control character as two\ncharacters, so a credential right after an escaped newline or tab had a\nletter in front of it in the re-written document, and the rules that need a\nboundary there missed it. parsed_kind now also asks each decoded key and\nstring value on its own, after the whole-document pass, which still reads a\nkey and its value together as an assignment.\n\nDecided in implementation:\n- D-1295-6 (dispatcher, overrides D-1295-3): the save asks each decoded key\n  and string value as well as the whole document. Until the settings\n  editors move onto parsed_kind (issue 1304, item 1) the save may refuse\n  what an editor accepted, which is the safe direction.\n\nRefs #1295\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-06T03:53:49+04:00",
          "tree_id": "469ca42c4ad027a10e87e9091c845e86e5d06122",
          "url": "https://github.com/purlis/purlis/commit/e691c31272ce7cc01aa7470c18deac99afc1dbfc"
        },
        "date": 1791244591191,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.43553,
            "unit": "ms",
            "extra": "median of 5 runs: 0.416, 0.432, 0.436, 0.437, 0.454 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.5720855,
            "unit": "ms",
            "extra": "median of 5 runs: 16.312, 16.365, 16.572, 16.618, 17.053 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.4245225,
            "unit": "ms",
            "extra": "median of 5 runs: 101.415, 102.246, 102.425, 102.491, 102.725 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "475e66594cab7d1c1b714519e588d802157a0efa",
          "message": "train 6: the tickets' tests meet main's guards and the renamed server\n\n- RN-8 gave rename-local's `Local` a `plugin` field after RN-6's keychain\n  test was written, so the test's `Local` names it as `None`, as the other\n  rename-local test does.\n- RN-8's three later rename-local plugin tests open with\n  `charter_core::unsteered!()`, as every test in charter-core's tests\n  directory must.\n- fake-harness's ACP handshake test expects the MCP server under its\n  current name (`chattools::SERVER`, `purlis` since RN-8) rather than the\n  old literal.\n\nRefs #1264\nRefs #1266\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-06T04:05:49+04:00",
          "tree_id": "ca728e37e1cd9f3c7711b6e0f3452f06ee79ee86",
          "url": "https://github.com/purlis/purlis/commit/475e66594cab7d1c1b714519e588d802157a0efa"
        },
        "date": 1791245615063,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.516165,
            "unit": "ms",
            "extra": "median of 5 runs: 0.482, 0.508, 0.516, 0.518, 0.520 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.075515,
            "unit": "ms",
            "extra": "median of 5 runs: 16.555, 17.027, 17.076, 17.081, 17.465 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.9795895,
            "unit": "ms",
            "extra": "median of 5 runs: 103.237, 104.544, 104.980, 105.285, 108.416 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "cba7f1f342aa6968261315c641b73c00a9ab3f04",
          "message": "A renamed workspace's todo, memory and session tabs come back at launch\n\nThe rename test's saved record now holds a todo tab (alpha/<slug>), a\nmemory tab (workspace/alpha/<slug>) and a session record tab\n(workspaces/alpha/sessions/<file>). After renaming alpha to beta, the\ntest reads the record the way the next launch does, through\nreopen::read_or_refusal, and checks that each tab is held and keyed by\nbeta, on beta's strip.\n\nThis is the after-a-rename half of the ticket. The rekeying (issue 1248)\nand the widened held() check both already on main make it pass; no code\nchange was needed. While writing it, a session key whose file name is not\na record's name (such as 2026-10-05-review.md) was dropped at read, which\nis correct: held() checks a session key with sessionrecord::locate. The\ntest uses a real record name.\n\nCloses #1297\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-06T04:58:32+04:00",
          "tree_id": "bdf323829507a2db89f8bb354f295fd4a5e17076",
          "url": "https://github.com/purlis/purlis/commit/cba7f1f342aa6968261315c641b73c00a9ab3f04"
        },
        "date": 1791248475346,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.276779,
            "unit": "ms",
            "extra": "median of 5 runs: 0.268, 0.270, 0.277, 0.289, 0.305 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.579482,
            "unit": "ms",
            "extra": "median of 5 runs: 16.430, 16.533, 16.579, 16.661, 16.686 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.33099899999999,
            "unit": "ms",
            "extra": "median of 5 runs: 101.140, 101.308, 101.331, 101.781, 102.187 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "aded4442c0473c7f79f87548a16feb0a34098d6f",
          "message": "train 7: RN-9's identity test sets the plugin field rename-local has since RN-8\n\nRN-8 (train 6) added a `plugin` field to rename-local's `Local`, and RN-9's new\ntest was written before it. The test now sets it to `None`, as the other\nrename-local tests do. Local clippy caught it.\n\nRefs #1267\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-06T05:28:46+04:00",
          "tree_id": "6aa113e5fb984709c6160f2a193baf670fb142a5",
          "url": "https://github.com/purlis/purlis/commit/aded4442c0473c7f79f87548a16feb0a34098d6f"
        },
        "date": 1791250219570,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.533472,
            "unit": "ms",
            "extra": "median of 5 runs: 0.520, 0.526, 0.533, 0.534, 0.535 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.8076875,
            "unit": "ms",
            "extra": "median of 5 runs: 16.341, 16.525, 16.808, 16.888, 17.299 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.72969,
            "unit": "ms",
            "extra": "median of 5 runs: 102.026, 102.263, 102.730, 103.283, 104.473 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "d1c462a6108b968446a45432ee37b6f033211b1e",
          "message": "locks: a kept store lock unlocks on drop; the 972 test models an exited host\n\nReview of the first commit for issue 1316:\n\n- held::Store::locked() forgot its guard on purpose, so memstore::lock_store\n  let go only when the descriptor closed: the same leak to a forked child.\n  It now returns a LockedStore that unlocks in Drop, and StoreLock::Held keeps\n  that. A new held test hands a copy of the store's descriptor to a sleeping\n  child, drops the kept lock and takes it again; it failed (refused after\n  the 5 s wait) before this change.\n- The issue-972 event log test passed trivially once Log unlocks on drop. It\n  now models a host that exits without unlocking: a plain File on LOCK,\n  locked, a copy given to `sleep 0.4` as stdin, the File dropped unlocked,\n  and Log::open must succeed after waiting at least 100 ms. It no longer\n  clears close-on-exec, which leaked the lock into parallel tests' children.\n  Setting LOCK_WAIT to zero makes it fail.\n- Log::_lock is declared last, so it drops after the log's own files.\n- filelock's module doc now names which locks use Held and which unlock in\n  guards of their own.\n\nDecided in implementation:\n- D-1316-6: a kept lock is its own type (LockedStore, returned by\n  Store::locked) rather than a Drop on Store. An unlocked Store is held and\n  dropped all the time, and the type says which stores hold a lock until\n  dropped, the way Locked already does for one taken in scope.\n\nRefs #1316\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-06T06:16:05+04:00",
          "tree_id": "1feae52f1b8c17087cf5016a30a7d4f63c699175",
          "url": "https://github.com/purlis/purlis/commit/d1c462a6108b968446a45432ee37b6f033211b1e"
        },
        "date": 1791253180210,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5059530000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.495, 0.498, 0.506, 0.528, 0.541 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.125711000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 16.486, 16.722, 17.126, 17.132, 17.324 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.6045575,
            "unit": "ms",
            "extra": "median of 5 runs: 102.701, 103.057, 103.605, 104.224, 104.956 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "4599019abf222bd61d3a23fbcaf66184d3dc9e2a",
          "message": "recorded: the two news scenarios print the renamed CHANGELOG (RN-11b)\n\n`purlis news` prints CHANGELOG.md, and the docs sweep renamed the product in its prose, so the\ntwo news scenarios are re-recorded. nameonly.py still finds only the name and the two\naccepted statusline crops: 539 scenarios, 254 re-recorded in all.\n\nRefs #1270\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-06T07:12:00+04:00",
          "tree_id": "3e0f11ce42325fb2e491f4310a515145004ac26b",
          "url": "https://github.com/purlis/purlis/commit/4599019abf222bd61d3a23fbcaf66184d3dc9e2a"
        },
        "date": 1791256404612,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4648855,
            "unit": "ms",
            "extra": "median of 5 runs: 0.443, 0.452, 0.465, 0.469, 0.475 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.300114999999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.227, 16.290, 16.300, 16.488, 16.839 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.37131600000001,
            "unit": "ms",
            "extra": "median of 5 runs: 100.910, 100.961, 101.371, 102.214, 102.695 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "29f2ddbaed7e2a60d027161a2924d2446d86dfb7",
          "message": "e2e: the app launched over an old charter install, migrated at launch (RN-10)\n\nA wdio run of its own (`npm run e2e:upgrade`, `wdio.upgrade.conf.ts`) starts the\nreal app over an old install built inside the run's own HOME: the config home\nunder `<home>/.config/charter/` (an approved, pinned project with a pinned\nworkspace, on the dev channel), and a project in git with `.charter/`,\n`charter.toml`, `charter.local.toml` and a keyring vault whose item is under\n`charter/…` in the fenced build's stub keychain.\n\nThe scenario build skips the launch's rename-local (D-RN5-7); the app now runs\nit there when `PURLIS_E2E_RENAME_LOCAL=1`, which only this run sets. The spec\nasserts the approval, the pins and the channel survived, the config home is\n`purlis/` with no `charter/` beside it, the project's state and local settings\nmoved, the vault reads under `purlis/…` with nothing left waiting for the\nkeychain, and the doctor's `rename-plane` runs by name in one commit. The run\nremoves the old install when it passes.\n\nCI is this spec's first real run: wdio does not run reliably on this machine.\nThe fixture and the migration were checked locally instead: the old install\nthe helper writes was migrated by the core's rename-local in a fenced test\nbuild (config home, local settings, state folder, vault copied and read back,\nnothing waiting), and rename-plane then made its one commit on a clean tree.\n\nDecided in implementation:\n- D-RN10-1: The switch is `PURLIS_E2E_RENAME_LOCAL`, read only by the `e2e`\n  build and on only for exactly `1`. It calls `renamelocal::at_launch` with no\n  plugin to move: the build is fenced, so its busy check asks only the\n  project's own sockets, its keychain is the stub, and no harness is touched.\n  The log-folder move stays off, since every run names its log folder.\n- D-RN10-2: A run of its own, as the layout run is: the Tauri service keeps one\n  app per run and the old install has to be on disk before it starts. The base\n  run excludes `*.upgrade.e2e.ts`; CI runs it after the layout run on both\n  OSes; stress.yml ignores its files.\n- D-RN10-3: The old install is written as files in the old layout, not made\n  with the `charter` CLI: CI's CLI is not fenced, so making a keyring vault\n  with it would reach the runner's real keychain. `upgrade.test.ts` holds the\n  fixture to the old layout, and the variable's spelling in the config to the\n  one `lib.rs` reads.\n- D-RN10-4: The config home is `<run tree>/home/.config`, named by\n  `CHARTER_CONFIG_HOME` as well, because GitHub runners export an\n  `XDG_CONFIG_HOME` of their own. The helper refuses a home outside the run's\n  tree.\n- D-RN10-5: The values are ones no launch sets on its own: an approval, a\n  project pin, the `beta` workspace pin (a first open pins the most active\n  workspace, `alpha`), the dev channel. A store lost in the move fails the spec.\n- D-RN10-6: No doctor row offers `rename-plane` (it runs by name only, RN-7),\n  so the spec runs it by name through the Doctor's own command,\n  `plane_doctor_fix`, and asserts one commit, `purlis.toml` and a clean tree.\n  The project is committed clean and its profile program lives outside it.\n- D-RN10-7: The store remembers the project by its real path, which the app\n  resolves it to; `CHARTER_ROOT` is the copied path inside the fence.\n- D-RN10-8: Cleanup in `onComplete`: a passing run removes the project and the\n  config home; a failing one keeps them as evidence. Its own run tree and app\n  process, so nothing reaches a later run.\n- D-RN10-9: No changelog fragment: nothing a user sees changes.\n\nCloses #1268\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-06T07:45:53+04:00",
          "tree_id": "4013e9f47dd6c31c99aa9ac18b68ad879f534899",
          "url": "https://github.com/purlis/purlis/commit/29f2ddbaed7e2a60d027161a2924d2446d86dfb7"
        },
        "date": 1791258423011,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5209405,
            "unit": "ms",
            "extra": "median of 5 runs: 0.505, 0.519, 0.521, 0.529, 0.534 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.1283735,
            "unit": "ms",
            "extra": "median of 5 runs: 16.520, 16.808, 17.128, 17.337, 17.406 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.49872099999999,
            "unit": "ms",
            "extra": "median of 5 runs: 104.850, 105.092, 105.499, 105.733, 108.273 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "43c789086c103b0c86906e435b9ae8eff7d8ed09",
          "message": "Bump source-map-js from 1.2.1 to 1.2.2 in /app\n\nBumps [source-map-js](https://github.com/7rulnik/source-map-js) from 1.2.1 to 1.2.2.\n- [Release notes](https://github.com/7rulnik/source-map-js/releases)\n- [Changelog](https://github.com/7rulnik/source-map-js/blob/main/CHANGELOG.md)\n- [Commits](https://github.com/7rulnik/source-map-js/compare/v1.2.1...v1.2.2)\n\n---\nupdated-dependencies:\n- dependency-name: source-map-js\n  dependency-version: 1.2.2\n  dependency-type: indirect\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-10-06T08:13:30+04:00",
          "tree_id": "5d12ce1d84edb271a5a11a244d646635cc0d8dbb",
          "url": "https://github.com/purlis/purlis/commit/43c789086c103b0c86906e435b9ae8eff7d8ed09"
        },
        "date": 1791260088192,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.290132,
            "unit": "ms",
            "extra": "median of 5 runs: 0.264, 0.280, 0.290, 0.297, 0.300 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.613564500000003,
            "unit": "ms",
            "extra": "median of 5 runs: 16.547, 16.600, 16.614, 16.614, 16.682 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.675401,
            "unit": "ms",
            "extra": "median of 5 runs: 100.334, 101.624, 101.675, 101.792, 102.265 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "e63fd8affbef57f60e0c43f079e51d893a591a95",
          "message": "Bump smol-toml from 1.8.0 to 1.9.0 in /app\n\nBumps [smol-toml](https://github.com/squirrelchat/smol-toml) from 1.8.0 to 1.9.0.\n- [Release notes](https://github.com/squirrelchat/smol-toml/releases)\n- [Commits](https://github.com/squirrelchat/smol-toml/compare/v1.8.0...v1.9.0)\n\n---\nupdated-dependencies:\n- dependency-name: smol-toml\n  dependency-version: 1.9.0\n  dependency-type: indirect\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-10-06T08:13:35+04:00",
          "tree_id": "5ee5c8d4594bea953298ef834a78deeb875ba485",
          "url": "https://github.com/purlis/purlis/commit/e63fd8affbef57f60e0c43f079e51d893a591a95"
        },
        "date": 1791261031035,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.526898,
            "unit": "ms",
            "extra": "median of 5 runs: 0.499, 0.505, 0.527, 0.536, 0.537 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.955509,
            "unit": "ms",
            "extra": "median of 5 runs: 16.882, 16.938, 16.956, 16.964, 17.349 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.68939750000001,
            "unit": "ms",
            "extra": "median of 5 runs: 102.990, 103.377, 103.689, 104.005, 104.262 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "2c8b8ffd786b204707d0f3b7aceace6b3b74092e",
          "message": "chats: name the local file once in the approval test\n\nThe test that a refused Retry reads the approval again rewrote the\nprofiles' local file in one statement that both named it and read it.\nThe local-layer guard reads that shape as a reader that skips\nsettings::layer_text, and it failed train 51's rust job. The test now\nputs the path in a local first. It is test code, so nothing it decides\nchanges.\n\nTrain fold-in.\n\nRefs #1246\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-06T09:55:03+04:00",
          "tree_id": "0205ada3e113972fb18f607577fc8bd251af7b90",
          "url": "https://github.com/purlis/purlis/commit/2c8b8ffd786b204707d0f3b7aceace6b3b74092e"
        },
        "date": 1791266170699,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.47029850000000006,
            "unit": "ms",
            "extra": "median of 5 runs: 0.457, 0.459, 0.470, 0.475, 0.477 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.2398685,
            "unit": "ms",
            "extra": "median of 5 runs: 16.195, 16.232, 16.240, 16.259, 16.331 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.713942,
            "unit": "ms",
            "extra": "median of 5 runs: 100.820, 101.597, 101.714, 101.862, 102.450 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "60b73fb0ddc749a676edba292ea3cd53bf58cd60",
          "message": "diffscan: a line-ending backslash with blanks after it, and blank lines, still join\n\nThe commit and push scan now joins the way the lexical decode does. A\nbackslash followed only by blanks to the end of its line joins, and the\njoin carries on through blank and whitespace-only added lines up to the\nnext line that is not blank. Each line is still read into one run at most.\n\nRefs #1315\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-06T13:56:42+04:00",
          "tree_id": "30043fe124c0cd9087fc6979806e9a9ab0e840ae",
          "url": "https://github.com/purlis/purlis/commit/60b73fb0ddc749a676edba292ea3cd53bf58cd60"
        },
        "date": 1791280675866,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5281399999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 0.509, 0.517, 0.528, 0.533, 0.556 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.0965445,
            "unit": "ms",
            "extra": "median of 5 runs: 16.749, 16.768, 17.097, 17.151, 17.290 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.736081,
            "unit": "ms",
            "extra": "median of 5 runs: 103.591, 104.126, 105.736, 106.055, 106.078 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "d7721362c2c1d2a60c5aa7c3d9d0c4f8ff80a182",
          "message": "tests: the sandbox block's Report may name the tracker it files on\n\nThe address guard (ADR 0083 §9) refused app/src-tauri/src/sandboxing.rs, which\n#1338 added: the Report dialog shows the repository it would file on and files\nthere through report::file on the person's press, as `purlis report` does\n(ADR 0059). It gains its allowance with that reason.\n\nRefs #1338\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-06T15:24:52+04:00",
          "tree_id": "b75597403e636b8ad224b2d0a7283341d1631ef9",
          "url": "https://github.com/purlis/purlis/commit/d7721362c2c1d2a60c5aa7c3d9d0c4f8ff80a182"
        },
        "date": 1791286675359,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.528253,
            "unit": "ms",
            "extra": "median of 5 runs: 0.512, 0.524, 0.528, 0.533, 0.552 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.198847,
            "unit": "ms",
            "extra": "median of 5 runs: 16.321, 17.015, 17.199, 17.319, 17.361 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.4774455,
            "unit": "ms",
            "extra": "median of 5 runs: 102.247, 103.486, 104.477, 104.713, 105.719 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "8f295ead543af155b131f9e75e329cb0603445fd",
          "message": "smart close: the closing Stop is sent, not waited on; the name guard drops session.rs\n\nThe two tests where a Stop closes the chat waited on it with the\n`reported` helper. That helper waits until the board shows the chat\nwaiting, but the Stop closes the chat and takes it off the board, so\nthe wait could never succeed. The Stop is now sent directly, and the\ntests wait for the tab to close.\n\nsession.rs no longer spells an old name, so its STILL_SPELLED entry\nis removed (the guard counts exactly).\n\nRefs #1332\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-06T16:27:18+04:00",
          "tree_id": "3c32b46c0c66b569fc8c013312951f9ef4388ff6",
          "url": "https://github.com/purlis/purlis/commit/8f295ead543af155b131f9e75e329cb0603445fd"
        },
        "date": 1791290129251,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5543435,
            "unit": "ms",
            "extra": "median of 5 runs: 0.502, 0.551, 0.554, 0.564, 0.611 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.013863,
            "unit": "ms",
            "extra": "median of 5 runs: 16.759, 16.789, 17.014, 17.746, 18.043 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.766386,
            "unit": "ms",
            "extra": "median of 5 runs: 102.206, 104.583, 105.766, 107.030, 109.012 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "af8adc1249e8ac2b501d4e27338a8a3dba341e74",
          "message": "brokered git test: quote the stand-in filter so it really runs\n\nThe operator's global `mark` filter in brokered_git.rs was written\nunquoted, `smudge = sh -c 'touch smudged; cat'`. git reads the `;` as\nthe start of a comment, so the command was cut to `sh -c 'touch smudged`,\nfailed, and, not being `required`, was skipped without a word. The\nterminal's clone therefore never ran it, and the assertion that the\napp's clone does not run it checked nothing. Linux CI caught the first\nhalf.\n\nThe value is now double-quoted and marks one absolute path in the\nworld's base directory, checked and cleared after each run, so the\nterminal's clone must run it and the app's clone and worktree must not.\nThe terminal's runs are asserted successful, and every failure prints\nthe run's stderr. The core broker tests' `evil` filter had the same\nunquoted value and is quoted too, so their \"the filter never ran\"\nassertions are no longer vacuous.\n\nRefs #1335\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-06T23:40:08+04:00",
          "tree_id": "48522879f673ca83b1983426a7a0be192c787f03",
          "url": "https://github.com/purlis/purlis/commit/af8adc1249e8ac2b501d4e27338a8a3dba341e74"
        },
        "date": 1791316143863,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.527177,
            "unit": "ms",
            "extra": "median of 5 runs: 0.499, 0.499, 0.527, 0.528, 0.564 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.922821,
            "unit": "ms",
            "extra": "median of 5 runs: 16.699, 16.747, 16.923, 17.117, 17.191 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.3217695,
            "unit": "ms",
            "extra": "median of 5 runs: 101.817, 103.494, 104.322, 105.027, 105.753 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "a8b6350e1872f04d6c9d805a9d55550bbe483e8b",
          "message": "settings: Sandbox says what a chat here can do\n\nSettings > Project > Sandbox opens on one sentence that follows the\nfiles as they change: what a chat can write (its folder, the project's\npackage caches with Package registries on, and the folders you allowed\nevery chat on this machine) and what it can reach (each preset on, the\nproject's hosts and yours that are actually granted, and on a Mac the\nsystem's certificate service once certificate checks are on). Until\nthe core has answered it says \"Reading this project's sandbox…\".\n\n- A note says a chat can read any file you can, except vaults and\n  purlis's own keys, so keep secrets in a vault.\n- Internet access is a box per preset, named and listed by the core:\n  Preset::title and Preset::own_hosts, handed to the window in\n  SandboxState.presets with each preset's hosts as sandbox::hosts\n  answers them, the project's forges among code hosting's.\n- sandbox::besides counts what every chat is granted besides its\n  presets: the project's hosts and yours (locks, this machine's\n  addresses and the ignore check applied) and the folders you allowed\n  on this machine that still judge as grantable. It and\n  Compiled::granted read the same two helpers, granted_hosts and\n  granted_folders, and a fixture test holds the two equal for a chat on\n  no persona. SandboxState.besides carries it.\n- The mode is a status line, \"On for everyone in this project. To run\n  one chat without it, use that chat's tab\", with the machine's\n  opt-out count capitalised; where it is off, Turn the sandbox on\n  writes mode = \"on\" with no Undo, as before.\n- Certificate checks can be set here. Each persona's own hosts are\n  named, with who else holds them: a chat naming no persona gets the\n  default persona's, a handoff may hold the asking chat's, and a Resume\n  the default persona's. Harnesses never sandboxed on this machine are said.\n- Two read-only lists, What chats can change (with the harness's own\n  state) and Always protected (Your approvals in purlis, not your\n  logins), each item with one line of why, drawn after the project's\n  hosts and found by the Settings filter.\n- Settings names the project's files as the project names them\n  (charter.toml or purlis.toml): every Project group's text, the file\n  choice, the override badge, and the core's refusals the tab shows\n  (settings::refusals and the hosts collection's, via\n  settings::named_at). Only a whole file name is renamed: a host or a\n  path that holds one (charter.toml.example.com) is quoted as written,\n  and the Sandbox's own pages, which quote hosts a person wrote, are\n  not rewritten at all.\n\nDecided in implementation:\n- D-1340-1: readable preset names live in core beside the word and the\n  host table, so every surface names a preset alike.\n- D-1340-2: presets in force are read from the committed file, so the\n  sentence follows a box before the core is asked again; a file with no\n  egress reads as every preset, as core reads it. Hosts and folders are\n  the core's counts.\n- D-1340-3: the two lists are app copy over ADR 0067's classes; the\n  runner-internals class is left out, since no project chat runs on a\n  runner, and workspace.md editing is left out until #1334 lands.\n- D-1340-4: a group may carry rows drawn after its collection (after),\n  so the explanations never read as entries of the hosts list.\n- D-1340-5: FileSetting gains checks and status kinds rather than\n  hand-written rows, so the driver's write, refusal and Undo rules hold.\n- D-1340-6: \"Your approvals in purlis\" replaces \"approvals and\n  sign-ins\", and the page says reads are not confined, so no login of\n  the person's reads as hidden from a chat.\n- D-1340-7: the files' names are resolved where text reaches the window\n  (core: settings::named_at; app: projectGroups), rather than threading\n  a name through every reader's message; only whole names, never part\n  of a host or a path.\n- D-1340-8: the certificate service is named only where the window runs\n  on a Mac (onAMac), the one system where the setting widens anything.\n- D-1340-9: Settings' counts and a chat's start share one helper for\n  hosts and one for folders, so they cannot drift apart.\n\nFollow-ups: issue 1422.\n\nCloses #1340\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-07T03:20:36+04:00",
          "tree_id": "42daaa49ed085915f55d0ea834685a60441a8df4",
          "url": "https://github.com/purlis/purlis/commit/a8b6350e1872f04d6c9d805a9d55550bbe483e8b"
        },
        "date": 1791329385209,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.272841,
            "unit": "ms",
            "extra": "median of 5 runs: 0.264, 0.267, 0.273, 0.274, 0.291 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.5258645,
            "unit": "ms",
            "extra": "median of 5 runs: 16.417, 16.506, 16.526, 16.567, 16.575 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.1325025,
            "unit": "ms",
            "extra": "median of 5 runs: 100.330, 100.845, 101.133, 101.372, 101.375 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "38e55d8b8c997b19117ccae8ca78c96bee2da0cc",
          "message": "sandbox: read bare script names, extensions, assignments and comma values\n\nFollow-ups from #1356, the whole #1418 checklist.\n\n- The reporting socket's folder is denied under every name a denied\n  path has: as written, as the kernel resolves it, and across a firmlink\n  (Settings::reporting_on, through the names_of helper now shared).\n- Bare names: a word a program opens as a file of code is named from\n  the folders the command runs in, with or without a `/` (`bash hook.sh`,\n  `awk -f sum.awk`, `awk -fsum.awk`, `python3 -mtools.x`, an MCP\n  server's `node server.js`, `source env.sh`).\n- Extension inference: such a file named without an extension its\n  runner adds is also named with each one (`node -r ./hook` names\n  hook.js and the other JS extensions, `ruby -r./x` names x.rb, x.so\n  and x.bundle, `gawk -i ./lib/util` names util.awk).\n- An assignment's value (`BASH_ENV=./x.sh`, git `-c core.hooksPath=./h`)\n  is named, and so is the whole word.\n- `-f./a=b.awk` names both `./a=b.awk` and `b.awk`.\n- Each value after `-W<letter>,` is read as a word of its own.\n- folds_case probes the real folder, and looks at the case-swapped\n  name with lstat, so a link cannot answer for it.\n- Scan::holds compares every spelling (link, data volume, case fold)\n  of the value against every spelling of the project, home, grantable\n  folders and cache home.\n- seatbelt::profile refuses a profile over MOST_PROFILE (768 KiB) with\n  TOO_LARGE, not an E2BIG at spawn. seatbelt::not_started writes the\n  refusal for the Codex and opencode wraps: the way out is a sentence\n  after \"so nothing was started\", as in the other refusals.\n- Review round 2:\n  - Subcommands are not scripts. deno reads a script only after `run`.\n    bun's own subcommands end the scan, and after `bun run` (or bare\n    `bun x`) a word is a file only with a `/` or a JS extension. tsx skips\n    `watch`. node's `--run` ends the scan.\n  - `python -m a.b` names a/b.py, a/b/__main__.py, a/b/__init__.py and\n    a/__init__.py, never the package folder.\n  - An existing folder given to a JS runner is its entry files, not the\n    folder.\n  - Value-taking long options for node and deno are skipped as values,\n    so the script after them is found.\n  - Launchers re-read the rest as a command (D-1418-9). `make -f`/\n    `--file`/`--makefile` is read, and versioned interpreters\n    (python3.12, node22) are read as their program.\n  - A folder named only after `=` is never denied, unless the variable\n    or git setting loads code (D-1418-10, amended in round 3).\n- Review round 3: RUNS_CODE, one table of the variables and git\n  settings that load code. The launchers read their `=` and attached\n  forms (`env --chdir=`, `-C<dir>`, `uv --directory=`/`--project=`),\n  skip BSD `env -P`/`-S` values, and read `npx tsx@4` as tsx and\n  `uv run -m mod` as python -m mod. The redundant BUN_OWN list is gone:\n  none of bun's own subcommands has a `/` or a JS extension, so the\n  package-script rule already ends the scan for each.\n\nDecided in implementation:\n- D-1418-1 Bare names. A word without a `/` is named only where the\n  program reads it as code: the script operand of a shell, `source`/`.`,\n  node/bun/tsx/ts-node/deno (after `run`), python, ruby, perl and php,\n  and the value of a program-file option (awk/gawk `-f`/`-E`/`-i`/\n  `--file`/`--exec`/`--include`, sed `-f`/`--file`, jq `-f`/\n  `--from-file`, node `-r`/`--require`/`--import`/`--loader`/\n  `--preload`, ruby `-r`, python `-m`, php `-f`, bash `--rcfile`/\n  `--init-file`). Each name is resolved from every folder the command\n  went to, as a `/` word is. These positions come from the program's\n  own grammar, not a guess, so an absolute value is named wherever it is\n  (`gawk -bf/opt/x.awk` now names /opt/x.awk). The script's own\n  arguments, code passed with `-c`/`-e`/`-p`/`-r`, npx's package names\n  and include folders (`-I lib`, `-L lib`) stay unnamed. Naming\n  include folders would make a repo's source folder read-only, and\n  none of those words is code a harness runs. Rejected: naming every\n  bare word (it denies data and log files a hook writes), and keeping\n  #1327's `/` rule (it misses the most common MCP form, `node\n  server.js`). A misparse can only miss a bare name, which today's rule\n  misses already. Over-reading stays inside the folders the command\n  runs in, so it never reaches an unrelated real folder.\n- D-1418-2 Extensions. A file a runner may find by adding an\n  extension is named with each one, only where its name has none of\n  them. JS runners use .js .mjs .cjs .json .node .ts .mts .cts .tsx\n  .jsx. Ruby -r uses .rb .so .bundle. awk -f/-E/-i use .awk. A python\n  -m module a.b names a/b.py, a/b/__main__.py, a/b/__init__.py and each parent\n  package's __init__.py, never the package folder. Search paths\n  (NODE_PATH, $LOAD_PATH, AWKPATH) are not followed. The siblings\n  sit beside a named file, never a new folder (D-1356-7 holds).\n- D-1418-3 Assignments. A word `<name>=<value>`, where the name reads\n  as a variable or a git key, names its value and the word as written\n  (a file may hold `=`). The name can be anything, so an absolute value\n  is a guess and follows D-1356-7: it is kept only where a chat could\n  write it (Scan::holds). `PREFIX=/usr/local` names no /usr/local.\n  Colon lists (PYTHONPATH=a:b) are not split.\n- D-1418-4 `=` precedence: a short option cluster with `=` gives both\n  the attached split and the value after `=`.\n- D-1418-5 Comma values: `-W<letter>,a,b` hands each piece on as a\n  word, so each is read as one (options recursively, so\n  `-Wl,--script=./x.ld` names x.ld). They are exact, so absolute ones\n  are named as a separate word is.\n- D-1418-6 holds() compares spellings (as written, lexical, resolved,\n  firmlink twin) of the value and of each root, case-folded where the\n  root's volume folds case. Over-counting only names more (fail closed).\n- D-1418-7 Case probe: folds_case asks real(path)'s deepest existing\n  folder, and reads the case-swapped name with lstat. A link beside\n  it, or a link on the way onto another volume, no longer answers.\n- D-1418-9 Launchers. env (skipping VAR=val, which stay words, -i,\n  -u NAME and -C D, -CD and --chdir=D as a folder the command runs in,\n  and BSD -P/-S taking a value), exec, nohup,\n  npx/bunx/pnpx, pnpm exec/dlx, yarn (a package's `@<version>` is\n  dropped), uv run (with --directory/--project, also with `=`, as\n  folders the command runs in; `uv run x.py` read as python x.py and\n  `uv run -m mod` as python -m mod), poetry run and pipenv run. Each re-reads the rest as a\n  command of its own. A package runner's command is re-read only when it\n  is a program purlis reads (LOADS). Otherwise it is the package's own\n  tool, and its words are read as before (`npx prettier` names no\n  prettier). `uv run .` keeps the runner folder rule. A versioned name\n  (python3.12, node22, bash5) is read as its program.\n- D-1418-10 A value after `=`, whether an assignment's, an option's\n  (`--target-dir=./target`) or a JVM agent's options, is named only\n  as a file: one that exists, or one that does not exist yet and has\n  an extension. Such a value is a search path or an output folder as\n  often as code, so a folder named only that way stays writable\n  (`CARGO_TARGET_DIR=./target`, `OUT=./dist`, `PYTHONPATH=./src`,\n  `NODE_PATH=./lib`). This rule decides from what is at the path, and\n  a chat can make that: a folder where the file will be, or a dotfile\n  with no extension. So it applies only to values that load no code.\n  Amended in round 3: a variable or git setting that loads code\n  (RUNS_CODE, one table) names its value whatever is there, absolute\n  ones too:\n  - paths: BASH_ENV, ENV, ZDOTDIR, PYTHONSTARTUP, GIT_SSH,\n    GIT_ASKPASS, SSH_ASKPASS, core.hooksPath, core.fsmonitor,\n    core.askPass, gpg.program;\n  - path lists, split at `:` or a space: LD_PRELOAD, LD_AUDIT,\n    DYLD_INSERT_LIBRARIES;\n  - commands, read as a line: PROMPT_COMMAND, GIT_SSH_COMMAND,\n    GIT_EXTERNAL_DIFF, GIT_PAGER, GIT_EDITOR, GIT_SEQUENCE_EDITOR,\n    GIT_PROXY_COMMAND, EDITOR, VISUAL, PAGER, core.sshCommand,\n    core.pager, core.editor, sequence.editor, diff.external,\n    credential.helper;\n  - options, read as that program's: NODE_OPTIONS (node's\n    -r/--require/--import/--loader), RUBYOPT (ruby -r),\n    JAVA_TOOL_OPTIONS, JDK_JAVA_OPTIONS and _JAVA_OPTIONS\n    (-javaagent:);\n  - PERL5OPT: each -I folder, and each -M/-m module as Module/Name.pm\n    from the command's folders and from each -I folder.\n  Search-path variables (PYTHONPATH, NODE_PATH, PERL5LIB, RUBYLIB,\n  LD_LIBRARY_PATH) are left out on purpose. They name the repo's own\n  source folders, which the chat edits by design (as D-1418-1 leaves\n  include folders), so they keep the file-only rule. node's `--env-file` and deno's\n  `--config`/`--import-map` are read as values, not files.\n  `deno --lock`/`--env-file`/`--v8-flags` take their value only after\n  `=`, so they are not skipped as taking the next word.\n- D-1418-8 Profile size: cap rather than `-f <file>`. A file in the\n  chat's tmp could be rewritten by an earlier process of the same chat\n  between the write and sandbox-exec's read. Any other place would be\n  a new trusted location. The cap is on the profile's bytes (768 KiB,\n  three quarters of ARG_MAX: two configs at MOST_NAMED with firmlink\n  twins fit, and a quarter is left for the environment). Bytes are\n  the quantity E2BIG is about, and a count of denials is not. Past it the start is refused with a sentence, through the\n  error path CONTROL already uses (fail closed).\n\nTwo existing tests used `gawk -bf<abs>` as an example of a speculative\nlater-letter split. Under D-1418-1 that form is read by gawk's grammar,\nso they now use `cc -xI<abs>`, whose letters purlis does not know.\n\nLeftovers are filed as one checklist, issue 1425. Round 2 adds `bash -s\narg`/`ruby -S rake` and the first-message E2BIG to it.\n\nCloses #1418\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-07T11:20:29+04:00",
          "tree_id": "605ebd03ab47612773226fc08a7c4b5ec730468b",
          "url": "https://github.com/purlis/purlis/commit/38e55d8b8c997b19117ccae8ca78c96bee2da0cc"
        },
        "date": 1791357720008,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5150684999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 0.487, 0.509, 0.515, 0.517, 0.522 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.003363999999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.318, 16.744, 17.003, 17.054, 17.097 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.834697,
            "unit": "ms",
            "extra": "median of 5 runs: 103.376, 104.702, 104.835, 105.334, 105.514 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "8abf6e50b31e25909239a8fef074fe95e7677545",
          "message": "The dev channel build takes about half the time\n\nA dev publish took 37 to 45 minutes, 34 of them in the macOS job. The\nnine decisions of issue 1429 (ADR 0092):\n\n- A dev channel build is compiled with `dev-release`: `release` with\n  thin LTO over sixteen codegen units, and nothing else changed. The\n  command line, the built-in extension and the app all use it.\n- A dev publish makes no `.dmg`, and takes down the one an earlier\n  publish left on the `dev` release.\n- When the AppImage is required, one `tauri build` makes both Linux\n  bundles, on both channels. A build started by hand keeps two steps.\n- `purlis --version`, About purlis and the run summary name the\n  profile. The workflow reads the line off the binary it built and\n  stops if it is not the profile the run asked for.\n- A nightly profile guard builds `main` as a stable build is built,\n  outside the `release` environment, and publishes nothing. A red\n  night opens one issue or rewrites it; a clean night shuts it.\n- The stable build keeps `release`, its `.dmg` and its checks. No job\n  with a secret restores a cache, on either channel.\n\n`plan` now names the two events that get the `release` environment (a\ntag push and a green `ci`), so the schedule gets no key and neither\ndoes an event added later. A build started by hand can pick the\nprofile, which is how the two were timed.\n\nMeasured on the runners, same commit, unsigned path: macOS command\nline 490 s to 263 s and app 1003 s to 708 s; Linux 350 s to 228 s and\n755 s to 593 s. With the `.dmg` gone the macOS job comes to about 17\nminutes. The first real dev publish is the measurement of the whole.\n\nCloses #1429\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-07T15:58:29+04:00",
          "tree_id": "95610a72e7a21f8e903c3f0a1fbc1768be4d70ec",
          "url": "https://github.com/purlis/purlis/commit/8abf6e50b31e25909239a8fef074fe95e7677545"
        },
        "date": 1791374524835,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.517252,
            "unit": "ms",
            "extra": "median of 5 runs: 0.487, 0.496, 0.517, 0.529, 0.534 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.1458045,
            "unit": "ms",
            "extra": "median of 5 runs: 16.930, 17.079, 17.146, 17.202, 17.426 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.554148,
            "unit": "ms",
            "extra": "median of 5 runs: 102.514, 104.345, 104.554, 104.999, 105.139 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "37c74d0e6945400dad80cb021b383f4e0692f2a4",
          "message": "chats: Restart chat, and a Notice when a sandbox change leaves chats behind\n\nA chat keeps the sandbox it started with. After a sandbox setting changed,\nrunning chats kept the old one and nothing said so; the only ways to apply it\nwere to close the tab and resume, or to press Allow on a block Notice.\n\n- Restart chat on a chat tab's menu and in the palette: the chat's program\n  ends and starts again on the same conversation, with the project's sandbox\n  as it compiles now. Mid-turn the row reads \"Restart chat … when this turn\n  ends\" and the restart waits for the turn; the row's note says the wait too,\n  so the palette says it. While a restart runs the chat's pane says\n  \"Restarting this chat…\", and a chat waiting on a permission prompt says, in\n  the core's sentence, that it restarts once that is answered.\n- A Notice after a sandbox setting changes while chats run under an older\n  sandbox: how many, Restart them (Restart it for one) and Dismiss. One\n  Notice for each change. \"Older\" is decided by compiling, never by a file\n  having been written, and by what settings decide alone: the hosts, what the\n  presets widen, and the folders every chat may write. The window asks again\n  after its own sandbox commands return, so revoking a folder in Settings'\n  Granted list raises it.\n- Settings › Sandbox says a change applies to a chat from its next start.\n\nOne restart path. The grant restart (issue 1342), Restart now for persona\ngrants (issue 1362) and Restart chat all go through Chats::restart and the\nrestart_chat command, driven by the window's one \"once the turn ends\" driver.\nThe person asks with ask_chat_restart, a command of the window with no\nhook-line route. restart_chat_for_grant and the old restart_chat are gone;\nChats::start_again is Start fresh alone again. The window's names say restart\nand no longer grant (restartOwed, chatRestarted, answerRestart, restartsSaid,\nand the cause restart:<session>).\n\nA hang that is on main too is mended here. Chats::restarted took what was\nqueued for a chat out of `owed` inside an `if let` and put it back under the\nsame lock, so anything queued while a restart ran (Allow on a block's Notice\nwhile that chat restarted) left the restart waiting on its own lock for good,\nand every close, grant and owed list of the project behind it. The queue is\nnow taken out in a statement of its own, and what was allowed meanwhile is\ncarried to the new run, as D-1342-13 meant.\n\nA person's opt-out is never inherited (ADR 0067 §7): a chat that ran without\nthe sandbox restarts in it, and its tab says so. Nothing here offers an\nopt-out, so a policy that forbids one has nothing to hide.\n\nDecided in implementation:\n- D-1428-1: one core restart, Chats::restart, refused for a chat owed\n  nothing; the person's ask owes it one with nothing to tell.\n- D-1428-2: a chat that reports no state restarts at once when the person\n  asks; its row and the Notice say purlis cannot tell whether it is mid-turn\n  and promise no wait, and only a known mid-turn chat waits.\n- D-1428-3: a chat with no conversation to resume is refused on its pane,\n  with what to do: send it a message first, or Start fresh.\n- D-1428-4: a restart the person asked for that is refused is not owed again,\n  and Restart now asks anew (the persona Notice no longer remembers a press).\n- D-1428-5: the Notice does not count a chat started without the sandbox, a\n  shell, a chat whose sandbox cannot be compiled now, or a chat already owed\n  a restart or restarting.\n- D-1428-6: a dismissal is kept for each chat by a SHA-256 of what settings\n  decide for it now, until no chat is behind that change; any change nobody\n  dismissed shows the Notice, which then counts every chat behind.\n- D-1428-7: the row's mid-turn and no-state words are read as the menu is\n  drawn (Offer.midTurn, Offer.noState), so a chat's turn does not rebuild the\n  catalogue; the palette shows the plain title and the note with the wait.\n- D-1428-8: no opt-out is offered on a restart; the picker and a block's\n  Notice remain the two places to choose it.\n- D-1428-9: a test-only seam decides an off-profile chat's sandbox, because a\n  stand-in harness lives where a chat can write and could never start\n  sandboxed through the real program check.\n- D-1428-10 (dispatcher): the Notice compares only what settings decide, not\n  what a start finds by walking the project's tree; Confines records the\n  writable folders for it, and a run for the chat is not widened by them.\n- D-1428-11: each chat is compiled for the comparison with the persona\n  grants and the chat's own grants it started with, so a grant for one chat\n  or a persona's hosts allowed on its tab never reads as the project's change.\n- D-1428-12: an ask while a chat restarts asks for nothing more, and only\n  what has something to tell is carried to the new run.\n- D-1428-13: a pane says one line of its restart: why it failed, else why\n  not yet, else that it is under way, else that it waits for the person.\n\nCloses #1428\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-07T19:31:38+04:00",
          "tree_id": "c94ecd8cc920f7e978fbd19d6e441e1280ee2678",
          "url": "https://github.com/purlis/purlis/commit/37c74d0e6945400dad80cb021b383f4e0692f2a4"
        },
        "date": 1791387628651,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.486674,
            "unit": "ms",
            "extra": "median of 5 runs: 0.468, 0.484, 0.487, 0.493, 0.506 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.761324,
            "unit": "ms",
            "extra": "median of 5 runs: 16.653, 16.654, 16.761, 16.795, 16.818 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.45965050000001,
            "unit": "ms",
            "extra": "median of 5 runs: 101.966, 102.379, 102.460, 102.817, 103.161 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "8f3d5172c4a3d1f19722a60b3551c0e90fe42246",
          "message": "hooks: a chat's spool is a file per line, so hooks that spool at once lose none\n\nA hook whose line the host does not take keeps it in the chat's spool for the\nnext open of the project. The spool was one file per chat, appended under a\nlock that each holder kept while it read the whole file and synced. A hook\nwaited at most 250 ms for that lock, so with several hooks of one chat behind\none sync on a loaded machine, a waiter ran out of its wait and said its line\nwas lost. `a_host_restart_during_a_busy_turn_loses_no_event` showed it twice\nin CI (176 of 200, \"another process held this chat's spool for 250 ms\").\n\nThe spool is now a folder per chat, `spool/<n>/`, with a file per line. A hook\ntakes the next number by making `<key>.<seq>.part` exclusively, writes and\nsyncs its line there, links it as `<key>.<seq>.json`, removes the `.part` name\nand syncs the folder. No hook waits for another. The line, its number, its MAC,\n`keys.json`, every check the drain makes and the sandbox's denial of `spool/`\nare unchanged. The drain reads the files that have a line's name, hands them\non, and removes what it read, so hooks may go on spooling while it runs and no\nline it read is handed on twice. A `<n>.jsonl` a build before left, or that\none of its hooks still writes, is drained first and as it was, and a line it\ngave is not taken from the folder again.\n\nA hook is still bounded: the write runs on a thread of its own and is given\n1 s, and a spool that already holds 16,384 names takes no more. Past the cap,\nor on a disk that refuses the write, the hook says the line is lost, as\nbefore. Past the second it now says the line may be lost, since the write is\nleft to finish and the next open records the line if it did.\n\nWhat this does not change: the drain forgets a key once it has drained it. A\nline still being written while a drain runs, like a hook that outlives the\ndrain, is rejected as no-key at the next one and its content is not recorded.\nThat needs a ruling on keys.json and is the first follow-up on issue 983.\n\ndocs/plane-format.md has the new shape and the old file as read-only; ADR 0068\n§6 has a dated note that the shape as built is a folder.\n\nSeen red, then green: a pure test with 16 writers and 2,000 lines for one chat\non a disk that takes 20 ms per line lost 1,208 lines on main's code, each with\nCI's message, and loses none now. The delivery test keeps each hook's error\nand prints them when a line is short. After review, eight more tests were seen\nred on the first head and green now: the copy between the two stores, a line\nfile against its name, a `.part` said and one dated ahead, the last number,\nthe in-flight line, the keys' order, a write that stops, and \"may be lost\".\n\nDecided in implementation:\n- D-983-1: A file per line with the number taken by an exclusive create, over one file appended without a lock (numbers need a writer's turn, and a drain's truncate races a write) and over a shorter critical section with a longer wait (still a wait that can run out, and the drain holds the lock while it records).\n- D-983-2: The number stays contiguous per key and under the MAC: a hook tries the number after the highest it listed, then the one after for each hook that took it first, so gaps mean what they meant.\n- D-983-3: A line gets a `.json` name by a hard link, which fails rather than replace a line, and only after its bytes are synced; the name it linked is checked to be the file it wrote.\n- D-983-4: A number a dead hook took is a gap even while its `.part` file is there, so a `.part` file cannot hide a removed line; the drain removes one an hour old.\n- D-983-5: One bound for the whole write, 1 s on a thread, replaces the 250 ms lock wait and the 250 ms directory wait. A line said lost for the time may still be drained, the side the delivery path already errs on.\n- D-983-6: The size cap counts names, 16,384, where it counted 16 MiB read: a hook lists names and reads no line.\n- D-983-7: The drain holds a lock on the folder that only another drain waits for; hooks never take it.\n- D-983-8: Lines under two keys of one chat are handed on in the order keys.json holds the keys, the order the host issued them in; file times are not read (amended in review, from the files' times).\n- D-983-9: The old `<n>.jsonl` is drained at every open and emptied, never removed and never created, since a hook of the build before may still append to it (N-1).\n- D-983-10: A line in flight while a drain runs is a gap at that drain and is rejected as no-key at the next, unless the host still holds its key, which only a drain that stopped leaves; that is the loss a hook outliving the drain already had, pinned in tests, and keys.json is not changed here (corrected in review).\n- D-983-11: The seam is `Bounds`, a hook's limits and a call made as the line reaches the disk, private to the module; tests slow the disk, fill it, shrink the cap and shorten the wait through it.\n- D-983-12: The drain gives a chat's folder what its old file already gave, and the same key, number and MAC there is repeated; the MAC is compared because a hook that changed builds mid-chat has a number 1 in both.\n- D-983-13: A line's file must hold that one line under its name's key and number, or it is unreadable: hooks number by names, so names are held to the lines.\n- D-983-14: A `.part` with no line of its name is said at each drain as rejected, why `unfinished`, a new value of an existing event's field; one dated in a time to come is a dead hook's.\n- D-983-15: After a link, a line's name that is already gone was drained, so the line landed; a hook removes a `.part` name only while it is still its own file.\n- D-983-16: A line the spool was still writing when the hook stopped waiting \"may be lost\", every other \"is lost\"; a write that ends without answering says it stopped, not that the disk was slow.\n\nRefs #983\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-07T23:42:42+04:00",
          "tree_id": "3f9ef8882add7d39b13bb593623873c61eb3fb25",
          "url": "https://github.com/purlis/purlis/commit/8f3d5172c4a3d1f19722a60b3551c0e90fe42246"
        },
        "date": 1791403445232,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5339725,
            "unit": "ms",
            "extra": "median of 5 runs: 0.512, 0.519, 0.534, 0.538, 0.540 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.2264915,
            "unit": "ms",
            "extra": "median of 5 runs: 16.318, 17.213, 17.226, 17.240, 17.266 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.068119,
            "unit": "ms",
            "extra": "median of 5 runs: 102.472, 103.623, 104.068, 104.764, 106.107 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "11cebd79cf8549c98e5d11022a240b4ec623fec2",
          "message": "sandbox: what waits to be told to a chat is denied where its hooks run outside (D-T59-19)\n\nA dispatched chat's report, and purlis's own word on a dispatch the person\nwas asked about, wait in the project's state folder until the chat they\nare for takes its next turn. A chat could write there, so it could put a\nline into another chat's turn in purlis's voice, and it could read a report\nwritten for another chat.\n\nThat folder is now denied, for reading and writing and under both names of\nthe state folder, to a sandboxed chat of a harness whose hooks run outside\nits sandbox, which is asked of the harness and is Claude Code today. The\napp leaves those files and purlis's hooks take them, so nothing inside\nsuch a chat needs the folder.\n\nIt is not denied where the sandbox is a wrap around the whole harness,\nCodex and opencode today: the hooks that deliver run inside the wrap and\nread and remove each file, so a denial there would cut every report off.\nOn those a chat can still read and write the folder, and what stands is\nthe check each file gets as it is read. The denial reaches them once\ndelivery moves onto the hook socket (issue 1457).\n\nOne command a chat can run did touch the folder: a workspace rename moves\nthe reports kept for the workspace. Run inside a chat that is denied the\nfolder it now leaves them and says who moves them, and the app does, when\nthe project is next opened, from the rename's own journal: only where the\nold workspace is gone and the new one is there.\n\nTested in the compiled forms of both kinds of harness, and live under the\nwrap: a report left for a wrapped chat can still be taken. ADR 0067's\nclass 2 is amended, and docs/plane-format.md and the changelog say what\nholds on which harness.\n\nD-T59-19, delegated by the dispatcher and flagged for the operator.\n\nRefs #1437\nRefs #1457\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-08T00:35:09+04:00",
          "tree_id": "195884ec47b498806e7ecf24efa0c375e1a7f74d",
          "url": "https://github.com/purlis/purlis/commit/11cebd79cf8549c98e5d11022a240b4ec623fec2"
        },
        "date": 1791406973004,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.507305,
            "unit": "ms",
            "extra": "median of 5 runs: 0.493, 0.505, 0.507, 0.519, 0.532 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.194558999999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.756, 16.808, 17.195, 17.416, 17.543 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.818565,
            "unit": "ms",
            "extra": "median of 5 runs: 102.602, 103.479, 103.819, 105.280, 106.374 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "0c1db31bf0e6dfccaae4592541503a1da29ad82d",
          "message": "Train 59: the joined tickets agree\n\nSix ticket commits were replayed onto the wiring: the person's ask and what\nhappens when a persona chat ends (issues 1438 and 1443), wait, list, cancel\nand messages (1441, 1442), stop and the needs-you roll-up (1448), and\npersonas no longer being sub-agents (1451). Each was written against a tree\nwithout the others. This commit is where they meet.\n\nD-T59-j1. A task the person started from a chat's tab gives that chat no\npower over it. Its record names the tab's chat, because the report goes\nthere, and every power over a task is one rule in the core\n(`dispatched::is_owner`): wait, read, cancel, tell and answer are refused in\nthe sentence any stranger is told. The task sends that chat no note and no\nquestion either (`dispatchtalk::up`). The chat reads the report and sees the\ntask listed, marked as started by the person. Tested in the core.\n\nD-T59-j2. A chat's `purlis dispatch list` reads the one list of a chat's\npersona chats (`handoff::persona_chats`), which is also what closing the\nchat asks about and what the window marks a task with. One word for a task\nstopped on a prompt: `waiting on the operator`, which is the fourth\nacceptance line of issue 1443. A task the chat dispatched itself still says\nmore where the app knows more: idle, cancelling, asking this chat, how it\nreported.\n\nD-T59-j3. One stop, and one word for \"the operator stopped it\".\n- The engine is the stop machine of issue 1448 (`stopping::Stops`). The\n  close dialog's \"Stop them\" begins the same stop for the chats at work\n  below (`stopping::press_below`), under the hold the asking chat is closed\n  under. It no longer closes them by a road of its own: each gets its one\n  short turn, and none starts a chat meanwhile.\n- The word is written in one place (`handoff::operator_stopped`) and told\n  by one mark, `Handback.stopped`, in one sentence. Stop, \"Stop them\" and\n  the Close of a task that had not reported all end there. The mark carries\n  whose task it was and the stopped chat's session record. The task part's\n  own `stopped` key and its second sentence are gone.\n- A task settled by a stop is settled for good, so the close that follows\n  says nothing twice.\n- Cancel stays the asking chat's own. A task in a stop is not cancelled as\n  well: the cancel is refused in fixed words. A cancelling task can be\n  stopped: its cancel stands down and nothing more is typed for it.\n- A chat in a stop is typed its stop's one line and no other.\n- When a program ends, one mechanism speaks: the cancel's report first,\n  the stop's word for a chat being stopped, else \"ended without a report\".\n- What a stop asks for while a close holds the deciding lock is carried\n  out when the hold is let go. Ending a chat takes that lock, and the\n  close of a chat whose parent waited in a stop would not have returned.\n- A cancelled task whose tab is closed, or that the person stops, is told\n  of as stopped: the person's act is the later word (the dispatcher's\n  ruling). The report a close wrote for it as cancelled is gone, with the\n  call that wrote it under the close's own lock.\n\nD-T59-j4. \"A chat in a stop, or below one, starts nothing\" is about what\nthe chat starts. The person may still ask a persona from that chat's tab,\nand a test says so. A stop now takes the lock a dispatch is decided and\nreserved under, and a dispatch decided first is asked again before its\nprogram runs, so it does not start. This amends D-1443-14.\n\nD-T59-j5 (D-1441-15). The dispatch record has a `cancelled` outcome, in\ndocs/plane-format.md first. A cancelled task is no longer recorded as\nfailed.\n\nD-T59-j10. And a `stopped` outcome beside it, for a chat the person\nstopped or closed before it reported. It did not fail by itself, and its\nrecord no longer says it did.\n\nD-T59-j6. The docs said only the person can stop a chat and that no\ncommand or tool does. A chat can cancel a task it dispatched; only the\nperson stops any other chat. The close dialog says what \"Stop them\" now\ndoes.\n\nD-T59-j7. The person's ask rides the wiring's one decision path\n(`dispatch_it` with `By::Person`): the same limits, profile, record and\npolicy locks as a chat's ask, and no grant. The branch's own twin of the\ndecision is gone. A refusal is still said in the person's words, now over\nthe wiring's refusal kinds.\n\nD-T59-j8 (delegated, flagged for the operator, under V98b). The\nsubcommands of issues 1441 and 1442 are pre-allowed for Claude Code each by\nits own name, beside the wiring's four named rules and with no wildcard\nover the rest: `--wait`, `wait`, `list`, `cancel`, `tell`, `note`, `ask`\nand `answer`. ADR 0064 says in one line each what it can reach.\n\nD-T59-j9 (D-1448-4). A chat the person is stopping is still given a\nticket. Its one last report is sent on one, by either report command, and\nboth asked for a ticket that a stopping chat was refused, so the report\nits stop asks for could not be sent. What is refused is spending a ticket\non an open or a dispatch, which is where it is known what the ticket is\nfor. An app test sends one report by each command, a second that is\nrefused, and an open and a dispatch that are refused.\n\nD-T59-j11. The stop's word names its chat by a task's rule when it is\nread, as the app's word on a held dispatch does (D-T59-14): the name is\ndrawn inside purlis's own sentence. The person can name a chat with the\nmarks purlis's lines are made of, so the app writes the name with them\nreplaced, as a stamp replaces the backtick, and the word of a chat so\nnamed still arrives. A report's name is a chat's, as before.\n\nD-T59-j12. The stop's word carries how the chat was started. A stopped\ntask reads \"the task you dispatched to it\", a stopped handoff \"the work\nyou handed to it\".\n\nD-T59-j13. The refusal of a profile that asks nobody (D-T59-13) holds on\nthe person's road, which is the same decision path. A test asks a persona\nwhose definition names such a profile from a chat's tab, and nothing\nstarts.\n\nD-T59-j14. The messages between an asking chat and its tasks wait in\n`said-<chat>/` inside the store D-T59-19 names, so its one rule holds\nthem: denied with the reports where a harness's hooks run outside its\nsandbox, and left reachable where the hooks run inside it, which is where\nthe hook that takes them runs. A test leaves a message and holds its file\nto both halves.\n\nD-T59-j15. Two lock mistakes, both found by reading after a run that did\nnot end.\n- A chat's list took the ledger's lock twice on one thread for a task it\n  dispatched that was waiting on the person, and never came back. It asks\n  the hold it already has. This was the joining commit's own, in j2.\n- A close holds the deciding lock while it ends the chat's program. The\n  end of that program is heard on another thread, which ended a chat still\n  in a stop and took the same lock to do it. So closing the tab of a chat\n  that was being stopped could wait on itself, where the close waits for\n  the program's end to be heard. A close now lets go of the chat's stop\n  before it ends the program, so that thread finds nothing to end. And a\n  close settles what the chat owed whatever became of the word, which is\n  what the end of a program relies on to not wait for the lock.\n\nD-T59-j16. The app tests that take those locks are bounded. An ask after\na task, a stop, a close and a close that stops the chats below are run on\na thread of their own in the tests of this commit, and a call that has not\ncome back in two minutes fails its test with a sentence, where it would\nhave hung the run with nothing said.\n\nTests the first full run found red, each read against the joined code.\nNone was a product fault but the two locks above.\n- The nine pane tests of issues 1441 and 1442 never had a report taken:\n  their stand-in is read as Claude Code, whose reports the board takes\n  only with a process and a conversation (ADR 0024), and the helper sent\n  neither. It sends both for such a chat now. One of them expected a\n  cancelled report from closing a cancelling task's tab; that is the\n  person's stop (D-1443-11, j3), and it says so.\n- A restart was acted out with the person's Close, which tells the asking\n  chat a task was stopped. It takes the restart's own road now.\n- A wait on a task whose tab was closed is answered with what the asking\n  chat was told, which is that it was stopped.\n- A task's report names its chat on the asking chat's row and is no item\n  (D-1448-6), and the report of the person's own task whose tab chat has\n  gone raises its item by a need of the app's (D-1448-8).\n- \"the persona devops\" is not \"the person\": the test of the refusals the\n  person reads compares words.\n\nD-T59-j17. An app's refusal is printed whole wherever the command prints\none, as D-T59-12 did for the dispatch itself: the refusal of a report, by\neither report command, the refusal of a handoff, and the note about a\nprofile that fell back on a handoff. The refusal of a message or of an ask\nafter a task is whole from the commit that added those commands. Each was\ncut at the 160 characters a name is drawn within, which took the last\nsentence, the one that says what to do.\n\nThe pre-allow test of D-T59-18 holds both lists now, exactly and in order,\nand that no rule is a wildcard over the rest.\n\nD-1448-17 holds. A report to an idle asking chat raises no needs-you item\n(`reported_to_its_asker`), and the asking chat is woken by the report\nitself (`dispatched::told`, typed once the lock is let go). Both are on\nthis train.\n\nA tab's menu test from issue 1448 now expects Restart chat above Start\nfresh, which the base train added.\n\nRecorded behaviour gains nine scenarios for the new subcommands that need\nno app (fold-in 10 of the review of issue 1441): what `list`, `wait`,\n`cancel`, `tell`, `answer`, `note` and `ask` say where no app answers, and\n`tell` and `note` refusing an empty message. Written in the recorder's\nformat and run by hand against the built command; the replay holds them.\nNo recorded row that was there moves: no sentence a recorded scenario\nprints is changed here.\n\nBindings are regenerated.\n\nRefs #1438\nRefs #1441\nRefs #1442\nRefs #1443\nRefs #1448\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-08T01:20:08+04:00",
          "tree_id": "3b89c5f71eaead25221b61e2fbe95aa12038876b",
          "url": "https://github.com/purlis/purlis/commit/0c1db31bf0e6dfccaae4592541503a1da29ad82d"
        },
        "date": 1791410722960,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.47187650000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 0.459, 0.471, 0.472, 0.473, 0.476 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.357584,
            "unit": "ms",
            "extra": "median of 5 runs: 16.179, 16.309, 16.358, 16.470, 16.914 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.2169155,
            "unit": "ms",
            "extra": "median of 5 runs: 100.907, 101.415, 102.217, 102.518, 103.029 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "0be46c8ba158d16f9c34d42f474f1db70219169e",
          "message": "Train 61: a handoff is held to the limits of the workspace it moves into (D-T61-7)\n\nReconciled: issue 1453 holds a task sent into another workspace to that\nworkspace's dispatch limits as well as the asking chat's, with a 0 there\nholding against a persona's own value (D-1453-17). A handoff has always named\na workspace, and the limits are the project's own configuration, so the one\ndecision now reads them for a handoff too: the same call, under the same lock,\nfirst refusal wins. The workspace is read by the name its folder has, so a\nspelling that differs only in case reads the same limits.\n\nThis amends D-T61-2 in one point. The rest of it stands: a handoff takes no\n`--in`, is cut no worktree, and the standing grant a chat nobody is at needs\nto cross workspaces stays a task's rule (ruling V99f is a handoff's).\n\nSeen in the core: a handoff into a workspace set to 0 is refused in that\nworkspace's sentence for a chat a person is at and for one nobody is at, a\npersona's value does not lift it, the asking chat's own 0 answers first, and\nwith nothing set it opens. One test in the app holds the same through a real\nhandoff; it starts a chat, so only CI runs it.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-08T08:03:16+04:00",
          "tree_id": "c98619ade7834f1e67fcb6f0aff630bdcc60b955",
          "url": "https://github.com/purlis/purlis/commit/0be46c8ba158d16f9c34d42f474f1db70219169e"
        },
        "date": 1791432289164,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.438983,
            "unit": "ms",
            "extra": "median of 5 runs: 0.419, 0.435, 0.439, 0.452, 0.455 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.480835,
            "unit": "ms",
            "extra": "median of 5 runs: 16.366, 16.418, 16.481, 16.605, 16.615 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.56867700000001,
            "unit": "ms",
            "extra": "median of 5 runs: 102.025, 102.306, 103.569, 103.831, 104.532 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "1a086ba68815cf4367196e72c492be43346c71d1",
          "message": "worktree: a sandboxed chat in a branch folder commits through the app\n\nA linked worktree keeps its index, its HEAD and its objects in its clone's\n`.git`, outside what a sandboxed chat may write, so `git add` and `git\ncommit` are refused there. By the operator's ruling (V99h) the worktree's git\ndirectory is not opened to the chat: purlis brokers the commit.\n\n`purlis worktree commit -m <message> [--all | <path>…]` sends the app an ask\nover the chat's hook socket that names the message and what to stage, and\nnothing else. The app commits in the folder it recorded the chat as standing\nin, on the branch that folder is on, with the repository's identity or the\nperson's and the chat's trailers (ADR 0074). Every git call is held to the\nchecked git directory wherever in the folder the chat stands, reads no\nglobal or system config, and runs no hook. A repository whose config names a\nprogram, has a config of the worktree's own, borrows objects or signs its\ncommits is refused. What was staged is read back: another repository inside\nthe folder, a change to `.gitmodules`, a name that differs from another only\nby case, and a file its attributes mark for a content filter are unstaged\nand refused. purlis's own scan runs in the app. It only commits.\n\nOne commit runs in a branch folder at a time, by its own deadlines. The app\nnotes what it staged in the worktree's git directory, so a commit it was\nstopped in the middle of is unstaged by the next ask.\n\nA dispatched task's own worktree is such a folder, and the chat started in\nit is now told the command where it was told it could not commit.\n\nADR 0067 is amended with the measurement the ticket asked for and the\ndecision. A chat started in a branch folder is told the command. The ticket\nstays open: no linked worktree has been measured under a sandbox's denial.\n\nDecided in implementation:\n- D-1055-1: a plain clone already commits inside the sandbox, by named\n  denials. The same recipe for a worktree was not taken (ADR 0067's note).\n- D-1055-2: the repository's own hooks do not run for a brokered commit; the\n  answer names each one that is there.\n- D-1055-3: the command is `purlis worktree commit`; `--all` is every change\n  to a tracked file, and a new file is named by path.\n- D-1055-7: no approval, and no pre-allow on Claude Code yet: the rule that\n  asks about a pre-allowed command sharing its call is not on main.\n- D-1055-8: the folder is the one the app recorded for the chat, so a chat\n  standing above a branch folder does not commit in it.\n- D-1055-9: staged changes the app did not stage are refused, so what is\n  committed is exactly what the ask named, and a refusal can put the index\n  back.\n- D-1055-10: the repository's own `user.name` and `user.email` win over the\n  person's global ones, as they do for git.\n- D-1055-11: a repository that signs its commits is refused: a signer is a\n  program and may prompt.\n- D-1055-13: a file marked for a content filter is refused, not committed\n  unfiltered: the person's filters are in the config this commit does not\n  read.\n\nTests: the ask's shape and the path rules are pure; the git half runs\nagainst real git on a worktree whose git data is kept beside it; the tests\non purlis's own layout write under a `.git`, so CI is their first run.\n\nRefs #1055\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-08T08:26:52+04:00",
          "tree_id": "757dc9c755910f8a181e06dfaf330e619f1bed54",
          "url": "https://github.com/purlis/purlis/commit/1a086ba68815cf4367196e72c492be43346c71d1"
        },
        "date": 1791433704879,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5309984999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 0.493, 0.507, 0.531, 0.549, 0.549 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.117109499999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.515, 16.589, 17.117, 17.214, 17.343 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.508757,
            "unit": "ms",
            "extra": "median of 5 runs: 103.008, 103.317, 104.509, 104.531, 106.128 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "1d70fbf15c11344b85fbfeedc868afcd4dedabac",
          "message": "A Notice in a pane's corner wraps, and what it opens is drawn under it\n\nReported by the operator from the dev build, 2026-10-08: \"ui is broken for\nquestions modals\". With two panes side by side, a refused vault's Notice had\nits sentence one word wide and fourteen lines tall, its three buttons broken\nover up to five lines beside it, and the block a button opened drawn to the\nright of the Notice and off the window. The dispatch-grant Notice in the other\npane had its brief over the middle of the pane, its buttons far below, and no\nsentence in view. The Notice was drawn over the Ask devops dialog, that\ndialog's \"What to ask\" box was a third of its width with its buttons below the\nwindow, and the band and the sidebar were cut at the left.\n\nCause. `Notice at=\"pane\"` was made for one short line (ADR 0062). `.notice-pane`\nwas one flex row that did not wrap, and the pane's corner was itself one row\nthat did not wrap, so a Notice's `under` was the next ITEM of the corner's row:\nbeside the Notice. Nothing bounded any of it by the pane. A button that\noverflowed was then focused, and the window (`overflow: hidden`, which a focus\ncan still scroll) was scrolled sideways to show it: that is what cut the band\nand the sidebar. The corner's `z-index: 2` sat in the page's root stacking\ncontext, where a dialog portaled to the body has no number, so the corner was\nover every dialog.\n\nWhat changed, as properties of the shared pane Notice and the pane's corner:\n\n- `Notice at=\"pane\"` draws one box (`notice-pane-box`): the line, and under it\n  what a way out opened. The band and the drawer are drawn as before.\n- The corner is a column: the chat at a glance (`pane-chips`), then the pane's\n  Notices one under another (`pane-notices`), never wider or taller than the\n  pane, scrolling when together they are taller. The pane's frame clips.\n- The sentence has the row. The ways out stand beside it only when all of it\n  fits unwrapped, and otherwise go under it; a button is not broken unless it\n  alone is wider than the Notice.\n- `#root` is one stacking context, so nothing in the window is over a dialog\n  or a menu. The terminal is one too, so its own layers stay under the corner.\n- Ask {persona}: the box you write in takes the row, the dialog fits the\n  window, and its answers stay at its bottom edge.\n\nBehaviour, the answer to \"is dispatching a manual job?\": it is the chat's own.\nThe chat had dispatched already and its request was held for the person; the\nbroken layout hid it, and the vault's Notice offered a second, manual, empty\nway beside it.\n\n- The dispatch-grant Notice is the pane's first, and reads sentence, answers,\n  then the brief in a box of about eight lines.\n- A refused vault whose persona this chat has already asked says so and offers\n  Show the request, which goes to that Notice, in place of Dispatch to\n  {persona}…. With nothing held, Dispatch to {persona}… is offered as before.\n- Ask {persona} opened from a chat's Notice says the chat's words are not\n  copied into it. It is still empty: nothing a chat produced is typed for the\n  person.\n\nDecided in implementation:\n\n- D-PN-1 The line and what it opened are one bordered box in a pane, and stay\n  siblings under the strip and in the drawer, where the band moves them.\n- D-PN-2 The sentence is as wide as it reads (`flex: 0 1 auto`) with no fixed\n  minimum: a minimum would widen a short one-liner, and without one the\n  sentence is never narrower than the Notice unless everything fits on a row.\n- D-PN-3 A way out is `flex: none; max-width: 100%` and wraps normally, not\n  `white-space: nowrap`: it is never broken while it fits, and wraps instead\n  of being cut off when it alone is wider than the Notice.\n- D-PN-4 40rem is the cap, on the stack. A Notice that has opened something\n  takes that width whole.\n- D-PN-5 `overflow: clip` on the pane's frame, not `hidden`: a hidden box can\n  be scrolled by a focus, which is the defect.\n- D-PN-6 The corner takes no pointer events of its own; its two rows do. It\n  can be as wide as the pane now, and the terminal is under the empty part.\n- D-PN-7 The stacking order is by `isolation: isolate` on `#root` and on the\n  terminal's holder. The stylesheet has no layer scale, and this adds no\n  number to it.\n- D-PN-8 The order in a pane is written, not ranked: the Notice that waits for\n  an answer before anything starts is first, the rest as they were.\n- D-PN-9 No label is shortened. \"Allow steward to use this vault\" is a verb\n  and what it acts on, as docs/ui-copy.md asks, and it now fits a row.\n- D-PN-10 The Notice's sentence stays purlis's own words. The chat's name for\n  its task is the chat's text, and is not put into it.\n- D-PN-11 Show the request puts the focus on the request's line, never on one\n  of its Allow buttons, so the next key cannot answer it.\n- D-PN-12 Both Notices read the held dispatches from one store\n  (`dispatchesHeld.ts`), so neither says one is waiting after it is answered.\n- D-PN-13 The persona's mark is the sentence's first word in a pane, so it is\n  not left alone on a row above a sentence that takes the next one whole.\n- D-PN-14 The dialog's rules are scoped to Ask {persona} (`ask-persona`), not\n  to every dialog or every settings row.\n- D-PN-15 The e2e case draws the shapes the components draw into real panes\n  and measures the built stylesheet: the suite's fake harness cannot make a\n  vault refusal or a held dispatch. Vitest holds the components to the shapes.\n\nTests: `Notice.pane.test.tsx` (the shape, and the rules as written),\n`VaultRefusedNotice.test.tsx` and `AskPersona.window.test.tsx` (the pointer,\nthe order, the dialog's line), `pane-notices.e2e.ts` (the measuring).\n\nCloses #1481\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-08T10:08:34+04:00",
          "tree_id": "305a9ac7f6bc1b56342dc504b8097acde367b5bd",
          "url": "https://github.com/purlis/purlis/commit/1d70fbf15c11344b85fbfeedc868afcd4dedabac"
        },
        "date": 1791439813592,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.6147245,
            "unit": "ms",
            "extra": "median of 5 runs: 0.595, 0.604, 0.615, 0.615, 0.617 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.468757,
            "unit": "ms",
            "extra": "median of 5 runs: 16.072, 16.217, 16.469, 16.568, 16.599 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.4769505,
            "unit": "ms",
            "extra": "median of 5 runs: 101.449, 101.615, 102.477, 102.526, 103.143 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "7d7b166168fc041df8947b1e7e25ec4ea1cbe777",
          "message": "The app's move test writes the keyring stub where its fixture's project is\n\nThe 1Password fixture's project moved to a folder of its own beside the\nstand-in `op`, and one test still wrote the keyring stub under the old root,\na folder that no longer exists.\n\nIt now writes through the fixture's own state folder (`ctx.state`), so the\npath follows the project wherever the fixture puts it.\n\nRefs #1516\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-08T19:55:02+04:00",
          "tree_id": "c8601348b7bdea247ec17c95aae9e2a2336bfc2c",
          "url": "https://github.com/purlis/purlis/commit/7d7b166168fc041df8947b1e7e25ec4ea1cbe777"
        },
        "date": 1791475728559,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.53555,
            "unit": "ms",
            "extra": "median of 5 runs: 0.519, 0.528, 0.536, 0.547, 0.548 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.2565885,
            "unit": "ms",
            "extra": "median of 5 runs: 16.174, 16.207, 16.257, 17.092, 17.235 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.9442165,
            "unit": "ms",
            "extra": "median of 5 runs: 100.982, 101.670, 102.944, 105.768, 107.019 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "918ce402c5d90ddc3d5dbbe3d759d85c520b2d5b",
          "message": "Where the tickets of this train meet: four seams the review found, and six smaller ones\n\nThe review of the joining work found four things wrong where two tickets meet,\nnone of them inside one ticket.\n\nA pane left on an ended task could draw another task's report. The finished row\nwas found by the asking chat and the task's name, and a chat dispatches again\nunder one name as a matter of course. FinishedTask now carries the number its\ntask's chat had when it ended in this launch (the ledger's memory of closed\ntasks says whether the number is still its own), and the pane finds its row by\nthat number and never by the name. A row with no number is no pane's.\n\nA session folded by itself over an open task that reads failed. A blocked task\nis no longer ended at its report and stays an open chat, and the list's own fold\ntook every end for over. Only an open chat that reads done or cancelled lets its\nsession fold now; one that failed, went without a report or only reported keeps\nit open, as a finished row that stands alone does. A folded row counts the open\nchats under it that are over with its finished tasks, so a done task the person\nis still reading is in a count.\n\nA task the person was looking at when its bound passed was never ended on a\nharness purlis hears nothing from, or where the reporting turn hangs: the bound\nwas spent, and nothing looked again once they looked away. When the hold goes\nand that turn is still not over, the bound is set again (Ends::Bound). Only for\nthe reporting turn: a later turn still has none.\n\nAfter a relaunch the core could go on taking a tab to show a task that was not\nput back, so the session on screen was notified about each time it needed the\nperson. A launch that leaves a finished task out takes `shows` off the tab that\nshowed it, and the window, which starts from what the core said each tab shows,\nsays \"its own chat\" for whatever it did not put back. docs/plane-format.md now\nsays what turns on `shows`.\n\nSmaller, in the same files. A look that found a task held re-reads once, so a\nmove that landed between the read and the answer is not lost. What a tab shows\nis said again where the core did not take it. A task's row stays in the Chats\nlist until the finished row that replaces it is read, so the rows below move\nonce. A comment that said a chat's number is dealt again at each launch says\nwhat is true. The three tests of the hold were seen failing against the old\ncheck.\n\nFrom the first CI run of the train, which was cancelled with its Rust job hung.\n\nThe hang was a test, and a drop with no bound under it. The test of \"a task\nthat had reported at a quit is a finished row at the next launch\" opened the\nsame project a second time in one process while the first was still open. Each\nopen binds the project's hook socket at one path, and a listener that is\ndropped wakes its own thread by connecting to that path: the first one's\nconnection reached the second listener, or nothing once that had gone, and its\njoin waited for good. Only where a socket can be bound, so not in the sandbox\nthe test was written in. The test now closes the app that quit before the next\none opens, under a bound. And a dropped listener waits five seconds for its\nthread and then leaves it, saying so: nothing that closes a project can be held\nfor good by it.\n\nThree older tests held expectations the tickets had changed and could not run\nwhere they were written. The refusal of a second report after a handoff now\nends \"Another answer needs a task\" (issue 1515's wording). A task whose report\nreaches its open asking chat is answered Finished, not Reported. A cancel of\nthe chat's own closed task says it has finished and that there is nothing to\ncancel; another chat's is still refused as not its own.\n\nThe breadcrumb did not shrink in a real engine: as an item of its line it was\nnever narrower than its own content, so the line clipped its end and no name\ngave way. It has `min-width: 0` now. The chats-list scenario looked for a row\nwith an empty second line, and every row's second line is always there and\nalways has something in the recorded markup: it now changes every second line,\nadding to some and emptying the rest, and checks that no row moves.\n\nThe Rust job in ci.yml has `timeout-minutes: 45`. It takes 19 on main; a hung\ntest now fails the job instead of holding a runner.\n\nRefs #1485, #1486, #1499\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-08T21:54:41+04:00",
          "tree_id": "f2ed9a2046e1691ff4a8ffc7df1cb0695ae9ed8d",
          "url": "https://github.com/purlis/purlis/commit/918ce402c5d90ddc3d5dbbe3d759d85c520b2d5b"
        },
        "date": 1791482570113,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5337485,
            "unit": "ms",
            "extra": "median of 5 runs: 0.521, 0.523, 0.534, 0.563, 0.567 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.973883999999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.763, 16.921, 16.974, 17.036, 17.112 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.47006049999999,
            "unit": "ms",
            "extra": "median of 5 runs: 102.406, 102.789, 104.470, 104.620, 106.354 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "290ad5650ab4776cccedcfc967b6142da3dd4525",
          "message": "A token is stored for the one vault whose tab it was put in from\n\nReview round for the vault tab's token box.\n\n- A store marks the vault the person is in and no other. Marking every\n  vault read through the same variable is taken out: a vault the\n  committed registry names is never given a token the person did not\n  put in from its own tab. The tab names the other vaults that still\n  have none, each a link to its own tab: a pointer, never a write.\n- A token stored again replaces the old one: the previous record's\n  keyring items are deleted once the new record is saved. A store that\n  fails part way deletes what it had stored and leaves the record as it\n  was.\n- A kept token that does not read the vault is no longer blamed for\n  every failure. The answer says what kind of failure it was, and the\n  tab says to replace the token only for a refused sign-in; a missing or\n  moved program and a network that is down say what they are. The box\n  stays in every case, with Read again beside it.\n- A vault read through several variables draws no box that every press\n  would be refused.\n- A read that fails after a store is never shown beside \"Stored\".\n- An open vault tab reads again when a token is stored in another.\n- An unreadable vault's health line is its refusal, not another run of\n  the provider.\n- The doctor row's hint is true on a machine with no window, the row is\n  not printed inside a chat, and it says \"marked as kept\".\n- The test that no answer carries the token now runs a provider\n  stand-in that prints the token it was handed.\n\nDecided in implementation: D-1526-7 (replaces D-1526-3) to D-1526-11.\n\nRefs #1526\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T00:01:06+04:00",
          "tree_id": "cde10f5196fbb0c4a57f180d50641d436c3b41d4",
          "url": "https://github.com/purlis/purlis/commit/290ad5650ab4776cccedcfc967b6142da3dd4525"
        },
        "date": 1791490254888,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5967985,
            "unit": "ms",
            "extra": "median of 5 runs: 0.594, 0.594, 0.597, 0.610, 0.622 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.077468000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 15.989, 16.052, 16.077, 16.155, 16.522 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.872724,
            "unit": "ms",
            "extra": "median of 5 runs: 101.068, 101.271, 101.873, 101.882, 102.175 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "1bab0fda317e95830c4dce7088309e48801933ad",
          "message": "Evidence-only jobs run nightly only, and the tests are built in a step of their own\n\nfedora, windows and the preview-image builds ran on every push to main and\nagain nightly (V70). fedora alone took 20 minutes, so main was 28 minutes\nbehind a merge whose gating jobs finished in 18, and each merge held four more\nrunners. They gate nothing, so they now run on the nightly schedule only. A\nbreak on one of those platforms is reported the next night.\n\nThe rust job's cargo test step was 916 s of an 18 minute job with nothing to\nsay how much was compiling and linking 208 test binaries and how much was\nrunning them. Building with --no-run first puts that split in the step\ntimings of every run.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4EJSYV68AZG1SWR231JAEZE\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T01:08:38+04:00",
          "tree_id": "090b5f49a8b5fd1ff6dca10c22393ca63a8bf3b1",
          "url": "https://github.com/purlis/purlis/commit/1bab0fda317e95830c4dce7088309e48801933ad"
        },
        "date": 1791494496168,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.479823,
            "unit": "ms",
            "extra": "median of 5 runs: 0.479, 0.480, 0.480, 0.486, 0.493 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.2989405,
            "unit": "ms",
            "extra": "median of 5 runs: 16.183, 16.240, 16.299, 16.305, 16.401 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.494464,
            "unit": "ms",
            "extra": "median of 5 runs: 101.089, 101.415, 101.494, 101.739, 101.771 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "a48646b07353bfdaad39b5b14e2f34ee826b0596",
          "message": "A dev build carries line tables, not full debug info\n\nA cargo test links over two hundred test binaries, each with the full debug\ninfo of every crate it links. That is most of what linking them costs and most\nof what a target directory weighs. Line tables keep file and line in a panic's\nbacktrace; a debugger session asks for variables with\nCARGO_PROFILE_DEV_DEBUG=full.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4EJSYV68AZG1SWR231JAEZE\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T01:36:58+04:00",
          "tree_id": "5875d3a78768b8787bf6962c75bef02947617e6b",
          "url": "https://github.com/purlis/purlis/commit/a48646b07353bfdaad39b5b14e2f34ee826b0596"
        },
        "date": 1791497735781,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.2750995,
            "unit": "ms",
            "extra": "median of 5 runs: 0.265, 0.267, 0.275, 0.276, 0.278 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.506196000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 16.458, 16.483, 16.506, 16.507, 16.601 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.68803150000001,
            "unit": "ms",
            "extra": "median of 5 runs: 100.488, 101.384, 101.688, 102.006, 102.565 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "50b34380602556a0601af3a5b95846d9aeab9b08",
          "message": "The rust job runs the tests with cargo-nextest\n\ncargo test runs the workspace's test binaries one after another, so the run\nstep took 690 to 715 s on a job whose build step took 333 s. nextest runs every\nbinary's tests at once, each test in a process of its own. The floods of\nunread_answers, which a static lock ran one at a time, keep that order through\na nextest test group. nextest does not run doctests, so cargo test --doc does.\n\nLocally nothing changes: cargo test --workspace runs the same tests.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4EJSYV68AZG1SWR231JAEZE\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T02:39:32+04:00",
          "tree_id": "25579888adb3851fc3e3f42877d7688b36143ab7",
          "url": "https://github.com/purlis/purlis/commit/50b34380602556a0601af3a5b95846d9aeab9b08"
        },
        "date": 1791500304908,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.530486,
            "unit": "ms",
            "extra": "median of 5 runs: 0.519, 0.529, 0.530, 0.531, 0.545 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.919734000000002,
            "unit": "ms",
            "extra": "median of 5 runs: 16.359, 16.847, 16.920, 16.955, 17.434 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.59214499999999,
            "unit": "ms",
            "extra": "median of 5 runs: 102.017, 103.469, 104.592, 105.076, 105.156 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "032d67c5911fa616accaaeb7f28d1cc94b3aff85",
          "message": "A vault only the committed half declares is given no token from a terminal\n\nThe re-review of the guided 1Password set-up found that `--token-stdin` on a vault only\nthe committed vaults.json declares made a keyring record that pinned settings a commit\nchose, which nobody was shown.\n\n- Such a vault is refused before anything is read. Its record is made in its tab in the\n  app, which shows its settings first. A vault this machine's half declares still has only\n  its token changed.\n- The token-missing sentence and doctor print the terminal command only where it works,\n  and point to the vault's tab otherwise.\n- A held set-up's limit is checked on the wall clock as well as the monotonic clock, so it\n  never outlives fifteen minutes of real time across a sleep. Its timer now starts as the\n  set-up is held.\n\nRefs #1527\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T02:52:34+04:00",
          "tree_id": "3ec0c97a699d364c93be1d8fc37a859c1c4f80c1",
          "url": "https://github.com/purlis/purlis/commit/032d67c5911fa616accaaeb7f28d1cc94b3aff85"
        },
        "date": 1791500754648,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.51508,
            "unit": "ms",
            "extra": "median of 5 runs: 0.482, 0.509, 0.515, 0.540, 0.557 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.507523000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 16.455, 16.473, 16.508, 16.647, 17.066 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.12326850000001,
            "unit": "ms",
            "extra": "median of 5 runs: 102.609, 102.807, 103.123, 103.399, 103.415 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "7b48cac1007caeedd17ddaab9e617934600c21eb",
          "message": "Only main saves a Rust cache\n\nThe repository's Actions caches held 12 GB against GitHub's 10 GB limit on\n2026-10-08: each pull request saved its own 0.8 to 1.2 GB Rust caches, and they\npushed main's out. The run on main after #1541 restored nothing and took 1289 s\nwhere the warm one before it took 914 s. With save-if, main saves and a pull\nrequest restores main's cache and saves none, as rust-cache's README advises.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4EJSYV68AZG1SWR231JAEZE\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T03:30:20+04:00",
          "tree_id": "1f996784336313db6ce34760e2e83362990d985c",
          "url": "https://github.com/purlis/purlis/commit/7b48cac1007caeedd17ddaab9e617934600c21eb"
        },
        "date": 1791502417520,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.493039,
            "unit": "ms",
            "extra": "median of 5 runs: 0.488, 0.491, 0.493, 0.508, 0.513 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.075015,
            "unit": "ms",
            "extra": "median of 5 runs: 16.541, 16.807, 17.075, 17.088, 17.131 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.899193,
            "unit": "ms",
            "extra": "median of 5 runs: 102.403, 103.847, 103.899, 105.534, 106.429 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "92a311251dbd5037ea8c1c753eb6de686231a4e3",
          "message": "Where the tickets of train 65 meet\n\nThe tab chip's menu, the breadcrumb line and the Chats list each meet\nmore than one ticket of this train. What is joined here:\n\n- J1. A working task's line in its tab's menu says what it is doing\n  (#1493's ChatDoingLine, one dim line cut short after how long; nothing\n  while it does not work, not even its gap). It reads its own chat, so no\n  line of the menu is drawn again for it.\n- J2. Brief from the tab's menu (#1494): a button at the end of a task's\n  line, beside the ways to move it, for an open task by its chat and for\n  an ended one by its dispatch record while it has a finished row. The\n  session's own chat has none. Opening it closes the menu.\n- J3. The session's Activity from the tab's menu (#1495): a line after\n  \"End a task\" opens the tab's own chat's Activity in a tab of its own,\n  as the catalogue's tab.activity row does.\n- J4. Fail closed for reads of what chats said: `activity` (a session's\n  timeline) and `task_question` (the question a task is paused on) join\n  `task_brief` on WINDOW_ONLY. Each reads words a chat wrote, about a\n  session the caller names, and no link can say which chat is asking.\n  Session-protocol and app tests pin both; the list is 35 names.\n\nTests: three window cases in TabChip.window.test.tsx for J1 to J3; the\nsession protocol refuses both reads on a link and the app's link client\nhas neither. The menu's line helper and the level test now leave out the\nActivity line, as they leave out \"End a task\".\n\nRefs #1493, #1494, #1495, #1496, #1483\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T04:25:01+04:00",
          "tree_id": "f75888a20498c2e0b17fd4025eefc7dff547d6e4",
          "url": "https://github.com/purlis/purlis/commit/92a311251dbd5037ea8c1c753eb6de686231a4e3"
        },
        "date": 1791505726312,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.460195,
            "unit": "ms",
            "extra": "median of 5 runs: 0.444, 0.456, 0.460, 0.467, 0.503 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.681836500000003,
            "unit": "ms",
            "extra": "median of 5 runs: 16.569, 16.659, 16.682, 16.782, 16.853 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.66681500000001,
            "unit": "ms",
            "extra": "median of 5 runs: 103.357, 103.438, 103.667, 103.675, 103.947 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "1d7cb8d09317f717972e0c3405f49a81aecfb27b",
          "message": "While you were away: one summary of what happened\n\nWhen the person comes back after five minutes or more away from the\nwindow, one Notice under each project's strip sums up what happened\nmeanwhile: \"While you were away: 7 tasks done, 1 failed, 2 waiting on\nyou\". Away is any of three signals: the window not in use (blur to\nfocus), hidden (visibilitychange), or left in front with no key, pointer\nor wheel input, from the last input to the first one after. A window that\nstarts without the focus is away from its start.\n\nEach project keeps its own times away whether or not it is in front: the\nproject's view holds them above where it stops drawing a project behind\nanother, so switching to it shows its summary, and switching back is no\nDismiss.\n\nOnly what happened while away is counted: a finished task by the time\nits record says it ended, in the count every surface puts it in\n(finishedBucketOf); a chat as waiting when it came into the needs-you\nqueue while away and is still there. A chat there only for tasks that\ncame to nothing (every reason it has is one failure's sentence) is\ncounted once, as the failure.\n\nEach part is a link. One item goes straight to it, named in the link's\naccessible name; several open a list. A chat waiting on you is shown\nwhere it lives, where its question or Notice is answered as before. A\nfinished task goes where a needs-you item's Go takes it, through the one\nhelper both now share: its chat if open, else its finished row, else the\nsession that asked, saying why. Going to a failed task marks that\nfailure looked at, as the Go does; that is the one thing a press writes.\nDismiss puts the summary away and nothing else: needs-you items,\nquestion Notices, failed rows and Activity lines all stay.\n\nA summary whose parts have all gone lets its times go, and so does one\nthat still counts nothing ten seconds after the return, so no record\nread later brings an old time back; a reload starts with none.\n\nSettings, You, Chats list: \"Away summary\", on by default, kept as\n`chats.away` in the layout file beside the other Chats list choices;\n`chats` is now described as how chats are listed and summed up. Off,\nnothing is watched and the window is as before.\n\napp/src/awayCounts.ts holds what is counted and said (plain inputs);\napp/src/AwaySummary.tsx watches the window (useTimeAway), keeps each\nproject's summary (useAwaySummary) and draws the Notice. A later source\nof things that happened while away joins as another part there.\n\nDocs: CONTEXT.md (Away summary), docs/plane-format.md and\ndocs/design-system.md (`chats.away`), changes/while-you-were-away.md.\n\nCloses #1514\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T04:56:23+04:00",
          "tree_id": "0b5fa39584503b86eea39c8d9ee5eed6f4ac7e92",
          "url": "https://github.com/purlis/purlis/commit/1d7cb8d09317f717972e0c3405f49a81aecfb27b"
        },
        "date": 1791507460210,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.28124299999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 0.272, 0.275, 0.281, 0.295, 0.301 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.563947,
            "unit": "ms",
            "extra": "median of 5 runs: 16.535, 16.537, 16.564, 16.605, 16.686 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.3495805,
            "unit": "ms",
            "extra": "median of 5 runs: 101.568, 102.200, 102.350, 102.574, 102.689 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "dc13fc2175c24d13ea72b08b5f8a2e8346e40402",
          "message": "dispatch: the loop rule reads the chain from the dispatch record, not from the chats still open\n\nA task's record of open chats now keeps the personas above it\n(chats[].from.above), written by the app when it starts the task from its\nown record of the asking chat. The loop rule and the person's never for a\nchat above read it there, so a chain still holds when a chat in the middle\nof it has finished, closed or been cleared. Nothing a chat sends goes into\nit. A finished task reopened as an ordinary chat names no asking chat and\nstarts a new chain.\n\nA record written before the key is read from the chats still open, as\nbefore. Where that walk meets a chat that has closed (or a loop), the chain\nis unread and fails closed: any dispatch to another persona is refused,\nand one to its own where the person said never to any persona dispatching\nthere, with a sentence saying an older version kept no record of the\nchain. A kept chain whose length is not the depth, or that names what\ncannot be a persona, reads as not kept.\n\nDocs: plane-format (chats[].from.above), handoff refusal table, ADR 0090\namendment item 18. Changelog fragment.\n\nReview round 1: a chain is read under the decision's lock, and an older chain's refusal says where it began\n\nReview round for the loop rule reading the chain from the dispatch record.\n\n- The asking chat is read from the chats open under the lock the decision\n  is made under, and one that has closed or ended by then is refused\n  (`Refused::NotOpen`). `asked_by_a_chat` no longer takes an earlier read\n  of it, so a chain read short is never written into a new chat.\n- The refusal for a chain that cannot be read whole says the chat's chain\n  began under an older version, which is true for the chats this build\n  dispatched from such a chain too (core and app sentences).\n- A pretend-host test asserts the handoff road writes the chain.\n- ADR 0090's still-open line and the changes fragment point at the\n  follow-up checklist (#1548) and name any chat, not only a task.\n\nCloses #1521\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T05:34:49+04:00",
          "tree_id": "3e5fb081925dc3eba0b5c82c4e198d7f874eccae",
          "url": "https://github.com/purlis/purlis/commit/dc13fc2175c24d13ea72b08b5f8a2e8346e40402"
        },
        "date": 1791509917498,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4710215,
            "unit": "ms",
            "extra": "median of 5 runs: 0.454, 0.469, 0.471, 0.473, 0.480 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.2950275,
            "unit": "ms",
            "extra": "median of 5 runs: 16.166, 16.225, 16.295, 16.339, 16.608 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.4872925,
            "unit": "ms",
            "extra": "median of 5 runs: 101.258, 101.427, 101.487, 101.504, 101.900 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "70a07de72d1aecb1d8c9971f62076cb6220732d3",
          "message": "Limits review: the clock looks when chats are put back; the footer hides only what it says\n\nFix round for the review of the #1540 and #1545 follow-ups.\n\n- M1: the end of Held::reopen tells the clock, through a channel it\n  waits on with recv_timeout until its next 30-second round. A session\n  past its token limit says so as soon as a launch puts its chats back\n  (after the reopen answer), not up to 30 s later. The 2 s poll is\n  gone. Test: the row shows right after put-back (red without the hook).\n- M2: `own` is set only where tasks alone are counted, and the window\n  leaves the row's words off the footer only where the footer's own\n  numbers show that limit (same number, as many running). A chat nobody\n  is at, and a held chat or one on the default persona, see the limit\n  said. Rust and window tests for both.\n- F1: the store is read with the clock's lock let go.\n- F2: an ended record is kept as the six fields the clock reads.\n- F3: what the clock kept for a closed project is let go of each round.\n- F4: the store's names are a HashSet.\n\nRefs #1540\nRefs #1545\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T05:43:59+04:00",
          "tree_id": "4f3cbb237925cfc016a69292e67d7db92cf6d831",
          "url": "https://github.com/purlis/purlis/commit/70a07de72d1aecb1d8c9971f62076cb6220732d3"
        },
        "date": 1791510779470,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5210535000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.503, 0.520, 0.521, 0.523, 0.542 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.118962,
            "unit": "ms",
            "extra": "median of 5 runs: 16.522, 16.609, 17.119, 17.303, 17.560 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.6947505,
            "unit": "ms",
            "extra": "median of 5 runs: 102.694, 103.770, 104.695, 105.020, 106.108 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "13b16012b45ccd2e67edc35a274ae918db61c29e",
          "message": "ChatDoing SC-3 test: give it the heavy window tests' 20 s limit\n\nThe fifty-working-tasks test ran past vitest's 5 s default on CI (5047 ms\nand 5154 ms, main dc13fc21) while the whole suite ran in parallel on a\nloaded runner; alone it takes about a second. What it checks is draws:\nonly the changed chat's second line redraws, no row does, counted through\nthe mocked useDoingSaid and chatDoingId. It never asserted a time budget,\nso the wall-clock limit was incidental. It now carries the same\n{ timeout: 20_000 } as the heavy window tests in MemoryLists and Notices,\nwith a comment saying why. No assertion changed.\n\nRefs #1551\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T06:18:33+04:00",
          "tree_id": "9184630f5c8b2e94c83fcf06205e79b86ec7b229",
          "url": "https://github.com/purlis/purlis/commit/13b16012b45ccd2e67edc35a274ae918db61c29e"
        },
        "date": 1791512392710,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5310145,
            "unit": "ms",
            "extra": "median of 5 runs: 0.525, 0.528, 0.531, 0.531, 0.542 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.0032675,
            "unit": "ms",
            "extra": "median of 5 runs: 16.542, 16.839, 17.003, 17.041, 17.042 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.29757899999998,
            "unit": "ms",
            "extra": "median of 5 runs: 102.115, 103.986, 104.298, 105.028, 105.541 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "048f82058a5412f22690b98b58ab50558b801364",
          "message": "Brokered git says when your git config would have mattered\n\nA clone or worktree the app makes for a sandboxed chat reads none of the\nperson's git config, by design (D-1335-7), and the difference from the terminal\nwas silent. `repocmd::unread` now says so where it shows:\n\n- a checkout whose top `.gitattributes` names a filter says it was checked out\n  as stored; for Git LFS, that the files are pointer files and `git lfs pull`\n  fetches them. Both a brokered clone and a brokered worktree add say it. The\n  file is read as `HEAD` commits it, through the hardened runner: a regular\n  file of at most 64 KiB, never a link and never the working tree the chat\n  writes;\n- a brokered clone that fails reads the person's global config files, through\n  the hardened runner with `--file` and no includes, only to see whether a\n  proxy, certificate, client certificate, cookie, extra-header or\n  `url.<base>.insteadOf` key would have changed the clone's route, and names\n  the key, never its value, with the terminal as the way round. Nothing read is\n  applied.\n\nThe CLI test that compares a brokered run with the terminal's now expects the\none filter note on the brokered side. `docs/plane-format.md` explains the\ndifference beside the git config purlis sets.\n\nThe `unread` fixtures commit with no global or system git config\n(`testgit::run_unconfigured`), so a machine whose config defines the\nGit LFS filter does not commit their `.gitattributes` as a pointer.\n\nCloses #1413\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T06:32:31+04:00",
          "tree_id": "0cf607ddd85641b701b1f5b6a498b0a6a6cbf07a",
          "url": "https://github.com/purlis/purlis/commit/048f82058a5412f22690b98b58ab50558b801364"
        },
        "date": 1791513711038,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5079605,
            "unit": "ms",
            "extra": "median of 5 runs: 0.492, 0.496, 0.508, 0.511, 0.535 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.852973,
            "unit": "ms",
            "extra": "median of 5 runs: 16.479, 16.782, 16.853, 16.996, 17.022 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.9661345,
            "unit": "ms",
            "extra": "median of 5 runs: 103.477, 103.691, 103.966, 104.117, 106.109 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "c51ca80f3c4cef5bf770a0585bae25e262a6bb91",
          "message": "changes: what a chat's Activity no longer keeps, and what it says it leaves out\n\nRefs #1520\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T06:53:10+04:00",
          "tree_id": "e958a4c705e7b8bec846195f38399a01c1a6b37b",
          "url": "https://github.com/purlis/purlis/commit/c51ca80f3c4cef5bf770a0585bae25e262a6bb91"
        },
        "date": 1791515510135,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.46394250000000004,
            "unit": "ms",
            "extra": "median of 5 runs: 0.454, 0.462, 0.464, 0.467, 0.474 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.8169075,
            "unit": "ms",
            "extra": "median of 5 runs: 16.778, 16.812, 16.817, 16.825, 16.853 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.73828,
            "unit": "ms",
            "extra": "median of 5 runs: 101.328, 101.684, 101.738, 101.895, 102.288 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "e914aaf28e242b77c312c93dbd59de589debba94",
          "message": "Where the tickets of train 66 meet: the person's Stop reads the one notion of a prompt\n\nTrain 63's stop engine decided on the board's `asking` alone, while the\nwindow's question and the asking chat's cancel read #1525's prompt rule.\nThe two disagreed both ways: after a prompt answered in the window, Stop\nand get its report interrupted the task and then ended it with no report;\nand with a permission ask open in the window and no Notification heard, a\nstop of the chat above it, or Stop them as its asker closed, interrupted\nthe task and typed its last-turn line into it.\n\nThe engine's facts now carry `ask_open`, and its three rules (the last\nturn, whether an interrupted task takes its line, and when a queued line\nis sent) read `Facts::prompt_showing`, which is\n`dispatched::Seen::prompt_showing`: an ask open in the window is a prompt\nin front of the person, and one answered there is not. A task with one\nopen is ended as it stands and nothing is typed into it; a task whose\nprompt was answered is asked for its report. Tested both ways, by the\ntask's own stop and by its asker's stop with what is below it. The\nasking chat's list and wait read the same rule for \"waiting on the\nperson\".\n\nStop all tasks (#1498) and purlis's stop at a time limit (#1512) press the\nsame stop with each task's own facts, so they read the same rule: a task\nwith an ask open is ended as it stands, one whose prompt was answered is\nasked for its report (tested on one press of both).\n\nThe Activity tab's restore test (#1495) now expects the view tab the person\nleft in front to come back in front, as #1525's restore does.\n\nThe typed smart close's test waits for the window's Closed step as it\nwaits for the tab to go: the chat leaves the open list before its close\nfinishes and tells the window, and the test read the log in between.\n\nReview fold-ins:\n- The UI RPC guard names the persona-hosts Allow once, beside the rule's\n  29, so the two lists make the 34.\n- A #1519 test is named for what it asserts: closed by the person.\n- The restart docs and the changelog name the task row's Try to start\n  again and End task.\n- A test pins that a task put back after a refused End task still carries\n  the instruction to carry on.\n\nRefs #1525, #1488, #1513, #1519, #1362\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T08:09:58+04:00",
          "tree_id": "e69de44defed26c372ae0810a72724eb5028f5f5",
          "url": "https://github.com/purlis/purlis/commit/e914aaf28e242b77c312c93dbd59de589debba94"
        },
        "date": 1791519081159,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5381085,
            "unit": "ms",
            "extra": "median of 5 runs: 0.503, 0.522, 0.538, 0.541, 0.553 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.102849499999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.900, 17.084, 17.103, 17.147, 17.659 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.2423445,
            "unit": "ms",
            "extra": "median of 5 runs: 104.219, 104.625, 105.242, 105.274, 106.830 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "382f2d649f83856f63a08a1fca284ab850915efc",
          "message": "review of #1551: a new wait is a chat coming into waiting; the Answer dialog holds the keyboard\n\nThe review's must-fix and five fold-ins (Refs #1551):\n\n- M1. \"Waiting on you\" no longer keys a new wait on the board's movedAt\n  stamp, which also moves for what only touches a chat (a task's failure,\n  a sub-agent ending, a report coming back). The window now notes, from\n  the chats store's own changes, each chat that comes into `waiting`, in\n  order, and a chat already in the queue counts only if it came into\n  waiting after the person left, or its item gives a new reason other than\n  one of its tasks' failures (those count as failed). Tests: a waiting\n  chat whose task fails while away (unit and window: \"1 task failed\"\n  alone), and a waiting chat whose sub-agent ends while away.\n- The Answer dialog focuses itself as it opens, so the keyboard is in it\n  in every state: while the question is read, and where there is none.\n- The refused-dispatches part is one link whatever its count, named with\n  the pairs it counts (a menu of identical destinations was a menu for\n  nothing, and could close the list it opened).\n- Opening the needs-you list from the summary reads it again, as a press\n  on the hand does.\n- Tests: the core refusing a stale answer (its sentence, the text kept,\n  the dialog open), the keyboard back on the task's row after Cancel, no\n  question to answer, two refused dispatches.\n- The SC-3 render counter says why only the row may call chatDoingId, and\n  the line says why it spells the id out.\n\nRefs #1551\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T08:39:42+04:00",
          "tree_id": "53bd0549bfb506e4770696962e7bc52ece781834",
          "url": "https://github.com/purlis/purlis/commit/382f2d649f83856f63a08a1fca284ab850915efc"
        },
        "date": 1791521011555,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.6146475,
            "unit": "ms",
            "extra": "median of 5 runs: 0.608, 0.610, 0.615, 0.616, 0.617 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.203035,
            "unit": "ms",
            "extra": "median of 5 runs: 16.085, 16.113, 16.203, 16.572, 16.641 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.372949,
            "unit": "ms",
            "extra": "median of 5 runs: 100.756, 101.089, 101.373, 101.772, 102.239 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "dd6371dd896214f4ec8aef208695b302afe7346b",
          "message": "dispatch: the review of #1543's follow-ups\n\n- The table's Notice and row notes say what is true in each state\n  (M1). Not checked since purlis started: the next dispatch checks\n  first, and where the history reads the grants count. History that\n  cannot be read: none counts; a chat someone is at is asked on its\n  tab, one nobody is at is refused and listed under Needs you. The core\n  tells the two apart (dispatcharrival::unsettled), and the window gets\n  ProjectUnsettled.\n- Nothing is said where nothing of the project's is accepted here: a\n  decline alone, or a commit kept from an earlier settling, no longer\n  shows it (F1).\n- Accepted project rows are greyed in either state (F2).\n- The crossing rule stands aside for an unsandboxed chat by the very\n  comparison the no-sandbox refusal makes, and both go by one read of\n  the asking chat: the decision is now asked with it\n  (request_dispatch_as, requested_as). The session-based entry points\n  remain for the tests (F3).\n\nRefs #1543\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T08:54:16+04:00",
          "tree_id": "46abf1c3c2e779db9249102f990acf8502b5c6a4",
          "url": "https://github.com/purlis/purlis/commit/dd6371dd896214f4ec8aef208695b302afe7346b"
        },
        "date": 1791522119693,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.43515800000000004,
            "unit": "ms",
            "extra": "median of 5 runs: 0.418, 0.434, 0.435, 0.444, 0.449 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.397793,
            "unit": "ms",
            "extra": "median of 5 runs: 16.330, 16.384, 16.398, 16.431, 16.832 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.89402100000001,
            "unit": "ms",
            "extra": "median of 5 runs: 100.875, 101.718, 101.894, 102.051, 102.790 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "1ff31a105f31512d24c102d5792c259673e121af",
          "message": "dispatch: a recovered chain is whole only at the lineage's first chat, never by depth alone\n\nReview round for reading an older chain from the dispatch records.\n\n- Where the records stop at a chat that has closed, the chain is whole only\n  when that chat's id is the asking chat's `root` (the chat the person\n  started, copied down each dispatch), and as deep as the asking chat's\n  record says. The depth is now an extra condition, never enough alone: a\n  chat dispatched below one from before the depth key was given a depth\n  that is short and names no root, so a chain whose records stopped there\n  read as whole while missing the chats above. Such a chain stays unread\n  and refused. Regression tests at the chain and at the decision.\n- An open chat that names no asking chat ends the chain only where it is\n  the chat the asking chat's `root` names, where one is kept.\n- `dispatchchain::Above` has no `Default`: \"nothing above\" offers every box,\n  so each caller now names it.\n- Docs: ADR 0090 item 18, the handoff refusal table and the changelog\n  fragment say what is whole; plane-format's `chats[].from` says the chain\n  is read from the dispatch records before refusing, and that a depth is\n  read as at least the chain the walk finds.\n\nRefs #1548\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T09:29:02+04:00",
          "tree_id": "4612163aed865560efc96cbc42f32b994410a8d4",
          "url": "https://github.com/purlis/purlis/commit/1ff31a105f31512d24c102d5792c259673e121af"
        },
        "date": 1791523813573,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5167165,
            "unit": "ms",
            "extra": "median of 5 runs: 0.499, 0.513, 0.517, 0.528, 0.536 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.1335045,
            "unit": "ms",
            "extra": "median of 5 runs: 16.868, 17.086, 17.134, 17.204, 17.218 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.64872500000001,
            "unit": "ms",
            "extra": "median of 5 runs: 103.317, 103.381, 103.649, 104.274, 104.594 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "97e6f06eb063135c2eef6ed1880e5cd839de98dd",
          "message": "ADR 0067: one chat's Allow is bound too; what stays per task\n\nRecords the #1508 follow-ups decided in implementation: a chat's own\nblock Allow is bound to the block it showed and is the window's alone;\n\"Start without the sandbox\" and a host a block did not name stay one\ntask's answer each; the tab's chip keeps one hand per chat waiting off\nscreen; the blocks are held in memory only, so after a relaunch nothing\nis asked until a chat is blocked again.\n\nRefs #1538\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T09:37:28+04:00",
          "tree_id": "63bad37ef3cbc85e757e485db6e703b7ef8878af",
          "url": "https://github.com/purlis/purlis/commit/97e6f06eb063135c2eef6ed1880e5cd839de98dd"
        },
        "date": 1791524594265,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5925955,
            "unit": "ms",
            "extra": "median of 5 runs: 0.580, 0.592, 0.593, 0.607, 0.621 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.116725,
            "unit": "ms",
            "extra": "median of 5 runs: 16.004, 16.082, 16.117, 16.147, 16.198 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.786967,
            "unit": "ms",
            "extra": "median of 5 runs: 100.973, 101.411, 101.787, 102.398, 102.723 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "65629aa1378b51556cfdb94d378925a9c0213f61",
          "message": "Review of #1546: a report kept again after a start died in, left before it is claimed\n\n- A start the app died in, followed by a start that did not happen, now keeps\n  the report for its workspace again: give_back re-keeps wherever the record\n  is still owed and the file it names is gone, not only where this start took\n  it (handback::still_kept). Test: died in, refused, let go.\n- hand_to leaves the report for the chat first and claims the record after,\n  under the lock: the app dying between the two leaves the report with the\n  chat and still owed, never with nobody. A second claimant takes back what it\n  left.\n- A task that reports is no longer said to ask again: Talk::close clears the\n  restart mark.\n- Only chats a launch started are marked as brought back; one waiting to start\n  is marked when it starts.\n- The refusal says \"Any question it asked before the restart\", since the task\n  may have asked none.\n- A chat that resumed the asker (a Resume, or a Reopen of a finished task's\n  row) and closes before reading what it was handed keeps it owed: its close\n  matches the records by the chat it resumed too.\n- ADR 0090: the heading says the launch is also held to the profile and the\n  folder, and what \"kept again\" means after a start died in.\n\nRefs #1546\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T09:54:03+04:00",
          "tree_id": "24e92adc1eb2bfda5bc9373e8d3d470bbe0df129",
          "url": "https://github.com/purlis/purlis/commit/65629aa1378b51556cfdb94d378925a9c0213f61"
        },
        "date": 1791525802423,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5295085,
            "unit": "ms",
            "extra": "median of 5 runs: 0.523, 0.525, 0.530, 0.530, 0.545 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.6281215,
            "unit": "ms",
            "extra": "median of 5 runs: 16.365, 16.544, 16.628, 16.826, 17.149 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.7269345,
            "unit": "ms",
            "extra": "median of 5 runs: 102.537, 102.738, 103.727, 104.175, 104.217 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "3f36e5419fbef9267aa306a1bc27fdac2f18b162",
          "message": "review of #1534: the explorer's remove and merge are the window's alone\n\n- worktree_remove and worktree_merge join WINDOW_ONLY: the explorer's\n  remove (with force, of uncommitted work) and merge of any branch\n  folder are the person's acts in their window, and no link serves them.\n  The count test moves by two; uiRpc.ts is regenerated without them.\n- \"also written by\" names each other chat once, where two are shown\n  under one name.\n- names_folder's doc says what is true: a record that does not parse is\n  skipped, so it keeps nobody off a folder.\n\nRefs #1534\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T10:09:16+04:00",
          "tree_id": "be9edb800b230fb497f8abf7fc4603a096536251",
          "url": "https://github.com/purlis/purlis/commit/3f36e5419fbef9267aa306a1bc27fdac2f18b162"
        },
        "date": 1791526782121,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5098855,
            "unit": "ms",
            "extra": "median of 5 runs: 0.508, 0.510, 0.510, 0.527, 0.529 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.895238499999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.480, 16.484, 16.895, 17.152, 17.315 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.8081605,
            "unit": "ms",
            "extra": "median of 5 runs: 101.692, 102.976, 104.808, 105.314, 105.842 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "63148fa5497000ce802cf0b714480bbb555919ea",
          "message": "Say why an ask that leaves the window unanswered is not looked at again\n\nRead every route by which a window ask can leave without an answer: its\ndeadline passing, and its hook going away. In both the hook decides\nnothing, so the harness asks in its pane (the person's to answer, a key\nthat stands the end down), or the harness stopped (its turn's end or its\nprogram's end is the look). None leaves a reported task with neither a\nprompt nor a bound while it works on. A bound the ask spent is\ndeliberately not given anew there, so an unannounced pane prompt is never\nended under the person. Recorded on Seen::prompt_showing. Whether Claude\nCode says a pane prompt with a Notification after a hook decides\nnothing is still to be probed on a real chat.\n\nRefs #1525\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T10:38:02+04:00",
          "tree_id": "71a42ee33aaba6ae0d417f0007969be37a60b02a",
          "url": "https://github.com/purlis/purlis/commit/63148fa5497000ce802cf0b714480bbb555919ea"
        },
        "date": 1791527950508,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5470395000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.504, 0.540, 0.547, 0.567, 0.574 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.384251499999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.357, 16.705, 17.384, 17.463, 17.538 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 106.56055,
            "unit": "ms",
            "extra": "median of 5 runs: 101.834, 103.640, 106.561, 107.549, 108.603 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "2b348df6e6ad4b748af6a365096fada3918a2071",
          "message": "Policy and the sandbox: refuse a locked host at the write, revoke what is locked, lead with the policy\n\nThree of the #1423 follow-ups (the macOS ACL line waits on a ruling):\n\n- A whole-file save (Edit as TOML, form edits, the Shared/this-machine move) now refuses a host\n  an administrator's policy locks out at the file's level when the write adds it, with the\n  sentence Settings' Add gives. Hosts the file already held are kept and never block another\n  edit (settings::hosts::locked_out, checked in settings::refused_of).\n- Settings > Sandbox > Granted draws Revoke on an entry policy has locked out. The backend\n  already took it away; a test now pins that, audited, while the policy stands.\n- Where policy requires the sandbox, a refusal to start a sandboxed chat leads with\n  \"policy requires the sandbox for every chat on this machine\" instead of the project, and\n  ends with the policy and who set it (sandbox::under_policy, LEAD, POLICY_LEAD), including\n  the app's two refusals that did not name the policy before.\n\nDocs: plane-format's policy section, ADR 0067 §4 and §5 wording, a changes/ fragment.\n\nRefs #1431\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T10:57:44+04:00",
          "tree_id": "75ef4b1a351d5b2a86297dd886ea1a6068beff61",
          "url": "https://github.com/purlis/purlis/commit/2b348df6e6ad4b748af6a365096fada3918a2071"
        },
        "date": 1791529130344,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.273453,
            "unit": "ms",
            "extra": "median of 5 runs: 0.255, 0.256, 0.273, 0.287, 0.312 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.386162000000002,
            "unit": "ms",
            "extra": "median of 5 runs: 16.354, 16.386, 16.386, 16.554, 16.602 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.690619,
            "unit": "ms",
            "extra": "median of 5 runs: 100.959, 101.089, 101.691, 101.707, 101.788 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "c919b964db0588d621fff28095a2008d10f09eb8",
          "message": "build(deps): bump taiki-e/install-action\n\nBumps the actions group with 1 update in the / directory: [taiki-e/install-action](https://github.com/taiki-e/install-action).\n\n\nUpdates `taiki-e/install-action` from 2.87.22 to 2.87.25\n- [Release notes](https://github.com/taiki-e/install-action/releases)\n- [Changelog](https://github.com/taiki-e/install-action/blob/main/CHANGELOG.md)\n- [Commits](https://github.com/taiki-e/install-action/compare/83ac0ad63c0167e6f06796fab0fce28db1bf3db0...183e4297cca2404691e9380e1307288dced5c82a)\n\n---\nupdated-dependencies:\n- dependency-name: taiki-e/install-action\n  dependency-version: 2.87.25\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: actions\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-10-09T11:27:07+04:00",
          "tree_id": "2ef9bef06fb119213cff1d90f7133283dca249ec",
          "url": "https://github.com/purlis/purlis/commit/c919b964db0588d621fff28095a2008d10f09eb8"
        },
        "date": 1791530896144,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.2848625,
            "unit": "ms",
            "extra": "median of 5 runs: 0.262, 0.284, 0.285, 0.296, 0.297 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.625498999999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.526, 16.585, 16.625, 16.644, 16.699 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.729733,
            "unit": "ms",
            "extra": "median of 5 runs: 100.765, 100.982, 101.730, 101.991, 102.211 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "47d653d758c9629da15ae9dbde449a8d1e256d7d",
          "message": "Review of #1556: an owed report's record is kept, a resumed asker is back\n\n- A dispatch record that still owes its asking chat a report is aged\n  from when it was last written, not from its end. It is marked owed\n  when the report reaches no chat, which can be long after the task\n  ended (an asking chat open for weeks that closes with the report\n  unread): aged from its end, it was collected at the next open, and\n  the chat's reopen was never handed the report. The collection\n  wording in the format docs, retention and the workspace removal's\n  sentence says so.\n- A task whose asking chat came back as a chat that resumed it keeps\n  its row and its words: settling at open (from the reopen record),\n  after a launch (open chats and chats waiting to start), and as a task\n  ends while the app runs. That chat is handed the report, so the\n  asking chat is not gone. dispatchrecord::settle_asked takes the\n  asker's answer apart from the worker's.\n- expire_talk and expire_in answer the ids they changed, and the daily\n  sweep tells an open Activity tab the lines whose words it took.\n\nRefs #1556\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T11:30:14+04:00",
          "tree_id": "79984aaaea5338d064f9e15f2f5038ba82f86941",
          "url": "https://github.com/purlis/purlis/commit/47d653d758c9629da15ae9dbde449a8d1e256d7d"
        },
        "date": 1791531929195,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5186615,
            "unit": "ms",
            "extra": "median of 5 runs: 0.485, 0.515, 0.519, 0.546, 0.565 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.919673,
            "unit": "ms",
            "extra": "median of 5 runs: 16.499, 16.808, 16.920, 17.085, 17.117 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.342896,
            "unit": "ms",
            "extra": "median of 5 runs: 102.968, 102.968, 104.343, 104.471, 105.846 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "5b17d37f635dd3160d1c3b0038fcd7f0372a6d2c",
          "message": "Check every opened project before naming an export, and harden the chat check\n\nReview of #1542: M2 and fold-ins A to E.\n\n- The hint after a conversion names a variable only when no vault of\n  this project or of any other project this machine opened still reads\n  it, since a shell profile's export is the whole machine's. Where\n  another project could not be checked, it speaks for this project\n  alone (`Marked::checked_every_project`, also on the app's\n  `SetupDone`).\n- Only a name a shell can export is ever named, so a committed source\n  never reaches a terminal raw.\n- The wording no longer claims a chat is exposed (a chat starts from an\n  allowlisted environment) or that the token sits outside the Keychain\n  after a change to the app's sign-in: it is the person's own shells\n  and what they start.\n- The `--token-stdin` chat check also reads the machine store under the\n  account's home as the user database records it, so an environment\n  pointed at an empty store does not hide the person's projects. A\n  fenced build reads no store outside its fence.\n- Of each project only the chats' processes are read, leniently, from a\n  record of any version (`reopen::chat_pids`). A doubt says how to clear\n  it: delete the record named, or forget that project under Settings,\n  This machine; and the token can be given in the vault's tab.\n- Tests for each, at the CLI too, and for a project a window had open.\n\nRefs #1542\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T11:56:31+04:00",
          "tree_id": "b6d779abe65f6c7a40ad105ecaca1b09044790d0",
          "url": "https://github.com/purlis/purlis/commit/5b17d37f635dd3160d1c3b0038fcd7f0372a6d2c"
        },
        "date": 1791533021009,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5206999999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 0.510, 0.518, 0.521, 0.558, 0.563 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.949353000000002,
            "unit": "ms",
            "extra": "median of 5 runs: 16.670, 16.798, 16.949, 17.030, 17.150 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.364626,
            "unit": "ms",
            "extra": "median of 5 runs: 101.907, 102.216, 103.365, 105.534, 106.931 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "7cb5a293d68ebc8aea0ae4105e21c47393c3ada8",
          "message": "Settings test mocks name purlis's own plugin purlis@inline\n\nHarnessPlugins.test.tsx and WorkspaceLevel.test.tsx mocked the core's\nrows for purlis's own plugin as charter@inline. The core names it\npurlis@inline (harness_plugin.rs), so the mocks now do too. The tests\ncheck the UI, not the id, and pass either way.\n\nOnly the TypeScript fixtures item of the RN-8 follow-ups; the other open\nitems stay.\n\nRefs #1305\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T12:32:36+04:00",
          "tree_id": "96f782a114ff8fcd0da3cb7123a3e66fcf591850",
          "url": "https://github.com/purlis/purlis/commit/7cb5a293d68ebc8aea0ae4105e21c47393c3ada8"
        },
        "date": 1791535407812,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5154655,
            "unit": "ms",
            "extra": "median of 5 runs: 0.497, 0.507, 0.515, 0.525, 0.537 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.192206499999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.520, 17.084, 17.192, 17.262, 17.425 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.7126835,
            "unit": "ms",
            "extra": "median of 5 runs: 102.991, 103.983, 104.713, 106.146, 106.590 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "a50dbf81bd323ff00ad65b53de5a692c862e60b4",
          "message": "Dispatch text guard: read a body a shell runs, give A6's remedy, and say what a listed row holds\n\nReview fold-ins for the dispatch core's loose ends.\n\n- A7e now reads a heredoc body a shell runs (`bash <<'EOF'`) as the shell reads it, with the\n  plan A7 and the leak guard share: a text subcommand in such a body beside a live\n  substitution is refused. A body a reader takes (`cat > notes.md <<'EOF'`) stays data.\n- The refusal gives A6's measured remedy: backslash-escape each backtick, or single-quote the\n  whole text when it holds no apostrophe. For a report it also names the `dispatch_report`\n  tool, whose text no shell reads.\n- ADR 0064's amendment says what a row of the list carries: chat number, name, persona,\n  workspace, state, age, and the branch purlis cut with how it stands; still only the asking\n  chat's own tasks.\n- The spec paragraph is rewrapped.\n\nRefs #1456\nRefs #1463\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T13:15:52+04:00",
          "tree_id": "1bdb5d8eeb6c4df49d245525479e8739767cadd6",
          "url": "https://github.com/purlis/purlis/commit/a50dbf81bd323ff00ad65b53de5a692c862e60b4"
        },
        "date": 1791537972522,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.41004450000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 0.396, 0.409, 0.410, 0.415, 0.429 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.5315395,
            "unit": "ms",
            "extra": "median of 5 runs: 16.521, 16.526, 16.532, 16.641, 16.715 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.660968,
            "unit": "ms",
            "extra": "median of 5 runs: 102.091, 102.191, 102.661, 103.068, 103.137 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "f69e92a4fdfd815ee47d089d65b9a894a69b3284",
          "message": "Add a standing dispatch grant from Settings\n\nSettings › Project › Dispatch has Add a grant: the asking persona, the\ntarget, for me on this machine or for everyone in this project, in one\nworkspace or in any. It is the one way to make a grant with no dispatch\nwaiting, which is what a chat nobody is at needs.\n\nadd_dispatch_grant is window-only (WINDOW_ONLY, STANDING_DISPATCH) and\naudited as the person's under no chat before it counts. add_grant holds it\nto every rule an Allow is held to, refusing with nothing audited: two\npersonas of the project (a persona's own dispatch needs none), a policy\nlock, the person's never or a list of nevers that does not read, a grant\nfor everyone across a pair declined on this machine, a workspace that is\nnot there, one that stands already or is in the file\nalready, and a project file that is missing or not editable. A grant for\neveryone is written and acknowledged here, bound to the project's history\nas any acceptance is. The Allow and Add now keep a standing grant through\none function, keep_standing.\n\nWhere the project has workspaces, the form's workspace choice starts empty\nand must be picked, \"any workspace\" included (ruling 2: narrower is the\ndefault; the asking persona has no workspace of its own to start from).\n\nThe unattended refusal and the crossing refusal now say to grant it under\nSettings › Project › Dispatch, and their pinned rows move with them.\n\nRefs #1465\nRefs #1472\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T13:28:51+04:00",
          "tree_id": "70b91ff836285cbcd4d5ffdbad0794b891b2d776",
          "url": "https://github.com/purlis/purlis/commit/f69e92a4fdfd815ee47d089d65b9a894a69b3284"
        },
        "date": 1791539280217,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.286639,
            "unit": "ms",
            "extra": "median of 5 runs: 0.277, 0.279, 0.287, 0.287, 0.306 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.4872315,
            "unit": "ms",
            "extra": "median of 5 runs: 16.428, 16.437, 16.487, 16.513, 16.600 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.38734149999999,
            "unit": "ms",
            "extra": "median of 5 runs: 99.810, 100.967, 101.387, 101.642, 102.096 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "a2a5758912ee377fc3a207a56c76db4a1a85d5e1",
          "message": "Test the clone that makes its workspace, not the old refusal\n\nrepo_commands.rs pinned the refusal issue 1382 removed: a clone into a\nmissing workspace exited 1. It now checks the new rule end to end: the\nworkspace is made and scaffolded, the clone lands in it, and one line\nsays so; a bad name or a linked workspace is still refused with nothing\nmade or written through the link.\n\nRefs #1382\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T14:09:05+04:00",
          "tree_id": "7b7258cc2ca02397e9fcd4b97db308ccf3387edc",
          "url": "https://github.com/purlis/purlis/commit/a2a5758912ee377fc3a207a56c76db4a1a85d5e1"
        },
        "date": 1791542314251,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4185065,
            "unit": "ms",
            "extra": "median of 5 runs: 0.398, 0.418, 0.419, 0.436, 0.458 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.499956499999996,
            "unit": "ms",
            "extra": "median of 5 runs: 16.283, 16.397, 16.500, 16.596, 16.938 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.74356350000001,
            "unit": "ms",
            "extra": "median of 5 runs: 101.370, 101.523, 101.744, 101.771, 101.805 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "5fff8e9fb4d9b3015b890f880e201466c33780a9",
          "message": "Keep the travelling-Local test honest, and say merged before landed\n\nChoices::read now keeps the project root, so the three assertions in\na_local_file_that_would_travel_decides_nothing.rs that compared it with\nchoices made from text would fail: the derived equality compares the\nroot too. `Choices::of_project` attaches the root, `read` uses it, and\nthe test's expected values say where they were read.\n\nWith a blocker merged outside purlis, a merged member that waited on it\nwas described as \"merged ahead of its blocker\", which claims an order\nnobody saw. It merged before its blocker landed, and that is what the\nrow and the changes view now say.\n\nRefs #1371\nRefs #1093\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-09T15:07:23+04:00",
          "tree_id": "775d3de89f2d7eeef68a1bdde1e613fd55347985",
          "url": "https://github.com/purlis/purlis/commit/5fff8e9fb4d9b3015b890f880e201466c33780a9"
        },
        "date": 1791545683813,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.271374,
            "unit": "ms",
            "extra": "median of 5 runs: 0.268, 0.270, 0.271, 0.274, 0.284 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.481527,
            "unit": "ms",
            "extra": "median of 5 runs: 16.454, 16.469, 16.482, 16.546, 16.576 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.0689565,
            "unit": "ms",
            "extra": "median of 5 runs: 100.164, 100.849, 101.069, 101.451, 101.620 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "f03a11f8db8234bce5914bd541cd8501a829157e",
          "message": "Ask a branch again when the reader gate was full, and say busy plainly\n\nWith six reads hung app-wide, a branch the watch resolves queues behind\nthem past its deadline and fails as busy. The window asks for its set\nonly when the set changes, so that branch went unwatched with nothing\nsaid. resolve() now asks again while the answer is busy, enough times for\nevery watched branch to have had its turn.\n\nThe busy message drops the doubled \"purlis\" and the \"no read came free\"\nmechanism (docs/ui-copy.md): \"purlis could not read the branch: it was\nbusy reading other branches for 30 seconds\". files::was_busy tells it\napart from a read that failed.\n\nRefs #1130\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T15:52:20+04:00",
          "tree_id": "1ef47d99a32d01c2c692f419013fc08d4dec4557",
          "url": "https://github.com/purlis/purlis/commit/f03a11f8db8234bce5914bd541cd8501a829157e"
        },
        "date": 1791548484795,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5180230000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.486, 0.487, 0.518, 0.534, 0.536 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.8520395,
            "unit": "ms",
            "extra": "median of 5 runs: 16.544, 16.562, 16.852, 17.054, 17.152 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.2836615,
            "unit": "ms",
            "extra": "median of 5 runs: 103.315, 103.467, 104.284, 104.861, 106.051 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "0d94480d8bd59632dd2a4dae27b1e052a26664c7",
          "message": "Needs-you list: rewrap the change note to the fragment width\n\nRefs #1146\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T16:30:32+04:00",
          "tree_id": "908e4721b938e75f68bdc474b4ec0168a3bcd42b",
          "url": "https://github.com/purlis/purlis/commit/0d94480d8bd59632dd2a4dae27b1e052a26664c7"
        },
        "date": 1791549570335,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5442940000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.516, 0.524, 0.544, 0.558, 0.579 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.736877,
            "unit": "ms",
            "extra": "median of 5 runs: 16.350, 16.584, 16.737, 16.937, 17.265 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.47363949999999,
            "unit": "ms",
            "extra": "median of 5 runs: 102.477, 103.287, 103.474, 106.076, 107.049 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "2f1eb2a3072dec038ba91246fc9b38e287218b63",
          "message": "Undo of a memory's Move restores its exact name\n\nA journal memory moved to a persona or shared memory and back by Undo\ncame home only to the minute: the move rebuilds a journal name from the\nstamp line, which holds no seconds. The window knows the first name, so\nUndo now passes it.\n\n- memstore::move_one takes an optional exact name. It must be one plain\n  name, the same memory's name once a journal prefix is taken off both,\n  and carry a journal prefix exactly when the target is a journal. So it\n  can choose only the prefix's stamp, never rename a memory. Anything\n  else is InvalidInput. It goes through every existing check: a name the\n  target holds (a file or a link) is AlreadyExists, nothing is replaced,\n  and the target is reached by descriptor as before.\n- memscope::move_memory_as carries it; move_memory keeps its signature,\n  so purlis-cli is unchanged.\n- The memory_move command gains restore_as: Option<String>. Undo passes\n  the slug the memory had, the tab's Move passes null. bindings.ts and\n  uiRpc.ts regenerated.\n\nKeeping the index line's position on Undo stays open on the issue, and\nthe in-flight guard on Undo is another lane's.\n\nRefs #1190\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T16:35:50+04:00",
          "tree_id": "e3aab23181d6029c3b7a0dc8d0cf1e1630788bbc",
          "url": "https://github.com/purlis/purlis/commit/2f1eb2a3072dec038ba91246fc9b38e287218b63"
        },
        "date": 1791550872152,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.47604199999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 0.457, 0.460, 0.476, 0.481, 0.494 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.263751499999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.144, 16.236, 16.264, 16.316, 16.334 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.32307399999999,
            "unit": "ms",
            "extra": "median of 5 runs: 100.629, 101.083, 101.323, 101.907, 102.206 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "46f00b13065e818503e612b17a53bb90309d20d1",
          "message": "The purlis binary finds the app bundle it shipped in\n\nThe command line, the footer's badges, the events a command tells and the\nsession-start briefing passed no bundle, so a built-in extension reached\nnone of them from the CLI. The binary now finds its bundle from its own\nexecutable's real path (extension::bundle): Contents/Resources/extensions\nbeside Contents/MacOS in a macOS .app, and usr/lib/purlis/extensions beside\nusr/bin in a .deb or a mounted AppImage. No environment variable, setting\nor record takes part. A binary outside a bundle, or a layout with a link\nbetween the executable's directory and the extensions directory, gets no\nbuilt-ins, and its line after an unknown word says it is outside the app's\nbundle.\n\nADR 0041 gains the amendment; the release workflow now checks the Linux\npackages keep the extensions under usr/lib/purlis exactly.\n\nCloses #1366\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T17:21:26+04:00",
          "tree_id": "0cf4789e7c82d467924ab14febbd870bc30b6ba3",
          "url": "https://github.com/purlis/purlis/commit/46f00b13065e818503e612b17a53bb90309d20d1"
        },
        "date": 1791552626950,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.37004950000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 0.356, 0.363, 0.370, 0.373, 0.374 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.602916,
            "unit": "ms",
            "extra": "median of 5 runs: 16.582, 16.584, 16.603, 16.623, 16.682 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.000181,
            "unit": "ms",
            "extra": "median of 5 runs: 100.840, 101.966, 102.000, 102.817, 103.026 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "b0df86ca0efb27cf0e1519a080806a2572d7e4a1",
          "message": "Hold the trailer bound to a whole Assisted-by value of 100\n\nThe test that pins the 100-character trailer value built a 100-character\nmodel, which with the harness in front is a 112-character value. Since\nthe model is kept at most 88 so the whole value fits, the writer leaves\nthe bare harness there. The test now sizes the model so the whole value is\nexactly 100, kept, and 101, left out. It holds a value that stands alone\n(the persona) to the same bound, and the reader to what the writer keeps.\n\nRefs #1021\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T17:26:08+04:00",
          "tree_id": "b3c295814b69fa0ca7da530275211bba3cf7393d",
          "url": "https://github.com/purlis/purlis/commit/b0df86ca0efb27cf0e1519a080806a2572d7e4a1"
        },
        "date": 1791553843226,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4485405,
            "unit": "ms",
            "extra": "median of 5 runs: 0.421, 0.426, 0.449, 0.459, 0.466 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.700328,
            "unit": "ms",
            "extra": "median of 5 runs: 16.382, 16.482, 16.700, 16.795, 16.921 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.18795750000001,
            "unit": "ms",
            "extra": "median of 5 runs: 101.854, 102.066, 102.188, 102.599, 103.884 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "afee8939deb819e1d505b6362f2e45c3d4585fbb",
          "message": "Count the renamed file src/ lost in the marks test\n\nThe roll-up now counts a renamed file in the folders above where it came\nfrom, as a file each of them lost (D-1130-1). In this test old.rs moves\nfrom src/ to lib/, so src/ holds three changes: the file committed in it,\nthe one deleted from it and the one renamed out of it. The test predated\nthe rule and still expected two. The branch's own folder is above both\nends and counts the rename once, so its five is unchanged.\n\nRefs #1130\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T18:03:17+04:00",
          "tree_id": "a5299f5afeddce2d77c6dd6222610383271a03e9",
          "url": "https://github.com/purlis/purlis/commit/afee8939deb819e1d505b6362f2e45c3d4585fbb"
        },
        "date": 1791555635799,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5622235,
            "unit": "ms",
            "extra": "median of 5 runs: 0.527, 0.529, 0.562, 0.564, 0.580 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.1332115,
            "unit": "ms",
            "extra": "median of 5 runs: 16.547, 16.787, 17.133, 17.265, 17.299 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.247716,
            "unit": "ms",
            "extra": "median of 5 runs: 104.156, 104.680, 105.248, 105.734, 106.085 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "ca5d3f80a6ed4291f40af052f52561e97ba5854e",
          "message": "Draw only the newest answer of the Granted list\n\nIts read and Revoke now answer off the window's thread, so an older answer\ncould land after a newer one and bring a revoked grant back on screen.\n\nRefs #1543\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T18:12:10+04:00",
          "tree_id": "6795165b1aa7d83a2d595e57d211577a6f3cbd09",
          "url": "https://github.com/purlis/purlis/commit/ca5d3f80a6ed4291f40af052f52561e97ba5854e"
        },
        "date": 1791556450725,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.52952,
            "unit": "ms",
            "extra": "median of 5 runs: 0.487, 0.523, 0.530, 0.537, 0.556 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.9671165,
            "unit": "ms",
            "extra": "median of 5 runs: 16.449, 16.744, 16.967, 16.995, 17.571 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.561722,
            "unit": "ms",
            "extra": "median of 5 runs: 103.258, 104.248, 104.562, 105.022, 105.259 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "6820c7455d156996a9c9d73396c546b8e1223176",
          "message": "Record the offered-settings note in the rows that wire a workspace\n\nA workspace wire now notes the settings it offers in the project's app\nstate, so 45 recorded rows leave one more file behind:\n.charter/app/mirrors-offered.json (and the folders it makes). Nothing\nelse in those rows moves; each digest is the one the row's own record\nnames for the generated settings.\n\nThe row that withdraws a file the plane no longer declares now starts\nfrom a plane whose app state noted that offer, as any plane wired by\nthis build has. Without the note the file is kept, which is the\nfail-closed upgrade case the unit tests pin.\n\nWritten by hand because this sandbox cannot bless (socket bind and\n.claude/settings.json writes are refused); the 45 rows pass\ntools/recorded-bless-check.mjs with --tree limited to the store's paths.\n\nRefs #1583\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T19:10:39+04:00",
          "tree_id": "5e3726b512ca8f7302551e67ef4a654365430d7b",
          "url": "https://github.com/purlis/purlis/commit/6820c7455d156996a9c9d73396c546b8e1223176"
        },
        "date": 1791559189531,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4184015,
            "unit": "ms",
            "extra": "median of 5 runs: 0.407, 0.415, 0.418, 0.425, 0.442 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.656422,
            "unit": "ms",
            "extra": "median of 5 runs: 16.238, 16.438, 16.656, 16.671, 16.820 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.7412955,
            "unit": "ms",
            "extra": "median of 5 runs: 101.221, 101.442, 101.741, 102.086, 102.300 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "b9a263c18dbdfa72a7ea01d7a46fe2f18d0299e2",
          "message": "Age a never-started cut's reflog too in the app's unclaimed-row test\n\nA cut is now dated by its branch's reflog, with the tree's .git file only as\nthe fallback, so ageing the file alone left the cut a moment old and the row\nsaid nothing. The test ages both, as a cut three days old has them.\n\nRefs #835\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T19:45:47+04:00",
          "tree_id": "c577bc4e509798ad988009ec9be85d3e182a6354",
          "url": "https://github.com/purlis/purlis/commit/b9a263c18dbdfa72a7ea01d7a46fe2f18d0299e2"
        },
        "date": 1791560909010,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5167055,
            "unit": "ms",
            "extra": "median of 5 runs: 0.501, 0.502, 0.517, 0.533, 0.546 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.055352499999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.195, 16.334, 17.055, 17.214, 17.221 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.73948899999999,
            "unit": "ms",
            "extra": "median of 5 runs: 102.202, 102.325, 103.739, 103.746, 103.860 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "03abc9ec2499e457a095a987c45fac7143fafdba",
          "message": "Hold the tab strips to the rule that a tablist owns only tabs\n\nWrites out axe's aria-required-children reading of what a tablist owns\n(its DOM children through role-less wrappers, plus what aria-owns\nnames, less what is hidden from assistive technology) and checks it\nagainst small trees first. One of those settles the design question in\nthe issue: aria-owns adds to what a tablist owns and never removes its\nDOM children, so a tablist that keeps its cells still owns every close\nbutton and gear in them.\n\nThen it renders the window's three strips and lists, exactly, what each\nowns today that is not a tab: the project's close button, gear and the\nstrip's own Open and New buttons; the focused workspace's gear; each\nchat tab's close button. A new control inside a strip fails the test,\nand the fix for the strips crosses entries off until the lists are\nempty. axe itself is not a dependency and was not run.\n\nRefs #1204\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T19:57:09+04:00",
          "tree_id": "fef90b9b7a2375e06b374f815d192e9882a76ba6",
          "url": "https://github.com/purlis/purlis/commit/03abc9ec2499e457a095a987c45fac7143fafdba"
        },
        "date": 1791561748486,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.47214049999999996,
            "unit": "ms",
            "extra": "median of 5 runs: 0.455, 0.462, 0.472, 0.480, 0.483 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.1053285,
            "unit": "ms",
            "extra": "median of 5 runs: 16.968, 17.025, 17.105, 17.140, 17.202 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.929596,
            "unit": "ms",
            "extra": "median of 5 runs: 103.652, 104.506, 104.930, 106.194, 106.396 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "6cc8dbb8bd384c00d568420a17b160d8351efef7",
          "message": "Keep a session's ephemeral memory whole when a name in it cannot be read\n\nThe ephemeral sweep listed a session's folders through `names`, which\nskips a name that is not UTF-8. A session folder holding such a name was\njudged without it: its month-old files were removed, and only the final\nfolder removal failed. The shape check now lists through `every_name`,\nwhich answers nothing when the folder cannot be listed or any name is not\nUTF-8, so the session is kept whole, as for any shape charter never\nwrites.\n\nRefs #1004\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T20:26:25+04:00",
          "tree_id": "8ff49a0877c9f4ac225693bd869001016a068650",
          "url": "https://github.com/purlis/purlis/commit/6cc8dbb8bd384c00d568420a17b160d8351efef7"
        },
        "date": 1791563455911,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.505154,
            "unit": "ms",
            "extra": "median of 5 runs: 0.482, 0.487, 0.505, 0.530, 0.532 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.9110725,
            "unit": "ms",
            "extra": "median of 5 runs: 16.300, 16.798, 16.911, 16.983, 17.022 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.0648585,
            "unit": "ms",
            "extra": "median of 5 runs: 102.496, 103.602, 104.065, 104.154, 104.745 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "f15495d62edd036e171d4aec0183f028c2fea375",
          "message": "Own a chat tab that goes into the background with its name open\n\nThe chat strip's tablist left out the tab being renamed, but a chat can\ngo into the background while its name box is open: the core's smart-close\nstep turns the tab into a chip, which is drawn as a tab, and the tablist\nthen missed it. The strip and the tablist now ask one predicate,\nnameBoxFor, whether a tab is drawn as its name box.\n\nRefs #1204\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T20:30:13+04:00",
          "tree_id": "b5fbd6f37349a959cc0fb90b960c0d9fa222e8ae",
          "url": "https://github.com/purlis/purlis/commit/f15495d62edd036e171d4aec0183f028c2fea375"
        },
        "date": 1791564722564,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.513369,
            "unit": "ms",
            "extra": "median of 5 runs: 0.503, 0.508, 0.513, 0.540, 0.541 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.983567999999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.463, 16.521, 16.984, 17.353, 17.445 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.5456825,
            "unit": "ms",
            "extra": "median of 5 runs: 103.496, 105.081, 105.546, 106.951, 107.120 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "482c80281828954de1caab5413ecfb8ebe31cfea",
          "message": "Declare the upstream's remote in the ahead/behind fixture, as a clone does\n\nThe test compares purlis's count with git's own `@{upstream}...HEAD`,\nand git maps `branch.main.merge` to a remote-tracking ref only through\nthe remote's fetch refspec. The fixture set the branch's remote and\nmerge but declared no remote, so git refused (\"not stored as a\nremote-tracking branch\") before anything was compared. The remote is now\ndeclared with the refspec `git clone` writes; nothing is fetched from it\nand no assertion changed.\n\nRefs #1130\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T21:30:24+04:00",
          "tree_id": "048cb7b82ff813ed728fc1d8a7f5a732bda3b4c1",
          "url": "https://github.com/purlis/purlis/commit/482c80281828954de1caab5413ecfb8ebe31cfea"
        },
        "date": 1791567311731,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.6291789999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 0.607, 0.627, 0.629, 0.638, 0.692 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.013136999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 15.955, 15.992, 16.013, 16.028, 16.534 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.119333,
            "unit": "ms",
            "extra": "median of 5 runs: 99.343, 101.067, 101.119, 103.219, 103.320 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "65e15f313d30cd4b4d2757e4bdd0ae3b5c1cec99",
          "message": "Drop a chat's word that its dispatch waits on memory when the chat closes\n\nThe wait goes with the chat that asked (HeldDispatches::forget), so moving\nthe word to its workspace would tell the next chat there of a dispatch that\nnever starts and a later word that never comes. The row's hover says the\nchat is told when each starts or gives up, not only on giving up. The vault\nNotice's test fixtures and ADR 0064's quote take the neutral \"the chat\".\n\nRefs #1617\nRefs #1538\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-09T21:30:26+04:00",
          "tree_id": "864142dcd4300d88090251223b9a1bec5df39ee2",
          "url": "https://github.com/purlis/purlis/commit/65e15f313d30cd4b4d2757e4bdd0ae3b5c1cec99"
        },
        "date": 1791568238402,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.539209,
            "unit": "ms",
            "extra": "median of 5 runs: 0.516, 0.535, 0.539, 0.540, 0.548 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.939282499999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.554, 16.729, 16.939, 16.949, 16.981 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.028697,
            "unit": "ms",
            "extra": "median of 5 runs: 102.442, 103.780, 104.029, 104.421, 104.625 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "0ea74452f71fe154ab502618d2731d0474c5d2da",
          "message": "Keep every installed package's .deb for the Linux packages cache\n\nThe check for which downloaded files are installed piped the installed list into\n`grep -q`. Under `pipefail`, grep leaving at its first match ended the writer with\nSIGPIPE once the list outgrew the pipe's buffer, as a runner image's thousands of\npackages do, so a match read as a miss and the cache came up short. It reads a\nhere-string now, and a test lists 4,000 installed packages.\n\nThe install limit is 120 s, so the worst case with each limit's kill grace and the\npause is 510 s, under the step's 540, and the script's own message is what is read.\nA run that finds nothing to keep says so in the job summary.\n\nRefs #1477\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T09:27:01+04:00",
          "tree_id": "09e062e05813421d6989fc4e4856867219594a7e",
          "url": "https://github.com/purlis/purlis/commit/0ea74452f71fe154ab502618d2731d0474c5d2da"
        },
        "date": 1791611564468,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.321033,
            "unit": "ms",
            "extra": "median of 5 runs: 0.309, 0.319, 0.321, 0.323, 0.324 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.174612,
            "unit": "ms",
            "extra": "median of 5 runs: 16.106, 16.144, 16.175, 16.284, 16.650 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.5935275,
            "unit": "ms",
            "extra": "median of 5 runs: 100.433, 101.481, 101.594, 101.637, 102.274 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "bacaa93df3218e3c76643ba2c4f57f9a79739974",
          "message": "Refresh a mirrored file only when purlis can confirm it wrote the one there\n\nA launch, a reinit and a guard change bring a generated or mirrored\nfile up to the project's newer text: the settings in a workspace\nfolder, and the settings, machine-local settings, agents and skills in\na checkout. That overwrite went by the record kept beside the file\nalone, and the record sits where a chat may be able to write it.\n\nNow the project's app state must vouch for the text on disk at that\nexact path as well, for the layer that offers it: the same note of\nwhat the project offered that a withdraw already asks (#1616). A file\nonly the record vouches for is left byte for byte as it is and reported\nas its own row, \"unconfirmed\": reinit says purlis has newer text but\ncannot confirm it wrote the file there, and names moving it aside and\n`purlis workspace reinit` as what writes the current one. A chat in a\npiece holding one is not started, as for a file purlis did not write.\nThe note is never filled in from a record.\n\nThe machine-local settings are noted now too, for the refresh alone.\nA chat's own AGENTS.md still goes by the record: a chat can write that\nfile itself.\n\nFive recorded rows that carry a rule into workspaces an older charter\nwired now start from a plane whose app state noted the text those\nworkspaces hold, as a plane wired by this build has, and leave both\ntexts noted. The row that withholds a machine-local file notes that\nfile's text too. Written by hand: this sandbox cannot bless.\n\nRefs #1583\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T09:44:05+04:00",
          "tree_id": "f12f6fbfb94aeea1307886083c7d4aad237836b8",
          "url": "https://github.com/purlis/purlis/commit/bacaa93df3218e3c76643ba2c4f57f9a79739974"
        },
        "date": 1791613613264,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5329355,
            "unit": "ms",
            "extra": "median of 5 runs: 0.523, 0.526, 0.533, 0.540, 0.541 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.020893,
            "unit": "ms",
            "extra": "median of 5 runs: 16.555, 16.566, 17.021, 17.152, 17.221 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.01471950000001,
            "unit": "ms",
            "extra": "median of 5 runs: 103.820, 104.002, 104.015, 104.652, 106.636 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "b840b494e85031eb6982583c69caa70d9b6ca5cd",
          "message": "Say the in-flight statuses were read from Claude Code's code\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T10:33:13+04:00",
          "tree_id": "47f93d2b4ea7378304e3afaf3f925fad02e51f2c",
          "url": "https://github.com/purlis/purlis/commit/b840b494e85031eb6982583c69caa70d9b6ca5cd"
        },
        "date": 1791615680271,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.2660955,
            "unit": "ms",
            "extra": "median of 5 runs: 0.260, 0.263, 0.266, 0.269, 0.274 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.358923500000003,
            "unit": "ms",
            "extra": "median of 5 runs: 16.316, 16.353, 16.359, 16.417, 16.420 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.6764485,
            "unit": "ms",
            "extra": "median of 5 runs: 101.119, 101.661, 101.676, 102.046, 102.345 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "93cab5f3c21aac3f2aed2093529db14a048ab725",
          "message": "Fail a licence an npm exception allows that none of its packages is under\n\nAn exception that lets one package pass under CC-BY-4.0 but also lists\nanother licence would let a later release of that package pass under the\nother one without anyone looking again. Each licence an exception lists\nmust now be named by one of the package entries it lets pass, or the\ncheck fails with that part called stale. An OR expression counts every\nlicence it names, so no false failure arises there.\n\nRefs #1145\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T11:01:05+04:00",
          "tree_id": "c000f5b3af4651863c9d42263f999ceda2c8f211",
          "url": "https://github.com/purlis/purlis/commit/93cab5f3c21aac3f2aed2093529db14a048ab725"
        },
        "date": 1791616804782,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.497049,
            "unit": "ms",
            "extra": "median of 5 runs: 0.485, 0.493, 0.497, 0.518, 0.531 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.8562265,
            "unit": "ms",
            "extra": "median of 5 runs: 16.287, 16.537, 16.856, 17.192, 17.198 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.806816,
            "unit": "ms",
            "extra": "median of 5 runs: 103.493, 104.430, 104.807, 104.818, 105.915 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "480071c6d0cb2540d2004d25178d8318565f6612",
          "message": "Name a working route on Linux, wipe kept tokens, and keep vault list off the keyring\n\nReview round on the #1638 fix:\n- Where the app runs no command for a sandboxed chat yet (Linux), the\n  refusal now says to run it in a terminal outside the chat instead of\n  naming a `secret exec` route the app would refuse.\n- The per-run memo holds each kept token in a zeroizing string, so it is\n  wiped when the command's context goes.\n- `vault list` no longer runs `op` for a 1Password vault whose token is\n  kept in the keyring; its status says where the token is and that\n  `vault verify` reads it, as the guide said (#1180).\n- ADR 0047's amendment, the secrets guide, the skill and the changelog\n  say so, including the Linux behaviour change.\n\nRefs #1638, #1180\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T11:20:02+04:00",
          "tree_id": "454414032cc56313a53d73a741c39170c0a02b7d",
          "url": "https://github.com/purlis/purlis/commit/480071c6d0cb2540d2004d25178d8318565f6612"
        },
        "date": 1791618183797,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.35428800000000005,
            "unit": "ms",
            "extra": "median of 5 runs: 0.330, 0.347, 0.354, 0.356, 0.389 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.495728999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.337, 16.416, 16.496, 16.530, 16.818 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.32423,
            "unit": "ms",
            "extra": "median of 5 runs: 102.244, 102.249, 102.324, 102.708, 102.876 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "a7297d49f1217151d6c605be6bb6cebb38931f53",
          "message": "Say the doctor's layout row offers workspace-reinit where it said none did\n\nDoctor.tsx's reasons beside FINDINGS_AS_NOTICES and the bare --fix test\nboth said no doctor row offers workspace-reinit. The `workspace layout`\nrow now does. It stays in the dialog, because a workspace behind the\nlayout blocks nothing and the Alerts drawer already shows it. One long\nline in install.md is rewrapped.\n\nRefs #1289\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T11:52:32+04:00",
          "tree_id": "53a85b498e18e64b92562cbc612c2a06b56b8e53",
          "url": "https://github.com/purlis/purlis/commit/a7297d49f1217151d6c605be6bb6cebb38931f53"
        },
        "date": 1791619121859,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5167395,
            "unit": "ms",
            "extra": "median of 5 runs: 0.489, 0.508, 0.517, 0.533, 0.544 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.027780999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.303, 16.839, 17.028, 17.039, 17.080 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.10549850000001,
            "unit": "ms",
            "extra": "median of 5 runs: 103.885, 104.045, 104.105, 104.803, 104.833 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "3c23459003415fafcc94c30bdb195e823ac956df",
          "message": "Install the cached Linux packages from apt's archives directory\n\napt takes a .deb named on its command line as already downloaded only when the file is in\nDir::Cache::Archives. From ~/.cache/linux-packages it queued every cached file as a download,\n--no-download dropped them, and each warm run failed with \"Unable to fetch some archives\"\nand fell back to the mirror. The cached files are now copied there and named from there.\n\nThe test's apt-get stand-in now behaves the same way, so the old call fails the warm-cache test.\n\nRefs #1477\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T12:03:28+04:00",
          "tree_id": "89039b5d12378cf7cc47d11b1dbfc064803c3d4d",
          "url": "https://github.com/purlis/purlis/commit/3c23459003415fafcc94c30bdb195e823ac956df"
        },
        "date": 1791620138910,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4672655,
            "unit": "ms",
            "extra": "median of 5 runs: 0.439, 0.454, 0.467, 0.468, 0.472 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.312037,
            "unit": "ms",
            "extra": "median of 5 runs: 16.212, 16.260, 16.312, 16.315, 16.454 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.862476,
            "unit": "ms",
            "extra": "median of 5 runs: 100.179, 101.229, 101.862, 102.009, 102.945 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "27f647d0f9e43f67987796142f257915dcc4ae09",
          "message": "Scan each Rust source once, in linear time, in the copy check\n\nThe Rust copy check timed out on CI at 5s. Its literal scanner copied the\nrest of the source at every character and counted lines from the top for\nevery literal, so each file cost quadratic time, and the command-error\nplaces scanned every file of the app again in each of three cases.\n\nThe scanner now matches sticky patterns at the current offset, counts\nlines on from the last literal, and keeps each source's scan in a map.\nThe command-error case drops from about 2.5s to about 0.12s locally.\n\nRefs #1156\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T12:52:59+04:00",
          "tree_id": "aba81bb568ff23a4c287dd87347441ed765e0892",
          "url": "https://github.com/purlis/purlis/commit/27f647d0f9e43f67987796142f257915dcc4ae09"
        },
        "date": 1791622846103,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.518076,
            "unit": "ms",
            "extra": "median of 5 runs: 0.502, 0.510, 0.518, 0.524, 0.534 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.685288999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.433, 16.539, 16.685, 17.131, 17.205 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.654235,
            "unit": "ms",
            "extra": "median of 5 runs: 103.250, 103.846, 104.654, 104.910, 105.321 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "f123f5601c1fb20b6843435305993f348e77d613",
          "message": "ci: annotate each failed scenario test on its spec's line\n\nThe scenario configs' afterTest and afterHook write each failure to\napp/logs/failures.jsonl, and a failure() step turns them into GitHub\nannotations with the assertion message (tools/e2e-annotations.mjs):\nten errors, ten warnings, then a notice counting the rest. A failed job\nwith no test recorded gets one annotation saying so. The checks API\ncarries annotations, so a failure reads there without the job's log.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T13:09:45+04:00",
          "tree_id": "4b12f325c489810651e8df6e10cb5ab7c8943400",
          "url": "https://github.com/purlis/purlis/commit/f123f5601c1fb20b6843435305993f348e77d613"
        },
        "date": 1791624199123,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5206035,
            "unit": "ms",
            "extra": "median of 5 runs: 0.510, 0.517, 0.521, 0.531, 0.562 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.124346,
            "unit": "ms",
            "extra": "median of 5 runs: 16.861, 16.891, 17.124, 17.137, 18.109 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.6598365,
            "unit": "ms",
            "extra": "median of 5 runs: 103.375, 103.536, 103.660, 104.039, 105.508 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "251aa2e4875b12b5a70d0630e43d9900726a0593",
          "message": "Hold a pane Notice's brief where the box under its line scrolls it\n\nThe row's cap at three fifths of the pane (#1647) means the box under a\nNotice's line gives way first and scrolls, as #1481 has it. With the\ndispatch's five answers in a narrow Linux pane, the brief ran 7px past\nthe bottom of that box unscrolled. The two narrow cases now scroll the\nbox to the brief and hold it there: at the box's width, with its top\nshown and the rest either shown or reachable in the same box.\n\nRefs #1647\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T14:38:29+04:00",
          "tree_id": "581a419bbec93ea4b6d5e51ed0745c5a1f8c9f8c",
          "url": "https://github.com/purlis/purlis/commit/251aa2e4875b12b5a70d0630e43d9900726a0593"
        },
        "date": 1791628782578,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.466708,
            "unit": "ms",
            "extra": "median of 5 runs: 0.453, 0.465, 0.467, 0.472, 0.479 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.367869499999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.221, 16.292, 16.368, 16.388, 16.660 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.44235800000001,
            "unit": "ms",
            "extra": "median of 5 runs: 99.928, 101.425, 101.442, 101.483, 101.826 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "f6f679930813763040447ff5cd25a34828595ed1",
          "message": "Refuse the workspace read in the said-link window test\n\nFollowing the clone's link opens Settings at the workspace's Repos, which\nreads the workspace. The test's core answered that read with nothing, and\non CI the read landed before the test ended and threw while drawing it.\nThe core now refuses the read, so the level says so.\n\nRefs #1201\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T15:20:53+04:00",
          "tree_id": "6128d8957d2c07becc625ebc8f2dc800ca436dc0",
          "url": "https://github.com/purlis/purlis/commit/f6f679930813763040447ff5cd25a34828595ed1"
        },
        "date": 1791631456764,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.513359,
            "unit": "ms",
            "extra": "median of 5 runs: 0.498, 0.508, 0.513, 0.514, 0.521 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.055292,
            "unit": "ms",
            "extra": "median of 5 runs: 16.946, 16.966, 17.055, 17.279, 17.298 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.025062,
            "unit": "ms",
            "extra": "median of 5 runs: 102.630, 103.877, 104.025, 104.151, 106.011 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "16e7af4d80a99414d1257ad922e0374933e892d6",
          "message": "Say in the handoff guide that an orphaned task ends at its report\n\ncrates/purlis-core/docs/handoff.md still said a task whose asking chat\nhad gone ends nothing and asks for the person. It now says the task ends\nat its report, as every other task does, and which tasks stay open and\nraise an item.\n\nRefs #1510\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T17:15:46+04:00",
          "tree_id": "a052d9749e8a4155e164bbbb49955c47aebac170",
          "url": "https://github.com/purlis/purlis/commit/16e7af4d80a99414d1257ad922e0374933e892d6"
        },
        "date": 1791638910004,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.509377,
            "unit": "ms",
            "extra": "median of 5 runs: 0.496, 0.508, 0.509, 0.522, 0.526 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.241044000000002,
            "unit": "ms",
            "extra": "median of 5 runs: 16.841, 16.920, 17.241, 17.351, 17.557 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.85768200000001,
            "unit": "ms",
            "extra": "median of 5 runs: 103.063, 103.352, 104.858, 104.860, 106.271 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "3e6c1725c35c04b67cbcac3f17cb9b514e46d29e",
          "message": "Open the once-per-run Keychain test with unsteered!, and say when the app wipes a kept token\n\nThe guard that every test in purlis-core/tests sheds the shell's steering\nfailed on the new test (#1654). The Kept doc said a token is wiped when the\nlast context holding it goes; in the app that memory is never dropped, so it\nnow says a token is wiped when purlis deletes its item, and otherwise goes\nwith the process.\n\nRefs #1654\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T18:21:02+04:00",
          "tree_id": "8cd8687f29173d854f54f426df87cc5bb25cc526",
          "url": "https://github.com/purlis/purlis/commit/3e6c1725c35c04b67cbcac3f17cb9b514e46d29e"
        },
        "date": 1791642948336,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.458235,
            "unit": "ms",
            "extra": "median of 5 runs: 0.444, 0.446, 0.458, 0.465, 0.477 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.3178855,
            "unit": "ms",
            "extra": "median of 5 runs: 16.186, 16.270, 16.318, 16.619, 16.888 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.136529,
            "unit": "ms",
            "extra": "median of 5 runs: 100.325, 100.942, 101.137, 101.703, 102.166 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "53e42d1ebc4d4a2f520b77d9fd982d61b694c42e",
          "message": "Show the focused workspace's chats in the Chats list\n\nThe Chats section listed every running chat of the project whatever\nworkspace the strip had focused. That was the 2026-10-07 design (#1447,\nADR 0038's amendment), not a regression; the operator's report of\n2026-10-10 replaces it.\n\nThe section now lists the trees whose top row works in the focused\nworkspace, each with every task below it wherever the task works. A chat\nstarted at the plane root is the root view's. An \"all workspaces\" chip\nbeside \"needs you\" and \"working\" lists every workspace again, and a row\nasked for elsewhere (the explorer's tasks-from-other-places line) turns it\non. A chat in another workspace that needs you is named on the line under\nthe filter with a Go button, and the title bar's queue still lists it.\n\nADR 0038 gains a 2026-10-10 amendment saying so.\n\nCloses #1655\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T18:27:39+04:00",
          "tree_id": "df274053d5e0274bc85faef3586e2fb83ff4cfd3",
          "url": "https://github.com/purlis/purlis/commit/53e42d1ebc4d4a2f520b77d9fd982d61b694c42e"
        },
        "date": 1791643945846,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5326215,
            "unit": "ms",
            "extra": "median of 5 runs: 0.490, 0.511, 0.533, 0.553, 0.572 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.97886,
            "unit": "ms",
            "extra": "median of 5 runs: 16.601, 16.736, 16.979, 16.984, 17.259 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.929711,
            "unit": "ms",
            "extra": "median of 5 runs: 102.928, 103.512, 103.930, 105.539, 106.180 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "f0dc1d257b14a678ae983275f7eabaafc08f03b2",
          "message": "Work out the Activity mark's contrast outside the page in its scenario spec\n\nThe marks test named its colour helpers inside browser.execute, which the\nspec's bundler wraps in a __name the page does not have, so the WebView\nthrew a ReferenceError on both engines. The page now reads only the colours\n(the nearest opaque background found in a loop), and the contrast is worked\nout in the spec, as the other specs that measure in the page do.\n\nRefs #1457\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T18:54:01+04:00",
          "tree_id": "9701bf6278daea11d401797bdf4ae0df318cf1d7",
          "url": "https://github.com/purlis/purlis/commit/f0dc1d257b14a678ae983275f7eabaafc08f03b2"
        },
        "date": 1791644842598,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.473017,
            "unit": "ms",
            "extra": "median of 5 runs: 0.462, 0.464, 0.473, 0.473, 0.475 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.671819999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.226, 16.526, 16.672, 16.690, 16.892 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.5374055,
            "unit": "ms",
            "extra": "median of 5 runs: 101.080, 101.457, 101.537, 102.420, 102.666 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "05e53dbbfac5043ed0a8cded2be287697e91d74a",
          "message": "Read the fresh mark by its words on main: \"project updated since this chat started\"\n\nRefs #1204\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T19:32:43+04:00",
          "tree_id": "ec4f506f91e5c75579e80ee9c5273f459a094ad8",
          "url": "https://github.com/purlis/purlis/commit/05e53dbbfac5043ed0a8cded2be287697e91d74a"
        },
        "date": 1791646725628,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.333057,
            "unit": "ms",
            "extra": "median of 5 runs: 0.319, 0.326, 0.333, 0.337, 0.339 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.4086585,
            "unit": "ms",
            "extra": "median of 5 runs: 16.349, 16.388, 16.409, 16.631, 16.795 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.978328,
            "unit": "ms",
            "extra": "median of 5 runs: 101.206, 101.744, 101.978, 102.187, 102.468 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "fc3f0a2ebaf869c5238e511a44fae1e43d1e68d1",
          "message": "Count the extension record's open among the window-only commands\n\nopen_extension_record joined WINDOW_ONLY (#1296), so the link-side test\nthat counts the list names it and counts one more.\n\nRefs #1296\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T20:08:05+04:00",
          "tree_id": "070d4206ebe72f6bd5fcbd121d5ec90ec7e094df",
          "url": "https://github.com/purlis/purlis/commit/fc3f0a2ebaf869c5238e511a44fae1e43d1e68d1"
        },
        "date": 1791649124838,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.496208,
            "unit": "ms",
            "extra": "median of 5 runs: 0.461, 0.487, 0.496, 0.506, 0.517 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.8854845,
            "unit": "ms",
            "extra": "median of 5 runs: 16.372, 16.629, 16.885, 17.077, 17.116 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.919783,
            "unit": "ms",
            "extra": "median of 5 runs: 102.727, 103.077, 103.920, 103.950, 106.234 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "5261af8971f74df1b3b1fe26951b1d335def65c1",
          "message": "Draw a hidden view as nothing: the views' display outranked hidden\n\n`.region-view { display: flex }` beats the browser's `[hidden] { display: none }`, so the real\nwindow drew the Explorer view under the Chats view (regions.e2e.ts caught it). App.css says\n`display: none` again for a hidden view, after it, and RegionFrame.test.tsx reads the cascade\nto hold it. keyboard-reach.e2e.ts counts the activity bar among the window's tab lists.\n\nRefs #1673\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T20:41:00+04:00",
          "tree_id": "32de7e1e07fc4d8cfaa45894de7dbf62bd702fc7",
          "url": "https://github.com/purlis/purlis/commit/5261af8971f74df1b3b1fe26951b1d335def65c1"
        },
        "date": 1791651099597,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5257050000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.509, 0.511, 0.526, 0.527, 0.539 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.8208755,
            "unit": "ms",
            "extra": "median of 5 runs: 16.268, 16.683, 16.821, 17.105, 17.299 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.857316,
            "unit": "ms",
            "extra": "median of 5 runs: 102.649, 103.119, 105.857, 105.869, 106.088 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "e0808d7e969517d96aacd0c378e3652d60c4c920",
          "message": "Count Blocked lately's Allow among the window's own commands\n\nThe window-only list gained `allow_blocked_host` (issue 1662), but the\ntest that pins the list's length and names each person's act did not.\nIt now names the command and counts it.\n\nRefs #1662\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T21:42:59+04:00",
          "tree_id": "2cae300a4b78bc003a3150908c0ed95e4a3ab940",
          "url": "https://github.com/purlis/purlis/commit/e0808d7e969517d96aacd0c378e3652d60c4c920"
        },
        "date": 1791654809010,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5062530000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.494, 0.504, 0.506, 0.512, 0.525 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.0864145,
            "unit": "ms",
            "extra": "median of 5 runs: 16.463, 17.081, 17.086, 17.264, 17.381 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.760216,
            "unit": "ms",
            "extra": "median of 5 runs: 103.115, 104.296, 104.760, 104.800, 107.536 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "9ca90c46d8f6ece5b1581572954c262d671723fa",
          "message": "Start the first task's second chat on the stand-in Claude Code\n\nThe tab suggests another harness for the second chat so the two runs\ndiffer, and picked codex, which this run does not have. The spec picks\nclaude for it, the one harness the run stands in.\n\nRefs #1670\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T21:48:03+04:00",
          "tree_id": "333c1dba70cc481046b944c23b677d9c100bc778",
          "url": "https://github.com/purlis/purlis/commit/9ca90c46d8f6ece5b1581572954c262d671723fa"
        },
        "date": 1791655729939,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.2659105,
            "unit": "ms",
            "extra": "median of 5 runs: 0.254, 0.255, 0.266, 0.275, 0.280 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.488737500000003,
            "unit": "ms",
            "extra": "median of 5 runs: 16.443, 16.444, 16.489, 16.523, 16.554 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.2083925,
            "unit": "ms",
            "extra": "median of 5 runs: 101.070, 101.136, 101.208, 101.459, 102.431 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "8a6bb2c0a2bbdbacdaa0014f433fb7b0fc6037e2",
          "message": "Train 38 review: pin the local-address check against every IPv6 spelling\n\nTwo pure tests hold the check against loopback, unspecified, link-local and metadata\naddresses, this machine's own IPv6 address, and the IPv6 forms that carry an IPv4 one:\nby literal, and by what a listed name resolves to.\n\nRefs #1664\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T22:35:12+04:00",
          "tree_id": "f53580d59d5a77fdfcd7f18e66702f284beaea7c",
          "url": "https://github.com/purlis/purlis/commit/8a6bb2c0a2bbdbacdaa0014f433fb7b0fc6037e2"
        },
        "date": 1791658068504,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.46569400000000005,
            "unit": "ms",
            "extra": "median of 5 runs: 0.458, 0.459, 0.466, 0.466, 0.467 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.519492,
            "unit": "ms",
            "extra": "median of 5 runs: 16.220, 16.330, 16.519, 16.637, 16.936 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.761952,
            "unit": "ms",
            "extra": "median of 5 runs: 100.295, 100.553, 101.762, 102.641, 103.183 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "66f533c9592329e655964be1de7b2f4e047fb7d9",
          "message": "Bump the site group in /site with 2 updates\n\nBumps the site group in /site with 2 updates: [@astrojs/markdown-remark](https://github.com/withastro/astro/tree/HEAD/packages/markdown/remark) and [astro](https://github.com/withastro/astro/tree/HEAD/packages/astro).\n\n\nUpdates `@astrojs/markdown-remark` from 7.3.1 to 7.3.2\n- [Release notes](https://github.com/withastro/astro/releases)\n- [Changelog](https://github.com/withastro/astro/blob/main/packages/markdown/remark/CHANGELOG.md)\n- [Commits](https://github.com/withastro/astro/commits/@astrojs/markdown-remark@7.3.2/packages/markdown/remark)\n\nUpdates `astro` from 7.3.5 to 7.3.6\n- [Release notes](https://github.com/withastro/astro/releases)\n- [Changelog](https://github.com/withastro/astro/blob/main/packages/astro/CHANGELOG.md)\n- [Commits](https://github.com/withastro/astro/commits/astro@7.3.6/packages/astro)\n\n---\nupdated-dependencies:\n- dependency-name: \"@astrojs/markdown-remark\"\n  dependency-version: 7.3.2\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: site\n- dependency-name: astro\n  dependency-version: 7.3.6\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: site\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-10-10T22:52:12+04:00",
          "tree_id": "2c009f779b27be08ce075c96c31ccb5a0cba072d",
          "url": "https://github.com/purlis/purlis/commit/66f533c9592329e655964be1de7b2f4e047fb7d9"
        },
        "date": 1791659336142,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5348200000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.525, 0.530, 0.535, 0.558, 0.559 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.0568085,
            "unit": "ms",
            "extra": "median of 5 runs: 16.593, 16.746, 17.057, 17.061, 17.253 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.60386700000001,
            "unit": "ms",
            "extra": "median of 5 runs: 102.911, 103.869, 104.604, 105.113, 109.945 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "9b3a1195051116b73879793c2133f6f55d0af331",
          "message": "ci: annotate the rust job with its slowest tests and test binaries\n\nnextest's ci profile writes a JUnit report; tools/slowest-tests.mjs reads it and\nprints two notices, the 30 slowest tests and the 30 binaries whose tests took\nlongest in all, so what makes \"Run the tests\" take ~460 s is readable through\nthe checks API without the job's log. Step 1 of the next CI speed-up round: it\ndecides which tests to fix before splitting the job across runners.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4EJSYV68AZG1SWR231JAEZE\nPurlis-Persona: steward",
          "timestamp": "2026-10-10T23:38:37+04:00",
          "tree_id": "e321c1a04d9e82e2020eab6d6bc80be17418b459",
          "url": "https://github.com/purlis/purlis/commit/9b3a1195051116b73879793c2133f6f55d0af331"
        },
        "date": 1791662321344,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.41330100000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 0.405, 0.408, 0.413, 0.428, 0.442 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.6557495,
            "unit": "ms",
            "extra": "median of 5 runs: 16.246, 16.624, 16.656, 16.782, 16.792 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.8031455,
            "unit": "ms",
            "extra": "median of 5 runs: 101.544, 101.697, 101.803, 102.254, 103.217 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "80f32e773310e7637407c069d0cd7331d5ffafe2",
          "message": "Hand the opencode permission hook its session by purlis's name alone\n\nThe shim's new permission hook also set the session variable's old name,\none spelling more than opencode.rs is allowed. Only a purlis that reads\nthe new name runs this hook, so the old one is dropped (issue 1691).\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T00:14:10+04:00",
          "tree_id": "b46dfd156c26773a2890495edf7e7a207f1f737d",
          "url": "https://github.com/purlis/purlis/commit/80f32e773310e7637407c069d0cd7331d5ffafe2"
        },
        "date": 1791664118307,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4826865,
            "unit": "ms",
            "extra": "median of 5 runs: 0.453, 0.470, 0.483, 0.484, 0.485 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.3159285,
            "unit": "ms",
            "extra": "median of 5 runs: 16.190, 16.256, 16.316, 16.454, 16.646 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.506145,
            "unit": "ms",
            "extra": "median of 5 runs: 100.996, 101.191, 101.506, 102.808, 103.137 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "2b617fc0c76ff57b494ba192f2a1700d4e3763b9",
          "message": "Bound every side view by its region, so nothing in one makes the window scroll\n\nWith the left side open, the window was 1369 px tall in a 775 px window and the status line's\nclick scrolled it by 377 px. The Chats rows' visually hidden state words (#1675) are placed\nabsolutely, and with no positioned box between them and the page, the words of rows far down\nthe list hung below the window and added to the page's own height. The Chats view is now their\ncontaining block, and every region view is positioned and clips what would hang out of it, so a\nview scrolls inside itself and never grows the window.\n\nA window test holds that every view is positioned and clips, and the status line scenario holds\nthat the window is not scrolled and no bigger than itself when the alerts drawer opens.\n\nRefs #1675, #1676\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T00:43:50+04:00",
          "tree_id": "ea9e492bee1230ebf5c03b85f044489facf21189",
          "url": "https://github.com/purlis/purlis/commit/2b617fc0c76ff57b494ba192f2a1700d4e3763b9"
        },
        "date": 1791665685866,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.466246,
            "unit": "ms",
            "extra": "median of 5 runs: 0.463, 0.466, 0.466, 0.476, 0.480 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.3111725,
            "unit": "ms",
            "extra": "median of 5 runs: 16.253, 16.276, 16.311, 16.849, 17.006 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.7040365,
            "unit": "ms",
            "extra": "median of 5 runs: 100.113, 101.148, 101.704, 102.345, 102.359 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "942ebc6e1d73c408d5e005ca721d03ea4936fecb",
          "message": "ci: run the rust suite and the macOS scenario specs in two parts each\n\nThe two jobs a pull request waits on longest each run on two runners: the rust suite with\nnextest's `--partition count:N/2` (fmt and clippy in part 1, doctests in part 2) and the macOS\nscenario specs with WebdriverIO's `--shard N/2` (the five single-app runs in part 1). Each part\nbuilds for itself; a handed-on build saves runner minutes, not the wait. Two small jobs carry the\nrequired checks' names, `rust (fmt, clippy, test)` and `scenario tests (macos-latest)`, and pass\nonly when both parts did; Linux's scenario job keeps its name as a job of its own with the same\nanchored steps. The job ids `rust` and `scenario` stay, so main's Rust caches keep their keys.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4EJSYV68AZG1SWR231JAEZE\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T01:13:32+04:00",
          "tree_id": "c6879cbdf30b4bce3924c3fbdeff650130b4adac",
          "url": "https://github.com/purlis/purlis/commit/942ebc6e1d73c408d5e005ca721d03ea4936fecb"
        },
        "date": 1791666923311,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4953475,
            "unit": "ms",
            "extra": "median of 5 runs: 0.474, 0.494, 0.495, 0.502, 0.508 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.122338,
            "unit": "ms",
            "extra": "median of 5 runs: 16.706, 16.969, 17.122, 17.142, 17.271 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.005258,
            "unit": "ms",
            "extra": "median of 5 runs: 102.357, 103.675, 105.005, 105.092, 105.596 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "a0b92fd73cada7442b1de2e3c1d3bf98e4f0e001",
          "message": "ci: run the web checks in two parts, and the doctests in rust's first\n\nAfter #1712, vitest's 8 to 10 minutes made `web` the longest job a pull request waits on. It\nruns in two parts with `vitest run --shard=N/2`: part 1 also the type check, lint and format\ncheck, part 2 the licences and the tools' tests. `web-checks` carries the required name\n`web (types, lint, format, test)`. The doctests move to rust's part 1, which finished three\nminutes before part 2 on #1712.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4EJSYV68AZG1SWR231JAEZE\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T01:34:41+04:00",
          "tree_id": "5fbec99c91bbe4082a6f6fae3f55f4c9631b393f",
          "url": "https://github.com/purlis/purlis/commit/a0b92fd73cada7442b1de2e3c1d3bf98e4f0e001"
        },
        "date": 1791668936607,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.525529,
            "unit": "ms",
            "extra": "median of 5 runs: 0.521, 0.521, 0.526, 0.533, 0.546 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.9763045,
            "unit": "ms",
            "extra": "median of 5 runs: 16.460, 16.831, 16.976, 17.117, 17.413 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.41169149999999,
            "unit": "ms",
            "extra": "median of 5 runs: 102.872, 103.634, 104.412, 104.500, 104.543 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "417c27b6be2b120c58152779b7d6ae9051d00085",
          "message": "Keep the window's own projects last when the whole layout file must shrink\n\nReview fix for train 44 (#1686). The whole-file guard let go of projects by recents alone,\nso a project another window opened more recently outlived the one in front of the window\nthat wrote; it now lets go of the projects the window did not send first. And a layout that\nwould still pass MAX_BYTES with no project left is not written, so the file on disk stays\nrather than being replaced by one the next launch refuses whole. A window test pins the\nversion 1 to version 2 path with a view's facet.\n\nRefs #1686\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T02:08:53+04:00",
          "tree_id": "913b30a4c076be42ab130091eafed9df10fba0b6",
          "url": "https://github.com/purlis/purlis/commit/417c27b6be2b120c58152779b7d6ae9051d00085"
        },
        "date": 1791671029674,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5293895,
            "unit": "ms",
            "extra": "median of 5 runs: 0.504, 0.522, 0.529, 0.530, 0.550 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.9524865,
            "unit": "ms",
            "extra": "median of 5 runs: 16.267, 16.914, 16.952, 17.065, 17.355 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.0651485,
            "unit": "ms",
            "extra": "median of 5 runs: 103.857, 104.980, 105.065, 105.352, 105.897 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "441b3dc160e054b5badd8729d604c8c042109369",
          "message": "Open the stop test unsteered first, and wait out a look's first look in a test\n\nThe reader test opened with its `use` lines before `unsteered!()`, which the\ndirectory's guard refuses. The new-workspace look test wrote its todo while\nthe look could still be taking its first look at the new folder, so on CI\nthe write was in that first look and nothing told it.\n\nRefs #1189, Refs #756\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T02:40:34+04:00",
          "tree_id": "5aa43b889f05ea7c6b0996f7d09c8a74bc9a78e7",
          "url": "https://github.com/purlis/purlis/commit/441b3dc160e054b5badd8729d604c8c042109369"
        },
        "date": 1791672383110,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.2896575,
            "unit": "ms",
            "extra": "median of 5 runs: 0.273, 0.283, 0.290, 0.294, 0.296 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.520027,
            "unit": "ms",
            "extra": "median of 5 runs: 16.421, 16.434, 16.520, 16.538, 16.600 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.346723,
            "unit": "ms",
            "extra": "median of 5 runs: 100.644, 101.300, 101.347, 102.037, 102.061 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "9c525dde6644d6fede4db1a84da720c53707a328",
          "message": "Train 46 review: a late failure silences no ask, and a grant waits for the row to settle\n\n- asknotify: a chat's own wait is an update only for the app's reasons that are not a task\n  that came to nothing. With #1693's derive, a chat in the queue only for failures asks\n  nothing; one that also waits on the person for itself is an ask, so a failure left\n  unlooked-at no longer silences that chat's notification (reasons_besides_failures).\n- Inbox: an update's Allow is held for SETTLE_MS after the row's own words or answers\n  change, or after it is laid out somewhere else, not only after the stops above it change.\n  Tests: the keyboard path, a changed grant sentence, a row moved by layout alone.\n- inboxupdates store: a file larger than any this store writes (MOST_BYTES) reads as none\n  and is written over, as an unreadable one is.\n- WINDOW_ONLY's count in the UI RPC test gains the three update commands.\n\nRefs #1693 #1694\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T03:07:03+04:00",
          "tree_id": "4b698b088d657d36f37ff893ed8bf35b20bd4d16",
          "url": "https://github.com/purlis/purlis/commit/9c525dde6644d6fede4db1a84da720c53707a328"
        },
        "date": 1791673973011,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5217645,
            "unit": "ms",
            "extra": "median of 5 runs: 0.477, 0.489, 0.522, 0.534, 0.542 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.019177,
            "unit": "ms",
            "extra": "median of 5 runs: 16.762, 16.814, 17.019, 17.129, 17.392 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.678424,
            "unit": "ms",
            "extra": "median of 5 runs: 104.038, 104.664, 105.678, 105.805, 105.892 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "080acb08862c216a811df26baae13f1abaf06de0",
          "message": "Expect a vault tab's heading to name its provider as New vault offers it\n\nD-1719-3 draws provider names from one table, so a keyring vault's heading now says\n\"System keychain\", not the id.\n\nRefs #1719\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T04:30:05+04:00",
          "tree_id": "76b376c9639bf5b4a6d27873aa98e042dcdb4cc8",
          "url": "https://github.com/purlis/purlis/commit/080acb08862c216a811df26baae13f1abaf06de0"
        },
        "date": 1791678856317,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5278594999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 0.499, 0.523, 0.528, 0.529, 0.573 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.693285,
            "unit": "ms",
            "extra": "median of 5 runs: 16.313, 16.514, 16.693, 16.965, 17.068 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.79307800000001,
            "unit": "ms",
            "extra": "median of 5 runs: 102.105, 103.788, 103.793, 103.963, 104.700 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "5eb46da04e67537b8e769d4621c8a381b11a17bf",
          "message": "Start the re-find test on the fixture's own first commit\n\nThe fixture's clone already commits README.md, so committing it again\nfound nothing to commit and failed on CI (the test's first run there).\n\nRefs #1152\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T04:43:38+04:00",
          "tree_id": "9c96db8f7681b73ba98d36119b17cde5dda941a6",
          "url": "https://github.com/purlis/purlis/commit/5eb46da04e67537b8e769d4621c8a381b11a17bf"
        },
        "date": 1791679908226,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.47616749999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 0.452, 0.465, 0.476, 0.479, 0.485 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.369581,
            "unit": "ms",
            "extra": "median of 5 runs: 16.306, 16.334, 16.370, 16.386, 16.497 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.70248050000001,
            "unit": "ms",
            "extra": "median of 5 runs: 100.451, 101.499, 101.702, 101.944, 102.539 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "ab5d1ffeb151088addef468ca8817a1f5f540268",
          "message": "Hand a chat's move to every listener in the Chats window tests\n\nThe test core gave a chat-moved event to the last listener alone. The asks registry listens\nfor moves too, so whether the hand counted a queued chat depended on which listened last: the\nhand-press test failed about one run in three (CI web part 2). Every listener hears it now, as\nevery one does from the core.\n\nRefs #1695\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T05:32:25+04:00",
          "tree_id": "dc6b5945b90416d788d85b7944cb810cdf486dd9",
          "url": "https://github.com/purlis/purlis/commit/ab5d1ffeb151088addef468ca8817a1f5f540268"
        },
        "date": 1791682415992,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.504537,
            "unit": "ms",
            "extra": "median of 5 runs: 0.471, 0.503, 0.505, 0.522, 0.536 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.005235499999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.967, 17.001, 17.005, 17.080, 17.327 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.48045400000001,
            "unit": "ms",
            "extra": "median of 5 runs: 102.583, 103.602, 104.480, 105.410, 105.702 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "5c16957674911a0099c91115026f657841a8fe9d",
          "message": "Add up the record's block counts without overflowing\n\nA line's times is read back from the file, so counts and hosts_refused\nsaturate rather than wrap or panic (train 51 review, #1681).\n\nRefs #1681\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T05:54:57+04:00",
          "tree_id": "cc523a40a40877e2034912b2614673059a6bc0a6",
          "url": "https://github.com/purlis/purlis/commit/5c16957674911a0099c91115026f657841a8fe9d"
        },
        "date": 1791683767400,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.289982,
            "unit": "ms",
            "extra": "median of 5 runs: 0.276, 0.288, 0.290, 0.292, 0.322 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.6181225,
            "unit": "ms",
            "extra": "median of 5 runs: 16.134, 16.571, 16.618, 16.710, 16.747 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.472139,
            "unit": "ms",
            "extra": "median of 5 runs: 100.962, 101.170, 101.472, 101.588, 101.683 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "66191aab7e25a00311506f70d6e8d9aee3173833",
          "message": "Wrap the profile users paragraph at the module's width\n\nTrain 47 review of lane KE: one line of harness_profiles' module comment\nran past 100 columns after the model: sentence joined it.\n\nRefs #1720\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T06:42:33+04:00",
          "tree_id": "bf1f130bc262512b2836b18f20c0f990bfd68073",
          "url": "https://github.com/purlis/purlis/commit/66191aab7e25a00311506f70d6e8d9aee3173833"
        },
        "date": 1791686632985,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5235485,
            "unit": "ms",
            "extra": "median of 5 runs: 0.517, 0.522, 0.524, 0.529, 0.562 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.7847425,
            "unit": "ms",
            "extra": "median of 5 runs: 16.574, 16.633, 16.785, 17.207, 17.323 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.1258655,
            "unit": "ms",
            "extra": "median of 5 runs: 101.230, 103.783, 104.126, 104.985, 106.134 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "dbfc60939fa5704de39690cac87da20669f63543",
          "message": "Write a tool line without a speaker exactly as before\n\nThe doing line's wire is unchanged where the hook read no run: the\nspeaker is left out, and read back as no one's. A line that names one\nround-trips.\n\nRefs #1601\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T07:29:29+04:00",
          "tree_id": "d5bded9c75c997ecb33040275b298fc5804ceeed",
          "url": "https://github.com/purlis/purlis/commit/dbfc60939fa5704de39690cac87da20669f63543"
        },
        "date": 1791689438450,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.46356549999999996,
            "unit": "ms",
            "extra": "median of 5 runs: 0.458, 0.458, 0.464, 0.469, 0.474 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.295592999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.156, 16.233, 16.296, 16.301, 16.831 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.7405675,
            "unit": "ms",
            "extra": "median of 5 runs: 101.245, 101.646, 101.741, 102.071, 102.206 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "863ec96f9b92b5df44328f32f499eac2c01e3ca1",
          "message": "Read a plane look's focus count before its thread starts\n\nThe look thread read the window focus count as its first act. A focus that\ncame between the watch's start and that read was taken as already seen, so\nthe look waited out its whole interval instead of looking. Under CI load\nthat is what a_storm_of_focus_changes_is_one_look hit: its twenty focuses\nlanded before the thread ran, and it timed out. With a 300 ms sleep put at\nthe top of the thread the old code fails the test every time and the new\ncode passes. The count is now read where the first look is taken, before\nthe spawn.\n\nCloses #1731\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T07:52:56+04:00",
          "tree_id": "2fa94147e07e1bc6aa129ffb07b619cc7046ec4b",
          "url": "https://github.com/purlis/purlis/commit/863ec96f9b92b5df44328f32f499eac2c01e3ca1"
        },
        "date": 1791690846130,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5508055000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.530, 0.548, 0.551, 0.559, 0.568 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.011319,
            "unit": "ms",
            "extra": "median of 5 runs: 16.503, 16.692, 17.011, 17.526, 17.666 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 106.304351,
            "unit": "ms",
            "extra": "median of 5 runs: 101.933, 105.198, 106.304, 106.343, 106.785 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "a66e84c7f7d5d7ae93d7d04fc623c174ccb7eab2",
          "message": "Draw the window's own lines as part of the Inbox's Notices\n\nInside the Inbox the window's list loses the band's rule: it reads as the top of\nthe Notices it is listed with (#1695).\n\nRefs #1695\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T08:53:34+04:00",
          "tree_id": "250a87ea2903783630e59296dd8fe6f71c64e453",
          "url": "https://github.com/purlis/purlis/commit/a66e84c7f7d5d7ae93d7d04fc623c174ccb7eab2"
        },
        "date": 1791694677536,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.528427,
            "unit": "ms",
            "extra": "median of 5 runs: 0.457, 0.520, 0.528, 0.531, 0.532 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.175812,
            "unit": "ms",
            "extra": "median of 5 runs: 16.583, 16.962, 17.176, 17.192, 17.687 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.301671,
            "unit": "ms",
            "extra": "median of 5 runs: 103.096, 103.892, 104.302, 105.624, 106.298 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "5d6c2d6e5bc6fedd754821aaa7c8404e8fbc1b22",
          "message": "Wait for every host to join before answering in the arriving-host window test\n\nmain's web job is red on this test since train 54: the Notice's hosts join\nas the registry lists them, so on a slow runner the press could fall inside\nthe settle time of the last join, or on the Notice as first drawn. The test\nnow waits until every host is drawn, settles, and answers the Notice on\nscreen then.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T09:34:23+04:00",
          "tree_id": "aef3e9305a47d3195486ae4d4f9a9fbbfa0a899b",
          "url": "https://github.com/purlis/purlis/commit/5d6c2d6e5bc6fedd754821aaa7c8404e8fbc1b22"
        },
        "date": 1791696940602,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.533684,
            "unit": "ms",
            "extra": "median of 5 runs: 0.519, 0.532, 0.534, 0.536, 0.540 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.2784735,
            "unit": "ms",
            "extra": "median of 5 runs: 16.711, 16.962, 17.278, 17.281, 17.377 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.057154,
            "unit": "ms",
            "extra": "median of 5 runs: 103.284, 103.981, 104.057, 106.383, 107.590 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "d8de874aaf389e8b66296ccaaacb9124747e0b06",
          "message": "Meet CI: a refused Discard's tests say what changed, a test sheds steering first\n\nThe discard test in handoff.rs still expected the plain \"changed since you\nwere asked\" refusal; it now names what changed (the file and the commit),\nand a folder written again with the same list says WRITTEN_AGAIN, as #1472\nwords them. The new worktree-dispatch test opens with unsteered!() before\nits comment, as the directory's guard reads it.\n\nRefs #1472\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T09:37:34+04:00",
          "tree_id": "d22805465cedf811ac8ff318e37c4525dd6b21dc",
          "url": "https://github.com/purlis/purlis/commit/d8de874aaf389e8b66296ccaaacb9124747e0b06"
        },
        "date": 1791697721389,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.47567099999999995,
            "unit": "ms",
            "extra": "median of 5 runs: 0.465, 0.465, 0.476, 0.478, 0.482 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.56595,
            "unit": "ms",
            "extra": "median of 5 runs: 16.524, 16.558, 16.566, 16.610, 16.654 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.6093185,
            "unit": "ms",
            "extra": "median of 5 runs: 101.610, 102.540, 102.609, 102.953, 103.247 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "e7bdfbe95bcf33bb612e115ce411855879a8ca85",
          "message": "Read a branch's status once its watch holds, as a folder is read\n\nThe explorer read what a newly open branch changed at the same moment it\nasked for the branch to be watched. A file an agent wrote after that read\nand before the watch held was told by nothing, so it was drawn without its\nmark until something else moved in the branch. On inotify, watching a branch\nlists its folders through the bounded reader and can take longer than the\nstatus read beside it: the Linux scenario spec \"marks a file an agent adds\nor changes without a refresh\" met that on main d8de874a and on train 57.\n\nA newly open branch's status is now read once the branch_watch naming it\nhas answered, or after WATCH_WAIT_MS, and read once more when a watch slower\nthan that answers: the contract branchFolders.ts keeps for folders (#1427).\nThe wait helpers move from branchFolders.ts's own watch to any watch's.\n\nRefs #1107\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T10:38:01+04:00",
          "tree_id": "8b2f6a5f9f3f6c2964d99e63fc2d5c4549558692",
          "url": "https://github.com/purlis/purlis/commit/e7bdfbe95bcf33bb612e115ce411855879a8ca85"
        },
        "date": 1791701476093,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4635515,
            "unit": "ms",
            "extra": "median of 5 runs: 0.446, 0.460, 0.464, 0.467, 0.472 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.4238855,
            "unit": "ms",
            "extra": "median of 5 runs: 16.198, 16.424, 16.424, 16.437, 16.745 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.613956,
            "unit": "ms",
            "extra": "median of 5 runs: 100.010, 101.286, 101.614, 102.154, 102.554 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "53007cda1a3481481bd9e7177b715f8e29eea83a",
          "message": "Docs say the guards read every level a shell is handed\n\nhooks.md and handoff.md still said the handoff and dispatch guards look\none level into a string a shell runs and no deeper. They now read every\nlevel up to the nesting cap (a -c string, an eval, a here-string, a pipe\nor process substitution into a shell, a heredoc fed to one, a find -exec),\nthe handoff guard a case branch and a function body too, and a string\nhanded on past the cap is refused unread. What stays unseen is named\nbeside it. The recorded handoff refusal sentence is left as recorded.\n\nRefs #1426 #1582 #1419\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T10:58:53+04:00",
          "tree_id": "a76575708d3c4718d656bf7ff15b82358e189d3f",
          "url": "https://github.com/purlis/purlis/commit/53007cda1a3481481bd9e7177b715f8e29eea83a"
        },
        "date": 1791702323022,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5149495,
            "unit": "ms",
            "extra": "median of 5 runs: 0.507, 0.508, 0.515, 0.518, 0.527 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.671315999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.367, 16.532, 16.671, 17.031, 17.070 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.01397,
            "unit": "ms",
            "extra": "median of 5 runs: 103.313, 104.757, 105.014, 105.016, 106.201 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "444fcbe153cfa8fb49be3fd96f3f5d1149fd5b54",
          "message": "Meet CI: scan an unparseable TOML file that is not the project's own\n\nTwo save-scan tests wrote a charter.toml that does not parse. That now\nmakes the project read-only (FR-24), and its save is refused before the\nscan runs. They scan a harness declaration instead.\n\nRefs #826\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T11:13:47+04:00",
          "tree_id": "7899c3df47633dee680cb66bfb50243151c00cfe",
          "url": "https://github.com/purlis/purlis/commit/444fcbe153cfa8fb49be3fd96f3f5d1149fd5b54"
        },
        "date": 1791703929438,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5192809999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 0.493, 0.513, 0.519, 0.544, 0.557 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.9793605,
            "unit": "ms",
            "extra": "median of 5 runs: 16.500, 16.854, 16.979, 17.060, 17.088 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.3192885,
            "unit": "ms",
            "extra": "median of 5 runs: 102.837, 105.052, 105.319, 105.394, 106.176 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "7779abe7199eff8935fee9269731354959c6e998",
          "message": "Check that a native select's and range's keys reach them, which is what the driver can show\n\nThe settings-limits spec failed on macOS and Linux alike: a typed letter\nnever picked in the override select, and ArrowRight never stepped the text\nsize. The cause is the suite's driver, not the app. The embedded WebDriver\n(tauri-plugin-wdio-webdriver 1.4.0, platform/executor.rs) presses a key by\ndispatching a script-made KeyboardEvent at the focused element. That event\nis untrusted, so no engine runs a select's or a range's default action for\nit; the driver acts out only a radio's arrows and a text box's typing.\n\nNothing of the app's eats the keys: no window, document or ancestor\nhandler prevents or stops arrows or letters for a form control. The spec\nnow checks exactly that, in the real WebView: the control holds the\nkeyboard, ArrowDown and a letter (select) and ArrowRight and ArrowLeft\n(range) arrive at it, reach the window unprevented, and the focus stays.\n\nThis also withdraws the WebKitGTK reading in the previous commit.\n\nRefs #626 #1545\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T11:46:32+04:00",
          "tree_id": "f1f4ddf1f826eb21cf9166b0c190bb7cbc642bb9",
          "url": "https://github.com/purlis/purlis/commit/7779abe7199eff8935fee9269731354959c6e998"
        },
        "date": 1791705102131,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.511881,
            "unit": "ms",
            "extra": "median of 5 runs: 0.484, 0.490, 0.512, 0.514, 0.514 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.684369500000003,
            "unit": "ms",
            "extra": "median of 5 runs: 16.312, 16.606, 16.684, 16.859, 17.272 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.3062285,
            "unit": "ms",
            "extra": "median of 5 runs: 102.889, 103.893, 104.306, 105.547, 105.646 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "144608420342d129e03f2189f22f92f8643fb615",
          "message": "Give the kept-link note its tier: device-bound, transient\n\nADR 0069 asks every Machine store to say whether it syncs. The note says why one machine\nkept a link, so it is device-bound, as the move journal beside it is.\n\nRefs #1698\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T11:59:20+04:00",
          "tree_id": "389b7acb86e8049f9bf625524909476729fd5e65",
          "url": "https://github.com/purlis/purlis/commit/144608420342d129e03f2189f22f92f8643fb615"
        },
        "date": 1791707069382,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5344789999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 0.529, 0.534, 0.534, 0.542, 0.555 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.133550999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.317, 17.091, 17.134, 17.147, 17.850 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.4999285,
            "unit": "ms",
            "extra": "median of 5 runs: 103.050, 104.117, 104.500, 105.358, 105.512 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "434b295ce2a9bfa6b1d0aecb9c1398241258ce79",
          "message": "Stop a chat's added token counts at their largest rather than wrap\n\nThe spend file adds each conversation's figure (#1457). Each count is\nthe harness's own number, so two large ones could overflow u64: a wrap\nin release, a panic in debug. The counts now saturate; a cost past the\nlargest float is not finite, which `spent` already reads as no figure.\n\nTrain 65 review fix. Also reflows one doc comment in filewatch.rs and\ndrops a stray space in branchwatch.rs.\n\nRefs #1457 #1745\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T12:55:53+04:00",
          "tree_id": "d2f790d1abfd728032f1657dc5b996321d48f0a7",
          "url": "https://github.com/purlis/purlis/commit/434b295ce2a9bfa6b1d0aecb9c1398241258ce79"
        },
        "date": 1791709085724,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4571745,
            "unit": "ms",
            "extra": "median of 5 runs: 0.448, 0.455, 0.457, 0.458, 0.459 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.311694000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 16.196, 16.221, 16.312, 16.565, 16.900 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.008883,
            "unit": "ms",
            "extra": "median of 5 runs: 100.792, 100.999, 101.009, 101.194, 102.838 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "4d937f48f8ed664f590a0dfaa56db6910a67a0d2",
          "message": "Say the read-only tab sentence is not drawn yet\n\nThe app now serves why a project is read-only (project_read_only), but\nno tab mounts ReadOnlyLine yet: that wiring is #826's remaining work.\nThe plane format marks the window's half decided, not yet written, and\nthe changelog fragment that said the tab shows it waits for the change\nthat draws it.\n\nRefs #826\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T14:01:34+04:00",
          "tree_id": "b640423f392fdf32a10e1a28588b1ddda863ad4e",
          "url": "https://github.com/purlis/purlis/commit/4d937f48f8ed664f590a0dfaa56db6910a67a0d2"
        },
        "date": 1791713094224,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.523398,
            "unit": "ms",
            "extra": "median of 5 runs: 0.510, 0.518, 0.523, 0.532, 0.535 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.154663,
            "unit": "ms",
            "extra": "median of 5 runs: 16.538, 17.139, 17.155, 17.469, 17.571 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.9075325,
            "unit": "ms",
            "extra": "median of 5 runs: 103.500, 104.384, 104.908, 105.715, 106.765 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "b49362b5e3a7420db9f758cdb2cc319cd9ce38e1",
          "message": "SandboxBlocks.window.test: wait for every refused host before reading them\n\nCI once found the Notice drawn before its third host's event had landed (`c.example.com:443`\nnot found). The hosts are now awaited, each of its copies a `code`.\n\nRefs #1738\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T14:33:08+04:00",
          "tree_id": "8c656823afe76629db501f17a5599a1469405327",
          "url": "https://github.com/purlis/purlis/commit/b49362b5e3a7420db9f758cdb2cc319cd9ce38e1"
        },
        "date": 1791715382946,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5360765,
            "unit": "ms",
            "extra": "median of 5 runs: 0.514, 0.532, 0.536, 0.557, 0.565 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.046534,
            "unit": "ms",
            "extra": "median of 5 runs: 16.411, 16.543, 17.047, 17.066, 17.212 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.70142000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 104.141, 104.213, 104.701, 105.279, 106.077 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "471271fcae6d7ff2b6909832bb35314e8545d3dc",
          "message": "e2e: drop train 62's leftover diagnostics; focus the left handle from the page\n\nTrain 62 left CI diagnostics in place while it chased the Linux cascade:\na recorder of every press on the window in regions.e2e, and failure\nmessages that dump the dialogs and picker state in opening.ts, vaults and\nworkspace-explorer. The cascade is fixed, so the specs go back to their\nplain waits and messages.\n\nThe width spec still clicked the left sidebar's handle, which is 0px\nacross in the Linux WebView, so the click could land beside it. It now\nfocuses the handle from the page, as the Panel's handles do.\n\nRefs #1737, #1738\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T15:02:23+04:00",
          "tree_id": "30bc735161fb04b0ddc4b34df880e08761fa73d9",
          "url": "https://github.com/purlis/purlis/commit/471271fcae6d7ff2b6909832bb35314e8545d3dc"
        },
        "date": 1791716721629,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5104285,
            "unit": "ms",
            "extra": "median of 5 runs: 0.496, 0.510, 0.510, 0.513, 0.535 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.146556,
            "unit": "ms",
            "extra": "median of 5 runs: 16.930, 16.969, 17.147, 17.228, 17.439 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.7687405,
            "unit": "ms",
            "extra": "median of 5 runs: 103.381, 103.543, 103.769, 104.263, 104.975 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "38c275e03f42641f037f7efb9be9434dd7e6d3d3",
          "message": "Draw the last hand-built form controls with the settings set\n\nThe design set's guard listed six files whose form controls were native\nand should have been the set's Field or Choice. Each is moved, and the\nguard's debt list is gone: a native input, select or textarea outside\nthe set is now either the surface itself, named with why, or a failure.\n\n- The set learns `chips` (any of a few over a list, one Tab stop with\n  the arrows between them) and a control with no row: `RowIds.label`\n  names it and `RowIds.title` carries its help, for a table's cell or a\n  pick inside a sentence. A select's option can be out of reach, and a\n  text field takes `inputMode` and `size`.\n- The Chats list's state chips are the set's chips; the dispatch\n  question's where-it-holds radios a Choice radio; the grant table's and\n  Add a grant's five selects Choice selects; the limits table's boxes\n  and the folder box Fields; and the host or folder to allow for a\n  sandbox block that named none a Field in a form row under its Notice.\n- Every moved control keeps its name, keyboard and tests; the busy\n  window tests pass unchanged.\n\nRefs #626\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T15:56:32+04:00",
          "tree_id": "6be42382ed6f2e0501eaebb4aefd7b9fbec5d94b",
          "url": "https://github.com/purlis/purlis/commit/38c275e03f42641f037f7efb9be9434dd7e6d3d3"
        },
        "date": 1791720443515,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.419597,
            "unit": "ms",
            "extra": "median of 5 runs: 0.407, 0.413, 0.420, 0.431, 0.437 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.471141,
            "unit": "ms",
            "extra": "median of 5 runs: 16.360, 16.405, 16.471, 16.604, 16.864 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.3981075,
            "unit": "ms",
            "extra": "median of 5 runs: 100.731, 101.366, 101.398, 101.967, 102.261 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "3d23c6bdd845e7255ad2acbdb50e47f7b97e580f",
          "message": "rulespelling: a unit test never reads the person's own settings file\n\nReview of lane QA for train 68 (#1325 line 4). The person's Claude Code\nsettings are read beside the project's; a unit test reaching the rules in\nforce would read the real ~/.claude/settings.json (or $CLAUDE_CONFIG_DIR)\nof whoever runs it, so its answer would hang on their machine. The tests\nthat need a file name it through Rules::in_force_with.\n\nRefs #1325\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T16:41:47+04:00",
          "tree_id": "7c9319f5939b04f76f390536600bb3479ab7f1d4",
          "url": "https://github.com/purlis/purlis/commit/3d23c6bdd845e7255ad2acbdb50e47f7b97e580f"
        },
        "date": 1791723078660,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5328035,
            "unit": "ms",
            "extra": "median of 5 runs: 0.512, 0.523, 0.533, 0.562, 0.563 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.087121,
            "unit": "ms",
            "extra": "median of 5 runs: 16.817, 17.040, 17.087, 17.225, 17.407 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.4358125,
            "unit": "ms",
            "extra": "median of 5 runs: 103.928, 104.223, 104.436, 104.461, 105.406 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "57ed1a6d132201f86832906df983ac86ad785c86",
          "message": "plane-format: needed_you counts waits on the person's input\n\nRefs #1457\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T17:30:23+04:00",
          "tree_id": "b37242d37d06e804b5307d275c54febff92c07b7",
          "url": "https://github.com/purlis/purlis/commit/57ed1a6d132201f86832906df983ac86ad785c86"
        },
        "date": 1791726138891,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5386545,
            "unit": "ms",
            "extra": "median of 5 runs: 0.515, 0.536, 0.539, 0.547, 0.548 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.438788,
            "unit": "ms",
            "extra": "median of 5 runs: 16.357, 16.376, 16.439, 17.034, 17.176 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.94136850000001,
            "unit": "ms",
            "extra": "median of 5 runs: 103.374, 103.572, 104.941, 105.000, 107.355 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "ddf300a2cdda23995f3ec0a22ae311e4b45d19a8",
          "message": "Let a tasks menu line's place give way before its state word\n\nThe footer fit spec named what ran past the menu's edge at 1600 px: the\nstate word of rows at the menu's 30rem cap. A line's place (\"in ...\",\na qualifier, or why a row cannot run) was flex: none, so the row could\nnot get narrower than its name's floor plus its place, age and state;\nthe place now shrinks with an ellipsis, and the state word stays whole.\n\nRefs #1545\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T17:44:23+04:00",
          "tree_id": "fa201d2a744f599d6ad303145029a9767f595954",
          "url": "https://github.com/purlis/purlis/commit/ddf300a2cdda23995f3ec0a22ae311e4b45d19a8"
        },
        "date": 1791726912120,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5293585000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.527, 0.529, 0.529, 0.539, 0.557 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.9065805,
            "unit": "ms",
            "extra": "median of 5 runs: 16.608, 16.889, 16.907, 17.002, 17.398 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.540732,
            "unit": "ms",
            "extra": "median of 5 runs: 103.666, 104.534, 104.541, 105.001, 106.764 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "bb9b8cc3c38daca84eeeedeb3c982d5750fddd7e",
          "message": "change store: a store that cannot be held answers exists as its path does\n\nexists answered yes for any changes/ it could not hold, so change create\nthrough a changes/ linked out of the project said the change already\nexists (exit 2) instead of refusing the link (exit 1), which\npurlis-cli's change test holds. It now answers as the look by path\nanswered: that reads nothing, and the read or write that follows still\ngoes through the hold and is refused in its own words.\n\nRefs #1083\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T19:05:18+04:00",
          "tree_id": "a5d137a508b1f900a991622be4d169a70d665227",
          "url": "https://github.com/purlis/purlis/commit/bb9b8cc3c38daca84eeeedeb3c982d5750fddd7e"
        },
        "date": 1791731559758,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.3576695,
            "unit": "ms",
            "extra": "median of 5 runs: 0.348, 0.349, 0.358, 0.358, 0.362 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.454271,
            "unit": "ms",
            "extra": "median of 5 runs: 16.382, 16.404, 16.454, 16.581, 16.621 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.563909,
            "unit": "ms",
            "extra": "median of 5 runs: 101.501, 101.552, 101.564, 102.006, 102.969 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "1489d0469c5eed3760ee589a9d689a7375ff3fa6",
          "message": "Give the keyboard back to a moved view; read the pick count out\n\nA view's element moved between regions while the keyboard is in it lost\nthe keyboard to the page: the region it leaves notes where it was and the\nregion it lands in gives it back (never into a hidden view). The count of\npicked file rows is a live status, so a screen reader hears it change.\nA whole-window test moves Search to the Panel and finds the same box with\nits words.\n\nRefs #1752\nRefs #1151\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T19:42:08+04:00",
          "tree_id": "6f4592e678e46b3d8d172e3225a57f60e139d241",
          "url": "https://github.com/purlis/purlis/commit/1489d0469c5eed3760ee589a9d689a7375ff3fa6"
        },
        "date": 1791733875248,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5293650000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.497, 0.525, 0.529, 0.553, 0.603 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.010182,
            "unit": "ms",
            "extra": "median of 5 runs: 16.370, 16.948, 17.010, 17.131, 17.298 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.9491515,
            "unit": "ms",
            "extra": "median of 5 runs: 103.767, 103.924, 103.949, 104.218, 105.785 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "500e0d32517f7c0c05de5783d63ec43236368569",
          "message": "ci: annotate each scenario job with its slowest spec files and tests\n\nThe scenario specs' own time doubled in a day as specs were added (Linux 313 s on 2026-10-10,\n587 s on 2026-10-11; 39 spec files then, 51 now), and they run one at a time in one app, so\nthe scenario jobs are again the longest wait. The scenario config's afterTest and a session's\nbeforeSession/afterSession write each test's and each spec file's time to\napp/logs/timings.jsonl (app/e2e/timings.ts, beside failures.ts); tools/e2e-timings.mjs ranks\nthem as two notices per job, as tools/slowest-tests.mjs does for the Rust suite. A report:\n!cancelled() and continue-on-error.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4EJSYV68AZG1SWR231JAEZE\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T21:22:19+04:00",
          "tree_id": "9cff13b470d4d0d374720883d0b9ff9cae4efbe9",
          "url": "https://github.com/purlis/purlis/commit/500e0d32517f7c0c05de5783d63ec43236368569"
        },
        "date": 1791742089105,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5144505,
            "unit": "ms",
            "extra": "median of 5 runs: 0.489, 0.503, 0.514, 0.519, 0.526 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.759681,
            "unit": "ms",
            "extra": "median of 5 runs: 16.318, 16.715, 16.760, 17.118, 17.452 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.384705,
            "unit": "ms",
            "extra": "median of 5 runs: 101.457, 103.138, 104.385, 105.643, 105.905 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "549fa7b56cdafd909d5b58c073a4e55956138d92",
          "message": "e2e: stop waiting out a picker that never comes, and time each spec's hooks\n\n#1765's first notices: opened-repo-forges.e2e.ts took 94 to 99 s, nearly all of it one test,\nbecause each of its two opens waited the full 45 s for a first chat's picker that neither repo\ndraws any more. It now waits 5 s of quiet (no trust gate, no picker up) and goes on; a picker\nor a gate that does come is handled as before.\n\nsettings-limits.e2e.ts took 109 to 121 s with no test of it past 2.7 s, so its time is in its\nhooks, which were not recorded. afterHook now writes each hook's time to timings.jsonl too,\nlisted with the tests under WebdriverIO's own title for it.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4EJSYV68AZG1SWR231JAEZE\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T22:31:30+04:00",
          "tree_id": "ed52efa2a0f87f67208fe6d49f823ef069144f84",
          "url": "https://github.com/purlis/purlis/commit/549fa7b56cdafd909d5b58c073a4e55956138d92"
        },
        "date": 1791748077164,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.509811,
            "unit": "ms",
            "extra": "median of 5 runs: 0.493, 0.501, 0.510, 0.527, 0.540 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.811578,
            "unit": "ms",
            "extra": "median of 5 runs: 16.454, 16.691, 16.812, 17.049, 17.140 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.7594435,
            "unit": "ms",
            "extra": "median of 5 runs: 103.666, 104.349, 104.759, 105.642, 106.258 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "eb1f9b451e1fc630bb2c33f262423f4de535ff4c",
          "message": "dependabot: hold WebdriverIO majors until @wdio/tauri-service moves\n\n@wdio/tauri-service 1.5.0, the latest, requires webdriverio ^9, so #1636's WebdriverIO 10\nbroke every scenario run and was left out of #1766. Without this the web group proposes it\nagain each week. The service is not held, so its own release for 10 is what proposes the move.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4EJSYV68AZG1SWR231JAEZE\nPurlis-Persona: steward",
          "timestamp": "2026-10-11T22:54:21+04:00",
          "tree_id": "07b6df506648f2f2289bcb994687206ef10ccaca",
          "url": "https://github.com/purlis/purlis/commit/eb1f9b451e1fc630bb2c33f262423f4de535ff4c"
        },
        "date": 1791751257217,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5361205,
            "unit": "ms",
            "extra": "median of 5 runs: 0.533, 0.533, 0.536, 0.539, 0.541 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.6625635,
            "unit": "ms",
            "extra": "median of 5 runs: 16.423, 16.548, 16.663, 16.991, 17.203 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.18790999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 101.896, 102.104, 103.188, 103.685, 104.227 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "49699333+dependabot[bot]@users.noreply.github.com",
            "name": "dependabot[bot]",
            "username": "dependabot[bot]"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "76be501433819bb1e57f27fbef3fd89575bc2552",
          "message": "Bump github/codeql-action/upload-sarif\n\nBumps the actions group with 1 update in the / directory: [github/codeql-action/upload-sarif](https://github.com/github/codeql-action).\n\n\nUpdates `github/codeql-action/upload-sarif` from 4.38.2 to 4.38.3\n- [Release notes](https://github.com/github/codeql-action/releases)\n- [Changelog](https://github.com/github/codeql-action/blob/main/CHANGELOG.md)\n- [Commits](https://github.com/github/codeql-action/compare/2892aa5e19bbd11bc0cff5427e3b750a04d9e3c2...24c54180a607b1449ed407dd24f251e4e9147c8d)\n\n---\nupdated-dependencies:\n- dependency-name: github/codeql-action/upload-sarif\n  dependency-version: 4.38.3\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n  dependency-group: actions\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-10-12T00:42:42+04:00",
          "tree_id": "a225477fce82e6118bb835fc3bb16ff4a1ab521f",
          "url": "https://github.com/purlis/purlis/commit/76be501433819bb1e57f27fbef3fd89575bc2552"
        },
        "date": 1791753189302,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.26684450000000004,
            "unit": "ms",
            "extra": "median of 5 runs: 0.263, 0.264, 0.267, 0.267, 0.273 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.4611665,
            "unit": "ms",
            "extra": "median of 5 runs: 16.350, 16.407, 16.461, 16.471, 16.498 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 100.741389,
            "unit": "ms",
            "extra": "median of 5 runs: 99.948, 100.510, 100.741, 101.164, 101.284 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "489bdbf795568f19eb831f1c224693a890a52e23",
          "message": "ci: run the macOS scenario part's single-app runs in part 2, the shorter one\n\nOn #1780 part 1 (its half of the specs, panes' fifty sessions among them, and the five runs\nthat each need an app of their own) took 13 minutes and part 2 took 7. The five runs, about\ntwo minutes on macOS, move to part 2.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4EJSYV68AZG1SWR231JAEZE\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T01:06:58+04:00",
          "tree_id": "7701e1f159f8a3c561dae12b2eb4203da480a92a",
          "url": "https://github.com/purlis/purlis/commit/489bdbf795568f19eb831f1c224693a890a52e23"
        },
        "date": 1791754694088,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5201260000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.493, 0.510, 0.520, 0.520, 0.541 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.174665,
            "unit": "ms",
            "extra": "median of 5 runs: 16.848, 17.116, 17.175, 17.194, 17.248 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.75309899999999,
            "unit": "ms",
            "extra": "median of 5 runs: 103.920, 104.604, 104.753, 105.365, 106.016 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "a216f7d8f7fee8edbe01766a7307dbf8a9e48cbc",
          "message": "Licence-header guard: the opencode shim is generated, so it has none\n\nThe bundled opencode shim is purlis_core::opencode's output, and an app\ntest compares the two byte for byte. Take the header back out, list the\nshim among the generated exclusions, and drop the first-line-contract\nrule that only it used.\n\nSigned-off-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T01:24:21+04:00",
          "tree_id": "5bc4f8d7fd1efc13ff9d9e7dbd0634a3f6719a5c",
          "url": "https://github.com/purlis/purlis/commit/a216f7d8f7fee8edbe01766a7307dbf8a9e48cbc"
        },
        "date": 1791756182802,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.521462,
            "unit": "ms",
            "extra": "median of 5 runs: 0.501, 0.520, 0.521, 0.540, 0.542 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.990294,
            "unit": "ms",
            "extra": "median of 5 runs: 16.437, 16.628, 16.990, 17.095, 17.182 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.315945,
            "unit": "ms",
            "extra": "median of 5 runs: 102.017, 105.068, 105.316, 105.768, 105.778 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "95ab8e262d566389c7b93889ab740dd999107d38",
          "message": "A task owed one more report may send notes and questions until it\n\nA task whose report came out blocked, or that the person took up again\nafter its report (Owed::Again, #1768), was refused progress notes and\nquestions to its asking chat as one that had reported for good. Now it\ntalks with that chat as a task that still owes its report does, until its\none more report: same per-pair limit, Activity lines and delivery, and its\nquestion can be answered by that chat or the person. Not once its stop is\nrecorded, as its report is not. Closes #1786.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4EJSYV68AZG1SWR231JAEZE\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T02:20:43+04:00",
          "tree_id": "2294f5466762e95a2da825f58e1c8d7392cc2ec9",
          "url": "https://github.com/purlis/purlis/commit/95ab8e262d566389c7b93889ab740dd999107d38"
        },
        "date": 1791758431219,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.38692950000000004,
            "unit": "ms",
            "extra": "median of 5 runs: 0.378, 0.384, 0.387, 0.389, 0.393 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.6266525,
            "unit": "ms",
            "extra": "median of 5 runs: 16.499, 16.553, 16.627, 16.657, 16.659 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.68435199999999,
            "unit": "ms",
            "extra": "median of 5 runs: 100.356, 101.244, 101.684, 102.051, 102.062 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "4062f870e9f57f06d4c4a9fc91fcc5d283c33bcd",
          "message": "The sandbox names a package folder once, refuses an over-long start in words, and tells refusals off the proxy's thread\n\n#1425: a package a runner loads from node_modules is named once, as its\nfolder, and a missing folder a name Node loads first starts with is\nnamed once in the folders the command runs in; a path a named folder\nalready holds is not named again, in a config and across the project's\nconfigs. Every path named before is still protected (a rule on a folder\nholds everything below it). MOST_NAMED stays 1024; the stress monorepo\nthat was refused names 291, and the unit-test monorepo 196 (was 561).\nThe files a runner tries beside a folder it is pointed at are named too.\nseatbelt::fits_exec measures a start's whole argv and environment\nagainst the system's limit and refuses with a plain sentence; its call\nin Applied::line is left for the round that owns sandbox.rs.\n\n#1353: the egress proxy keeps a refusal in its record at once, answers\nthe client, and tells the app on a teller thread of its own. One chat\nholds at most a fifth of the network record, letting go of its own\noldest lines. The doctor counts purlis's own blocks at most once per\nchat a day.\n\nRefs #1425\nRefs #1353\nRefs #1683\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T03:12:49+04:00",
          "tree_id": "0723d0eb356f58f70aab9b7bedfea91ad74c5c83",
          "url": "https://github.com/purlis/purlis/commit/4062f870e9f57f06d4c4a9fc91fcc5d283c33bcd"
        },
        "date": 1791761639849,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.47082250000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 0.467, 0.468, 0.471, 0.486, 0.486 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.3746355,
            "unit": "ms",
            "extra": "median of 5 runs: 16.209, 16.315, 16.375, 16.406, 16.479 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.460619,
            "unit": "ms",
            "extra": "median of 5 runs: 101.240, 101.393, 101.461, 101.721, 102.320 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "fc7131dd9e138f1e19890fe177a15a6fada9bee1",
          "message": "End the Curate refusal with the harness card's line and label\n\nTrain 74 CI (harness-card.e2e.ts, #1134 line 2): every Curate row was\noff with \"No curation chat can open on the default profile ...: <card\nline> See What opencode can do here. Make another profile the default\nto curate from here.\", and the menu drew no card row under them: the\nwindow links a card only where a reason ends with its label (ADR 0072\n§3, cardLink.tsx), and this one went on past it. The advice now comes\nfirst and the card's line and label end the sentence. The unit test\nasserts the ending.\n\nRefs #1134\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T04:03:48+04:00",
          "tree_id": "bf7408a4881f86f4682940ebd2d455cc3c3b56e1",
          "url": "https://github.com/purlis/purlis/commit/fc7131dd9e138f1e19890fe177a15a6fada9bee1"
        },
        "date": 1791764170386,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5191445000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.492, 0.501, 0.519, 0.526, 0.531 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.093542499999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.949, 17.000, 17.094, 17.127, 17.167 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.8854715,
            "unit": "ms",
            "extra": "median of 5 runs: 103.417, 103.781, 103.885, 104.180, 106.438 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "37ec362e67dd2ad8804dcbbeff9b6d324285915d",
          "message": "Old-name guard: NewProject spells no old name now\n\nUG's help names purlis.toml, so its one allowed `charter.toml` is gone\nand the guard asks for the entry to go too (shrink only).\n\nRefs #1719\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T04:23:25+04:00",
          "tree_id": "295662b18a6717a214ff31a5f3d820e2e160e1ee",
          "url": "https://github.com/purlis/purlis/commit/37ec362e67dd2ad8804dcbbeff9b6d324285915d"
        },
        "date": 1791766088131,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.275725,
            "unit": "ms",
            "extra": "median of 5 runs: 0.264, 0.273, 0.276, 0.277, 0.280 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.5468765,
            "unit": "ms",
            "extra": "median of 5 runs: 16.443, 16.465, 16.547, 16.552, 16.596 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.125725,
            "unit": "ms",
            "extra": "median of 5 runs: 100.251, 101.114, 101.126, 101.551, 101.793 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "464109e4834371c932481180c431290eedb567ff",
          "message": "Train 77 review: purlis mcp serves a read-only project; joined bash 3.2 lines are looked at, not walked\n\n- CI: the new read-only MCP test found the server refused whole on a\n  read-only project (FR-24), so it never served (#1051 l1 was not built).\n  `mcp` now runs there, after the one stderr line, and every tool that\n  writes is refused at its call, before it runs, whatever road it would\n  write by (a session record or a dispatch without the app included). The\n  read tools answer. plane-format.md lists it. The test now says which\n  call the server stopped at, and what it said, should it stop again.\n- CI: an ordinary 64 KiB commit message was refused as too deeply nested.\n  The lines of a body bash 3.2 runs whole were joined and walked as a\n  level of their own, so each paragraph nested once more. What the joined\n  lines hand on is now looked at, one level, never walked further in. The\n  bash 3.2 handoff test still refuses; the 4,993-command differential\n  still shows 0 refuses-less and 0 everyday changes.\n\nRefs #1089 #1051 #1426\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T04:45:08+04:00",
          "tree_id": "a782013da554b47dfdd9c396af04d340ec935bb3",
          "url": "https://github.com/purlis/purlis/commit/464109e4834371c932481180c431290eedb567ff"
        },
        "date": 1791767865165,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.45316599999999996,
            "unit": "ms",
            "extra": "median of 5 runs: 0.440, 0.440, 0.453, 0.453, 0.458 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.479114000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 16.384, 16.394, 16.479, 16.511, 16.562 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.3446225,
            "unit": "ms",
            "extra": "median of 5 runs: 100.852, 102.183, 102.345, 102.437, 103.415 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "2605072b06de338895ece86f89f942eb10338081",
          "message": "search-to-chat.e2e.ts knows its chat by the tab in front once it started\n\nOn Linux an earlier spec can leave a chat of the same name open, so the chat this spec started\nwas not a new name on the strip and was never found among those offered. The chat is now the\ntab in front once it is up; the after hook ends that one and closes the view tabs it opened.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T05:54:29+04:00",
          "tree_id": "07a25b3d9c0e721698dd9bb661dedb291a5bf64e",
          "url": "https://github.com/purlis/purlis/commit/2605072b06de338895ece86f89f942eb10338081"
        },
        "date": 1791770348908,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.39277249999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 0.389, 0.389, 0.393, 0.420, 0.425 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.797070499999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.168, 16.170, 16.797, 16.800, 16.864 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.4878465,
            "unit": "ms",
            "extra": "median of 5 runs: 101.726, 102.045, 102.488, 102.518, 103.094 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "30a7daebb0d2fe6c5c702af6cb2da734f97560fc",
          "message": "Secret scan: a prefilter in front of the vendor and personal-data rules; linear-time tests compare sizes\n\nLane VA (#773) took the per-line rules from 9 vendor and 3 personal-data\npatterns to 63 and 12, each run over every line. On PR #1800\nmany_escaped_addresses_and_many_joined_lines_read_in_linear_time took 14.66 s\nand then 14.54 s against its 10 s bound.\n\nEach gitleaks rule now carries its keywords, as gitleaks' own rules do, and one\nRegexSet of them, read case-insensitively, says which vendor rules a line can\nhold; the personal-data rules are asked once as a RegexSet for the same. Only\nthose rules run. No rule or check changes: the_prefilters_never_hide_a_hit\nholds the prefiltered scan to the scan asking every rule over every planted\nvalue, near-miss, personal datum and two source files, upper-cased and with\nthe long s and Kelvin sign a (?i) rule folds.\n\nThe four linear-time tests the scan owns now time a size and four times it,\nthree rounds each, and fail when the larger takes 8 times as long or more\n(linear is 4, quadratic 16), under a 120 s ceiling. Turning escaped_leaks' seen\nset into a list fails two of them (8.9x and 13.1x).\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T07:24:04+04:00",
          "tree_id": "acd4b8cba573a2468b89ffaddbbff39bc4b32615",
          "url": "https://github.com/purlis/purlis/commit/30a7daebb0d2fe6c5c702af6cb2da734f97560fc"
        },
        "date": 1791775562680,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.27288650000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 0.255, 0.272, 0.273, 0.281, 0.291 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.51869,
            "unit": "ms",
            "extra": "median of 5 runs: 16.401, 16.487, 16.519, 16.525, 16.580 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 100.88948350000001,
            "unit": "ms",
            "extra": "median of 5 runs: 100.715, 100.724, 100.889, 101.632, 101.677 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "ec72f237b6328673b007eea6d61914caba0c9dd3",
          "message": "A dev release builds nothing when its source carries another release.yml\n\nRelease run 38189104321 was started by ci at fc7131dd, which went green\njust after train 76 (37ec362e) merged. A workflow_run executes main's\nrelease.yml (37ec362e, with the new third-party notices check) but builds\nthe triggering commit (fc7131dd, from before the notices existed), so the\ncheck's grep found no app/dist/THIRD-PARTY-NOTICES and exited 2 on both\nplatforms. The notices themselves are fine: run 38190972158 built\n37ec362e under the same check and passed it.\n\nplan now checks out the release.yml it executes (github.workflow_sha,\nsparsely) beside the source, and a workflow_run whose two files differ\nskips its build with a notice; the newer commit's own ci starts the\nrelease that builds it. A new release-manifest test pins the notices'\npath across the generator, vite, tauri.release.conf.json and the check,\nand pins the skew skip.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T07:52:26+04:00",
          "tree_id": "a87af27fbda4252470d20ba00bd58b6980450d25",
          "url": "https://github.com/purlis/purlis/commit/ec72f237b6328673b007eea6d61914caba0c9dd3"
        },
        "date": 1791777217476,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.469519,
            "unit": "ms",
            "extra": "median of 5 runs: 0.443, 0.451, 0.470, 0.471, 0.486 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.2921165,
            "unit": "ms",
            "extra": "median of 5 runs: 16.143, 16.228, 16.292, 16.498, 17.018 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.1544325,
            "unit": "ms",
            "extra": "median of 5 runs: 100.612, 100.685, 101.154, 101.456, 102.085 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "5b6d627387fc0b1b2379d219f703c6fd1f95ecac",
          "message": "pane-notices.e2e.ts names no helper inside an execute\n\nOn macOS the run failed in `raise`: the spec's bundler wrapped the\npage-side helper in a `__name` call the page does not have, as\ngauge.e2e.ts and view-tabs.e2e.ts already note. Every helper the page\nruns is now held in a list rather than bound by name.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T08:27:26+04:00",
          "tree_id": "6b72845cb65edaf956f98a8c1caf70d6c60b86e6",
          "url": "https://github.com/purlis/purlis/commit/5b6d627387fc0b1b2379d219f703c6fd1f95ecac"
        },
        "date": 1791779317210,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5262455,
            "unit": "ms",
            "extra": "median of 5 runs: 0.504, 0.516, 0.526, 0.541, 0.561 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.9928505,
            "unit": "ms",
            "extra": "median of 5 runs: 16.350, 16.804, 16.993, 17.198, 17.271 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.2799195,
            "unit": "ms",
            "extra": "median of 5 runs: 102.631, 104.224, 104.280, 104.718, 105.378 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "c90766a7ea647d63dd648d35cc11ce5a7eaab628",
          "message": "A launch's first event is redacted even while the log's rules are still building\n\nThe rules are built on a thread beside the launch now, so the first event\ncan be raised before they are ready. Every write already asks the rules\nthrough the same OnceLocks, so that event waits for them and is never\nwritten as it was said. A test proves it in a child of the test binary,\nwhose first event carries a token while the rules are still building: the\nfile and standard error hold the redaction line and never the token. A\nvariant that lets events through until the rules are ready fails it.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T11:10:44+04:00",
          "tree_id": "1c46be1c4c77e4bd4b0aee2e015945c765026855",
          "url": "https://github.com/purlis/purlis/commit/c90766a7ea647d63dd648d35cc11ce5a7eaab628"
        },
        "date": 1791789169006,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.522066,
            "unit": "ms",
            "extra": "median of 5 runs: 0.516, 0.522, 0.522, 0.533, 0.540 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.1595505,
            "unit": "ms",
            "extra": "median of 5 runs: 16.575, 17.135, 17.160, 17.403, 17.457 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.41402,
            "unit": "ms",
            "extra": "median of 5 runs: 104.218, 105.113, 105.414, 105.683, 106.062 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "40f2ac86b9471cfb153825eda867a96f8b0bbac3",
          "message": "A Go tool's refused certificate check is read from a failed command\n\nA sandboxed chat's `glab api` failed with `x509: OSStatus -26276` and no\nNotice came up. A Go program refused the system's certificate check exits\nnon-zero, so its words reach the block hook only in PostToolUseFailure's\n`error`, which is read as standard output: a line counts only where it\nends as a refusal, and no ending matched the certificate check. The one\nexisting test fed the line as standard error of a command that succeeded,\na shape real Go CLIs never produce for this failure.\n\n`ends_as_a_refusal` now ends on `x509: OSStatus -26276`, so the existing\ncertificate-check block, and its Notice, comes up. A line that only\nquotes the words is still not a block. Nothing is granted.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T11:18:22+04:00",
          "tree_id": "09d518c68fb95e77c39601dd4ab334be10717cdf",
          "url": "https://github.com/purlis/purlis/commit/40f2ac86b9471cfb153825eda867a96f8b0bbac3"
        },
        "date": 1791790222440,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4719875,
            "unit": "ms",
            "extra": "median of 5 runs: 0.472, 0.472, 0.472, 0.473, 0.483 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.3659525,
            "unit": "ms",
            "extra": "median of 5 runs: 16.279, 16.291, 16.366, 16.723, 16.869 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.3378015,
            "unit": "ms",
            "extra": "median of 5 runs: 100.220, 100.830, 101.338, 101.627, 102.410 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "9cb49c8295431c94460eb8dc9d153226fde2f3e4",
          "message": "Train 80 CI: every removed or added submodule pointer in the fixture names its own commit\n\nThe dropped and the added pointer named one commit too, which both git and gitoxide then read\nas a rename. The added pointer now names a third commit.\n\nRefs #704\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T14:29:29+04:00",
          "tree_id": "3825fa91d4b639a3eda5e78d007871f30215ffca",
          "url": "https://github.com/purlis/purlis/commit/9cb49c8295431c94460eb8dc9d153226fde2f3e4"
        },
        "date": 1791801063951,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4939875,
            "unit": "ms",
            "extra": "median of 5 runs: 0.468, 0.490, 0.494, 0.498, 0.508 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.826183999999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.337, 16.663, 16.826, 17.025, 17.070 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.3360015,
            "unit": "ms",
            "extra": "median of 5 runs: 101.535, 102.839, 105.336, 105.572, 106.990 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "2e24d553b8377c380b90bf587d2f22c053b48a95",
          "message": "A fenced build's stand-in config home is its process's own\n\nA test process of the app's suite kept this machine's sandbox record in one\nfolder of the temporary directory that every test process shared, so one\ntest's folders could refuse another's write. It is per user and process now;\na run that keeps its store across launches names one inside its fence.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T15:24:07+04:00",
          "tree_id": "f6d38a79fd42190114e329549b586e5148911e5d",
          "url": "https://github.com/purlis/purlis/commit/2e24d553b8377c380b90bf587d2f22c053b48a95"
        },
        "date": 1791804914689,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4692305,
            "unit": "ms",
            "extra": "median of 5 runs: 0.443, 0.469, 0.469, 0.472, 0.472 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.3787115,
            "unit": "ms",
            "extra": "median of 5 runs: 16.264, 16.326, 16.379, 16.399, 16.410 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.341888,
            "unit": "ms",
            "extra": "median of 5 runs: 100.518, 101.223, 101.342, 101.734, 102.173 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "d4fec603f5eaa2fc2ccd387e7afd31cb60e495ac",
          "message": "e2e: the gauge spec runs purlis in the run's own environment\n\nWith the harness and CLI built with the app (purlis-core/fenced), gauge.e2e.ts's statusline and\nhook calls were refused: they ran with the runner's own environment, so purlis opened the\nmachine store at ~/.config, outside the run's fenced tree. Unfenced, that was the operator's own\nstore on a local run (charter-app#129). Both now take theRunsEnvironment(plane), as the app does.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4EJSYV68AZG1SWR231JAEZE\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T15:41:36+04:00",
          "tree_id": "c15ee8da67b0b3b86195a188511f2b7bc655333a",
          "url": "https://github.com/purlis/purlis/commit/d4fec603f5eaa2fc2ccd387e7afd31cb60e495ac"
        },
        "date": 1791808485678,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.6082945,
            "unit": "ms",
            "extra": "median of 5 runs: 0.598, 0.604, 0.608, 0.609, 0.619 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.230038,
            "unit": "ms",
            "extra": "median of 5 runs: 16.132, 16.162, 16.230, 16.333, 16.379 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.880342,
            "unit": "ms",
            "extra": "median of 5 runs: 100.545, 101.064, 101.880, 101.885, 102.062 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "01526f59a8bbf1ccb9ba4dfac0fad8fb01f27778",
          "message": "ci: stress and standing restore a gating job's Rust cache and save none\n\nThe repository's Actions caches held 10.89 GB on 2026-10-12, against the 10 GB GitHub keeps\nbefore it drops the oldest, `main`'s gating caches among them (as on 2026-10-08). 3.5 GB was\nevidence-only jobs' own: stress on Linux and macOS (1.94 GB), standing (0.5), search-at-scale\n(0.83), bench (0.2). stress builds what the scenario jobs build and standing builds purlis-core's\ntests as `rust` does, so each now restores that job's cache (`shared-key`) and saves none; the\ncaches' keys share the environment hash per OS, so the restore hits. search-at-scale builds in\nrelease, which no gating job does, and keeps its own. About 2.4 GB freed.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4EJSYV68AZG1SWR231JAEZE\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T16:41:14+04:00",
          "tree_id": "9d9967b5dba3b836542328f253f5183015c3f528",
          "url": "https://github.com/purlis/purlis/commit/01526f59a8bbf1ccb9ba4dfac0fad8fb01f27778"
        },
        "date": 1791809419810,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5444009999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 0.509, 0.537, 0.544, 0.550, 0.551 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.6823615,
            "unit": "ms",
            "extra": "median of 5 runs: 16.584, 16.630, 16.682, 16.916, 16.941 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.24194299999999,
            "unit": "ms",
            "extra": "median of 5 runs: 103.389, 104.922, 105.242, 105.252, 105.715 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "66b716207ad81a926dffaf500adbc6c129ea2b49",
          "message": "ci: sccache for the macOS scenario job's own crates; its single-app runs back in part 1\n\nrust-cache keeps the dependencies but never the workspace's own crates, so each macOS scenario\npart compiled purlis-core and the app afresh, 3.5 to 4 minutes on the slowest runners. sccache\n(mozilla-actions/sccache-action v0.0.11, sccache v0.18.0) keeps each compiled crate in the\nActions cache, written by `main` and only read by a pull request (SCCACHE_GHA_RW_MODE). It is\nswitched on after rust-cache's restore, because rust-cache keys its cache on every RUST* and\nCARGO* variable. A notice per part says its hits and misses, to keep it only if it pays.\n\nThe five single-app runs move back to part 1: part 2's half of the specs is the heavier, 13 to\n16 minutes against part 1's 7 to 11 even with the app's own build there now.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4EJSYV68AZG1SWR231JAEZE\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T17:20:39+04:00",
          "tree_id": "6015e3687ec8492401a6afbfb7aef6a28c99374c",
          "url": "https://github.com/purlis/purlis/commit/66b716207ad81a926dffaf500adbc6c129ea2b49"
        },
        "date": 1791811312793,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.27452350000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 0.263, 0.271, 0.275, 0.280, 0.287 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.605104,
            "unit": "ms",
            "extra": "median of 5 runs: 16.544, 16.591, 16.605, 16.653, 16.656 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.4140235,
            "unit": "ms",
            "extra": "median of 5 runs: 100.803, 100.836, 101.414, 101.456, 101.458 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "980d293bbe20b7757e2d0226372a722e37cb2cc6",
          "message": "ci: the sccache notice carries its whole report\n\n#1829's run showed no sccache notice on either macOS part: the step filtered the report to five\nlines and printed nothing. It now carries the report's first 30 lines, escaped for a workflow\ncommand, or says sccache said nothing.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4EJSYV68AZG1SWR231JAEZE\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T17:48:40+04:00",
          "tree_id": "0cb04a44cb64ff5743d1a90888a27ba438bc9faa",
          "url": "https://github.com/purlis/purlis/commit/980d293bbe20b7757e2d0226372a722e37cb2cc6"
        },
        "date": 1791815217991,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4703675,
            "unit": "ms",
            "extra": "median of 5 runs: 0.465, 0.467, 0.470, 0.484, 0.487 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.267460999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.192, 16.197, 16.267, 16.572, 16.852 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.42695,
            "unit": "ms",
            "extra": "median of 5 runs: 100.890, 101.284, 101.427, 101.768, 102.237 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "03c808c6525c8553d04557776ef4ee90947885ee",
          "message": "Train 82 CI: the publish test reads its stand-in remote with plain git, and your-repos waits for its clone\n\n- publishproject's hands test read the bare repository standing in for\n  the forge through the runner, which opens a bare repository only\n  where it is named; it reads it with plain git and --git-dir, as\n  createproject's does.\n- your-repos.e2e.ts waits for the picked project's clone to land, then\n  for the trust question's own Open project, rather than any dialog.\n\nRefs #1820, #1818\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T19:16:28+04:00",
          "tree_id": "69ea56cfc91ff29b87697445d7cd65773778fe1e",
          "url": "https://github.com/purlis/purlis/commit/03c808c6525c8553d04557776ef4ee90947885ee"
        },
        "date": 1791818426021,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.488007,
            "unit": "ms",
            "extra": "median of 5 runs: 0.478, 0.485, 0.488, 0.509, 0.529 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.971825,
            "unit": "ms",
            "extra": "median of 5 runs: 16.350, 16.879, 16.972, 17.050, 17.244 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.295286,
            "unit": "ms",
            "extra": "median of 5 runs: 102.909, 102.947, 104.295, 104.835, 107.299 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "5360f04bfe3a6bf6df3ad08685e3f4375dbf9fde",
          "message": "review(train 83): the two CI reds on 1ee260e5\n\n- plane-format.md: the offered-mirrors record's tier reads as a tier,\n  then its reason after a dash, as the tier check reads every store.\n- hooks: the drain-while-running test expects the run its live call is\n  under first, as every live hook test does; the dedupe is unchanged.\n\nRefs #1583\nRefs #952\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T23:00:27+04:00",
          "tree_id": "1f23403e10e21940adfb56cda4c7bc160f33468b",
          "url": "https://github.com/purlis/purlis/commit/5360f04bfe3a6bf6df3ad08685e3f4375dbf9fde"
        },
        "date": 1791831699741,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5306649999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 0.510, 0.525, 0.531, 0.540, 0.577 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.219911999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.846, 16.910, 17.220, 17.258, 17.356 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.3853025,
            "unit": "ms",
            "extra": "median of 5 runs: 102.591, 105.385, 105.385, 106.183, 106.916 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "09ea69d06b812bafabaec2e0433ac6082bc5a388",
          "message": "review(train 84): the Inbox's Updates heading wraps its acts in a narrow sidebar\n\nThe diagnostic named it: `.inbox-updates-head` (\"Updates\", \"Mark all\nread\", \"Dismiss all\") ran 178px in 157px on macOS. DS-12 (#1844) drew the\ntwo acts as `.ui-button`, which keeps each label on one line, in a head\nthat never wrapped, so the line widened the Inbox. The head wraps now:\nheading and acts on one line where there is room, the acts under the\nheading where there is not.\n\nRefs #1844\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T01:03:39+04:00",
          "tree_id": "70d5390ff39ef6e1ee7dac2931eb5a5d4688cfc2",
          "url": "https://github.com/purlis/purlis/commit/09ea69d06b812bafabaec2e0433ac6082bc5a388"
        },
        "date": 1791839252972,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5073574999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 0.493, 0.493, 0.507, 0.512, 0.520 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.265883000000002,
            "unit": "ms",
            "extra": "median of 5 runs: 16.440, 17.255, 17.266, 17.413, 17.716 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.934834,
            "unit": "ms",
            "extra": "median of 5 runs: 102.360, 104.056, 104.935, 105.511, 107.071 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "f67fa029ab371cc9a104181d9226f40c07ccca19",
          "message": "review(train 85): the your-repos spec waits for the trust question's own button\n\nThe wait took the Welcome's open-by-path \"Open project\" for the trust\nquestion's, so it ended before the clone landed and the .git check failed.\nThe button is now looked for inside the dialog, as get-project.e2e does.\n\nRefs #1858\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T01:20:18+04:00",
          "tree_id": "b2a0f67212f57b1e4313f6ffbbfd055a6cba3a45",
          "url": "https://github.com/purlis/purlis/commit/f67fa029ab371cc9a104181d9226f40c07ccca19"
        },
        "date": 1791840693702,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.517315,
            "unit": "ms",
            "extra": "median of 5 runs: 0.516, 0.517, 0.517, 0.520, 0.538 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.981992,
            "unit": "ms",
            "extra": "median of 5 runs: 16.402, 16.811, 16.982, 17.035, 17.128 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.69965099999999,
            "unit": "ms",
            "extra": "median of 5 runs: 102.726, 103.582, 103.700, 103.909, 105.639 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "692bdf8ee446ed9d64e4f07a5dd51bc48184af85",
          "message": "review(train 86): the vault-value store's fail-closed path is tested\n\nThe store's remember and holds become methods taking the key, so a test\ncan hand it none and see every later text said to hold a value. The\nprocess's store still makes its one random key; nothing else changes.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T01:56:10+04:00",
          "tree_id": "ad1b893a1308332f1daa959b66956b75a71e9153",
          "url": "https://github.com/purlis/purlis/commit/692bdf8ee446ed9d64e4f07a5dd51bc48184af85"
        },
        "date": 1791842605248,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.47165599999999996,
            "unit": "ms",
            "extra": "median of 5 runs: 0.460, 0.470, 0.472, 0.472, 0.476 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.4033645,
            "unit": "ms",
            "extra": "median of 5 runs: 16.144, 16.398, 16.403, 16.692, 16.701 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.03860350000001,
            "unit": "ms",
            "extra": "median of 5 runs: 99.881, 101.994, 102.039, 102.110, 102.146 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "9558d26fa212304f3a04e0adabe0757d818739b1",
          "message": "The your-repos spec waits for the page to take the folder pick\n\nThe spec pressed a repo as soon as the folder pick's answer was read,\nbefore the page was known to hold the place picked; a repo picked then\nclones into ~/Projects, and the trust question comes for that folder\nwhile the spec's folder stays empty. It now waits until the page says\nthe place picked, and when the clone is not where it expects, it names\nthe folder the trust question asks about and what each candidate\nfolder holds.\n\nRefs #1858\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T03:08:34+04:00",
          "tree_id": "77a563a37c559854bc5df9bb921e9f4cf4bdf901",
          "url": "https://github.com/purlis/purlis/commit/9558d26fa212304f3a04e0adabe0757d818739b1"
        },
        "date": 1791846654171,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5400225000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.521, 0.521, 0.540, 0.553, 0.555 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.850327999999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.594, 16.642, 16.850, 17.283, 17.391 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.8595265,
            "unit": "ms",
            "extra": "median of 5 runs: 103.514, 103.640, 103.860, 104.577, 105.157 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "25df81a66a564919e555651b2c81cd76ae66b954",
          "message": "review(train 87): the cold start reports and does not gate, until #1876\n\nThe relative gate's first runs on this train held the same app code at\n0.98-1.03 in two jobs and 1.17-1.20 in two others, steadily across\nevery round. Nothing in the train runs at launch on Linux. Until the\ncause is found the cold start is evidence on pull requests too.\n\nRefs #1866, #1876\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T03:40:37+04:00",
          "tree_id": "d54f484c4f78b65e6857b77fb365286fe3db66a1",
          "url": "https://github.com/purlis/purlis/commit/25df81a66a564919e555651b2c81cd76ae66b954"
        },
        "date": 1791848511491,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.3869475,
            "unit": "ms",
            "extra": "median of 5 runs: 0.373, 0.383, 0.387, 0.393, 0.395 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.849494999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.669, 16.692, 16.849, 16.886, 16.979 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.450125,
            "unit": "ms",
            "extra": "median of 5 runs: 101.921, 102.129, 102.450, 103.530, 103.682 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "99b429fcabf7020dfef2003c8e165534d3e30c97",
          "message": "review(train 88): a tab's counts go before its whole title, on any strip\n\nThe floor-only check left the 900px case red: the button sat above its\nmin-width and its title still drew nothing. The chip yields whenever\nthe title's drawn box is not a pixel wide, as the previous review had\nit. The tab-chip case that expected all three counts on its strip now\nasks for the counts that fit beside a drawn title, from the start and\nwhole: on that strip the third count only fitted by squeezing the\ntitle out, which is what #1867 decides against (D-1867-1).\n\nRefs #1867\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T05:04:01+04:00",
          "tree_id": "e51722786989b413f5ea390bc3cc569f43b88b66",
          "url": "https://github.com/purlis/purlis/commit/99b429fcabf7020dfef2003c8e165534d3e30c97"
        },
        "date": 1791853515510,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.446566,
            "unit": "ms",
            "extra": "median of 5 runs: 0.432, 0.436, 0.447, 0.447, 0.460 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.3244635,
            "unit": "ms",
            "extra": "median of 5 runs: 16.129, 16.243, 16.324, 16.356, 16.470 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.35945749999999,
            "unit": "ms",
            "extra": "median of 5 runs: 101.949, 102.108, 102.359, 102.557, 103.687 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "c430fde076ab33e8990b4d924a425cf7b78351e6",
          "message": "review(train 89): the window-only count takes the two editor commands\n\nqw309 put open_in_your_editor and open_their_agents_md on WINDOW_ONLY;\nthe session protocol's count of that list now names them and takes two.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T05:49:31+04:00",
          "tree_id": "0ceb1bd96fc360776310d0b25eaea2c928609f5c",
          "url": "https://github.com/purlis/purlis/commit/c430fde076ab33e8990b4d924a425cf7b78351e6"
        },
        "date": 1791856443750,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.27580899999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 0.273, 0.274, 0.276, 0.279, 0.287 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.6569295,
            "unit": "ms",
            "extra": "median of 5 runs: 16.583, 16.586, 16.657, 16.706, 16.761 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.5393165,
            "unit": "ms",
            "extra": "median of 5 runs: 100.477, 101.249, 101.539, 101.657, 101.746 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "110881e6fd248ead35ee50f75aefc28565a515a7",
          "message": "review(train 90): the app-identity rename test stores its token through a stand-in op\n\nA token is now stored only where an `op` is found to pin (issue 1517).\nThe rename test that follows the app to its new identity stored one on\na context with no PATH, so it stores through a stand-in `op`, as the\nkeychain-copy test does.\n\nRefs #1517\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T07:03:05+04:00",
          "tree_id": "964853ab744e070cbc1605e7743461a8541bc03d",
          "url": "https://github.com/purlis/purlis/commit/110881e6fd248ead35ee50f75aefc28565a515a7"
        },
        "date": 1791860670227,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5031865,
            "unit": "ms",
            "extra": "median of 5 runs: 0.483, 0.502, 0.503, 0.509, 0.526 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.9834245,
            "unit": "ms",
            "extra": "median of 5 runs: 16.894, 16.947, 16.983, 17.357, 17.373 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.946272,
            "unit": "ms",
            "extra": "median of 5 runs: 104.277, 105.377, 105.946, 106.439, 106.642 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "51a88a0b71b6dc7134ce2bea6aa181ccdb52c71f",
          "message": "review(train 91): a declared harness's curation test declares it before its profile is read\n\nThe fixture read its `work` profile before the test wrote the harness\nthat profile's kind names, so the profile was not there. The plane now\ntakes the declaration first.\n\nRefs #967\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T08:09:12+04:00",
          "tree_id": "fed1930f787644aeba67e8cf9f0a728f1e331352",
          "url": "https://github.com/purlis/purlis/commit/51a88a0b71b6dc7134ce2bea6aa181ccdb52c71f"
        },
        "date": 1791864627015,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.509995,
            "unit": "ms",
            "extra": "median of 5 runs: 0.501, 0.504, 0.510, 0.518, 0.532 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.12348,
            "unit": "ms",
            "extra": "median of 5 runs: 16.711, 17.115, 17.123, 17.146, 17.412 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.46412000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 103.603, 103.853, 104.464, 104.514, 105.048 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "e968a31d877e1b566767c3898ddc035710d8b657",
          "message": "review(train 92): the forge CLI test expects the preflight's sentence\n\nThe doctor this test file builds is the preflight, so where gh is found its\nsign-in row says the preflight asks no forge CLI. The test expected another\ndoctor's sentence and had only run where gh is absent.\n\nRefs #994\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T11:18:45+04:00",
          "tree_id": "907877e4ca411a6e19adddd13b833e6dbcdaa0d8",
          "url": "https://github.com/purlis/purlis/commit/e968a31d877e1b566767c3898ddc035710d8b657"
        },
        "date": 1791876020808,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.522799,
            "unit": "ms",
            "extra": "median of 5 runs: 0.512, 0.521, 0.523, 0.541, 0.561 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.8641535,
            "unit": "ms",
            "extra": "median of 5 runs: 16.306, 16.483, 16.864, 16.901, 16.963 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.562768,
            "unit": "ms",
            "extra": "median of 5 runs: 102.573, 103.020, 104.563, 105.070, 105.801 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "e968a31d877e1b566767c3898ddc035710d8b657",
          "message": "review(train 92): the forge CLI test expects the preflight's sentence\n\nThe doctor this test file builds is the preflight, so where gh is found its\nsign-in row says the preflight asks no forge CLI. The test expected another\ndoctor's sentence and had only run where gh is absent.\n\nRefs #994\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T11:18:45+04:00",
          "tree_id": "907877e4ca411a6e19adddd13b833e6dbcdaa0d8",
          "url": "https://github.com/purlis/purlis/commit/e968a31d877e1b566767c3898ddc035710d8b657"
        },
        "date": 1791878201029,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.556457,
            "unit": "ms",
            "extra": "median of 5 runs: 0.521, 0.521, 0.556, 0.557, 0.567 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.543504,
            "unit": "ms",
            "extra": "median of 5 runs: 16.446, 16.493, 16.544, 16.601, 16.891 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.868325,
            "unit": "ms",
            "extra": "median of 5 runs: 102.186, 102.442, 103.868, 104.633, 105.089 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "d7d7e7d62ced63b2b57d1b7c5d9848f410828f9d",
          "message": "sandbox: a docker run down a pipe names its socket, so its Notice offers Allow\n\nA sandboxed Claude Code chat ran `docker run … 2>&1 | tail`, the sandbox\nrefused Docker its socket, and the pipe ended with tail's status. Claude\nCode handed the result over as one that came back, with Docker's words on\nstandard output and no failure beside them. A socket was named only from a\nfailed command (ADR 0093, D-sock-5), so the block reached the app with no\nsocket and the Notice said \"The sandbox did not say which socket it\nrefused\", with Start without the sandbox as its only way out.\n\nNo harness says how each program of a line ended, so the line is the one\nfailure signal left in the payload: where one of its own programs is a\ncontainer engine's client (docker, docker-compose, podman, podman-compose,\nnerdctl), by name after the shell's plumbing, the socket its output says it\nwas refused is named, on either stream and in Codex's and opencode's one\nstring. A line that only reads (cat, grep, git show, curl) still names\nnone, and neither does a violation line in a one-string result. Only a\nsocket is named this way; the certificate check keeps to a failed command.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T12:19:45+04:00",
          "tree_id": "f2a3f62376c18fa57ddb43eb50a008cc4a5f27f4",
          "url": "https://github.com/purlis/purlis/commit/d7d7e7d62ced63b2b57d1b7c5d9848f410828f9d"
        },
        "date": 1791881592924,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5237430000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.498, 0.518, 0.524, 0.528, 0.538 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.0585435,
            "unit": "ms",
            "extra": "median of 5 runs: 16.448, 16.964, 17.059, 17.169, 17.268 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.24265349999999,
            "unit": "ms",
            "extra": "median of 5 runs: 104.495, 104.568, 105.243, 105.958, 107.400 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "d20a36e8db24b004ca4ae91e6e57f1ee44023b80",
          "message": "sandbox: window tests for the certificate check's Allow, its Settings row and a restart that waits\n\nThe Settings row's control is a named group with the set's button class; the\ntests' stand-in for the asks registry leads the certificate check with this\nmachine's Allow, as the registry does.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T16:31:30+04:00",
          "tree_id": "4f5d5fc77800dc936ef0b332d7ad886428d43505",
          "url": "https://github.com/purlis/purlis/commit/d20a36e8db24b004ca4ae91e6e57f1ee44023b80"
        },
        "date": 1791894767340,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5521590000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.522, 0.552, 0.552, 0.553, 0.557 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.6399385,
            "unit": "ms",
            "extra": "median of 5 runs: 16.557, 16.563, 16.640, 16.712, 16.806 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.8997135,
            "unit": "ms",
            "extra": "median of 5 runs: 100.703, 101.668, 101.900, 102.174, 102.329 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "51bf31b6fcb276f997e09e61131765fc37e62487",
          "message": "review(train 93): git is asked about the local file only where it declares a harness\n\nThe declarations are read several times at every chat start. Reading the\nlocal layer whole asked git about charter.local.toml at each of those\nreads, in every project that has the file, and fifty chats starting at\nonce slowed on it. settings::local_layer_holding asks the same check only\nof a file that holds the table, and is nothing to its reader otherwise.\n\nRefs #968\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T16:33:54+04:00",
          "tree_id": "0da23b17b9982f50b20afa26025bf4981a5b7c7b",
          "url": "https://github.com/purlis/purlis/commit/51bf31b6fcb276f997e09e61131765fc37e62487"
        },
        "date": 1791895761322,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5098225,
            "unit": "ms",
            "extra": "median of 5 runs: 0.506, 0.508, 0.510, 0.532, 0.564 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.875794499999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.440, 16.850, 16.876, 17.521, 17.891 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.79074399999999,
            "unit": "ms",
            "extra": "median of 5 runs: 102.334, 104.023, 105.791, 106.334, 107.330 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "4c7854eedd980eb2271f3c90d6b7d34af9e163cf",
          "message": "fix(design): a menu's rows keep their height, and the menu scrolls inside the window\n\nA chat tab's menu in a project with many personas drew each row over the\ndescription of the row above it.\n\nThe one menu style (DS-2, issue 1834) made the menu a column with a cap on\nits height and gave each row a written least height (`--size-control`). A\nwritten least height takes the place of the automatic one a column gives\nits items, so a menu taller than its cap squeezed every row to a control's\nheight, and a row with a note under its words overflowed onto the next.\nThe shells it replaced had no written least height, so their rows never\nshrank.\n\n- Nothing in a `.ui-menu` is squeezed (`flex-shrink: 0`): rows, the line\n  between groups, a label, a task's report. A row with a note is as tall\n  as its title and note say.\n- A menu is capped at the room the window has where it opened, which Radix\n  measures for a context menu and a dropdown, and scrolls inside. The\n  three surfaces' own shares of the window (60vh, 70vh) are gone.\n- A row is written at `--font-size-md`, the dense lists' step, and its\n  note at `--font-size-xs`.\n- A note drawn in a row is not repeated in the row's tooltip. A tooltip\n  still says why a row cannot run.\n\nEvery menu wears the class, so the tab, workspace, project, pane, explorer\nand memory menus, their submenus, the strip's show-more menu, the task\nchip's menu and a setting's Move to menu all take the same rules.\n\nHeld by primitives/controls.test.tsx and Menus.test.tsx, and by a new\nrange check in the real WebView, e2e/specs/tab-menu.e2e.ts.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T16:54:19+04:00",
          "tree_id": "9c9b11c5ae04242b62aff6150695efdca15fa52a",
          "url": "https://github.com/purlis/purlis/commit/4c7854eedd980eb2271f3c90d6b7d34af9e163cf"
        },
        "date": 1791896674306,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.523382,
            "unit": "ms",
            "extra": "median of 5 runs: 0.516, 0.522, 0.523, 0.525, 0.547 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.40967,
            "unit": "ms",
            "extra": "median of 5 runs: 16.690, 17.260, 17.410, 17.438, 17.471 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.34337550000001,
            "unit": "ms",
            "extra": "median of 5 runs: 101.232, 104.130, 105.343, 106.309, 108.107 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "e827217c0bed714461a5e8407129c5880db09737",
          "message": "review(train 94): signing out of gitlab.com in the browser revokes its token there\n\nADR 0077 §3 says a sign-out on GitLab also revokes the token, which a\npublic client can do, and the GitLab sign-in left that out.\n\n- `forge::revoke::sign_out` signs out on this machine first, then posts one\n  form to gitlab.com's revocation address with the client id and the token,\n  and no secret. The sign-out never waits on GitLab's yes: when GitLab\n  cannot be asked or will not, the person is signed out here all the same,\n  and the row says so and where on gitlab.com to end the token.\n- GitHub is not asked: ending an app's grant there is authenticated with\n  the app's client secret, and the registration has none. The row goes on\n  saying the token lapses within eight hours and where to end it now.\n- A pasted or imported token is never revoked, nor one on a host purlis has\n  no registration on, and a test build asks no forge.\n- A sign-out waits for a refresh in flight, so a renewed pair is never\n  written back for an account that was just signed out.\n\nRefs #730\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T17:06:51+04:00",
          "tree_id": "0a9dc08594decb314fade75b346c14791a394509",
          "url": "https://github.com/purlis/purlis/commit/e827217c0bed714461a5e8407129c5880db09737"
        },
        "date": 1791897661514,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.2981195,
            "unit": "ms",
            "extra": "median of 5 runs: 0.276, 0.296, 0.298, 0.303, 0.313 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.556836999999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.511, 16.536, 16.557, 16.607, 16.663 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.52650349999999,
            "unit": "ms",
            "extra": "median of 5 runs: 101.002, 101.391, 101.527, 101.746, 101.843 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "672fa978b2d8cb90eddc835967b4e7c73a519cae",
          "message": "review(qw353): a here-document's body and the shell's own test name no certificate check\n\nReview of the lane, each red first:\n\n- A here-document's body was read as lines the shell runs, so a line of readers that printed\n  the check's words back through one named the service. Every body line is left out before\n  the line is read; a program beside a here-document still names the check.\n- `[[ … ]]` and the loop words (`break`, `continue`, `return`, `shift`, `wait`) run nothing:\n  a line of readers behind them still only reads.\n- A result that holds both streams in one text named the check from whatever was printed;\n  it keeps to the same rule now (the sandbox's own line, or a line that runs a program).\n\nThe linear-time test reads here-documents too.\n\nAn unnamed certificate check keeps the Notice main gained meanwhile, which lets the person\nchoose the service (#1884). The lane's own sentence for it said no Allow is offered and was\nwritten before that, so it is taken out with its arm of `offered`: the app is as main has it,\nand this change is the naming alone.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T17:56:15+04:00",
          "tree_id": "c71a0fd3d796226f4d7b8c6d6589db8f9aa04481",
          "url": "https://github.com/purlis/purlis/commit/672fa978b2d8cb90eddc835967b4e7c73a519cae"
        },
        "date": 1791899855444,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5253535,
            "unit": "ms",
            "extra": "median of 5 runs: 0.502, 0.524, 0.525, 0.527, 0.541 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.957302499999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.370, 16.774, 16.957, 17.021, 17.232 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.942283,
            "unit": "ms",
            "extra": "median of 5 runs: 103.138, 103.154, 105.942, 105.968, 107.140 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "96d5e152bb4463d0a3f617021ec90a9a3ac62360",
          "message": "fix(explorer): a files browser for the strip's workspace; tree chevrons centred\n\nExplorer no longer lists the workspaces: the strip is the one place a\nworkspace is selected, and its tab already carries the press, the menu and\nthe live mark the section's rows repeated. The Explorer view is Files, of\nthe workspace the strip is on, named in its heading. On the plane root's\ntab the heading names the plane root and the section says it lists a\nworkspace's files. A layout file that folded the retired section is read\npast without an Inbox line.\n\nThe shared tree twist is a box one step wide and one row high, held in\nthe middle of its row by its margins. Explorer gave the class to the icon\nitself, which kept its own height between two insets and sat at the top\nof the row; its twists now use the documented span holding the icon.\n\nADR 0038 gets a dated amendment for the removed section.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T18:23:44+04:00",
          "tree_id": "3c89b486dc48e07ea7cbc7a04a350b6ec32478a8",
          "url": "https://github.com/purlis/purlis/commit/96d5e152bb4463d0a3f617021ec90a9a3ac62360"
        },
        "date": 1791901653821,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.27782450000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 0.267, 0.274, 0.278, 0.282, 0.284 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.394391,
            "unit": "ms",
            "extra": "median of 5 runs: 16.366, 16.386, 16.394, 16.410, 16.422 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.5587685,
            "unit": "ms",
            "extra": "median of 5 runs: 101.189, 101.449, 101.559, 101.628, 101.635 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "4ac110f2c335d48f5425d86c198d79e58e3a9a3f",
          "message": "review(qw355): the harness's note below a report is one paragraph that ends the text\n\nReview of the lane:\n\n- The report was read past any text that began with the note's opening, so a report quoted\n  in what a command printed, its note with it and more output below, read as the harness's\n  own. The note is read past only on a line of its own, below the report, and as one\n  paragraph that ends the text (red first).\n- The new module was mutated by hand, 24 ways. Three mutants lived and each has its test now:\n  a target the kernel put in quotes, a line that names no target (the system's, and nothing\n  to grant), and words before a proxy's `deny` (a sentence, not a line).\n\nADR 0093's new section is marked \"to be confirmed\" and says the note's rule.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T18:38:49+04:00",
          "tree_id": "0e922e5e4c3529ce35cf22983bb08e2971650d67",
          "url": "https://github.com/purlis/purlis/commit/4ac110f2c335d48f5425d86c198d79e58e3a9a3f"
        },
        "date": 1791902426759,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.27030350000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 0.262, 0.267, 0.270, 0.280, 0.296 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.605078,
            "unit": "ms",
            "extra": "median of 5 runs: 16.491, 16.571, 16.605, 16.652, 16.691 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.4601515,
            "unit": "ms",
            "extra": "median of 5 runs: 100.645, 101.057, 101.460, 101.495, 101.897 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "8781c1677587529fd03facb18e14ae7735a6a41d",
          "message": "review(qw354): an Allow keeps what was shown as it was shown, a declaration is bounded, and a chat that has the check is not told to restart\n\nFound in review of the lane, before it merged.\n\n- What a project expects is drawn once and may be answered much later. The\n  window now sends each item as it drew it, with the name an Allow would keep,\n  and nothing for one drawn as \"cannot be allowed here now\". An item is kept\n  only while it still stands that way at the press; one that changed is\n  neither kept nor recorded as answered, the Allow says so, and the list is\n  read again.\n- A project declares at most 32 sockets and services together, so the question\n  is always one a person can read whole.\n- A chat that started with the certificate check and is refused it all the same\n  is told the check is on already and that something else refused the tool,\n  with Start without the sandbox, never Restart. Whether a chat has the check is\n  the app's own record of its start.\n\nADR 0093, plane-format.md and the two change fragments say each.\n\nRefs #1886\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T20:14:45+04:00",
          "tree_id": "c7d9426bba2bc128d316e8d146701bd568c2651c",
          "url": "https://github.com/purlis/purlis/commit/8781c1677587529fd03facb18e14ae7735a6a41d"
        },
        "date": 1791908151883,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.279574,
            "unit": "ms",
            "extra": "median of 5 runs: 0.271, 0.275, 0.280, 0.290, 0.291 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.5808685,
            "unit": "ms",
            "extra": "median of 5 runs: 16.524, 16.546, 16.581, 16.601, 16.639 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.569141,
            "unit": "ms",
            "extra": "median of 5 runs: 100.747, 101.087, 101.569, 101.620, 101.625 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "dd46646cc3f27be076dcbb0c2e3972781222afcd",
          "message": "app: the status line is one status bar, and its region toggles go\n\nThe operator, 2026-10-13: the two toggle icons at the bottom left are\nredundant, the Notices and Doctor buttons do not sit in the bar, and the\nproject's directory is too large there.\n\nThe region toggles are removed. Each side's activity bar, its key and\nthe palette's row already put a side away and bring it back, so nothing\nis stranded. ADR 0038 is amended.\n\nNotices, Doctor and the pin are the control set's small ghost button:\nflat, the chrome's height, an icon, a word and a count, with severity as\na colour and a word. Where you are is on the left and gives way first;\nwhat wants a look is on the right. Every size on the line is a core\ntoken, so 21 geometry and 5 control allowance lines go.\n\nThe project is its name, the directory's last segment, and the whole\ndirectory is the tooltip, under the pointer and on keyboard focus. The\nscenario specs read the directory from the item.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T20:15:50+04:00",
          "tree_id": "d874a887a4614c6d414b021d41c250ca9c570742",
          "url": "https://github.com/purlis/purlis/commit/dd46646cc3f27be076dcbb0c2e3972781222afcd"
        },
        "date": 1791909145040,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.269431,
            "unit": "ms",
            "extra": "median of 5 runs: 0.260, 0.261, 0.269, 0.284, 0.290 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.446608,
            "unit": "ms",
            "extra": "median of 5 runs: 16.379, 16.379, 16.447, 16.450, 16.532 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 100.94044249999999,
            "unit": "ms",
            "extra": "median of 5 runs: 100.107, 100.925, 100.940, 101.335, 101.434 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "0854cd21f1eb308c51b2e9da31e8d99b460e3754",
          "message": "fix(app): the status bar's project name takes its weight from the token\n\nMain is red on the type guard: the guard (#1920) and the status bar's\nrework landed one after the other, each green without the other. The\nproject's name wrote a weight the guard refuses, and the allowance still\nlisted a size the status line no longer writes.\n\nThe name's weight is the strong token, the stale line is deleted and the\nceiling goes down with it. Nothing drawn changes.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T20:46:51+04:00",
          "tree_id": "eb78a59e4735a252fd2897550615d51b09d44d3a",
          "url": "https://github.com/purlis/purlis/commit/0854cd21f1eb308c51b2e9da31e8d99b460e3754"
        },
        "date": 1791910166049,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.46380699999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 0.444, 0.455, 0.464, 0.472, 0.474 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.567988,
            "unit": "ms",
            "extra": "median of 5 runs: 16.469, 16.560, 16.568, 16.583, 16.626 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.8633155,
            "unit": "ms",
            "extra": "median of 5 runs: 101.154, 101.529, 101.863, 101.896, 102.791 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "952c8fc580b95494160df067376524bac6866cea",
          "message": "review(qw356): Remove… opens Delete workspace, and a fourth question is not asked\n\nReview of the workspace removal a chat asks the person for.\n\n- Remove… on the question removes nothing. It opens the window's own Delete\n  workspace dialog, the one the sidebar opens, and the person's press there\n  removes. As first built, one press on the Notice or on the Inbox row removed\n  a workspace the guard found nothing at risk in; the guard is about clones,\n  the Inbox row gave no counts, and it sat as a primary button among Allows. A\n  removal is for good and a chat raises the question, so it takes the dialog.\n  The command the first build added to remove for a question is taken out:\n  Keep is the one command the ask adds to the window. The Inbox notes no\n  answer for a press that only opened the dialog. Decided in review, for the\n  operator to reverse.\n- A chat with its three questions waiting is not asked a fourth, in a fixed\n  sentence. As first built the oldest was put away for it, so a chat going\n  round many names could turn over what the person was reading and raise a\n  new question each time.\n\nThe skill, the ADR 0067 note and the change fragment say the same.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T21:35:34+04:00",
          "tree_id": "81ed2e8cc9f790e2f6263416c6cdf615898ff7cd",
          "url": "https://github.com/purlis/purlis/commit/952c8fc580b95494160df067376524bac6866cea"
        },
        "date": 1791913002639,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.3555965,
            "unit": "ms",
            "extra": "median of 5 runs: 0.349, 0.353, 0.356, 0.359, 0.361 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.445821000000002,
            "unit": "ms",
            "extra": "median of 5 runs: 16.351, 16.370, 16.446, 16.582, 16.586 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.0507675,
            "unit": "ms",
            "extra": "median of 5 runs: 100.687, 101.926, 102.051, 102.367, 102.373 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "08ef4207362c3468209ce613f79dec9c09fe1495",
          "message": "feat(app): the Explorer's and the panels' words wear the text roles\n\nThe second surface of the text roles. The Explorer set 12px on its root and\na panel 11.9px, and both told their words apart by opacity and three greys.\nEach word is now one of the roles: a section's title the label, a row's name\nthe body (13px), a sentence the secondary (12px, undimmed), a count and a\nnote the meta. A changed file's name keeps its change's colour, so a row's\nname takes the body from the root rather than from a class of its own.\n\nThe type guard's App.css list is 13 lines shorter and the geometry guard's 2.\n\nRefs #1920\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T21:35:52+04:00",
          "tree_id": "d40c20a12f172e2029bf8729f308ac2c93f53837",
          "url": "https://github.com/purlis/purlis/commit/08ef4207362c3468209ce613f79dec9c09fe1495"
        },
        "date": 1791914014636,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.52731,
            "unit": "ms",
            "extra": "median of 5 runs: 0.477, 0.493, 0.527, 0.533, 0.540 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.157235,
            "unit": "ms",
            "extra": "median of 5 runs: 16.726, 16.751, 17.157, 17.209, 17.296 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.0485055,
            "unit": "ms",
            "extra": "median of 5 runs: 103.167, 103.762, 104.049, 104.149, 104.655 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "f5d87330890c050647f8b4ca56edbda66bc5297e",
          "message": "ci: a cold start that cannot be taken says why, and passes while the gate only reports\n\nThe step \"Cold start on i3 under X11\" ended with exit 1 twice on 2026-10-13, on\npull requests that could not move cold start, within a second of the bare case\nbefore it and with no annotation. Report-only (`EVIDENCE_ONLY=1`) softened only a\nmissed limit or ratio: i3 not coming up, the X server not coming up, a window\nthat never appeared and a build leaving before its first frame all failed the\nstep, and said so only in the log.\n\n- `tools/coldstart-linux.sh`: no X server, or no i3 after three bounded starts,\n  is \"no measurement\": one annotation naming the cause, the case and what the\n  program said, a line in the step summary, a passing step under\n  `EVIDENCE_ONLY=1` and a failing one without. i3 leaving at once is started\n  again, and a later start that works is said as a warning with the earlier\n  starts' output. An unknown case and a build that is not where the job put it\n  fail either way, with an annotation.\n- `tools/bench.mjs`, `tools/coldstart-gate.mjs`: a launch with no first frame\n  (a window that never appeared, main's build leaving first) is the same \"no\n  measurement\". This change's own build leaving before its first frame fails\n  even under `--evidence`: that is an app that does not start. Any other error\n  in the measurement leaves an annotation before it fails. The app of a launch\n  that is given up is ended, not left behind.\n- Tests: the script with stand-ins for xvfb-run, i3 and the bench; the bench\n  against stand-in apps (Linux); the first-frame decision on its own.\n\nRefs #1876\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T21:58:20+04:00",
          "tree_id": "e20444e2a01e7e492c8fcf2c34c6a3433b6ab494",
          "url": "https://github.com/purlis/purlis/commit/f5d87330890c050647f8b4ca56edbda66bc5297e"
        },
        "date": 1791914875211,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4598485,
            "unit": "ms",
            "extra": "median of 5 runs: 0.446, 0.458, 0.460, 0.477, 0.479 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.3088675,
            "unit": "ms",
            "extra": "median of 5 runs: 16.238, 16.245, 16.309, 16.429, 16.539 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.657442,
            "unit": "ms",
            "extra": "median of 5 runs: 100.827, 101.616, 101.657, 101.798, 103.000 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "026db107f571b8f9eda94950c7172c2eaec32cdc",
          "message": "fix(app): a finished task's refusal is its name's size\n\nReview of the Chats list's move onto the text roles:\n\n- Why a finished task was not reopened wore no role, so it took the\n  section's body size, 13px, over the 12px name it stands under. It\n  keeps the danger colour, which no role has, and one rule built from\n  a token sizes it: 11.9px -> 12px, danger.text as before.\n- The section's comment said no rule below writes a size; two do, each\n  a token, and it now names them.\n- The changelog said nothing in the list is under 11px. The scope\n  switch, a persona's initials and the second step of ending a task\n  still are, so it says what is true: no line of words under a row.\n\nRefs #1920\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T22:36:17+04:00",
          "tree_id": "0f2a8f74e7c9a24753382b1789ffd31be82b8efe",
          "url": "https://github.com/purlis/purlis/commit/026db107f571b8f9eda94950c7172c2eaec32cdc"
        },
        "date": 1791916652483,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5446679999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 0.509, 0.527, 0.545, 0.554, 0.568 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.2411465,
            "unit": "ms",
            "extra": "median of 5 runs: 16.828, 16.990, 17.241, 17.263, 17.315 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 106.05864,
            "unit": "ms",
            "extra": "median of 5 runs: 104.895, 105.086, 106.059, 106.239, 107.125 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "3928cea1adf3893e37fdcdb53c2301bc787fdd21",
          "message": "review(qw368): hardening — plan names the branch, and a dev build's commit is checked against main's history\n\n- plan's `if:` names `head_branch == 'main'` beside the conclusion, the event\n  and the repository, so the four conditions are the whole rule in one place.\n  The concurrency group repeats all four.\n- A second gate, independent of the `if:`, before anything from the checkout\n  runs: the commit must be in main's history by the forge's comparison, or\n  the run fails.\n- tools/release-workflow.test.mjs: each condition is needed by itself, and\n  the gate is the first thing plan's script does.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T23:00:34+04:00",
          "tree_id": "d33f5140056e1cbdd396c3dd07650984b2e5afbc",
          "url": "https://github.com/purlis/purlis/commit/3928cea1adf3893e37fdcdb53c2301bc787fdd21"
        },
        "date": 1791918105824,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.523353,
            "unit": "ms",
            "extra": "median of 5 runs: 0.496, 0.508, 0.523, 0.525, 0.539 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.84819,
            "unit": "ms",
            "extra": "median of 5 runs: 16.253, 16.473, 16.848, 16.988, 17.033 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.155275,
            "unit": "ms",
            "extra": "median of 5 runs: 102.773, 103.997, 104.155, 104.780, 107.454 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "2cd63f690c372e2f0330ecd27d933d82e7870ef2",
          "message": "app: on the plane root's tab, Files lists the whole project's files\n\nThe operator's rulings of 2026-10-13: at the plane root Explorer shows the whole\nplane's files (G3), less what purlis keeps private, the exclusions a chat's\nsandbox applies to reads (F1), and with a workspace's clone as one row that leads\nto that workspace's tab (F2).\n\nThe core's branch type gains the plane root as a place no workspace, repo or\npiece name spells, read by the same bounded reader, tree, folder watch and file\ntab as a branch. files/kept.rs is the one place that decides what it leaves out.\nThe window's exclusion list and the sandbox's read denials are one list; a path\nis judged by spelling and by identity, so another name, a link or a second name\nof a kept file reaches nothing. A list that cannot be told whole lists nothing.\nNothing of the plane root is compared, found by name or searched.\n\nThe window reads it through two commands of its own, plane_tree and plane_file,\nwhich are the window's alone: no link is served them, the commands a link is\nserved still read a branch and nothing else, and a folder watch that names the\nplane root is refused on a link.\n\nADR 0038 and ADR 0081 are amended, and ADR 0067 notes the one list.\n\nRefs #1916\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T23:57:18+04:00",
          "tree_id": "531c2173804ebb66d781ea2902d43e4738652c49",
          "url": "https://github.com/purlis/purlis/commit/2cd63f690c372e2f0330ecd27d933d82e7870ef2"
        },
        "date": 1791921510570,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.28082250000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 0.274, 0.279, 0.281, 0.283, 0.290 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.5273275,
            "unit": "ms",
            "extra": "median of 5 runs: 16.508, 16.513, 16.527, 16.540, 16.635 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 100.8873505,
            "unit": "ms",
            "extra": "median of 5 runs: 99.594, 100.403, 100.887, 101.586, 101.918 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "e3b5b2f2b26bb8c19d84cfd9b8d9f99811d6d031",
          "message": "fix(app): a link is served a command only by name\n\nRemoval, grant, settings, install, run, publish and credential commands,\nand what is typed into a session as the person, are the window's alone,\nbeside the ones already listed; the list is the source of which they are.\n\nA link is served a command only where `SERVED_ON_A_LINK` names it. The\nhost's server keeps no other method, whatever it is built with, and a\ntest in the app requires every command to be on that list or to be the\nwindow's alone, never both and never neither. So a command added later\nis the window's alone until someone lists it as served. The\nvalue-bearing class is held to the window as a class.\n\nThe link's generated client names each window-only command and sends\nnothing for it: called off the window it answers, in that command's own\nerror shape, that it is done in purlis's own window, and `inTheWindow`\nreads it as a call the page cannot make, so a page that asks first draws\nno control. The Allowed list asks first for Remove.\n\nRefs #1927\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T00:28:48+04:00",
          "tree_id": "1b97452bcd087e569509b99bf5748d8b7f888370",
          "url": "https://github.com/purlis/purlis/commit/e3b5b2f2b26bb8c19d84cfd9b8d9f99811d6d031"
        },
        "date": 1791923395628,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4755125,
            "unit": "ms",
            "extra": "median of 5 runs: 0.473, 0.475, 0.476, 0.479, 0.481 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.3439465,
            "unit": "ms",
            "extra": "median of 5 runs: 16.125, 16.248, 16.344, 16.371, 16.460 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.130496,
            "unit": "ms",
            "extra": "median of 5 runs: 100.732, 100.946, 101.130, 101.850, 101.889 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "883e39f7fd8f026f783e74d4c03f00e70e2d5c58",
          "message": "docs(contributing): where tests live, and what a scanner's finding gets\n\nA module's tests are in `<module>/tests.rs` (or its `tests/`), which is what code scanning reads\nas test code; older layouts move when their module is next changed. A finding is answered by\nmaking the code's intent explicit before any dismissal, and a dismissal says why.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T01:07:11+04:00",
          "tree_id": "60984f1a4a63da8f063e8eb35980b7b098d6db25",
          "url": "https://github.com/purlis/purlis/commit/883e39f7fd8f026f783e74d4c03f00e70e2d5c58"
        },
        "date": 1791925770893,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5291965000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.504, 0.509, 0.529, 0.537, 0.545 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.937714999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.494, 16.804, 16.938, 16.971, 17.435 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.75045549999999,
            "unit": "ms",
            "extra": "median of 5 runs: 103.005, 103.038, 103.750, 103.915, 104.002 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "3f410c4c91c0beafd9e64ddf07724974188ea591",
          "message": "guard: a sandboxed chat never leaves a clone half-updated, and only the move that would is refused\n\nA chat's sandbox denies it the later-code names in every folder it writes (ADR 0067 §5), and\nmany repositories track one (.vscode/, .husky/, .claude/settings.json, .mcp.json, .envrc). git\nchecks a tree out file by file, skipping what it may not write. Measured in a clone: a pull,\nmerge, rebase or hard reset whose other commit changed such a file wrote every other file and\nfailed with the commit unmoved; a checkout or switch did the same and exited 0 at the new\ncommit, the file left behind as a local change; a clone wrote what it could and left its index\nempty.\n\n- A3d (`cloneupdate`): for a chat the app sandboxed, git that rewrites a working tree other\n  than the root's from another commit is judged before it starts. git is asked which paths\n  differ between the folder's commit and the commit the command moves it to, and only a move\n  that would write a denied path is refused, naming it. A rebase also answers for every\n  commit it replays; a forced move for a denied file changed in the folder.\n- Where the commit cannot be named before the command runs (a pull, another git that writes\n  before it on the line, a form whose commits are not on the line, a word that may be an alias\n  the guard could not read), it is refused only where the repository tracks a denied path, and\n  told how to make it checkable.\n- Restores to the current commit, --abort, --quit, --continue and --skip are never refused. A\n  move sent to a folder the guard cannot name, or with a work tree set on the line, is refused\n  and told to spell it out. One line is asked about four such commands at most.\n- The arm fails closed where it cannot ask: a repository purlis could not read is not taken\n  for one that tracks nothing. A command run below a clone's top is asked about that clone,\n  and a checkout or switch about the branch of a name before a tag of it.\n- `git clone` of a repository on this machine that tracks a denied path is refused up front,\n  naming `purlis clone` and the person's terminal. A clone from a URL cannot be asked.\n- A net under it: after a git command that said it could not write a later-code file, the\n  chat is told in that turn what the folder now is and the command that puts it back or\n  finishes it, in the one line the block word prints.\n- git is asked through the hardened runner alone, with no global or system configuration, by\n  plumbing that compares commits. A path that reads as prose is counted, never repeated.\n- The root's guard (A3c) stays as it was.\n\nThe walk the project-root guards share read every git on a line against each directory the\nshell may be in by asking the disk again, and each `cd <name> && …;` left one more directory,\na level deeper: a 16 KB line of them took those guards 27 seconds, the cube of its length.\nEach directory is now asked of the disk once for the line, in one pass for both of the walk's\nanswers, and the directories in play are capped by size as well as number: past 64 of them, or\n8 KiB of their names, none is spelt and a git there is one the guard cannot name. The\nlinear-time test keeps its bounds, and gains the arm, its three shapes and that one.\n\nThe whole verdict then left its bound on a runner, on a line of exports before many gits: the\nnew arm read the line once more, and every arm read the line's configuration for each git\nwhatever its word. The root's arm and the clone's are now asked of one reading of the line,\nand the configuration is read only for a word that may be an alias.\n\nAn independent review then found the arm let these through, each now refused or asked as git\nreads it: a folder with no commit yet, or never checked out, where git writes the whole tree;\na name git takes for a remote branch; a move after any command that may write, not only git;\na forced checkout of a branch; a clone's attached options and `file:` source. Only moves are\ncounted against what one line is asked about, git is asked once for a repository on a line,\ndenied paths are gathered through a set, and the net reads each sandbox's refusal.\n\nRefs: lanes qw349 and qw367.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T01:08:00+04:00",
          "tree_id": "db7a9e873ba31f746456964a332090c2bd8fb8ff",
          "url": "https://github.com/purlis/purlis/commit/3f410c4c91c0beafd9e64ddf07724974188ea591"
        },
        "date": 1791926791080,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5393165,
            "unit": "ms",
            "extra": "median of 5 runs: 0.505, 0.518, 0.539, 0.542, 0.543 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.8159815,
            "unit": "ms",
            "extra": "median of 5 runs: 16.192, 16.591, 16.816, 16.876, 17.116 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.0296605,
            "unit": "ms",
            "extra": "median of 5 runs: 101.513, 103.890, 104.030, 104.136, 105.290 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "063efbb4677bf720a4c36a811eb46c9db770cf0a",
          "message": "fix(profiles): a git that never answers is ended with what it started\n\n`a_git_that_never_answers_refuses_too_because_a_hang_is_not_a_pass` gives the ignore check a\ngit that is a shell script running `sleep 20`. At its two second deadline the check killed\nthe script alone, so the `sleep` ran on for eighteen seconds with nobody waiting for it.\n`tools/no-orphans.sh` fails the Rust tests when a process the suite started is still there\nfive seconds after it ends, so the job was red whenever this test began in the suite's last\nfifteen seconds, and green otherwise: main at 3f410c4c and two pull requests on it were red\non \"rust tests (part 2 of 2)\", and the same commit was green when run again. The run of this\nbranch's first commit named it: \"pid 136857, running for 00:19: sleep 20\".\n\nThe program the check is given now leads a process group of its own, and the group is\nkilled at the deadline, as `sandbox::program` ends its probe. The test's git says which\nprocess it started and the test holds that it is gone.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T02:32:45+04:00",
          "tree_id": "3ceb9d664435e5f0e07cc9d6426b8af57bc587bd",
          "url": "https://github.com/purlis/purlis/commit/063efbb4677bf720a4c36a811eb46c9db770cf0a"
        },
        "date": 1791930831287,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.276123,
            "unit": "ms",
            "extra": "median of 5 runs: 0.256, 0.257, 0.276, 0.277, 0.283 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.546208999999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.510, 16.538, 16.546, 16.566, 16.608 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.1920015,
            "unit": "ms",
            "extra": "median of 5 runs: 100.663, 100.825, 101.192, 101.594, 102.802 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "11ae3e9c7ca8ffdf4e73f0b46173c0afbaa6777d",
          "message": "review(qw371): a request from the save's branch into its target that purlis did not open is refused\n\nThe contract case the third review named as a gap: it fails if the\nopened-by-purlis condition is removed.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T02:59:36+04:00",
          "tree_id": "59b7260b882b1db9666c61b910a1c243ef465165",
          "url": "https://github.com/purlis/purlis/commit/11ae3e9c7ca8ffdf4e73f0b46173c0afbaa6777d"
        },
        "date": 1791933144128,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4589135,
            "unit": "ms",
            "extra": "median of 5 runs: 0.444, 0.453, 0.459, 0.461, 0.471 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.2765005,
            "unit": "ms",
            "extra": "median of 5 runs: 16.163, 16.216, 16.277, 16.771, 17.009 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.720237,
            "unit": "ms",
            "extra": "median of 5 runs: 101.301, 101.359, 101.720, 101.911, 101.964 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "eb9629c4f09a933dd0686e939bf168ef5fd7792e",
          "message": "guard: the leak guard follows a long run of cds in linear time\n\nThe leak guard follows every cd on a line, since what a later command reads is read from\nwhere the shell then stands. For each one it built the directory again from the start and\nnormalised the whole of it again, so a run of them cost the square of its length: 184 ms at\n16 KB and 2.8 s at 64 KB in a test build. The family test's two sizes sat just inside its\nallowance on most runs and just outside it on a slow runner, which is how main went red.\n\nThe normalised place is now carried from one cd to the next (pypath::Normalised, held equal\nto normpath of the whole by a property test), noted only when the shell has moved, and the\ndirectory is appended to in place and borrowed for a segment. What the guard answers is\nunchanged, and the recorded answers replay as they were.\n\nOne test is added, at 16 KB against 64 KB on three runs of relocations, so the step cannot\nhide inside the family test's allowance again. No bound is raised and no shape exempted.\n\nRefs: lane qw377.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T03:32:15+04:00",
          "tree_id": "4abede45c4d25378076d0908c9d11ab82c786861",
          "url": "https://github.com/purlis/purlis/commit/eb9629c4f09a933dd0686e939bf168ef5fd7792e"
        },
        "date": 1791934405950,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.363011,
            "unit": "ms",
            "extra": "median of 5 runs: 0.348, 0.351, 0.363, 0.369, 0.377 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.4993975,
            "unit": "ms",
            "extra": "median of 5 runs: 16.461, 16.485, 16.499, 16.556, 16.696 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.35064700000001,
            "unit": "ms",
            "extra": "median of 5 runs: 100.996, 101.551, 102.351, 102.557, 103.251 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "859fe395d7297cf9d86571a72c13c76664aa24b3",
          "message": "review(qw372): a warning is never cut, a name is never in capitals, and Stop is in the last group\n\nFrom the independent review of #1946:\n\n- A group named by data (a persona's group under Curate) wore the label\n  role and was headed in capitals, against ruling T4. It now wears the\n  secondary role as the name is written.\n- What stands under a row's words wraps and is never cut: a warning could\n  lose \"There is no undo.\" to an ellipsis, and a greyed row's reason had its\n  rest only in a platform tooltip that no keyboard opens.\n- The show-more and the away menus keep their own width: their rows have no\n  part that gives way to an ellipsis.\n- A vault's warning claimed every vault's secrets are lost; only a keychain\n  vault's are. Stop all tasks says the chat keeps running, and a workspace's\n  names what is deleted.\n- Stop, whose rows each end something, is in the last group above End, and\n  the test for \"what cannot be undone is last\" looks inside submenus.\n- The task chip's Activity row uses the menus' tooltip. The every-menu test\n  covers a session record, a memory, a finished task and a refused read, and\n  Read the workspace again has its place in the model.\n- `Groups` is exported for a menu whose rows are not the catalogue's, and\n  the docs say how such a menu declares its rows.\n- Comments in actions.ts that said where a menu draws a row are corrected;\n  no behaviour there is changed.\n\nNot seen rendered.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T04:10:48+04:00",
          "tree_id": "fe4aae765ec09378138d44187d9f2e90032b7a46",
          "url": "https://github.com/purlis/purlis/commit/859fe395d7297cf9d86571a72c13c76664aa24b3"
        },
        "date": 1791936710179,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.2710845,
            "unit": "ms",
            "extra": "median of 5 runs: 0.260, 0.267, 0.271, 0.275, 0.285 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.464796,
            "unit": "ms",
            "extra": "median of 5 runs: 16.368, 16.407, 16.465, 16.526, 16.682 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.150842,
            "unit": "ms",
            "extra": "median of 5 runs: 100.854, 101.075, 101.151, 101.250, 101.511 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "10463db493a7ba5fc5a7a0b4dd0fc53319a56eab",
          "message": "review(qw376): a git that never said what it started fails the profiles test\n\nThe pid marker is still waited for with a bound, but its absence is a\nfailure again: without the id, the assertion that the group was ended\ncould pass without holding anything.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T04:41:06+04:00",
          "tree_id": "2f29e87d7873ff57e0a35bbb608215625b712663",
          "url": "https://github.com/purlis/purlis/commit/10463db493a7ba5fc5a7a0b4dd0fc53319a56eab"
        },
        "date": 1791938545336,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5173845,
            "unit": "ms",
            "extra": "median of 5 runs: 0.514, 0.516, 0.517, 0.529, 0.537 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.969526000000002,
            "unit": "ms",
            "extra": "median of 5 runs: 16.484, 16.763, 16.970, 17.118, 17.133 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.19409099999999,
            "unit": "ms",
            "extra": "median of 5 runs: 103.489, 104.125, 104.194, 104.303, 104.809 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "7aa486a43e5230e72a67d49894620f5077044676",
          "message": "sandbox: a chat does not start when purlis cannot tell what to keep from it\n\nA chat's sandbox is compiled from the list of what purlis keeps from a chat.\nThat list is whole or it is nothing, for every reader of it.\n\nEvery entry of the vault registry, in the committed half and in this machine's,\nis judged under whatever name it is written. A vault of a kind purlis knows\nresolves to where it keeps its secrets, which is denied. A registry that does\nnot read, a `vaults` that is not an object, a plain-file vault whose file is\nsaid in a way purlis cannot use, and a vault's file with more names than the\nproject can be looked through for refuse the start. The refusal is one more\nreason a sandboxed chat does not start, given before any harness's compiler is\nasked, so it is the same for Claude Code, Codex and opencode on every system,\nand for a new, resumed, relaunched, restored or dispatched chat. A running chat\nis not stopped. The sentence says why, gives the path of the file to put right\nand names Doctor; the new-chat picker says it before anything starts.\n\nOrdinary states start: no registry, no state folder, an empty registry, vaults\nin the keyring, a plain-file vault this machine has not set up yet, an entry\nleft over on this machine, and a vault of a kind this purlis does not know,\nwhose named files are kept all the same. Doctor's\n`sandbox kept files` row says each of these, and says why the list is not whole\nwhenever it is not, sandbox on or off. The vault's view says the same of each\nentry.\n\nA vault's file is kept under the other names purlis finds for it.\n\nThe window reads by the same list: a branch's folder leaves out what the\nproject keeps, in its tree, opens, placed and named paths, folder watch, editor\nhand-off, search, file palette, comparisons and change marks. In a project that\nruns its chats sandboxed a branch is not read where the list is not whole; the\nplane root keeps its own rule, and the one file that still opens there is the\nregistry to put right. The offered list fails closed by itself. A link's folder\nwatch reaches the app only when every folder names a branch.\n\nADR 0067 and ADR 0038 each carry a dated section, to be confirmed.\n\nRefs #1916\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T05:12:56+04:00",
          "tree_id": "dd5feb5184535d208ca2bf38952e2f1a2f95c70f",
          "url": "https://github.com/purlis/purlis/commit/7aa486a43e5230e72a67d49894620f5077044676"
        },
        "date": 1791940453202,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.42889299999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 0.408, 0.415, 0.429, 0.430, 0.448 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.4350055,
            "unit": "ms",
            "extra": "median of 5 runs: 16.145, 16.168, 16.435, 16.718, 16.893 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.605799,
            "unit": "ms",
            "extra": "median of 5 runs: 101.346, 101.545, 101.606, 102.079, 102.528 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "5c1f53608b732c19ed31753d58f3c08bc51d0ae4",
          "message": "review(qw376): the quit test's hook id is read off unix too\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T05:34:04+04:00",
          "tree_id": "ad8a16719918685eacef1cb22a31041342c3f43d",
          "url": "https://github.com/purlis/purlis/commit/5c1f53608b732c19ed31753d58f3c08bc51d0ae4"
        },
        "date": 1791941714148,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4973905,
            "unit": "ms",
            "extra": "median of 5 runs: 0.480, 0.482, 0.497, 0.499, 0.499 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.3744165,
            "unit": "ms",
            "extra": "median of 5 runs: 16.308, 16.331, 16.374, 16.377, 16.390 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 100.952509,
            "unit": "ms",
            "extra": "median of 5 runs: 100.511, 100.847, 100.953, 101.176, 101.448 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "15d71518bfaa267523675538d27ed6d8cb7ad68e",
          "message": "review(qw370): no overlay of a project behind is ever mounted, whenever it was opened\n\nFrom the second review of the pull request.\n\nA view kept behind another is not drawn again by its parent, but a part of it\ncan still set its own state. Land… asks the core and opens its question on the\nanswer: with nothing over the window while it waits, the person could go to\nanother project, and the answer then opened \"Land widget's pull request #7?\"\nover that one, naming no project, where Land would merge in the first. The rule\nthat looked as a project leaves the front could not see it.\n\nSo every overlay is written through overlays/, one file per Radix package: the\nsame parts, with the portal, the content and the scrim drawn only while their\nproject is in front. Nothing of a project behind is mounted over the window, so\nnothing a dialog does to the document as it mounts happens either, and the\nquestion is there when the person is back in the project it is about. A guard\nfails for an overlay imported from the package, for a kind with no file there,\nand for a new portal. The guards that find a file's dialogs and menus by what\nit imports read both spellings.\n\nThe look as a project leaves stays as a backstop, and is made once the press is\nover: a menu's row switches and then closes its menu, in that order.\n\nThe switch clock measures a command only where it was asked for a switch, and\nkeeps one measure of each name. A view's scrolled places are let go as they\npile up, and not only as it leaves.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T05:53:04+04:00",
          "tree_id": "afa07ba07ef64e4a983599b12a521a57995f9be0",
          "url": "https://github.com/purlis/purlis/commit/15d71518bfaa267523675538d27ed6d8cb7ad68e"
        },
        "date": 1791942852614,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5495785,
            "unit": "ms",
            "extra": "median of 5 runs: 0.518, 0.528, 0.550, 0.550, 0.565 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.154533999999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.459, 16.466, 17.155, 17.187, 17.374 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.0951925,
            "unit": "ms",
            "extra": "median of 5 runs: 102.574, 104.050, 104.095, 104.874, 104.913 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "b4b57accbc5f6e0150bec0c2846174990120c5b1",
          "message": "feat(terminal): a right-click in a terminal has Copy, Paste and Select all\n\nA chat's or a shell's terminal had no menu of its own: a right-click\nopened the panes' menu (a new tab, the splits, ending the chat), and\nthere was no pointer route to the clipboard.\n\nA terminal now opens its own menu, declared and drawn as every context\nmenu is (a GroupDrawn of its own rows, then the pane's groups from\nmenuOf, drawn by Groups): Copy, Paste and Select all, with a Mac's keys\nat the rows' ends, and the pane's rows after them, so none is stranded.\n\n- Copy is off with nothing selected, writes xterm's own selection text\n  and keeps the selection. It reads the selection at the press.\n- Paste (the operator's ruling of 2026-10-14): a window-only command,\n  clipboard_text, reads the clipboard in the core on the press, and the\n  text goes through xterm's own paste, the path the keyboard's paste\n  takes, bracketed when the program asked. The command takes no\n  argument, reads only when asked by a handle of its own, keeps and\n  writes down nothing, and answers text only, up to 1 MiB; a read that\n  fails says it failed. It is of the value-bearing class and no link is\n  served it (ADR 0052, amended).\n- A menu opened from the keyboard rests on no row, so Enter after the\n  menu key never pastes.\n- While a program has mouse reporting on, a plain right-click is the\n  program's and opens no menu; Shift+right-click always opens it, a\n  Mac's Ctrl+click counting as the right button. The press kept from\n  xterm has its default prevented, and the menu puts the keyboard in\n  the terminal when it closes on Escape or one of its own rows.\n- The menu key opens the menu under the cursor. Shift+F10 does while no\n  program has the mouse, and a second Shift+F10 hands the key back; a\n  key held down is one press.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T06:17:09+04:00",
          "tree_id": "508079a2abdcd2f589f76186bcd1f458b5457de7",
          "url": "https://github.com/purlis/purlis/commit/b4b57accbc5f6e0150bec0c2846174990120c5b1"
        },
        "date": 1791944304718,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5275814999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 0.506, 0.514, 0.528, 0.536, 0.547 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.7121425,
            "unit": "ms",
            "extra": "median of 5 runs: 16.464, 16.574, 16.712, 16.901, 17.166 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.146585,
            "unit": "ms",
            "extra": "median of 5 runs: 102.623, 102.876, 104.147, 105.091, 105.717 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "5a1f2aafaa4e3ccab4e876fd5129995f5b8eac58",
          "message": "fix(qw378): the merge's question is an overlay of its project, held back while the project is behind\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T06:47:49+04:00",
          "tree_id": "28b649f1cb465fb2d6cc7edef85b1a35e51fc242",
          "url": "https://github.com/purlis/purlis/commit/5a1f2aafaa4e3ccab4e876fd5129995f5b8eac58"
        },
        "date": 1791946138803,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.46963299999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 0.453, 0.459, 0.470, 0.471, 0.480 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.275183499999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.154, 16.269, 16.275, 16.326, 16.370 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.332551,
            "unit": "ms",
            "extra": "median of 5 runs: 101.155, 101.174, 101.333, 101.666, 101.991 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "14a4e8dabf956a25788d5a3babeab83b9b1cd38d",
          "message": "fix(app): a pane's gauge is read after a switch is painted, and an overlay has one name\n\nTwo things left by the review of the pull request that kept a project's view\nbehind another.\n\nA chat's gauge was read at once as its project came back. Its command is one\nthe core answers on the window's own thread, and every pane of the tab in front\nhas a gauge, so that was one such command per pane ahead of the paint: the cost\nthat grows with a window, on the path that is meant to wait on nothing. It is\nread once the project has been painted, as every other read made again is. The\ncost test's budgets are drawn in to just above what a switch does now, and a\nnew case holds the commands sent before the paint to the same number with one,\ntwenty and forty panes in the tab in front.\n\nRadix exports each part twice, `Content` and `DialogContent`. The window's\noverlay files handed the package on whole, so the long name was the same part\nwith nothing asked of it. They hand on the parts by their short names alone,\nand the guard fails for a part that draws handed on as it is under any name.\n\nThe test of the scrolled places being let go as they pile up, which the review\nnoted missing.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T07:07:12+04:00",
          "tree_id": "d86424612700612b624d36180e71d1f6809d12cd",
          "url": "https://github.com/purlis/purlis/commit/14a4e8dabf956a25788d5a3babeab83b9b1cd38d"
        },
        "date": 1791947300023,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.478609,
            "unit": "ms",
            "extra": "median of 5 runs: 0.454, 0.466, 0.479, 0.480, 0.482 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.2299975,
            "unit": "ms",
            "extra": "median of 5 runs: 16.122, 16.150, 16.230, 16.391, 16.587 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.34237949999999,
            "unit": "ms",
            "extra": "median of 5 runs: 101.034, 101.289, 101.342, 102.010, 102.373 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "9b7cc465b3e2833aacc57154b509a9b6e626ea1e",
          "message": "docs(secrets): a tool's example reads its values itself; curl expands its own\n\nThe several-values example is now `aws`, which reads AWS_ACCESS_KEY_ID and\nAWS_SECRET_ACCESS_KEY from its environment. curl reads no credential from\na variable, and a chat's arguments reach a tool with no shell between, so\nthe secrets guide shows curl's own expansion instead (curl 8.3.0 or\nlater): both values as env, then\n`curl --variable %KIBANA_USER --variable %KIBANA_PASS\n--expand-user '{{KIBANA_USER}}:{{KIBANA_PASS}}' <url>`.\nThe guide's first `secret exec ... curl` line is written the same way.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T11:57:23+04:00",
          "tree_id": "f7f967a304bd3f18ca443a1ba94e078b0b124bd6",
          "url": "https://github.com/purlis/purlis/commit/9b7cc465b3e2833aacc57154b509a9b6e626ea1e"
        },
        "date": 1791964711516,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.2854605,
            "unit": "ms",
            "extra": "median of 5 runs: 0.262, 0.277, 0.285, 0.287, 0.299 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.635003,
            "unit": "ms",
            "extra": "median of 5 runs: 16.577, 16.583, 16.635, 16.664, 16.709 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.870088,
            "unit": "ms",
            "extra": "median of 5 runs: 101.064, 101.189, 101.870, 101.928, 101.960 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "1fe33d08f3a6f96b77bb9a942f4c712deb62a383",
          "message": "review(qw379): the lanes' file list names the files as purlis does\n\nRefs #1949\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T12:46:06+04:00",
          "tree_id": "f2c949df6852afae592889573299478d4b94e383",
          "url": "https://github.com/purlis/purlis/commit/1fe33d08f3a6f96b77bb9a942f4c712deb62a383"
        },
        "date": 1791967642279,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.537123,
            "unit": "ms",
            "extra": "median of 5 runs: 0.507, 0.525, 0.537, 0.546, 0.562 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.724379,
            "unit": "ms",
            "extra": "median of 5 runs: 16.332, 16.530, 16.724, 16.895, 17.177 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.959295,
            "unit": "ms",
            "extra": "median of 5 runs: 103.455, 103.832, 103.959, 104.323, 106.742 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "981ba86a5bcab13d5c48ff3d7f0100230bd4c7a8",
          "message": "review(qw382): a chat is offered by what git stopped in, not by unmerged files\n\nA rebase stopped with every file added has nothing unmerged, and the view\noffered a chat, which may not continue a rebase in the plane root. The core\nnow says what git is stopped in (`stopped_by`: merge, rebase, am,\ncherry-pick, revert, bisect, or conflicts), and the view offers a chat only\nfor a merge or bare unmerged files. `rebase_by_hand` is set only while the\nconflict record is why the save is blocked.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T13:36:04+04:00",
          "tree_id": "415440c68d4c873d8c13da9350ab8d406d0f674d",
          "url": "https://github.com/purlis/purlis/commit/981ba86a5bcab13d5c48ff3d7f0100230bd4c7a8"
        },
        "date": 1791970634739,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.304761,
            "unit": "ms",
            "extra": "median of 5 runs: 0.282, 0.296, 0.305, 0.311, 0.330 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.725832500000003,
            "unit": "ms",
            "extra": "median of 5 runs: 16.662, 16.718, 16.726, 16.730, 16.756 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.564769,
            "unit": "ms",
            "extra": "median of 5 runs: 101.153, 101.512, 101.565, 101.606, 101.913 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "73f710c974e64e5ffd6de179f8cdb3b482a4e6ee",
          "message": "review(qw383): the list takes programs that copy or encode, and says it is not exhaustive\n\n- GENERAL_PURPOSE adds the programs that copy, encode or rewrite what\n  they are given (cat, cp, tee, dd, sed, base64, xxd, od, hexdump,\n  openssl, gzip, tar, zip), beside the network ones; a core test covers\n  them.\n- secrets.md, the plane-format tools row, ADR 0067's T-4 section and the\n  changes fragment say the list is not exhaustive: a tool without the\n  sentence is not thereby a safe one, since any program can show what it\n  is handed if its arguments ask it to. Tests name such tools \"unlisted\".\n- The Notice keeps one space between the vault sentence and what follows,\n  with or without the sentence; secrets.md's paragraph joins again.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T13:59:51+04:00",
          "tree_id": "a59ba7e2caa6b943beab0985d52f6d378ed55e86",
          "url": "https://github.com/purlis/purlis/commit/73f710c974e64e5ffd6de179f8cdb3b482a4e6ee"
        },
        "date": 1791972068109,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.429109,
            "unit": "ms",
            "extra": "median of 5 runs: 0.413, 0.422, 0.429, 0.439, 0.440 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.4218105,
            "unit": "ms",
            "extra": "median of 5 runs: 16.241, 16.325, 16.422, 16.525, 16.711 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.1213495,
            "unit": "ms",
            "extra": "median of 5 runs: 100.366, 100.971, 101.121, 102.021, 102.168 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "3a9d2ded5811202ab44087f8c629c7c9737c0a35",
          "message": "review(qw381): the failed-start commands are listed as in-memory window commands\n\nAfter the rebase onto #1956: mcp_failures and put_away_mcp_failure read and\nput away what the app holds in memory, so they stay synchronous with that\nreason on the list.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T14:37:47+04:00",
          "tree_id": "da1911e195495fcdf5a341be3d490c52e17ebd92",
          "url": "https://github.com/purlis/purlis/commit/3a9d2ded5811202ab44087f8c629c7c9737c0a35"
        },
        "date": 1791974364500,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5156825,
            "unit": "ms",
            "extra": "median of 5 runs: 0.492, 0.505, 0.516, 0.522, 0.532 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.765201500000003,
            "unit": "ms",
            "extra": "median of 5 runs: 16.254, 16.518, 16.765, 16.920, 17.074 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.4669595,
            "unit": "ms",
            "extra": "median of 5 runs: 104.031, 104.440, 105.467, 105.619, 105.736 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "86112c50f1ad1243577a433874ccf0cc963b0a1d",
          "message": "review(qw379b): the guard follows commands to the writes, and an open after the quit lets go\n\n- The lane guard (lane_writes_guard_tests.rs) follows each window command two\n  calls down, qualified calls to that module's file and plain calls in the\n  same file, and holds every command that reaches a listed write to that\n  site's `by`. Any function of the sandbox's local file that is not one of its\n  known reads counts as a write. Test modules are skipped by their braces, so\n  code after one is read. A row no command writes says why. Self-tests hold\n  each of these, with fixtures: a new command writing through a listed\n  helper, code after a test module, a new local function.\n- It found what the table missed: the settings commands that prune confirmed\n  hosts through `file_of`, choosePlaneMode through the settings writer, the\n  away list's Dismiss, and the sandbox's start tally (`local::count`).\n- An open still in its sweep when the quit stops waiting lets go of the\n  project it built instead of holding it: the quit sets a flag the open reads\n  under the registry's lock.\n\nRefs #1949\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T14:57:48+04:00",
          "tree_id": "96bcd46eace2853b52aa623486cbe4ece6d20678",
          "url": "https://github.com/purlis/purlis/commit/86112c50f1ad1243577a433874ccf0cc963b0a1d"
        },
        "date": 1791975541239,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.276521,
            "unit": "ms",
            "extra": "median of 5 runs: 0.269, 0.270, 0.277, 0.277, 0.288 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.497253,
            "unit": "ms",
            "extra": "median of 5 runs: 16.370, 16.471, 16.497, 16.525, 16.598 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.51117149999999,
            "unit": "ms",
            "extra": "median of 5 runs: 101.103, 101.338, 101.511, 101.578, 101.584 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "d21610f8d6417871057c8b1354fd967c2d9d9fb9",
          "message": "docs(changes): name the fragment for the rule it states\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T15:49:32+04:00",
          "tree_id": "b4dc72c61e43bf70a9c35396fc57d59167935428",
          "url": "https://github.com/purlis/purlis/commit/d21610f8d6417871057c8b1354fd967c2d9d9fb9"
        },
        "date": 1791978651889,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5957574999999999,
            "unit": "ms",
            "extra": "median of 5 runs: 0.590, 0.590, 0.596, 0.603, 0.607 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.2673435,
            "unit": "ms",
            "extra": "median of 5 runs: 16.232, 16.258, 16.267, 16.455, 16.554 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.6161135,
            "unit": "ms",
            "extra": "median of 5 runs: 100.704, 101.337, 101.616, 101.642, 103.456 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "4189c5cb7f3c45c0e1bb23699e6aa504b2f7463c",
          "message": "review(qw385): the key-listing test lives in a file of its own\n\nThe lane-writes guard (#1949) reads vaults.rs's code and closed its test\nmodule early, so the test's grant read as a write the window's table must\nlist. A `_tests.rs` file is skipped whole.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T16:50:50+04:00",
          "tree_id": "aed7353074b7cb62ecb0a9ceff2950e457629030",
          "url": "https://github.com/purlis/purlis/commit/4189c5cb7f3c45c0e1bb23699e6aa504b2f7463c"
        },
        "date": 1791982317929,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.540405,
            "unit": "ms",
            "extra": "median of 5 runs: 0.520, 0.525, 0.540, 0.556, 0.574 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.1570205,
            "unit": "ms",
            "extra": "median of 5 runs: 16.131, 16.154, 16.157, 16.236, 16.286 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.7695645,
            "unit": "ms",
            "extra": "median of 5 runs: 101.999, 102.339, 102.770, 102.836, 104.659 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "7ddfb1719207004c4acfed4802355a6a210223c3",
          "message": "review(qw382b): Skip checks for itself, the read-back follows git's filters, attributes are honoured\n\n- Skip drops a commit only while nothing is unmerged and nothing is left to\n  commit, whatever the window last drew.\n- What Continue staged is compared with the checked bytes as git stores them\n  for that path (`hash-object --path`), so line-ending and clean-filter\n  projects carry on.\n- At a stop, a markable file with no marker in it (a `.gitattributes`\n  `binary`, `-merge` or merge driver) is the person's to settle.\n- Where the window did not see the stop, its answer leads with carrying on\n  in a terminal, which keeps what is settled, and says Abort discards it.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T17:22:41+04:00",
          "tree_id": "f16d8babad54dc2befc7c5dea192d47551c63fca",
          "url": "https://github.com/purlis/purlis/commit/7ddfb1719207004c4acfed4802355a6a210223c3"
        },
        "date": 1791984230634,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.517655,
            "unit": "ms",
            "extra": "median of 5 runs: 0.462, 0.505, 0.518, 0.528, 0.536 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.567778,
            "unit": "ms",
            "extra": "median of 5 runs: 16.297, 16.382, 16.568, 17.057, 17.225 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.581657,
            "unit": "ms",
            "extra": "median of 5 runs: 101.542, 103.516, 103.582, 103.966, 105.055 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "01ab033d5f33693fc01aa014597b7ed7d0eb3c74",
          "message": "review(qw390): the deny job prints all of cargo-deny's output, pass or fail\n\nThe annotations stay at ten (GitHub keeps ten error annotations a step);\nthe notice for the rest now points at the log, which has every finding\nand warning.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T18:40:18+04:00",
          "tree_id": "8a19c036891e4bcf2e5bb25cd0291b567ff1ffc3",
          "url": "https://github.com/purlis/purlis/commit/01ab033d5f33693fc01aa014597b7ed7d0eb3c74"
        },
        "date": 1791988905030,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.541696,
            "unit": "ms",
            "extra": "median of 5 runs: 0.528, 0.538, 0.542, 0.550, 0.559 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.0213675,
            "unit": "ms",
            "extra": "median of 5 runs: 16.420, 17.014, 17.021, 17.392, 17.544 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.72968,
            "unit": "ms",
            "extra": "median of 5 runs: 103.434, 103.841, 104.730, 105.113, 106.632 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "68472aa3f7158ee6f4d9717dd2470946b0db1999",
          "message": "review(qw387): the sparse allowlist is each subcommand's own, and an editor counts as configuration\n\nA flag harmless to one subcommand may be another's command to run\n(`cherry-pick -x` is `rebase --exec`), so each move's options are held\nto its own subcommand's list. An editor set on the line (GIT_EDITOR,\nGIT_SEQUENCE_EDITOR, EDITOR, VISUAL) turns the filter off, as\nconfiguration does.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T22:59:40+04:00",
          "tree_id": "41d069cacc129b118d1cbd182d8453a5e6aebab8",
          "url": "https://github.com/purlis/purlis/commit/68472aa3f7158ee6f4d9717dd2470946b0db1999"
        },
        "date": 1792004471679,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.285295,
            "unit": "ms",
            "extra": "median of 5 runs: 0.265, 0.271, 0.285, 0.286, 0.286 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.600793499999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.530, 16.572, 16.601, 16.620, 16.664 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.5766305,
            "unit": "ms",
            "extra": "median of 5 runs: 101.112, 101.458, 101.577, 101.773, 101.999 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "da2b57943c7348d38172402649d0ec69e5d18536",
          "message": "The terminal and its engine in a crate of their own (#2066)\n\nA session host owns terminals without linking the rest of the core\n(#1995). `purlis-terminal` (crates/terminal) holds the session, the\nengine, the fork lock every program is started under, the process-group\nwait a session's end makes, and the start-size limit a chat's start is\nchecked against. It depends on nothing of git, HTTP or the keyring, and\nnot on purlis-core; a guard test reads cargo's resolution and fails on\none.\n\npurlis-core re-exports `engine` and `session`, keeps its `forklock`\ngate (the session bus step, then the terminal crate's lock), and\nre-exports the moved helpers where it used them, so no caller changes.\nThe moved tests keep their names and bodies; only paths into the core's\nsandbox module now name the terminal crate's `exec`.\n\nCloses #1995\n\n\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-14T23:06:17+04:00",
          "tree_id": "745715e7d3d1f8273e39320f9c3bf6808eea270b",
          "url": "https://github.com/purlis/purlis/commit/da2b57943c7348d38172402649d0ec69e5d18536"
        },
        "date": 1792005590636,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5177555,
            "unit": "ms",
            "extra": "median of 5 runs: 0.491, 0.491, 0.518, 0.527, 0.529 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.018364499999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.407, 16.413, 17.018, 17.121, 17.465 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.57156800000001,
            "unit": "ms",
            "extra": "median of 5 runs: 103.060, 103.513, 104.572, 104.852, 105.838 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "5f0fda7994e51030cc82d626a5f072da3b398fc0",
          "message": "fix(app): the lane guard counts what the sandbox records as seen, and reads past comments\n\n- `hosts_seen_by_you` and `presets_seen_by_you` write this machine's sandbox\n  file (they acknowledge what the window wrote): they are no longer among the\n  local file's reads, and their site, `settings.rs seen_by_you`, is listed\n  with the commands that reach it through a method call.\n- The guard blanks block comments (nested) and raw strings that span lines\n  before it reads a file a line at a time, so a call inside one is not a\n  write, and a quote in a character literal starts no string.\n- The reasons the forge and profile commands stay out of the sandbox lane\n  also cover the read they answer with, which drops confirmed hosts the file\n  no longer lists.\n\nRefs #1949\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T23:40:09+04:00",
          "tree_id": "72a24c5a23d92e2085d608732af89779298cdf39",
          "url": "https://github.com/purlis/purlis/commit/5f0fda7994e51030cc82d626a5f072da3b398fc0"
        },
        "date": 1792007014807,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4599225,
            "unit": "ms",
            "extra": "median of 5 runs: 0.434, 0.454, 0.460, 0.476, 0.478 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.6563595,
            "unit": "ms",
            "extra": "median of 5 runs: 16.531, 16.547, 16.656, 16.660, 16.945 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.85233600000001,
            "unit": "ms",
            "extra": "median of 5 runs: 101.307, 101.720, 101.852, 102.445, 102.773 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b8a320a8744e9075c122cd52b9cf6c4f4df40a30",
          "message": "Leftover persona sub-agent files: a Notice with Fix, not a line in every briefing (#2072)\n\n* Leftover persona sub-agent files: a Notice with Fix, not a line in every briefing\n\nThe briefing no longer tells each chat that the project still holds the sub-agent files purlis\ngenerated for a persona. The doctor's `personas` row, when it offers `persona-agents`, now\nstands as a Notice in the Inbox (FINDINGS_AS_NOTICES), whose Fix runs the same fix the dialog's\nrow runs through `plane_doctor_fix` (already window-only), and which goes once the doctor\nchecks again without the files (#1990, Q67).\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(1990): The briefing's call site and the Notice rule name the persona-agents exception\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T00:07:33+04:00",
          "tree_id": "fd44089c7dc7e8ed8489053ac0a7163a9ab187a7",
          "url": "https://github.com/purlis/purlis/commit/b8a320a8744e9075c122cd52b9cf6c4f4df40a30"
        },
        "date": 1792008532480,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.512025,
            "unit": "ms",
            "extra": "median of 5 runs: 0.493, 0.504, 0.512, 0.516, 0.526 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.981439,
            "unit": "ms",
            "extra": "median of 5 runs: 16.585, 16.857, 16.981, 17.064, 17.103 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.49175550000001,
            "unit": "ms",
            "extra": "median of 5 runs: 102.929, 103.249, 103.492, 104.470, 104.554 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "dd12dcfd6b51606a830d8e411dfeebd772843994",
          "message": "A service on this Mac is allowed by its one port (qw384) (#1966)\n\n* feat(sandbox): a service on this Mac is allowed by its one port (qw384)\n\nA sandboxed chat that reaches a service listening on this machine (kubectl\nagainst a local cluster at 127.0.0.1:<port>, a database in a container, a\ndev server) now gets a Block naming the port, and its Notice offers Allow\nfor this chat or for this project on this machine.\n\n- core: sandbox/localport.rs judges one port (1-65535, never purlis's own),\n  reads Go's and Node's refusal words, and binds purlis's own proxy and\n  tunnel ports away from any port a chat was started with.\n- grant::What::Port, kept for one chat in memory or in this machine's\n  sandbox file (ports_mine); policy's socket-grants lock holds it too.\n- a start compiles each allowed port into the chat's Seatbelt profile\n  (Codex, opencode) and its proxy's reach, as exactly 127.0.0.1, ::1 and\n  localhost at that port.\n- sandboxblock::Kind::LocalService: Go/Node words, the sandbox's own\n  network-outbound line for a loopback address, and the proxy's refusal of\n  a loopback address each name the port.\n- app: BlockOffer::Port, GrantWhat::Port, the asks adapter, the Granted\n  list and Remove; a port the refusal did not name is typed.\n- Claude Code's sandbox takes no single loopback port, so its Notice says so\n  and offers Start without the sandbox (follow-up).\n- ADR 0067 section (to be confirmed), CONTEXT term, plane-format, changes.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* fix(sandbox): review of a service on this Mac (qw384)\n\n- One lock over purlis's own ports and the ports chats were given:\n  a start and a new listener each judge and mark in one step.\n- A forge sign-in's redirect ports are purlis's own.\n- Loopback is exactly 127.0.0.1, ::1 and localhost, as the profile and\n  the proxy carry it.\n- In a Codex or opencode chat a violation block is printed text, and\n  printed words never name a port: they pre-fill the Notice's port\n  field, said to come from the command's output, for the person to check.\n- A port you allowed that purlis holds after a restart has its own sentence.\n- The Granted list says where a port applies.\n- Docs say exactly when the port is named and when it is pre-filled.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* fix(qw384): the lane-writes guard knows granted_ports only reads\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* fix(qw384): a port purlis holds is freed before the service starts again\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* style(qw384): fmt\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T00:15:16+04:00",
          "tree_id": "7fed19141c21e04fb7150567667dc2a8e0eae9b0",
          "url": "https://github.com/purlis/purlis/commit/dd12dcfd6b51606a830d8e411dfeebd772843994"
        },
        "date": 1792009640645,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5083015,
            "unit": "ms",
            "extra": "median of 5 runs: 0.481, 0.506, 0.508, 0.517, 0.527 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.7396175,
            "unit": "ms",
            "extra": "median of 5 runs: 16.501, 16.597, 16.740, 16.997, 17.046 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.4704435,
            "unit": "ms",
            "extra": "median of 5 runs: 102.687, 103.572, 104.470, 105.893, 106.004 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "dbc9ce99421b092022848eae27fd11e1e5e9d8d8",
          "message": "fix(e2e): every failure's evidence block is recorded, and what covered an element is said (#2080)\n\n* fix(e2e): every failure's evidence block is recorded, and what covered an element is said\n\n- A test's or hook's failure records its block even when it is empty, said as\n  \"no command was late; the core logged nothing\", so a quiet core reads apart\n  from a block never recorded.\n- The first five failures that carry a block show it. The left-chats guard\n  writes a record of its own with none, which used up a place: in run\n  115925513874 it pushed the first failure of the next spec past the three.\n- A failure that could not press or see an element (\"still not clickable\",\n  \"still not displayed\") records what lay over it: what is at the element's\n  centre, what has the keyboard, and every dialog, menu, listbox or popper\n  open (app/e2e/covers.ts), which a dialog sweep alone does not see.\n\nRefs #1949\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw389b): the log has the room the late commands and covers leave; covers are checked for secrets\n\nA long core log no longer pushes the window's late line and the log's header out of the\nannotation: the late commands and the covers are kept whole and the log is cut, newest lines\nkept, to the room they leave. The covers are read from the page, which the core's redaction\nnever saw, so a line that looks like it holds a secret is replaced whole, as the log does.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T00:32:48+04:00",
          "tree_id": "faf2ef6ddd4a4137a98b4e3bbcc7d908598406c7",
          "url": "https://github.com/purlis/purlis/commit/dbc9ce99421b092022848eae27fd11e1e5e9d8d8"
        },
        "date": 1792010723310,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.30946700000000005,
            "unit": "ms",
            "extra": "median of 5 runs: 0.294, 0.295, 0.309, 0.313, 0.320 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.6951865,
            "unit": "ms",
            "extra": "median of 5 runs: 16.540, 16.597, 16.695, 16.729, 16.767 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.4577015,
            "unit": "ms",
            "extra": "median of 5 runs: 100.761, 101.442, 101.458, 101.495, 101.768 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "736ffff653e2f9b1c621a84d1d70d5c69ebe8f1a",
          "message": "feat(qw387c): a branch folder is cut from the default branch, kept current and pushed (#1983)\n\n* feat(qw387c): a branch folder is cut from the default branch, kept current and pushed\n\n`purlis worktree add` cuts from the branch the remote names its default\n(origin/HEAD), or `--base <branch>`, brought up to its remote-tracking\nbranch where that is a fast-forward nothing has checked out; `--base\nHEAD` keeps the clone's HEAD. The base stays recorded as before.\n\n`purlis worktree update [--base] [--continue | --abort]` fetches,\nbrings the base up, and rebases the folder onto it, stopping on a\nconflict with the files named; `--continue` stages exactly those files.\nFor a sandboxed chat, a move that would write a file its sandbox denies\ndoes not start. `purlis worktree push [--force-with-lease]` pushes the\nfolder's own branch after purlis's scan, with a lease only right after\nan update. In a chat the app runs both, for a chat standing in the\nfolder or above it, as it runs `purlis worktree commit`.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* fix(qw387c): a base checked out and behind its remote is built on as the remote has it\n\nThe clone usually has its default branch checked out, so `update` could\nnever bring it up and rebased onto the stale branch. Where the base is\nbehind its remote-tracking branch and checked out in a worktree, `add`\ncuts from and `update` rebases onto the remote-tracking branch, the\nbase stays recorded, and the note says Changes count the remote's newer\ncommits until the clone is brought up.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* docs(qw387c): a checked-out base behind its remote is built on as the remote has it\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw387c): an app-run cut is judged as the chat's own checkout would be\n\nFor a sandboxed chat the app fetches the base's remote before it cuts,\nso the remote-tracking branches it reads are its own fetch's, and a cut\nthat would write a file the chat's sandbox denies, where the clone has\nthat file otherwise or not at all, is refused, naming it.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw387c): a task's worktree cut on a chat's behalf is judged too\n\nA dispatch cuts a task's worktree from the asking chat's branch, or the\nclone's HEAD, which a chat can move; the cut is judged as that chat's\nown checkout would be, and a refusal names the file to the asking chat.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* fix(qw387c): a stopped update goes on or back on no branch, and a remote is judged as recorded\n\n`--continue` and `--abort` run in a folder a stopped rebase leaves on\nno branch. A remote's host is read from the address the clone records,\nnot from what a rewrite in git's config makes of it.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* fix(qw387c): a rebase is under way only while git's own folder for it is there\n\nA finished rebase can leave a marker behind; `update`, `--continue`,\n`--abort` and `push` read a rebase as stopped only while `rebase-merge`\nor `rebase-apply` is in the folder's git directory, and take git's own\n\"no rebase in progress\" as the answer.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw387c): app-run checkouts run on the commit ids they judged\n\nA cut, a dispatch's cut and an update read each commit once, judge the\nids and run git on them; the update moves the branch only from the\ncommit it judged, and puts the folder back on it. A lease is given only\non purlis's own record of its last update, kept in the folder's git\ndirectory. The remote's default branch is asked of the remote. A cut is\njudged by mode as well as content, and a remote by every address git\ndials.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw387c): an update is judged for the two ids the rebase is handed\n\nThe rebase's judgement reads what differs between the head and onto\nids it is given, and what the commits that head has and onto lacks\nchanged, never what HEAD names by then.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>\nCo-authored-by: Aaron Yordanyan <aharon@purlis.com>",
          "timestamp": "2026-10-15T00:46:09+04:00",
          "tree_id": "1aee1c83849c89599575e580141682a59b626257",
          "url": "https://github.com/purlis/purlis/commit/736ffff653e2f9b1c621a84d1d70d5c69ebe8f1a"
        },
        "date": 1792011990681,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.3135945,
            "unit": "ms",
            "extra": "median of 5 runs: 0.310, 0.312, 0.314, 0.316, 0.329 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.740211000000002,
            "unit": "ms",
            "extra": "median of 5 runs: 16.032, 16.718, 16.740, 16.760, 16.908 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.9146585,
            "unit": "ms",
            "extra": "median of 5 runs: 101.495, 101.756, 101.915, 101.955, 102.067 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "eyehollow@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple"
          },
          "committer": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "distinct": true,
          "id": "db3ea9746698e7bb577ecc6e7ae516cc627fd262",
          "message": "review(qw388b): a command is masked as written, and the ADR names its exception\n\nA command line is masked before its white space is folded, so a value\nholding white space or a line break is found. ADR 0090 names keeping a\ntask's masked command lines in memory as the operator-ruled exception to\nthe digest-only rule of ADR 0066 and ADR 0075, and marks the counts, the\nkeeping after close and the plugin's wait as to be confirmed.\n\nRefs #1981\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-15T01:20:12+04:00",
          "tree_id": "a94819d831c1e56a5f5dd8bda25613b61d2884a0",
          "url": "https://github.com/purlis/purlis/commit/db3ea9746698e7bb577ecc6e7ae516cc627fd262"
        },
        "date": 1792013026370,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.523918,
            "unit": "ms",
            "extra": "median of 5 runs: 0.506, 0.514, 0.524, 0.525, 0.543 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.7752335,
            "unit": "ms",
            "extra": "median of 5 runs: 16.381, 16.731, 16.775, 16.842, 16.984 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.2237385,
            "unit": "ms",
            "extra": "median of 5 runs: 102.862, 103.478, 104.224, 104.285, 104.973 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b724a0d3da5e865eb4747fe4738c03bf7d163ea2",
          "message": "A waiting chat's pane Notice can be dismissed (qw386) (#2090)\n\n* feat(sandbox): a service on this Mac is allowed by its one port (qw384)\n\nA sandboxed chat that reaches a service listening on this machine (kubectl\nagainst a local cluster at 127.0.0.1:<port>, a database in a container, a\ndev server) now gets a Block naming the port, and its Notice offers Allow\nfor this chat or for this project on this machine.\n\n- core: sandbox/localport.rs judges one port (1-65535, never purlis's own),\n  reads Go's and Node's refusal words, and binds purlis's own proxy and\n  tunnel ports away from any port a chat was started with.\n- grant::What::Port, kept for one chat in memory or in this machine's\n  sandbox file (ports_mine); policy's socket-grants lock holds it too.\n- a start compiles each allowed port into the chat's Seatbelt profile\n  (Codex, opencode) and its proxy's reach, as exactly 127.0.0.1, ::1 and\n  localhost at that port.\n- sandboxblock::Kind::LocalService: Go/Node words, the sandbox's own\n  network-outbound line for a loopback address, and the proxy's refusal of\n  a loopback address each name the port.\n- app: BlockOffer::Port, GrantWhat::Port, the asks adapter, the Granted\n  list and Remove; a port the refusal did not name is typed.\n- Claude Code's sandbox takes no single loopback port, so its Notice says so\n  and offers Start without the sandbox (follow-up).\n- ADR 0067 section (to be confirmed), CONTEXT term, plane-format, changes.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* fix(sandbox): review of a service on this Mac (qw384)\n\n- One lock over purlis's own ports and the ports chats were given:\n  a start and a new listener each judge and mark in one step.\n- A forge sign-in's redirect ports are purlis's own.\n- Loopback is exactly 127.0.0.1, ::1 and localhost, as the profile and\n  the proxy carry it.\n- In a Codex or opencode chat a violation block is printed text, and\n  printed words never name a port: they pre-fill the Notice's port\n  field, said to come from the command's output, for the person to check.\n- A port you allowed that purlis holds after a restart has its own sentence.\n- The Granted list says where a port applies.\n- Docs say exactly when the port is named and when it is pre-filled.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* fix(qw384): the lane-writes guard knows granted_ports only reads\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* fix(qw384): a port purlis holds is freed before the service starts again\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* style(qw384): fmt\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* feat(notices): a waiting chat's pane Notice can be dismissed (qw386)\n\nThe pane Notice that another chat waits on you for a permission, or is\nstopped on a prompt, gets the Notice's own Dismiss. It hides the line in\nmemory for this run and that wait only, answers nothing, and says so as\nits tooltip (new optional Notice prop dismissSays): the tab's hand and the\nneeds-you list still show the wait. A new prompt shows again by its own\ncause; the stopped line shows again at the chat's next wait.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* fix(notices): a dismissed wait is keyed by the wait, not by a pane being drawn (qw386)\n\nThe stopped line is keyed by the moment the core says the chat last moved,\nso a new wait is a new line whether or not the pane was drawn while the\nold one ended; no release effect is needed. A prompt put away while purlis\nheld it carries to the stopped line of the same wait within the pane that\nsaw the hold end.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T01:28:30+04:00",
          "tree_id": "488d67bc624a07da1519b4d0b359b3aa736de293",
          "url": "https://github.com/purlis/purlis/commit/b724a0d3da5e865eb4747fe4738c03bf7d163ea2"
        },
        "date": 1792013945520,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.545231,
            "unit": "ms",
            "extra": "median of 5 runs: 0.520, 0.532, 0.545, 0.565, 0.580 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.1192795,
            "unit": "ms",
            "extra": "median of 5 runs: 16.663, 16.664, 17.119, 17.181, 17.213 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.940361,
            "unit": "ms",
            "extra": "median of 5 runs: 101.896, 103.194, 104.940, 105.212, 105.387 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d8c63d31e34a7521e593a8a42b09872ef05bc8a0",
          "message": "test(qw392): fixtures are tracked under neutral names, so a sandboxed chat can always update a clone (#2094)\n\n* test(qw392): fixtures are tracked under neutral names, so a sandboxed chat can always update a clone\n\nA fixture whose name a sandboxed chat may never write (an agent's\n`.claude/settings.json`, `opencode.json`) is tracked as `<name>.fixture`\nand copied to its real name at test time (`purlis_core::testfixture`,\nand `fixtureNames` in the e2e harness and the benchmark). The editor\nrecommendation `app/.vscode/extensions.json` is no longer tracked. A test\nfails on any tracked path the sandbox denies a chat (#2089).\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw392): the guard judges a fresh checkout, and the bench copies anew\n\nThe guard test judges every tracked path as a fresh clone would write\nit, so a file under a folder a chat may not make counts whatever this\ncheckout already holds. `testfixture` sits apart from `testrun`'s note,\nand the hook benchmark removes its last copy before it copies again.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw392): the guard test's checkout answers every question the clone guard asks\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <aharon@purlis.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T01:42:53+04:00",
          "tree_id": "6d70e12598a4e8efde0d4f6e826f681641dd4e45",
          "url": "https://github.com/purlis/purlis/commit/d8c63d31e34a7521e593a8a42b09872ef05bc8a0"
        },
        "date": 1792014905329,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.390313,
            "unit": "ms",
            "extra": "median of 5 runs: 0.376, 0.382, 0.390, 0.395, 0.396 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.676585000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 16.538, 16.627, 16.677, 16.694, 16.737 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.978897,
            "unit": "ms",
            "extra": "median of 5 runs: 101.553, 101.631, 101.979, 102.011, 102.274 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "279dcfa723287464c747f6af22b61f625785a469",
          "message": "fix(qw387d): git's own variables count as configuration, and a selection says what landed (#2099)\n\n* fix(qw387d): git's own variables count as configuration, and a selection says what landed\n\n- Any `GIT_*` assignment or export on the line turns the clone guard's\n  sparse filter off, as `GIT_CONFIG*` and the editors did.\n- `select::set_active` answers what it wrote: a pointer the state folder\n  refused is `Scope::Unwritten`, and `workspace use` / `create --use`\n  fail saying nothing was selected.\n- A `cd` whose `&&` chain ends before a branch move in the plane root\n  gets the hint to join it.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw387d): the loose-cd hint stays out: it changes recorded plane-root answers\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <aharon@purlis.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T02:29:33+04:00",
          "tree_id": "76fca178735b60cc85c4c0a72530125140d9c6ea",
          "url": "https://github.com/purlis/purlis/commit/279dcfa723287464c747f6af22b61f625785a469"
        },
        "date": 1792017049603,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.521875,
            "unit": "ms",
            "extra": "median of 5 runs: 0.506, 0.514, 0.522, 0.532, 0.540 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.8970405,
            "unit": "ms",
            "extra": "median of 5 runs: 16.349, 16.877, 16.897, 17.016, 17.381 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.8536355,
            "unit": "ms",
            "extra": "median of 5 runs: 103.022, 104.603, 104.854, 105.707, 106.483 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c6f53a5c8bb0284765365655c5e3f646fb5ce147",
          "message": "state: a second prompt within one wait stamps the chat (#2100)\n\nWhere a chat already waits on a prompt and an ask changes the prompt or\nwho asked, the board counts it a move a reader sees, so the window takes\nit for a wait of its own: a line put away for the first prompt shows for\nthe second.\n\nCloses #2098\n\n\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T04:23:09+04:00",
          "tree_id": "7da4214010390a37410789d43c74f4a2ebfadf3d",
          "url": "https://github.com/purlis/purlis/commit/c6f53a5c8bb0284765365655c5e3f646fb5ce147"
        },
        "date": 1792023860282,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.51444,
            "unit": "ms",
            "extra": "median of 5 runs: 0.504, 0.508, 0.514, 0.553, 0.564 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.926733999999996,
            "unit": "ms",
            "extra": "median of 5 runs: 16.778, 16.875, 16.927, 16.985, 17.275 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.474705,
            "unit": "ms",
            "extra": "median of 5 runs: 101.190, 104.149, 105.475, 105.721, 105.931 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "93945919aa909c441422e02389ad1272bf586fad",
          "message": "dispatch: reading a task is a tool too, and asks nobody (#2101)\n\nThe dispatch_read chat tool reads what a task the chat dispatched said\nlast and ran, as the read command does. Both join the pre-allowed reads\n(ADR 0064, amended), and a helper is still refused either. The\ndispatch_report description says what follows a turn without a report.\nCONTEXT.md names reading a task and a quiet note.\n\nRefs #2084\n\n\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T04:23:16+04:00",
          "tree_id": "a5127c65246a4baf367612bb66a894a5d340d89b",
          "url": "https://github.com/purlis/purlis/commit/93945919aa909c441422e02389ad1272bf586fad"
        },
        "date": 1792024886836,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.27481300000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 0.259, 0.267, 0.275, 0.281, 0.282 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.531349499999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.385, 16.401, 16.531, 16.535, 16.723 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.6400705,
            "unit": "ms",
            "extra": "median of 5 runs: 101.186, 101.206, 101.640, 101.720, 101.912 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "05264a28fe7a205b4bf7a1d2131604966862c8bd",
          "message": "A Claude Code chat is allowed a service on this Mac, reached by name (qw384b) (#2097)\n\n* fix(qw384b): a chat's proxy carries a granted port by name, and a chat's kubeconfig names it so\n\nThe reach table carries this-mac.internal:P, and a name that resolves to this machine's\nloopback alone (kubernetes.docker.internal), to 127.0.0.1:P while the chat holds P, at the\nlayer that granted it; on any other port it is this machine, refused and never asked about.\nNames are resolved only for a chat that holds a port. A port withdrawn while the chat runs is\ncarried by no copy of its reach any more.\n\nkubeconfig copies one context for one chat: only a cluster on this Mac (127.0.0.1, localhost,\nkubernetes.docker.internal), its server named https://this-mac.internal:P with\ntls-server-name the name it had; a context elsewhere, with a proxy of its own or skipping the\ncertificate check is refused with why.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* fix(qw384b): a Claude Code chat is allowed a service on this Mac, reached by name\n\nA Claude Code chat's port Block now offers the same Allow as Codex and opencode. Its commands\nreach the port through the chat's proxy as this-mac.internal:<port> (or a name resolving to\nthis machine's loopback, such as kubernetes.docker.internal), only while the chat holds it, and\nthe chat is told the name when it restarts. Removing a port's grant withdraws it from every open\nchat's proxy at once, but one that holds it for itself.\n\nPoint kubectl at it, on the Notice once the port is allowed, lists the person's kube contexts\n(kubectl config view) and copies one whose cluster is on this Mac for that chat alone: server\nhttps://this-mac.internal:<port>, tls-server-name the name it had, written 0600 into the chat's\nown folder at each start and named by KUBECONFIG for it alone. Any other context is listed with\nwhy; a context with a proxy of its own or no certificate check is refused. ~/.kube is never\nwritten.\n\nA refusal of this-mac.internal:<port>, or of a name resolving to loopback, is a Block naming the\nport. ADR 0067 amendment, CONTEXT term and changes fragment.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* fix(qw384b): Point kubectl's commands are the window's alone, and its tests arm the sandbox\n\nkube_contexts reads the person's kubeconfig and point_kubectl widens what a chat reaches, so\nneither is served on a link. The tests that start a sandboxed chat arm it as the app ships an\nopencode chat. A refusal's words name no command that is not one.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* style(qw384b): the withdraw test keeps its pretend host unnamed\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* test(qw384b): the Claude Code port route is compared whole, never printed\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw384b): names of this Mac are a fixed list, pinned to loopback; the copy says it holds credentials\n\nThe proxy carries a granted port by this-mac.internal, kubernetes.docker.internal and\nhost.docker.internal alone, never looked up: Reach::decide_by_name decides them, and connect\nis made to this machine's loopback on that port and nowhere else (egress::addresses). No other\nname is looked up on this account, in the decision or in the Block's port.\n\nBefore a context is chosen, Point kubectl at it says the chat gets its own copy of it,\ncredentials included, which the chat can read. The copy is made new each time, never through\na link. Removing a port stops new connections at once; one already open ends when the chat\nrestarts, as the ADR and the changes fragment now say.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T04:52:21+04:00",
          "tree_id": "8ef35adbe725947de58c15299b789344bc5e72e5",
          "url": "https://github.com/purlis/purlis/commit/05264a28fe7a205b4bf7a1d2131604966862c8bd"
        },
        "date": 1792025849709,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.46423800000000004,
            "unit": "ms",
            "extra": "median of 5 runs: 0.460, 0.460, 0.464, 0.467, 0.475 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.388755000000003,
            "unit": "ms",
            "extra": "median of 5 runs: 16.214, 16.291, 16.389, 16.491, 16.519 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.421685,
            "unit": "ms",
            "extra": "median of 5 runs: 100.439, 100.746, 101.422, 101.777, 101.852 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "6232deedaee8d83e76357dc93bb20b943c62895f",
          "message": "A proposed change, part A: core and CLI (#1975) (#2102)\n\n* feat(sandbox): a proposed change, core and CLI (#1975, part A)\n\nA chat's edit of purlis's own configuration becomes a proposal the person\napplies, per the operator's rulings PC-1..PC-14.\n\n- proposed: which files may be proposed (the manifest at the root and in\n  each workspace, a persona's mcp.json and persona.md, .claude/skills files;\n  plane only, no link on the way), the change worked out exactly as the\n  edit tool would make it, and the base it was worked out against.\n- hookwire: Ask::Propose carries the file, the base and the whole content.\n- pretooluse-edit: in a sandboxed Claude Code chat, a proposable edit is\n  handed to the app and the tool refused with fixed words; where the app\n  does not take it, the edit is refused as before.\n- purlis propose <file> --from <draft>: the same, for other harnesses; the\n  draft must be in the chat's own folder.\n- policy: sandbox.proposed-changes = \"off\" turns it off (PC-14).\n- The app answers a proposal as not held yet, so nothing changes until\n  part B shows it to the person.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* fix(sandbox): a proposed change reads only the file its name says (#1975, part A review)\n\n- The file is read only as one regular file with one name, each folder and\n  the file opened without following a link and without waiting on it.\n- An edit that cannot be made a proposal is said in fixed words.\n- Below .claude/skills, no dot-part, package manifest or hooks folder.\n- The size cap is what one line of the hook channel carries whole; a\n  longer proposal is not made, never cut.\n- What Claude Code would refuse is refused: an edit that changes nothing,\n  no edits, a MultiEdit one of whose edits fails.\n- changes/ fragment for sandbox.proposed-changes.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T04:56:32+04:00",
          "tree_id": "4403dc7f68d997b9632de255166e24098ce7aa46",
          "url": "https://github.com/purlis/purlis/commit/6232deedaee8d83e76357dc93bb20b943c62895f"
        },
        "date": 1792026863917,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.296716,
            "unit": "ms",
            "extra": "median of 5 runs: 0.295, 0.295, 0.297, 0.302, 0.311 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.5783445,
            "unit": "ms",
            "extra": "median of 5 runs: 16.523, 16.560, 16.578, 16.682, 16.742 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.442834,
            "unit": "ms",
            "extra": "median of 5 runs: 100.473, 100.795, 101.443, 101.732, 102.085 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9850f295c4edeade2724c9e9d43c93aa4ad6669d",
          "message": "fix: a removal says when the selection could not move; one long log line never cuts the late commands (#2103) (#2104)\n\n* fix: a removal says when the selection could not move; one long log line never cuts the late commands\n\nAfter a workspace removal, a selection pointer that could not be written\nis said as that, with a failing exit, as workspace use does; never as a\nreset (#2103). In the e2e annotations, a single core log line longer than\nthe room left after the late commands and covers is cut to that room, so\nthe final cut never reaches the late commands.\n\nCloses #2103\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* docs(select): each function keeps its own doc\n\nRefs #2103\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T05:15:48+04:00",
          "tree_id": "a667bd2f141c5746298d78d0d02a98d2e89c998c",
          "url": "https://github.com/purlis/purlis/commit/9850f295c4edeade2724c9e9d43c93aa4ad6669d"
        },
        "date": 1792027918632,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4434795,
            "unit": "ms",
            "extra": "median of 5 runs: 0.429, 0.438, 0.443, 0.451, 0.458 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.442633,
            "unit": "ms",
            "extra": "median of 5 runs: 16.422, 16.431, 16.443, 16.444, 16.728 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.71432150000001,
            "unit": "ms",
            "extra": "median of 5 runs: 101.599, 101.670, 101.714, 101.939, 102.143 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2118b1bf7931db172538c79713a954370276f1e3",
          "message": "feat(sandbox): your own level per project and persona, and moving plain-file vaults into the keychain (#2071, PR B) (#2105)\n\n* feat(sandbox): your own level per project and persona, and a move of plain-file vaults into the keychain\n\nSettings › Project › Sandbox › Your level sets on, floor or off for the project and for each\npersona, in this machine's purlis.local.toml ([sandbox] level, [sandbox.personas]); the save\nand the reader refuse a value that is not a level. A local file git would carry turns nobody's\nsandbox down. The same page lists the project's plain-file vaults and moves each one this\nmachine registers into the keychain, each secret written and read back before the file goes.\n\nA chat's start reads the level once the floor (#2081) lands. Part of #2071 (PR B).\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* test(palette): the Your level page is one more row\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(#2105): a level counts only as far as you confirmed it, and a vault moves before it is switched\n\n- The vault move builds the keychain vault first, writes and reads back every secret, and only\n  then switches the registration and removes the file and its rotation record, all under the\n  vault's keys' lock. A move that stops deletes what it wrote and its index and says anything\n  it could not take back. An old same-named keychain index is removed when empty and refused\n  when it lists items. Rollback is tested with a store that fails a write, a read-back and a\n  delete.\n- A chat can write purlis.local.toml, so a level it holds counts only where it is the one you\n  set or confirmed in Settings on this machine (levels_mine, in the config home), or stricter.\n  A lower one waits on the page with Confirm. Picking a level confirms it; a raw save does not.\n- personal.rs says so; the unused writer is gone; plane-format names purlis.local.toml.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(#2105): a Confirm carries the level it showed, and a move lists each item before writing it\n\n- confirm_sandbox_level takes the level its row showed and is refused where the file says\n  another one, or none, by then: nothing written between the showing and the click is\n  confirmed in its place.\n- The move lists each key in the keychain index before writing its item, so a move that dies\n  mid-way leaves no item nothing names; the team-vault check runs under the keys' lock.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <aharon@purlis.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T06:27:46+04:00",
          "tree_id": "769b9637694fa331042beb63efa2f47fb06e6fab",
          "url": "https://github.com/purlis/purlis/commit/2118b1bf7931db172538c79713a954370276f1e3"
        },
        "date": 1792031336001,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.523207,
            "unit": "ms",
            "extra": "median of 5 runs: 0.511, 0.512, 0.523, 0.542, 0.545 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.710880999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.246, 16.436, 16.711, 16.884, 16.895 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.11769699999999,
            "unit": "ms",
            "extra": "median of 5 runs: 101.583, 103.485, 104.118, 104.731, 105.767 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2ecd685fab1508c2f200d9c06e059f969fc1536e",
          "message": "A proposed change, part B: the person applies it in the window (#1975) (#2106)\n\n* feat(sandbox): a proposed change, core and CLI (#1975, part A)\n\nA chat's edit of purlis's own configuration becomes a proposal the person\napplies, per the operator's rulings PC-1..PC-14.\n\n- proposed: which files may be proposed (the manifest at the root and in\n  each workspace, a persona's mcp.json and persona.md, .claude/skills files;\n  plane only, no link on the way), the change worked out exactly as the\n  edit tool would make it, and the base it was worked out against.\n- hookwire: Ask::Propose carries the file, the base and the whole content.\n- pretooluse-edit: in a sandboxed Claude Code chat, a proposable edit is\n  handed to the app and the tool refused with fixed words; where the app\n  does not take it, the edit is refused as before.\n- purlis propose <file> --from <draft>: the same, for other harnesses; the\n  draft must be in the chat's own folder.\n- policy: sandbox.proposed-changes = \"off\" turns it off (PC-14).\n- The app answers a proposal as not held yet, so nothing changes until\n  part B shows it to the person.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* fix(sandbox): a proposed change reads only the file its name says (#1975, part A review)\n\n- The file is read only as one regular file with one name, each folder and\n  the file opened without following a link and without waiting on it.\n- An edit that cannot be made a proposal is said in fixed words.\n- Below .claude/skills, no dot-part, package manifest or hooks folder.\n- The size cap is what one line of the hook channel carries whole; a\n  longer proposal is not made, never cut.\n- What Claude Code would refuse is refused: an edit that changes nothing,\n  no edits, a MultiEdit one of whose edits fails.\n- changes/ fragment for sandbox.proposed-changes.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* feat(proposed): the person applies a chat's proposed change in the window (#1975, part B)\n\nA proposal carries the change, not the file: an edit's own replacements,\nor a whole new content, with the hash of the file as it was read. The app\nreads the file itself (one regular file, one name, no link on the way),\nmakes the change, shows it, and on Apply writes exactly that as the\nperson, only while the file is still what it was, audited first\n(trust.proposed.apply).\n\n- proposals (app): one a file a chat, both chats' held, applying one makes\n  the others stale; stale offers Dismiss alone; a chat's proposals end\n  with it; the chat hears applied, dismissed or stale on its next turn.\n- asks registry: a proposed-change source with the change shown whole,\n  Apply only beside a change shown whole and still applying, and Dismiss;\n  a pane Notice on the chat's tab; Apply is in the sandbox lane.\n- Window-only commands: apply_proposed_change, dismiss_proposed_change.\n- Docs: CONTEXT term, ADR 0067 section citing PC-1..PC-14, the event\n  kind, a changes/ fragment.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* test(proposed): the Notice's hunks as the bindings name them\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* fix(proposed): Dismiss is answered off the window's thread\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* fix(proposed): Apply judges again, records its outcome and holds the project's lock (#1975, part B review)\n\n- Apply makes the change again and writes only what the row's own rules\n  show whole; any other is refused, whatever asked.\n- The audit is an intent before the write and the outcome after it:\n  applied, stale, or failed with why.\n- One lock per project from the read of the file to its rename: of two\n  Applies against the file as it was, the second is stale.\n- The new file beside the one written has a name drawn at random, made\n  new and never followed; one left behind never blocks.\n- A proposal makes no folder but a skill's: no new persona or workspace\n  (decision, to be confirmed in ADR 0067).\n- The Inbox's wait for the change holds Apply as it holds an edit's\n  Allow (tested).\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* fix(proposed): the no-new-persona sentence names no command\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T07:11:02+04:00",
          "tree_id": "87d834c2151161850cf99069d458e71e4cc3dd5b",
          "url": "https://github.com/purlis/purlis/commit/2ecd685fab1508c2f200d9c06e059f969fc1536e"
        },
        "date": 1792033931170,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5241115000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.496, 0.507, 0.524, 0.534, 0.552 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.066692,
            "unit": "ms",
            "extra": "median of 5 runs: 16.634, 16.673, 17.067, 17.120, 17.131 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.56111849999999,
            "unit": "ms",
            "extra": "median of 5 runs: 101.940, 103.377, 103.561, 103.833, 104.267 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "99247378ab95150861a4e8966b8e7cf9c89aac56",
          "message": "A folder or socket grant restarts the chat at its next pause, saying what will end (#2079)\n\n* A restart owed for a grant waits for the chat's next pause, and an ended chat restarts for nothing\n\nThe window's restart driver decides through one pure scheduler (restartAtPause.ts):\na chat between turns restarts, one mid-turn waits, one whose harness says nothing of\nits turns waits for the person, and one whose program ended is owed nothing. Restart\nnow restarts at once unless the program has ended. Two grants owe one restart.\n\nRefs #1986\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* A folder's or a socket's Allow says first what the restart will end\n\nThe Ask that restarts the chat says it does so at its next pause between turns, and\nwhat will stop: each program below the chat listening on a TCP port, by name and port,\nand how many others its commands started, read from ps and lsof by the core\n(purlis_core::restartends, what_a_restart_ends). Where purlis cannot read either it\nsays it cannot tell. A chat whose program ended is owed no restart in the core either.\nADR 0067 amended for ruling Q63; changelog fragment added.\n\nCloses #1986\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(1986): the Inbox and a tasks' question say what a restart ends, and only an exit drops it\n\n- The Inbox's folder or service row says the restart's sentence, naming the chat; a\n  folder's question for several tasks says it for each task listed.\n- A chat between turns as the Ask is drawn is said to restart now.\n- The window drops an owed restart for a chat reading ended only once the core, which\n  hears the program exit, owes it nothing: a SessionEnd hook alone drops nothing.\n- ps and lsof (-b -w) run within a two-second deadline, their output read while they\n  run; on timeout the Ask says purlis cannot tell. What lsof printed is read even when\n  it exits 1.\n- quiet() is no_env().\n\nCloses #2086\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T08:32:14+04:00",
          "tree_id": "cf7dd6ba17b2154c6ab3c48298a4b2231d560051",
          "url": "https://github.com/purlis/purlis/commit/99247378ab95150861a4e8966b8e7cf9c89aac56"
        },
        "date": 1792038802092,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.26952699999999996,
            "unit": "ms",
            "extra": "median of 5 runs: 0.269, 0.269, 0.270, 0.273, 0.280 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.4576305,
            "unit": "ms",
            "extra": "median of 5 runs: 16.425, 16.442, 16.458, 16.481, 16.486 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.3089315,
            "unit": "ms",
            "extra": "median of 5 runs: 100.407, 100.593, 101.309, 101.446, 101.642 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "32e43eb7be3f74d3e9cb42f6e0789a97cd21c06c",
          "message": "Every Ask's outcome reaches the chat as one plain line (#2082)\n\n* Every Ask's answer reaches the chat as one plain line\n\nThe person's answer to an Ask a chat's refusal raised (a host, a folder, a\nlocal socket or service, a vault) is told to that chat in one line written\nin one place, purlis_core::askoutcome::Outcome::line:\n\"Allowed for this chat — retry\" with the scope chosen, or \"Kept blocked by\nthe person — don't retry; <their reason, quoted as data>\" / \"no reason\ngiven\". A restart's first message, the next turn's word\n(Chats::tell_outcome) and a vault's typed line all take their text from it.\nKeep blocked now tells the chat for every kind, and the Inbox's row of a\nsandbox ask takes an optional reason.\n\nCloses #1987\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(#1987): a Keep says don't work around it, an answered block is a quiet no-op, Dispatch to tells nothing\n\n- The Keep line says \"don't retry or work around it\" (D-1987-8).\n- A live Allow at once is said as \"once, for this run of this chat\".\n- A Keep of a block no longer held does nothing and tells nothing, on the\n  Notice and in the Inbox alike; the Notice keeps every kind by one path.\n- Dispatch to {persona}… on a refused vault puts its Notice away with the\n  new put_away_refused_vault and tells the chat nothing.\n- Table rows for hostile reasons; ADR 0067's amendment splits the ruling\n  from what was decided in implementation.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* A service on this Mac allowed by its port is told in the same one line\n\nThe local-service block kind (qw384) is an Ask whose answer reaches the\nchat in Outcome::line too: About::Port and its GrantWhat arm, with\n\"connecting to the service on this Mac at port `N`\", and rows in both\ntable tests.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T08:55:45+04:00",
          "tree_id": "d5b0145bc129f37859f7c257e02070866c87c0ba",
          "url": "https://github.com/purlis/purlis/commit/32e43eb7be3f74d3e9cb42f6e0789a97cd21c06c"
        },
        "date": 1792040226607,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.408649,
            "unit": "ms",
            "extra": "median of 5 runs: 0.407, 0.409, 0.409, 0.412, 0.414 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.2813925,
            "unit": "ms",
            "extra": "median of 5 runs: 16.192, 16.225, 16.281, 16.306, 16.422 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.663262,
            "unit": "ms",
            "extra": "median of 5 runs: 101.234, 101.364, 101.663, 101.824, 102.358 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b7a2f169fb4734620feb840983ed3a6126695c62",
          "message": "feat(proposed): a long proposed change opens in its own tab (#1975, part C) (#2116)\n\nPC-7: a change too long for a row offers Open, which opens a view tab with\nthe whole change in the comparison view and Apply there.\n\n- proposed_change (window-only) reads a proposal whole for its tab: the\n  file before and after and the hunk, or why it is not shown.\n- Apply says where it was pressed (row or tab) and is judged again by that\n  place's rules: a row applies only what it shows whole; the tab also one\n  too long for a row; a credential-shaped or otherwise unshowable change is\n  applied from nowhere.\n- The asks registry offers Open beside such a change; the window opens the\n  tab on it, and Apply there counts only once the change is drawn.\n\n\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T09:00:56+04:00",
          "tree_id": "98009ea7b40689c60a39c4a7b296f767fdfcdb99",
          "url": "https://github.com/purlis/purlis/commit/b7a2f169fb4734620feb840983ed3a6126695c62"
        },
        "date": 1792041357834,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5252485,
            "unit": "ms",
            "extra": "median of 5 runs: 0.518, 0.522, 0.525, 0.525, 0.535 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.112251,
            "unit": "ms",
            "extra": "median of 5 runs: 16.441, 17.071, 17.112, 17.178, 17.181 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.4479375,
            "unit": "ms",
            "extra": "median of 5 runs: 103.013, 104.627, 105.448, 105.758, 105.838 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "5c6100b2aeb3936cb5eb6f4eb70c7c6ab2fb30e0",
          "message": "A stale workspace repo is a Notice with a one-click Fix (#1985, part 3) (#2095)\n\n* A stale workspace repo is a Notice with a one-click Fix (#1985, part 3)\n\nA workspace's repo left on a branch purlis cut for a chat or a task, not\non its default branch, or on a remote the forge said moved, is one Notice\nin the Inbox. Its Fix switches the repo back only when nothing would be\nlost (clean tree, every commit kept elsewhere, no chat mid-turn in it), or\npoints origin at the forge's new address; otherwise it says what blocks\nit and does nothing. purlis's own fetches from origin keep the forge's\nredirect for it.\n\nRefs #1985\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Stale repo review round: any other branch, the shown address, mid-turn (#1985)\n\n- Any branch of the repo itself other than its default counts; the\n  Notice says \"is on x, not main\" with what it means now.\n- The remote fix sets only the address the Notice shows, rebuilt from\n  checked parts on origin's own host and port.\n- The branch fix waits for a chat in the repo that is running, asking,\n  stopped on a prompt or not yet heard from.\n- A dismissal is keyed by the repo and each finding.\n- What could not be read is said, not dropped.\n\nRefs #1985\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T09:19:03+04:00",
          "tree_id": "b4fa927af6baf00bc2e18eec4767c138111abe8a",
          "url": "https://github.com/purlis/purlis/commit/5c6100b2aeb3936cb5eb6f4eb70c7c6ab2fb30e0"
        },
        "date": 1792042460675,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.2760825,
            "unit": "ms",
            "extra": "median of 5 runs: 0.268, 0.274, 0.276, 0.290, 0.296 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.5845685,
            "unit": "ms",
            "extra": "median of 5 runs: 16.408, 16.524, 16.585, 16.664, 16.709 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.5737005,
            "unit": "ms",
            "extra": "median of 5 runs: 101.078, 101.297, 101.574, 101.778, 101.983 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "fc3f42f04c2ffe1f5c581bf295af240eff8a5e5a",
          "message": "docs(adr): the items the operator ruled on 2026-10-15 say so (#2120)\n\n* docs(adr): the items the operator ruled on 2026-10-15 say so\n\nEach \"to be confirmed\" marker for an item the operator confirmed on\n2026-10-15 now says ruled by the operator, 2026-10-15, in ADRs 0051,\n0061, 0064 and 0067. Other open markers are left as they are.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw398): the 0051 note says only that it is ruled; one bullet re-wrapped\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T09:47:58+04:00",
          "tree_id": "6ea85c7319e3a882cb6dbf523a410361f45549b4",
          "url": "https://github.com/purlis/purlis/commit/fc3f42f04c2ffe1f5c581bf295af240eff8a5e5a"
        },
        "date": 1792043594293,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.544562,
            "unit": "ms",
            "extra": "median of 5 runs: 0.507, 0.514, 0.545, 0.547, 0.553 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.9350795,
            "unit": "ms",
            "extra": "median of 5 runs: 16.678, 16.804, 16.935, 17.335, 17.475 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.3048465,
            "unit": "ms",
            "extra": "median of 5 runs: 101.625, 103.573, 104.305, 105.352, 106.288 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "6053c4fca74a1f8bd44843020f1af7ca10bfb3d1",
          "message": "dispatch: a chat closes its own finished task (#1981 D-4) (#2119)\n\n* dispatch: a chat closes its own finished task (#1981 D-4)\n\nRuled 2026-10-15: #1485 stands. A finished task stays a readable row with\nReopen; the asking chat may close its own with the close command, which\nends a task kept open after its report and clears its row. Refused for a\ntask that has not reported, one in a turn, one being stopped, and one the\nperson typed in.\n\nCloses #1981\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw397): close holds the report lock and the #1485 holds, and its docs sit on it\n\nThe close reads every refusal again under the lock a report is taken\nunder and ends the chat in that hold, as a cancel does; it is refused\nwhile the person looks at the task or a task of its own is at work, the\nholds the #1485 end keeps. Doc comments are back on their own functions.\nTests cover each refusal and that close is never pre-allowed.\n\nRefs #1981\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T10:07:00+04:00",
          "tree_id": "77a061c85f580df91b9d8f399f12a1f562687a8e",
          "url": "https://github.com/purlis/purlis/commit/6053c4fca74a1f8bd44843020f1af7ca10bfb3d1"
        },
        "date": 1792044698120,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.4747405,
            "unit": "ms",
            "extra": "median of 5 runs: 0.459, 0.469, 0.475, 0.476, 0.492 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.357892500000002,
            "unit": "ms",
            "extra": "median of 5 runs: 16.285, 16.348, 16.358, 16.361, 16.366 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.816294,
            "unit": "ms",
            "extra": "median of 5 runs: 101.062, 101.633, 101.816, 101.907, 102.110 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7bf7311762ab64cd3ef983b4f9990ca92e38acc2",
          "message": "feat(sandbox): the floor is the default for a project that has not turned the sandbox on (#2081)\n\n* feat(sandbox): the floor is the default for a project that has not turned the sandbox on\n\nat the floor: no network limits, no folder limits, no Allow asks, but the\ndenial classes stay (vaults, purlis's keys, what a later program runs),\nthe folders on the person's PATH and the harness's own program are not\nwritten, and a brokered secret exec still runs only declared tools.\n\n- Claude Code: its sandbox with denyRead/denyWrite, every folder writable\n  but those, no allowed domains, no proxy, local binding on, web tools.\n- Codex and opencode on macOS: purlis's own profile from (allow default)\n  with only the floor's denials.\n- Where the floor cannot be applied, the chat starts as before and its\n  tab says why. `mode = \"on\"` keeps the full sandbox, unchanged.\n- Each chat's tab shows its level.\n\nRefs #2071\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw391a): the floor holds what reaches outside it, at every level\n\n- Codex and opencode at the floor: no Apple Event, no Launch Services, no\n  program that hands a job to launchd, cron or at; no unix socket but the\n  hook socket and granted ones, whose folder is never written; no port of\n  this machine but the chat's own proxy's.\n- Every harness: the later-code names at every depth and at the person's\n  own level (git's and ssh's config, launch agents, shell startup files,\n  each harness's and editor's user settings and hooks).\n- The network goes through the chat's own proxy, whose reach carries every\n  host and asks about none; Claude Code lists no domains, local binding off.\n- The PATH hold follows each link to its package, and a harness's program\n  is held with its whole package; a held folder's block offers Allow.\n- The certificate check as the project and policy say; the start check\n  ignores temp folders at the floor; a floorless start always says why, on\n  every start path.\n- ADR 0067, SECURITY.md and CONTEXT.md say what the floor is.\n\nRefs #2071\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw391a): the floor holds launchd jobs by operation, names in either case, and package prefixes\n\n- Codex and opencode at the floor: no job created in launchd, whatever\n  program asks.\n- The later-code names are held in either letter case.\n- Homebrew's whole prefix, npm's global packages and the person's own\n  Python site folders are held, each grantable from a block's Allow.\n- ssh: its config, config.d, authorized_keys, rc and environment and the\n  folder itself are held; known_hosts stays writable.\n- ADR 0067 and SECURITY.md say exactly what the floor holds, and what it\n  does not.\n\nRefs #2071\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw391a): a service on this Mac a person allowed is reached at the floor too\n\nAfter the rebase onto #1966: the port grants reach a floor chat as they\nreach one at the full sandbox, through its proxy and in the wrap.\n\nRefs #2071\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw391a): a program the floor cannot be bound to starts as before and says why\n\nAt the floor, a profile whose program does not answer as its harness, or\nnot in time, starts with no sandbox and its tab says why (#2071, Q2), on\nthe profile and window start paths and in the picker. A program a chat\ncould have written is refused at every level.\n\nRefs #2071\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw391a): a program that does not answer as its harness is refused at the floor, not started bare\n\nReverts the floorless start of a program that does not answer `--version`. At the floor, as at\nthe full sandbox, a Claude Code profile's program that answers as something else, or not at\nall, is refused with a sentence naming the program, what it answered and how to fix it. One\nthat does not answer in time is asked once more with twice the wait before it is refused. Only\na system or backend where the floor cannot be applied at all starts a chat without it, and its\ntab now says only the reason.\n\nThe test stand-ins answer `--version` as Claude Code does (`stand_in::claude_code`, the\nscenario tests' profile program) and live beside their project, not in it.\n\nRefs #2071\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw391a): the floor reads the PATH once per burst, and its tab mark leaves the name alone\n\n- The floor's machine-wide hold (package prefixes, Python site folders, PATH folders and where\n  their links lead) is read once and kept five seconds per PATH and home, and each project and\n  grant is filtered from it cheaply: every open chat's compile, as the window asks whether any\n  runs on an older sandbox, no longer follows thousands of links each.\n- The sandbox mark is named by `aria-label`, not by words in the tab, so the tab's text stays\n  the chat's name; and in a tab squeezed to its floor it gives way before the name, taking no\n  gap of its own.\n- Test builds ship a stand-in purlis binary (`Shipped::default` under `cfg(test)`), as every\n  real build does: a Claude Code chat now starts at the floor in every project, and a sandboxed\n  chat is refused where the app cannot arm its harness.\n\nRefs #2071\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw391a): test builds ship a stand-in plugin too; the raw-prompt scenario says where its chat went\n\nA Claude Code chat is armed only with purlis's plugin, so the test builds' stand-in Shipped\ncarries an empty plugin folder beside the stand-in binary. The explorer's raw-prompt wait now\nsays, on a red run, the window's pane frames and terminals, the tab in front, and the core's\nview of the chat.\n\nRefs #2071\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw391a): the floor confines a chat to no folder, and says why it could not be applied on the tab's mark\n\n- A floor chat with no folder of its own, one outside the project or one reached through a link\n  is not refused for it: the floor confines a chat to no folder. The denials of the folder it\n  stands in hold as at every level; the full sandbox still refuses all three.\n- Where the floor cannot be applied (Q2), the reason travels with the chat (`Ready::floorless`,\n  `OpenChat.sandbox_why`) and its tab's sandbox mark says it (\"no sandbox: <why>\"), instead of a\n  Notice in every such chat's pane.\n- The one-time sandbox offer says the project runs its chats at the floor, not without the\n  sandbox.\n\nRefs #2071\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw391a): every test's stand-in Claude Code answers as one and lives beside its project\n\nThe floor starts every harness chat sandboxed, so the remaining fixtures meet its program check:\n- the relaunch scenario's `claude`, the core's integration tests (profiles, resumed records,\n  worktree AGENTS.md, a harness found as a shell finds it, declared harnesses) and the app's\n  pretend-host chats now run a stand-in that answers `--version` as Claude Code does, outside\n  the project;\n- the handoff tests about a chat with no sandbox start it without the sandbox, as a person\n  would, since a plain project's chats now run at the floor;\n- the doctor's \"app that ships nothing\" test names a Shipped with nothing in it;\n- what later code loads at the person's own level (launch agents, git's user config) is never\n  granted, so its block offers the way that works, and a folder that holds an editor's user\n  settings is the one listed with what it holds.\n\nRefs #2071\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw391a): the not-Claude-Code refusal test reads the new sentence; a person's opt-out in the handoff tests is recorded\n\n- The profile test whose program is not Claude Code keeps its non-answering stand-in and reads\n  the refusal as it now says it: the program, and how to fix it.\n- The handoff tests that start their asking chat without the sandbox keep an event log, as the\n  app does: a person's opt-out starts a chat only once it is recorded.\n\nRefs #2071\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw391a): the floorless sentence for a system with no backend says only why\n\nIt said \"yet\", which the deferral check reads as a promise with no ticket; and it now reads as\nevery other floorless sentence does: no sandbox, because purlis has no backend here.\n\nRefs #2071\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw391a): the floor holds every project's purlis files by name, and no project's vaults are read\n\n- At the floor, every project's state folder (each spelling) and manifests (each name) are held\n  from being written wherever they are, in either case: Seatbelt by regex, Claude Code by glob\n  for its commands and its own tools. The full sandbox writes none of these in the chat's own\n  project, so nothing is given back; Claude Code's held names cannot be lifted inside, so its\n  own project's nested manifests are held too (D-391-8).\n- At every level, the vault files of every project are unreadable.\n- `~/.local/lib` is held whole, so a Python folder made after the chat started is held too.\n- The live floor test has its controls: the copied launchctl's submit goes through under the\n  same profile less `(deny job-creation)`, and a name beside the case-variant one is written.\n- Tests: another project's approvals, manifests and vaults at the floor; the rules rendered at\n  both levels, for Codex and Claude Code; a full-sandbox Codex chat reads no other project's\n  vault.\n\nRefs #2071\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw391a): Claude Code's held names match in either case, and macOS's matching is measured\n\n- Every name purlis holds by glob in Claude Code's sandbox (the later-code names, every\n  project's state folder and manifests, every project's vaults) is written with a class of both\n  cases for each letter. Claude Code 2.1.300 keeps a `[...]` class when it turns a glob into its\n  profile's regex, and reads a path holding one as a glob. Its own tools' permission rules carry\n  the name as written and the cased form.\n- `tools/seatbelt-case.sh`, a step of the macOS scenario job: whether macOS's sandbox matches a\n  name as typed or as it is on disk, measured with the rule as Claude Code builds it from a\n  plain glob and reported as a notice, with controls; and the cased rule asserted to hold\n  `.PURLIS/VAULTS` and `library/launchagents`.\n\nRefs #2071, #2107\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* docs(qw391a): macOS's sandbox matches a name as it is on disk, measured\n\nThe macOS scenario job's seatbelt-case step refused a read through .PURLIS/VAULTS and a write\nthrough library/launchagents under rules spelled .purlis/vaults and Library/LaunchAgents, with\nits controls passing. Recorded in ADR 0067; the case classes stay.\n\nRefs #2071, #2107\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw391a): the case measurement has a no-rule control and covers new names; Claude Code holds each name as written too\n\n- tools/seatbelt-case.sh: every case-variant operation is run first under no rule at all and\n  must go through, so a refusal is never an absent file on a disk that compares names as\n  written; it also measures a held name not yet on disk (a new `.ZSHRC`, `LAUNCHAGENTS` or\n  `PURLIS.TOML`) under the plain and the cased rule, and asserts the cased rules refuse them.\n  Its cased rules are the core's own: a test holds each line to what\n  `vaults_everywhere_rules`, `floor_planted_rules` and `every_project_rules` generate.\n- Claude Code's denyRead and denyWrite carry each held name as written and in either case, as\n  its tools' rules already did.\n- ADR 0067 says only what was measured: a name already on disk is matched as on disk; a name\n  not yet on disk is held by the classes; Linux is not measured.\n- The doc of `names_for` is restored, and `either_case`'s opens with what it does.\n\nProbing the profiles purlis emits on the runner is #2128.\n\nRefs #2071, #2107\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw391a): the case step asserts what ADR 0067 states of a name already on disk\n\nThe one-spelling rule's refusal of a read through `.PURLIS/VAULTS` and a write through\n`library/launchagents` is now asserted, not only reported, as the ADR states it; a name not yet\non disk stays reported for that rule and asserted for the cased one.\n\nRefs #2071, #2107\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T10:11:24+04:00",
          "tree_id": "8010cda00537d87d2d67601e6e2efc90b87d8a0a",
          "url": "https://github.com/purlis/purlis/commit/7bf7311762ab64cd3ef983b4f9990ca92e38acc2"
        },
        "date": 1792045815924,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.597043,
            "unit": "ms",
            "extra": "median of 5 runs: 0.586, 0.597, 0.597, 0.605, 0.610 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.056729,
            "unit": "ms",
            "extra": "median of 5 runs: 15.975, 16.054, 16.057, 16.152, 16.355 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.9511965,
            "unit": "ms",
            "extra": "median of 5 runs: 99.886, 100.661, 101.951, 103.003, 103.552 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "99c1d670b0dca249d3fa584e4a12bf45ebaf9ebc",
          "message": "feat(sandbox): your confirmed level off starts the chat without the sandbox (#2071, wiring part 1) (#2122)\n\n* feat(sandbox): your confirmed level off starts the chat without the sandbox\n\nA chat's start reads your own level through sandbox::your_level: personal::in_force (what\nyou set or confirmed in Settings on this machine, never the file's word alone), bounded by\npolicy. A confirmed off, for the project or the chat's persona, starts the chat as \"Start\nwithout the sandbox\" does: every class lifted, audited as the person's (by: your-level),\ncounted as an opt-out, its folder check skipped, and its tab says \"No sandbox — your\nsetting\". An off a chat wrote into purlis.local.toml changes nothing. on and floor start the\nfull sandbox until the floor lands, and the Settings page says so. A policy that forbids the\nopt-out forbids off too; sandbox::bounded is the seam the team policy plugs into.\n\nAdds the test that no harness lets a sandboxed chat write this machine's sandbox-grants.\n\nPart of #2071 (part 1 of the wiring).\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(#2122): a start a chat asks for runs at the stricter of the two levels, and the audit says how it was made\n\n- Every start says how it was made (sandbox::Made: the window, a resume, a relaunch, or asked\n  for by another chat, naming its persona from the app's record). sandbox::your_level reads a\n  start another chat asked for, a dispatch under a grant or not or a handoff, at the stricter\n  of the asking chat's level and the target's, at its first start and every later one. A start\n  that does not say how it was made, or whose asker the record cannot name, never takes off.\n- The your-level audit names how the start was made and who made it: the person in the window,\n  purlis for a relaunch, a chat for one a chat asked for.\n- A chat that names no persona takes the project's level, never the default persona's.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(#2122): a chat asked for is held to the strictest level across the whole chain that asked\n\nMade::Asked carries the chain of chats above the new one, nearest first, the chat the person\nstarted last, from the app's record (dispatchdecision::chain_asking at a dispatch or handoff,\nthe record's own chain at every later start). sandbox::your_level holds the new chat to the\nstrictest level across that chain, so a chat reaches no further than any chat above it,\nhowever many hops away. An unknown or empty chain never takes off. A chain entry naming the\ndefault persona, which stands for a chat on none, is held to the project's level too.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* feat(sandbox): your confirmed level off lifts the floor too, through the same seam\n\nWith the floor (#2081) the default where a project has not turned the full sandbox on, a\nstart there reads your level through the same seam (sandbox::yours_lifted, your_level,\nbounded) as one in a project at the full sandbox: a confirmed off, held to the chain of chats\nthat asked, starts the chat without the sandbox; anything else starts it at the floor. The\nSettings page, ADR 0096 §8, the plane-format row and the fragment say where Floor and Full\nsandbox start now.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <aharon@purlis.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T11:09:13+04:00",
          "tree_id": "7a740c06f30cab8e2043c775b23900e7fde5d1c6",
          "url": "https://github.com/purlis/purlis/commit/99c1d670b0dca249d3fa584e4a12bf45ebaf9ebc"
        },
        "date": 1792048224802,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.48645499999999997,
            "unit": "ms",
            "extra": "median of 5 runs: 0.471, 0.472, 0.486, 0.488, 0.517 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.33374,
            "unit": "ms",
            "extra": "median of 5 runs: 16.116, 16.301, 16.334, 16.487, 16.554 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.7433465,
            "unit": "ms",
            "extra": "median of 5 runs: 100.581, 100.728, 101.743, 102.876, 103.545 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ba10b852611f6a9d3e4dd30e203ae9da8f327edb",
          "message": "Profiles: create, import and remove profiles (#2096)\n\n* Profiles: create, import and remove profiles (#2034)\n\nA profile of this machine's is made in Settings > You > Profiles or with\npurlis profile create, kept in purlis's data home and available in every\nproject. Its folder is 0700, its settings are imported from the machine's\nharness settings by a list of settings files per harness and never a\nsign-in, and it records when and offers Import again (purlis profile\nimport). A chat on it runs its harness with the declared config_env and\ndata_env set to its folder. Remove names the conversations that go and\nasks first (--yes on the command line); it deletes only a folder directly\nunder the store, and an adopted harness folder is only forgotten.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Profiles: the window's commands typed, the group in the catalogue\n\nThe generated bindings and UI RPC client carry the five machine_profile\ncommands; You > Profiles is in the Settings catalogue, the palette and\nthe guards' lists, its row buttons wear ui-button, and its Remove\nquestion has a way in for the dialog guard.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2034): a stand-in harness on the start test's PATH, no old file name in the window\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2034): the records are #2033's file and shape\n\nThis machine's profiles are now recorded in <config>/profiles.json, the\nfile #2033's adoption writes: version 1, and each record its name,\nharness, title and folders, with adopted for a folder adopted in place\nand imported for one purlis made. A record or key this purlis cannot\nread is kept as written. A made profile's folder stays under\n<data>/profiles/<name>/, and an adopted record starts as its built-in\ndid.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2096): an import copies no link and no file with another name; chats are denied the profiles\n\n- An import never follows a link inside the settings folder, and never\n  copies a file that has another name on the disk; a file is opened\n  without following a link and judged by what was opened.\n- A chat's sandbox denies <data>/profiles/, reading and writing, with the\n  person's other powers; the records are in the config home it may not\n  write. plane-format.md says so for both.\n- control-plane.md: Profiles of this machine is a section of its own.\n- The import's words say the settings files are copied as they are.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2096): made profiles are records of #2033's store\n\nThe profiles purlis makes are now MachineProfile records with an\nimported field, changed through profilestore::update under its one\nlock; the duplicate store, its reader and its lock are gone. Making,\nimporting and removing live in profilestore::made, which reads where a\nharness keeps its settings from the store's table of default folders.\nRemoving an adopted profile forgets its folder, so the next launch does\nnot adopt it again. plane-format.md has one row for the store.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2096): the git hooks test asks that no denied read covers the hooks, and the merged tab test is formatted\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2096): an import reads the settings folder by descriptor; one data-home ladder\n\n- The import opens the settings folder once and each name in it, and in\n  each listed folder, relative to the descriptor of the folder it was\n  listed in, with O_NOFOLLOW and O_NONBLOCK, judged by fstat. A test\n  swaps a listed folder for a link while imports run.\n- A chat's sandbox finds the data home by the store's own ladder, so a\n  relative $PURLIS_DATA_HOME names one folder for both.\n- The profiles deny test covers opencode's wrap too.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2096): the start test's stand-in harnesses answer their version, as a sandboxed start asks\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T11:27:07+04:00",
          "tree_id": "f32f03dcbb8614018183b66a569f21493cfdc595",
          "url": "https://github.com/purlis/purlis/commit/ba10b852611f6a9d3e4dd30e203ae9da8f327edb"
        },
        "date": 1792049320829,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.49959600000000004,
            "unit": "ms",
            "extra": "median of 5 runs: 0.492, 0.493, 0.500, 0.512, 0.525 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.779762499999997,
            "unit": "ms",
            "extra": "median of 5 runs: 16.583, 16.751, 16.780, 16.892, 17.037 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.16676000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 102.373, 103.412, 104.167, 105.405, 105.510 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "31538baec8b747ba737519beed6cdcfecd27e511",
          "message": "test: a chat started on a profile made here says how it was made (#2149)\n\nThe start in #2096's test, which landed beside #2122's new field.\n\n\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\nCo-authored-by: Aaron Yordanyan <aharon@purlis.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T12:33:20+04:00",
          "tree_id": "02e272b8fdb97df935264f291d8ceb1d7ad74de5",
          "url": "https://github.com/purlis/purlis/commit/31538baec8b747ba737519beed6cdcfecd27e511"
        },
        "date": 1792053275745,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.27443150000000005,
            "unit": "ms",
            "extra": "median of 5 runs: 0.268, 0.271, 0.274, 0.279, 0.281 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.497958500000003,
            "unit": "ms",
            "extra": "median of 5 runs: 16.371, 16.464, 16.498, 16.551, 16.624 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.4222715,
            "unit": "ms",
            "extra": "median of 5 runs: 100.802, 101.018, 101.422, 101.755, 101.773 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "321c8e388d1cbebecc98678dc5043abb0d5e3936",
          "message": "purlis serve runs, started by the app (#2085)\n\n* Move the local socket check's tests into their own file\n\nA move alone, before the module changes for #1997: no test's name or body changes.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* A peer below the window that holds the chats is inside a chat\n\nWhile chats run in the window's process, the session host's check that no person's scope is\nadmitted from inside a chat takes the window as one more chat's program, for every peer but\nthe window itself: purlis_same_user::inside_a_chat's one check, with every doubt a refusal.\n\nRefs #1997\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* The session host listens on its socket and admits only its scopes\n\npurlis-host serves the session protocol on one socket per user per device: the folder under the\nruntime directory made this user's alone, an exclusive lock beside the socket so a second host\nleaves the first serving, the socket made 0600, a fresh credential for every scope, and the\nprotocol crate's checks on every connection. It runs no program yet. `purlis serve` runs it on a\nmachine with no window. The core names where the socket is, and a chat's sandbox denies that\nfolder as it denies the credentials'.\n\nRefs #1997\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* The window starts its session host as its first client\n\nThe app starts its own binary again as the session host, so the program and its Keychain\nidentity are the window's and nothing prompts (ADR 0068 §1, as amended 2026-10-14). It starts\noff the first paint, holds the host's standard input so the host ends with the window however\nit goes, connects as local-ui once the host is ready, and closes the host's input at a quit.\nWhile chats run in the window, the host takes every process below the window as inside a chat.\n\nADR 0068 is amended for #1997; plane-format names the socket and its tier.\n\nCloses #1997\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* No extension may take `serve` as its id\n\n`purlis serve` is a core command now, so the core's list of words an extension may not take\nnames it.\n\nRefs #1997\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* The session host's socket is named by the user, not by an inherited variable\n\nOne function names the socket's place for the window, `purlis serve` and a chat's sandbox: on\nmacOS the user's own temporary directory as the system names it, never $TMPDIR; on Linux\n$XDG_RUNTIME_DIR only when it is absolute, this user's and closed to others; else\n/tmp/purlis-<uid>. The window makes the folder before any chat starts, so every sandbox denies\na folder that is there, and still starts the host off the first paint. `run` keeps the mobile\nentry point attribute. ADR 0068's amendment marks its decided items to be confirmed, and the\nwindow connecting only to its own host as interim until reattach (#646).\n\nRefs #1997\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* The core's words count `serve` beside main's newest\n\nRefs #1997\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* purlis serve's test keeps its inherited temporary directory inside the fence\n\nThe test build is fenced to the temporary directory, so the hostile `$TMPDIR` it hands\n`purlis serve` is the test's own folder; the socket still goes to the runtime directory, and\nnothing else is made beside it. A host that does not say it is ready is reported with what it\nsaid.\n\nRefs #1997\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T13:09:32+04:00",
          "tree_id": "ae6d09dab0c4ae1ad11faffebb1b4cd209b3ab43",
          "url": "https://github.com/purlis/purlis/commit/321c8e388d1cbebecc98678dc5043abb0d5e3936"
        },
        "date": 1792055740093,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5459825,
            "unit": "ms",
            "extra": "median of 5 runs: 0.524, 0.543, 0.546, 0.548, 0.570 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.5054935,
            "unit": "ms",
            "extra": "median of 5 runs: 16.343, 16.441, 16.505, 16.793, 17.039 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.6682955,
            "unit": "ms",
            "extra": "median of 5 runs: 102.122, 103.690, 104.668, 104.956, 105.924 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "6c85b7dc05b2b95a7d296faae4a96109a6f14499",
          "message": "The team's policy bounds each person's sandbox level, and its off reaches them on their Accept (#2071, PR C) (#2139)\n\n* feat(sandbox): the team's policy bounds each person's level, and its off reaches them on their Accept (#2071, PR C)\n\ncharter.toml's [sandbox] gains policy = \"never\" | \"local\" (absent: local), and mode = \"off\"\nis the team's off instead of a refusal (the operator's ruling of 2026-10-15).\n\n- sandbox::team::allowed_levels(root) is the bound a start clamps a person's level to, and\n  level_for(root, persona) the level a chat runs at: the person's confirmed level within it.\n- never: nobody runs below the full sandbox. Locks::of(root) carries it, so Start without the\n  sandbox is refused everywhere the administrator's opt-out lock refuses it, with a sentence\n  naming the project's policy; a lower personal level is refused when set; mode = \"off\"\n  beside it is refused. A policy that cannot be read is never.\n- mode = \"off\" under local: the default becomes no sandbox for a person once they accept the\n  commit that turned it off, kept in this machine's config home where no chat writes; until\n  then their own level or the floor. A working-tree off asks nobody; a later off asks again.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* feat(app): the project's sandbox off is asked once, and Settings shows the team's policy (#2071, PR C)\n\n- TeamSandboxOffNotice: \"Your project turned the sandbox off (commit <sha> by <author>)\" with\n  Accept and Keep it on for me, answered by answer_team_sandbox_off (window-only, written to\n  this machine's sandbox-grants record, in the sandbox lane).\n- Settings › Sandbox › Your level: The project's policy row says why no level goes below the\n  full sandbox under policy = \"never\", or shows the answer to the team's off and the other one.\n- SandboxState gains team_off and level_held.\n- Docs: plane-format [sandbox].mode and .policy, ADR 0067's 2026-10-15 section citing the\n  operator's rulings, a changes fragment.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* fix(sandbox): the never policy's sentence names the manifest through names\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* feat(sandbox): a chat's start reads the team's bound through the one seam (#2071, PR C)\n\nsandbox::bounded clamps a person's level to team::allowed_levels: the team's default where\nthey chose none (the floor, or no sandbox once they accepted the team's off), and never below\nthe full sandbox under policy = \"never\" or an administrator's opt-out lock. your_level reads\nevery level of the chain through it, so #2122's strictest-across-the-chain rule still holds:\nan on asker keeps the chat it dispatched sandboxed under an accepted team off, and a start\nthat cannot say who asked never takes it.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* test(sandbox): the team-off fixture keeps its local file out of git, as a project does\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* review(qw391c): only [sandbox].mode says when the team's off arrived; ADR 0096 says the bound lifts the floor too\n\nThe arrival is read from the manifest's history by what its [sandbox] says, commit by commit,\nnever by a mode line in any table. ADR 0096 section 8: an administrator's no-opt-out lock or\npolicy = \"never\" holds every level at the full sandbox, the floor included.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T13:54:40+04:00",
          "tree_id": "ea9b63f449d4ca29fd0b5923c398d1699302c4d7",
          "url": "https://github.com/purlis/purlis/commit/6c85b7dc05b2b95a7d296faae4a96109a6f14499"
        },
        "date": 1792058166370,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.377672,
            "unit": "ms",
            "extra": "median of 5 runs: 0.371, 0.376, 0.378, 0.392, 0.394 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.6605825,
            "unit": "ms",
            "extra": "median of 5 runs: 16.322, 16.601, 16.661, 16.669, 16.695 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.7636465,
            "unit": "ms",
            "extra": "median of 5 runs: 101.450, 101.631, 101.764, 102.637, 102.800 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d114c2488845cee6aeb8d3c1e0e65b7bd33ecf70",
          "message": "The team's off ask rests only on what purlis verified (#2156) (#2165)\n\nThe ask for a project's committed sandbox off names a commit and its\nauthor only where that commit is in the history of the upstream commit\npurlis's own fetch found, read as written; otherwise it says the off is\nlocal and not on that upstream. Each fetch of the plane keeps the\nremote's own answer for its branch in this machine's config home.\n\nThe person's answer is kept for the off as purlis saw it, not for a\ncommit: a history rewritten under an off that stays asks nobody again,\nand the answer is let go once purlis reads the project's file not\nturning the sandbox off, so an off seen to leave and come back asks\nagain. The window's answer command no longer takes a commit.\n\nADR 0067's 2026-10-15 section, plane-format and a changes fragment say\nso.\n\nCloses #2156\n\n\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T14:54:30+04:00",
          "tree_id": "378a5c4ac5590d9e7bf346df89941c17e2c38e18",
          "url": "https://github.com/purlis/purlis/commit/d114c2488845cee6aeb8d3c1e0e65b7bd33ecf70"
        },
        "date": 1792061739527,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.2747405,
            "unit": "ms",
            "extra": "median of 5 runs: 0.258, 0.272, 0.275, 0.278, 0.293 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.4436645,
            "unit": "ms",
            "extra": "median of 5 runs: 16.409, 16.410, 16.444, 16.579, 16.692 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.428853,
            "unit": "ms",
            "extra": "median of 5 runs: 100.348, 101.326, 101.429, 101.440, 101.724 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a82dee2c644e03a9653257894b9a93cc8afccd76",
          "message": "Profiles: sign in a subscription through the harness's own flow (#2148)\n\n* Profiles: sign in a subscription through the harness's own flow (#2035)\n\nA profile purlis made is signed in through its harness's own sign-in\n(claude auth login, codex login, opencode auth login), run with the\nprofile's folder set in a terminal the person drives: a shell tab the\nwindow opens from Sign in (Settings > You > Profiles, or beside the\npicker's refusal), or purlis profile sign-in <name> in their own\nterminal. purlis reads none of what it prints.\n\nWhether a profile is signed in is the harness's own answer: how its\nstatus command exits (claude auth status, codex login status), nothing\nit prints read, or for opencode whether auth.json is there by name. A\nchat on a made profile its harness says is signed out is refused before\nit starts, with Sign in as the way out; a harness that cannot say never\nstops a chat. Adopted, built-in and clone profiles are never asked.\n\nCloses #2035\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2035): Sign in only beside its own refusal; \"shell tab\" in the refusal\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2035): one Ask per profile to sign in; opencode plans; remembered answers; Windows\n\n- A start refused because its profile is signed out leaves one wait per\n  project and profile, from any path; the window lists it as one Ask in\n  the Inbox, \"Sign in <profile>\", about no chat. Its Sign in opens the\n  shell tab running the harness's own sign-in; Dismiss puts it away; it\n  leaves once the harness says the profile is signed in, asked again\n  every few seconds after Sign in. The picker's refusal points to it.\n- opencode is signed in to the plan purlis names, by its own --provider:\n  ChatGPT or GitHub Copilot, never a Claude plan.\n- What a harness says is remembered for a minute, forgotten on Sign in.\n- No sign-in line is typed on Windows; no word with a quote, backslash\n  or control character is typed into any shell.\n- purlis-cli: each function its own doc comment; --provider.\n- ADR 0095's implementation block records the Ask and its shell tab.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2035): bindings regenerated by the app's own generator\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T15:01:27+04:00",
          "tree_id": "496b4757178a99e0c5a708d0efdc7561a11603ad",
          "url": "https://github.com/purlis/purlis/commit/a82dee2c644e03a9653257894b9a93cc8afccd76"
        },
        "date": 1792062979639,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.432488,
            "unit": "ms",
            "extra": "median of 5 runs: 0.412, 0.421, 0.432, 0.434, 0.438 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.3253995,
            "unit": "ms",
            "extra": "median of 5 runs: 16.305, 16.320, 16.325, 16.571, 16.888 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 102.13154850000001,
            "unit": "ms",
            "extra": "median of 5 runs: 101.813, 101.854, 102.132, 102.983, 103.300 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "87c42d380d5c6fa4d0a5ad74e318d40b46f88197",
          "message": "A chat runs in the session host, behind the start-time switch (#2164)\n\n* Session protocol 1.2: what a chat needs to run in the host\n\nHolding and letting go of a project, the full start (a program's id, run and\nnumber, persona, profile, size, resume, first message, the person's opt-out\nof the sandbox, and the window's own composed launch, which only local-ui may\ngive), close beside stop, readiness, a started program's number and process,\na listed chat's number and run, and two pushed notices: a program ended, and a\nview ended for good. Each frame is recorded in the protocol's fixture.\n\nRefs #1998\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* A terminal's view, followed without a thread\n\nSession::follow hands a session host the screen as it is now apart from an\nasynchronous stream of what the program writes after it, taken together under\nthe terminal's lock so nothing is lost or doubled. The stream ends with the\nprogram, and letting go of it closes the view.\n\nRefs #1998\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* The session host runs chats: hold a project, start, view, drive, close\n\npurlis-host gains Hosting, the session protocol's host for programs: it holds\nprojects, deals each chat's number in its project, runs what its binary\ncomposed in a terminal of its own, and views, writes to, resizes, stops and\ncloses it, telling subscribers as each ends. How a start is composed is the\nbinary's (Compose): the window's binary and `purlis serve` each compose a\nstart the window made with purlis_core::chatspec, the same code the window's\nown path runs, so the host links no core. Every hosted program is inside a\nchat to the admission check, below the window or below the host itself.\n\nRefs #1998\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* A chat runs in the session host, behind the start-time switch\n\nWith PURLIS_CHATS_IN_THE_HOST=1 at start, each project's chats run in the\nsession host the window started (HostedSessions): the window composes the\nstart as it always has, deals the number and issues the token, and hands the\nhost its launch; the host composes the environment with the same code and\nruns the program; the window draws, types, resizes, stops and closes it over\nthe session protocol, and hears its end. With the switch on and no host of\nthe window's own, every chat start is refused with the reason. The switch is\noff by default, and then nothing changes: Sessions::open now composes through\npurlis_core::chatspec, the code both paths share.\n\nThe behaviour suite starts here (behaviour.rs): each behaviour written once\nagainst SessionHost, run in the window and in the host with real terminals\nand the fake harness.\n\nRefs #1998\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* ADR 0068: a chat runs in the session host behind the start-time switch\n\nRecords #1998's decisions, to be confirmed: where a start is composed, the\nswitch, the terminal's view source, protocol 1.2, the window-only launch, the\nnumber and token the window keeps until #2000, the admission holder, ending\non stop, and the behaviour suite's targets.\n\nRefs #1998\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Review fixes for chats in the session host\n\n- A program's end marks only that program: a chat closed and put back under\n  its number runs on. A start the host numbers otherwise than asked fails and\n  is closed. `close` answers once the host has let go of the program, so its\n  number is free; ending every chat stops each first.\n- Holding a project is the window's alone, counted per link: a link lets go\n  only of its own holds, they end with it, and a held project is not moved.\n- A closed program's process stays inside a chat to the admission check for\n  as long as the host runs.\n- Tests: the host ends every program as `serve` stops; a behaviour checks a\n  chat's PATH and a kept variable on both targets; every refusal code is\n  recorded in the protocol's fixture.\n- ADR 0068: D-1998-10, holds, and the suite's real-host target (#2170).\n\nRefs #1998\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T16:58:29+04:00",
          "tree_id": "e2bcb5b814430e563cc889ee35223c4fd83ed6a2",
          "url": "https://github.com/purlis/purlis/commit/87c42d380d5c6fa4d0a5ad74e318d40b46f88197"
        },
        "date": 1792069195663,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.546316,
            "unit": "ms",
            "extra": "median of 5 runs: 0.518, 0.540, 0.546, 0.555, 0.564 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.7176605,
            "unit": "ms",
            "extra": "median of 5 runs: 16.322, 16.398, 16.718, 16.893, 17.320 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.961464,
            "unit": "ms",
            "extra": "median of 5 runs: 103.798, 103.887, 103.961, 104.075, 105.424 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "bcf1db353b164d37831581d88a3e5006642335d6",
          "message": "Flaky: projects.e2e waits past 30 s for the open-this-project ask on the ubuntu shard (#2129)\n\n* Wait for the trust question by its words, so a spec cannot hold the dialog before it (#2126)\n\nThe cause: making a project ends in two dialogs, one in the other's place.\nNew project stays up saying \"Creating...\" while the second create_project\nruns, then the trust question replaces it. The spec took\n$('[role=\"dialog\"]') right after pressing GitHub, so it could take the New\nproject dialog in its last moment. Once that dialog went, the held node was\ndetached, and the WebDriver server inside the app hands a script a detached\nnode without calling it stale. WebdriverIO's isDisplayed then answers false\non every look and never looks for the selector again, so the spec waited out\n30 s with the question on screen. Its cleanup's Escape then cancelled the\nquestion, which is why the failure's record found no dialog and no project\nopened. The desktop portal line in the record is the launch's own, about\n100 s before, and is on every ubuntu run: nothing on this path asks the\nportal or the session bus.\n\nopenThroughTheTrustQuestion() in e2e/opening.ts waits for a dialog titled\n\"Open this project?\" with one browser.execute per look, holding no node,\nthen presses Open project inside it. The two waits in projects.e2e.ts and\nthe one in switching.ts use it, and opening.test.ts fails on any scenario\nsource that waits on the question's words itself.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Re-read the page for every dialog a scenario waits on, not a node found before it (#2126 review)\n\nThe same race as the trust question: a spec that holds the first\n[role=\"dialog\"] can hold one that another dialog then replaces, and the\ndriver inside the app never calls the detached node stale.\n\n- pickAndStart, answerTheAsk and closeProject wait for their dialog by the\n  answer it offers (aDialogOffers, run in the page), and wait for it to go\n  the same way, instead of holding a node and asking not.toBeDisplayed.\n- The trust question's wait in twelve more specs goes through\n  openThroughTheTrustQuestion(), which takes a shorter wait and an\n  optional question for windows.e2e.ts. your-repos.e2e.ts names the\n  question in its selector.\n- The guard says what it checks, reads bench/ too, takes any quotes, and\n  also fails on pressing Open project in whichever dialog came first.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Wait for the picker and the asks to go by their Cancel, not by the button pressed (#2126 review)\n\nThe first fix round waited for a dialog to go by the button that was\npressed. That button changes its words while the dialog is still up\n(\"Starting...\"), so the wait ended at once. The next press then landed\nwhile the picker was still starting, and the second chat started on a\nprofile the plane does not declare (palette.e2e.ts:314, lifecycle.e2e.ts:121\nand :135 on both OSes). Each of these dialogs keeps its Cancel while it\nworks, so the dialog is gone when no dialog with its words offers Cancel.\nThe picker is also found by its title.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T17:04:33+04:00",
          "tree_id": "f06c66b9ca1813ccc79b9c19fbdde7e729d0ba16",
          "url": "https://github.com/purlis/purlis/commit/bcf1db353b164d37831581d88a3e5006642335d6"
        },
        "date": 1792070406458,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5031065000000001,
            "unit": "ms",
            "extra": "median of 5 runs: 0.474, 0.496, 0.503, 0.506, 0.532 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.4037745,
            "unit": "ms",
            "extra": "median of 5 runs: 16.303, 16.323, 16.404, 16.508, 16.611 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 104.003556,
            "unit": "ms",
            "extra": "median of 5 runs: 101.659, 102.386, 104.004, 104.250, 105.144 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "bc79e7834c987bdcc7a40780698a439a8a53baae",
          "message": "fix(chats): the fifty-sessions scenario waits per session with a ceiling per open; the older-sandbox check compiles once per kind of start (#2157) (#2162)\n\n* fix(chats): opening a chat stays quick with many open; the fifty-sessions scenario waits per session (#2157)\n\nCause: since the floor became the default (#2081, #2122), every chat in a\nproject that has not turned the sandbox on runs sandboxed, so every open\nchat is counted by `chats_on_older_sandbox`. The window asks that whenever a\nchat opens or closes, and the core compiled each open chat's sandbox again\non every ask: opening the n-th chat cost n compiles, and fifty chats over a\nthousand, each running beside the next start. The panes scenario's\nfifty-sessions test went from about 85 s to 170-180 s on the macOS runner\nacross those two merges, against its fixed 180 s budget. The \"no program\ncalled claude\" line in the failing run's log is from an earlier spec's\nreopen, a minute and a half before panes.e2e.ts began.\n\nFix: one ask compiles each distinct start once (harness, persona, level,\nhow it was made, its grants) and compares every chat started alike with\nthat one result. The test opens each tab and waits for that tab's own\nsession to come up, with a time per session rather than one fixed total.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* debug(#2157): temporary timings of a chat's start, to be reverted\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* fix(#2157): drop the start timings; the cause, as measured on the macOS runner\n\nMeasured on the macOS scenario runner (the timings this reverts): since\nthe floor became the default (#2081, #2122) every chat start compiles a\nsandbox, about 200 ms a compile there (about 17 ms on a developer's Mac).\nEach \"New tab\" compiles six times: the picker's options compile once per\nprofile it lists (five) and the start once, and asks the program its\nversion. One open went from about 1.6 s to about 3.4 s, so the fifty\nsessions test went from about 85 s to 170 s against its fixed 180 s\nbudget, and crossed it on a slow run. The chats never failed to start: the\n\"no program called claude\" log line came from an earlier spec's reopen.\n\nBesides that, every open and close asked whether any chat ran on an\nolder sandbox, and that compiled every open chat: up to ten seconds of\ncompiles per open at fifty chats, beside the next start. It now compiles\neach distinct start once.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* fix(e2e): the fifty-sessions allowance goes on the describe, where WebdriverIO reads it (#2157)\n\nA this.timeout() inside a test body arrives after WebdriverIO has read the\nrunnable's timeout, so the test kept running under the 180 s it outgrew.\nbudget.test.ts now also holds the fifty-sessions allowance above 180 s.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(#2157): a ceiling on each open, a cap on tabs, and chats started apart compiled apart\n\n- The fifty-sessions test stops opening past FIFTY + 10 tabs and says so, and\n  fails an open that takes over 8 s from press to its own session, by its\n  number. Each open's time goes to logs/fifty-opens.jsonl.\n- A chat whose grants differ is compiled under its own, and only the chat\n  whose sandbox fell behind is listed.\n- StartKey and CompiledNow name what one ask compiles once per start.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T17:35:36+04:00",
          "tree_id": "720f83842686aa5697757acd9cea830e9e0922a0",
          "url": "https://github.com/purlis/purlis/commit/bc79e7834c987bdcc7a40780698a439a8a53baae"
        },
        "date": 1792071890546,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.5298285,
            "unit": "ms",
            "extra": "median of 5 runs: 0.514, 0.520, 0.530, 0.535, 0.560 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.958354,
            "unit": "ms",
            "extra": "median of 5 runs: 16.405, 16.435, 16.958, 17.004, 17.199 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 103.9912105,
            "unit": "ms",
            "extra": "median of 5 runs: 103.235, 103.661, 103.991, 105.141, 105.219 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "69d1d88802290b9e938c93663b125c7747d08147",
          "message": "Profiles: personas, teammates and the rules that stay (#2169)\n\n* Profiles: a persona's tab offers to create the profile it names (#2039)\n\nA persona whose definition names a profile this machine does not have\nsays so on its tab, as a Notice whose fix, Create profile <name>...,\nopens a form with the name fixed and the harness to pick. Create runs\nmachine_profile_create, the command Add profile runs; nothing is made\nuntil it is pressed, and the persona's view is asked again after.\n\nRegression tests hold a profile made here to the project's dispatch\nlist, to the approval of a clone's own profile of the same name, and\nto the refusal of a command that switches permission prompts off.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2039): [sign_in] plans in harness declarations; X12 is #2168's\n\n- A harness declaration lists the plans a subscription signs in to,\n  [sign_in] plans = [{ provider, title }], an allow-list; a harness\n  declaring subscription with no plans signs in to its own vendor's\n  plan. anthropic is refused in any declaration but Claude Code's own,\n  when the declaration is read. opencode's ChatGPT and GitHub Copilot\n  move from OPENCODE_PLANS into opencode.toml, and the sign-in Ask, a\n  profile's row and purlis profile sign-in --provider read it. Key\n  confirmed by the operator on 2026-10-15 (ADR 0073, amended;\n  docs/plane-format.md).\n- ADR 0095's What changes where gives X12 and its store to #2168.\n- The persona's Notice no longer repeats its Profile line, and Create\n  says what it made and imported, as Add profile does.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T17:49:40+04:00",
          "tree_id": "74d9ba560e859a7151418925116ab2493be1e5e9",
          "url": "https://github.com/purlis/purlis/commit/69d1d88802290b9e938c93663b125c7747d08147"
        },
        "date": 1792073021856,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.3020865,
            "unit": "ms",
            "extra": "median of 5 runs: 0.275, 0.284, 0.302, 0.313, 0.314 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 16.598121499999998,
            "unit": "ms",
            "extra": "median of 5 runs: 16.450, 16.570, 16.598, 16.603, 16.783 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 101.26683,
            "unit": "ms",
            "extra": "median of 5 runs: 101.030, 101.202, 101.267, 101.355, 101.825 ms"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "aaron.yor@gmail.com",
            "name": "Aaron Yordanyan",
            "username": "diazoxide"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "63089dee4c5274b9d10a10861966cffae6e22e36",
          "message": "Profiles: API key and cloud profiles; a model choice is a profile plus a model (#2152)\n\n* Profiles: a key or cloud profile names its vault entries; the picker shows who pays (#2036)\n\nA profile of this machine's keeps its sign-in in its record: its kind, the\nlabel it is billed to, and for api-key or cloud each variable its harness's\nadapter is handed, by vault reference, never a value. A start carries the\nreferences; the values are read where the program is spawned, under the\nvault's rules for the chat's persona, and set on that program's\nenvironment alone. Where ADR 0087 section 7 sends a key through the proxy,\na chat on such a profile does not start and says so.\n\nThe picker shows every profile's model (the harness's default) and what a\nchat on it is billed to; a run's model source is its profile's sign-in\nkind. ADR 0087 amended, CONTEXT and plane-format updated.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Profiles: the app's starts carry the sign-in; a vault the persona may not read starts nothing (#2036)\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2036): the proxy refusal names the ticket that opens it\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Profiles: a cloud profile's chat is handed its credentials; a key waits for the proxy (#2036)\n\nThe operator ruled on 2026-10-15 (#2151): cloud credentials go into the\nchat's harness environment now, since a proxy that swaps one header\ncannot carry a cloud's signed requests; a key keeps waiting for the key\nproxy (#822). profilesignin::handing opens for cloud kinds where\nvariables alone sign the harness in; Vertex's credentials file and\nCodex's Azure provider configuration are refused in words and stay with\n#2151. A key profile's row and refusal say it waits for the key proxy.\nADR 0087 and 0095 record the ruling.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Profiles: #2148's start check asks only a profile on the harness's own sign-in (#2036)\n\nA profile kept as signing in with a key, a cloud or a local model is not\nasked whether its harness's subscription is signed in; #2148's test\nrecord carries the sign-in field.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2036): every row says how its chat would start; the read comes before anything is recorded\n\n- handing() decides both the start and the picker's row: a key waits\n  for the key proxy, Vertex and Codex on Azure are refused in words,\n  and this version hands no cloud's credentials (#2151); a row whose\n  credentials would go into the environment says the commands it runs\n  can read them (ADR 0087 section 7).\n- The refusal strings keep their line continuations; a test holds every\n  refusal and row free of double spaces.\n- The vault is read before the sandbox's trust event and the state\n  hooks, so a chat that never starts leaves nothing recorded.\n- A chat its profile hands credentials is never started in the session\n  host: the values are set only on a program the window starts.\n- SignIn keeps the fields this build does not know.\n- A cloud profile whose harness is signed out is not asked to sign in;\n  a kept subscription profile that is signed out still is.\n- ADR 0087: the run's model_source values, the residual stated, and\n  D-2036-1..7 as decided in implementation, to be confirmed;\n  plane-format's run.started row matches.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2036): the signed-out cloud test opens unsteered\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T18:10:37+04:00",
          "tree_id": "2af40a1c1e266c535215ac960ed0c3116507a094",
          "url": "https://github.com/purlis/purlis/commit/63089dee4c5274b9d10a10861966cffae6e22e36"
        },
        "date": 1792074136133,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "keystroke under ten flooding panes",
            "value": 0.535327,
            "unit": "ms",
            "extra": "median of 5 runs: 0.507, 0.527, 0.535, 0.537, 0.547 ms"
          },
          {
            "name": "2 MB burst, asked to drawn",
            "value": 17.1272055,
            "unit": "ms",
            "extra": "median of 5 runs: 16.656, 16.850, 17.127, 17.151, 17.546 ms"
          },
          {
            "name": "13 MB burst, asked to drawn",
            "value": 105.5061665,
            "unit": "ms",
            "extra": "median of 5 runs: 104.576, 105.008, 105.506, 105.692, 107.216 ms"
          }
        ]
      }
    ]
  }
}