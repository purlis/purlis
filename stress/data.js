window.BENCHMARK_DATA = {
  "lastUpdate": 1792450096205,
  "repoUrl": "https://github.com/purlis/purlis",
  "entries": {
    "stress (macos-latest)": [
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
        "date": 1791771751164,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 867,
            "unit": "MB",
            "extra": "median of 3 samples: 636, 867, 1000 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 177.9,
            "unit": "MB",
            "extra": "median of 6 samples: 201.9, 201.2, 199.4, 155.6, 156.3, 127.3 MB"
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
        "date": 1791777563140,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 842,
            "unit": "MB",
            "extra": "median of 3 samples: 708, 842, 923 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 188.7,
            "unit": "MB",
            "extra": "median of 6 samples: 207.1, 188.3, 189.8, 188.4, 189.1, 164.6 MB"
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
        "date": 1791779264078,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 750,
            "unit": "MB",
            "extra": "median of 3 samples: 616, 750, 928 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 189.1,
            "unit": "MB",
            "extra": "median of 6 samples: 208.4, 190.4, 188, 188.8, 189.5, 164.1 MB"
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
        "date": 1791780722699,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 841,
            "unit": "MB",
            "extra": "median of 3 samples: 629, 841, 880 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 195.8,
            "unit": "MB",
            "extra": "median of 6 samples: 196.7, 196.1, 194.8, 195.5, 196.1, 107.4 MB"
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
        "date": 1791791129448,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 882,
            "unit": "MB",
            "extra": "median of 3 samples: 882, 882, 1078 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 172.9,
            "unit": "MB",
            "extra": "median of 6 samples: 190.9, 189.5, 192.6, 155.6, 156.3, 122.9 MB"
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
        "date": 1791793718518,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 747,
            "unit": "MB",
            "extra": "median of 3 samples: 524, 747, 914 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 188.2,
            "unit": "MB",
            "extra": "median of 6 samples: 209.1, 207.9, 189.1, 186.6, 187.3, 160.2 MB"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "name": "Aaron Yordanyan",
            "username": "diazoxide",
            "email": "aaron.yor@gmail.com"
          },
          "committer": {
            "name": "Aaron Yordanyan",
            "username": "diazoxide",
            "email": "aaron.yor@gmail.com"
          },
          "id": "40f2ac86b9471cfb153825eda867a96f8b0bbac3",
          "message": "A Go tool's refused certificate check is read from a failed command\n\nA sandboxed chat's `glab api` failed with `x509: OSStatus -26276` and no\nNotice came up. A Go program refused the system's certificate check exits\nnon-zero, so its words reach the block hook only in PostToolUseFailure's\n`error`, which is read as standard output: a line counts only where it\nends as a refusal, and no ending matched the certificate check. The one\nexisting test fed the line as standard error of a command that succeeded,\na shape real Go CLIs never produce for this failure.\n\n`ends_as_a_refusal` now ends on `x509: OSStatus -26276`, so the existing\ncertificate-check block, and its Notice, comes up. A line that only\nquotes the words is still not a block. Nothing is granted.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T06:59:36Z",
          "url": "https://github.com/purlis/purlis/commit/40f2ac86b9471cfb153825eda867a96f8b0bbac3"
        },
        "date": 1791800572154,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 873,
            "unit": "MB",
            "extra": "median of 3 samples: 766, 873, 958 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 158.8,
            "unit": "MB",
            "extra": "median of 6 samples: 204.1, 184.7, 174.7, 142.1, 142.8, 117.5 MB"
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
        "date": 1791803039759,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 721,
            "unit": "MB",
            "extra": "median of 3 samples: 721, 721, 847 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 200.4,
            "unit": "MB",
            "extra": "median of 6 samples: 188.8, 200.6, 203.1, 200.2, 200.8, 126.6 MB"
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
        "date": 1791806214615,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 786,
            "unit": "MB",
            "extra": "median of 3 samples: 651, 786, 935 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 158.8,
            "unit": "MB",
            "extra": "median of 6 samples: 182, 189.8, 163.5, 153.5, 154.2, 123.5 MB"
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
        "date": 1791810480873,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 757,
            "unit": "MB",
            "extra": "median of 3 samples: 639, 757, 853 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 168.5,
            "unit": "MB",
            "extra": "median of 6 samples: 182.1, 179.6, 182.5, 156.7, 157.4, 123.8 MB"
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
        "date": 1791813966211,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 750,
            "unit": "MB",
            "extra": "median of 3 samples: 692, 750, 873 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 174.2,
            "unit": "MB",
            "extra": "median of 6 samples: 202.8, 175.2, 178.8, 172.5, 173.2, 113.2 MB"
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
        "date": 1791816331045,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 886,
            "unit": "MB",
            "extra": "median of 3 samples: 717, 886, 932 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 136.3,
            "unit": "MB",
            "extra": "median of 6 samples: 180.3, 175.3, 134.6, 136.1, 136.6, 121 MB"
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
        "date": 1791820255297,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 519,
            "unit": "MB",
            "extra": "median of 3 samples: 432, 519, 584 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 279.3,
            "unit": "MB",
            "extra": "median of 6 samples: 278.4, 277.6, 278.8, 279.7, 280.6, 369 MB"
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
        "date": 1791833959746,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 726,
            "unit": "MB",
            "extra": "median of 3 samples: 695, 726, 880 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 168.7,
            "unit": "MB",
            "extra": "median of 6 samples: 242.6, 186.7, 182.6, 154.1, 154.9, 128.2 MB"
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
        "date": 1791841922678,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 875,
            "unit": "MB",
            "extra": "median of 3 samples: 724, 875, 1023 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 175.5,
            "unit": "MB",
            "extra": "median of 6 samples: 253.2, 193.8, 180.4, 170, 170.7, 133.1 MB"
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
        "date": 1791843609428,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 883,
            "unit": "MB",
            "extra": "median of 3 samples: 689, 883, 883 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 222.8,
            "unit": "MB",
            "extra": "median of 6 samples: 252.6, 219.7, 221.8, 223.7, 224.5, 124.2 MB"
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
        "date": 1791845971034,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 815,
            "unit": "MB",
            "extra": "median of 3 samples: 815, 815, 937 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 185.9,
            "unit": "MB",
            "extra": "median of 6 samples: 216, 222.3, 185.8, 185.3, 186, 117.8 MB"
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
        "date": 1791848532043,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 817,
            "unit": "MB",
            "extra": "median of 3 samples: 652, 817, 829 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 199.9,
            "unit": "MB",
            "extra": "median of 6 samples: 236.5, 200, 201.8, 199.2, 199.8, 135.1 MB"
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
        "date": 1791850869337,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 797,
            "unit": "MB",
            "extra": "median of 3 samples: 742, 797, 971 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 172.4,
            "unit": "MB",
            "extra": "median of 6 samples: 208.8, 206.3, 175.3, 168.6, 169.4, 116.1 MB"
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
        "date": 1791855442762,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 899,
            "unit": "MB",
            "extra": "median of 3 samples: 836, 899, 994 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 186,
            "unit": "MB",
            "extra": "median of 6 samples: 216.6, 203.2, 200.1, 171.4, 172, 121.9 MB"
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
        "date": 1791858186959,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 937,
            "unit": "MB",
            "extra": "median of 3 samples: 720, 937, 1006 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 185,
            "unit": "MB",
            "extra": "median of 6 samples: 248.7, 216, 188.9, 180.5, 181.2, 137.2 MB"
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
        "date": 1791862941124,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 862,
            "unit": "MB",
            "extra": "median of 3 samples: 767, 862, 1023 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 222.4,
            "unit": "MB",
            "extra": "median of 6 samples: 214.7, 226.5, 223.2, 222, 222.7, 119.2 MB"
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
        "date": 1791866635332,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 895,
            "unit": "MB",
            "extra": "median of 3 samples: 813, 895, 981 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 194.6,
            "unit": "MB",
            "extra": "median of 6 samples: 236.7, 194.1, 195.9, 194.3, 195, 119.5 MB"
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
        "date": 1791882702230,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 844,
            "unit": "MB",
            "extra": "median of 3 samples: 765, 844, 995 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 178.2,
            "unit": "MB",
            "extra": "median of 6 samples: 206.5, 216.3, 180.6, 175.1, 175.8, 111.3 MB"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple",
            "email": "eyehollow@gmail.com"
          },
          "committer": {
            "name": "Aaron Yordanyan",
            "username": "diazoxide",
            "email": "aaron.yor@gmail.com"
          },
          "id": "d7d7e7d62ced63b2b57d1b7c5d9848f410828f9d",
          "message": "sandbox: a docker run down a pipe names its socket, so its Notice offers Allow\n\nA sandboxed Claude Code chat ran `docker run … 2>&1 | tail`, the sandbox\nrefused Docker its socket, and the pipe ended with tail's status. Claude\nCode handed the result over as one that came back, with Docker's words on\nstandard output and no failure beside them. A socket was named only from a\nfailed command (ADR 0093, D-sock-5), so the block reached the app with no\nsocket and the Notice said \"The sandbox did not say which socket it\nrefused\", with Start without the sandbox as its only way out.\n\nNo harness says how each program of a line ended, so the line is the one\nfailure signal left in the payload: where one of its own programs is a\ncontainer engine's client (docker, docker-compose, podman, podman-compose,\nnerdctl), by name after the shell's plumbing, the socket its output says it\nwas refused is named, on either stream and in Codex's and opencode's one\nstring. A line that only reads (cat, grep, git show, curl) still names\nnone, and neither does a violation line in a one-string result. Only a\nsocket is named this way; the certificate check keeps to a failed command.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T07:24:31Z",
          "url": "https://github.com/purlis/purlis/commit/d7d7e7d62ced63b2b57d1b7c5d9848f410828f9d"
        },
        "date": 1791887368606,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 823,
            "unit": "MB",
            "extra": "median of 3 samples: 808, 823, 1002 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 209.9,
            "unit": "MB",
            "extra": "median of 6 samples: 209.5, 220.6, 210.4, 209.9, 210, 96.5 MB"
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
        "date": 1791896315275,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 937,
            "unit": "MB",
            "extra": "median of 3 samples: 777, 937, 1017 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 175.3,
            "unit": "MB",
            "extra": "median of 6 samples: 213.6, 221.4, 177.2, 172.6, 173.4, 133.7 MB"
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
        "date": 1791899006532,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 894,
            "unit": "MB",
            "extra": "median of 3 samples: 732, 894, 1014 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 192,
            "unit": "MB",
            "extra": "median of 6 samples: 213.3, 220, 190.7, 191.7, 192.3, 121.7 MB"
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
        "date": 1791901516673,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 910,
            "unit": "MB",
            "extra": "median of 3 samples: 821, 910, 1007 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 222,
            "unit": "MB",
            "extra": "median of 6 samples: 209.5, 220.6, 223.3, 224.1, 224.7, 143 MB"
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
        "date": 1791904111144,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 783,
            "unit": "MB",
            "extra": "median of 3 samples: 722, 783, 905 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 217.4,
            "unit": "MB",
            "extra": "median of 6 samples: 213.4, 218.3, 217.1, 217.8, 218.5, 124.8 MB"
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
        "date": 1791906472420,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 868,
            "unit": "MB",
            "extra": "median of 3 samples: 739, 868, 1087 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 176,
            "unit": "MB",
            "extra": "median of 6 samples: 189.6, 192, 196.1, 161.8, 162.4, 110.8 MB"
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
        "date": 1791910559144,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 825,
            "unit": "MB",
            "extra": "median of 3 samples: 761, 825, 1012 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 198.4,
            "unit": "MB",
            "extra": "median of 6 samples: 208.4, 205, 200.1, 196.2, 196.7, 126.9 MB"
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
        "date": 1791913053559,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 867,
            "unit": "MB",
            "extra": "median of 3 samples: 783, 867, 1005 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 183.5,
            "unit": "MB",
            "extra": "median of 6 samples: 204.1, 213.3, 184.4, 181.9, 182.6, 131.2 MB"
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
        "date": 1791915344622,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 937,
            "unit": "MB",
            "extra": "median of 3 samples: 823, 937, 1005 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 210.6,
            "unit": "MB",
            "extra": "median of 6 samples: 218.3, 217.9, 210, 210.3, 210.9, 127.4 MB"
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
        "date": 1791916960398,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 578,
            "unit": "MB",
            "extra": "median of 3 samples: 510, 578, 662 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 286.8,
            "unit": "MB",
            "extra": "median of 6 samples: 284.2, 284.3, 285.9, 287.8, 288.5, 352.2 MB"
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
        "date": 1791922697545,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 1054,
            "unit": "MB",
            "extra": "median of 3 samples: 784, 1054, 1136 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 180.7,
            "unit": "MB",
            "extra": "median of 6 samples: 240.3, 189.8, 179.4, 180.3, 181, 134 MB"
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
        "date": 1791924909026,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 935,
            "unit": "MB",
            "extra": "median of 3 samples: 794, 935, 969 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 177.4,
            "unit": "MB",
            "extra": "median of 6 samples: 206.1, 212, 179.5, 174.5, 175.2, 135.6 MB"
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
        "date": 1791927298365,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 820,
            "unit": "MB",
            "extra": "median of 3 samples: 751, 820, 977 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 220.4,
            "unit": "MB",
            "extra": "median of 6 samples: 237.3, 223.3, 222, 218.1, 218.9, 127.6 MB"
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
        "date": 1791929800443,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 896,
            "unit": "MB",
            "extra": "median of 3 samples: 831, 896, 970 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 204.3,
            "unit": "MB",
            "extra": "median of 6 samples: 203.6, 213.5, 215.1, 204, 204.6, 142 MB"
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
        "date": 1791932282681,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 566,
            "unit": "MB",
            "extra": "median of 3 samples: 465, 566, 668 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 285.9,
            "unit": "MB",
            "extra": "median of 6 samples: 283.4, 283.9, 285.6, 286.2, 286.9, 343.8 MB"
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
        "date": 1791935027740,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 988,
            "unit": "MB",
            "extra": "median of 3 samples: 783, 988, 1068 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 198.8,
            "unit": "MB",
            "extra": "median of 6 samples: 205.2, 217.1, 215.1, 191.8, 192.4, 128.4 MB"
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
        "date": 1791937207655,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 971,
            "unit": "MB",
            "extra": "median of 3 samples: 754, 971, 1043 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 169.2,
            "unit": "MB",
            "extra": "median of 6 samples: 208.6, 214.5, 167.9, 168.9, 169.6, 120.2 MB"
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
        "date": 1791939142586,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 837,
            "unit": "MB",
            "extra": "median of 3 samples: 746, 837, 892 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 209.4,
            "unit": "MB",
            "extra": "median of 6 samples: 210.3, 217.1, 215.3, 208.3, 208.5, 120.9 MB"
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
        "date": 1791941341159,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 891,
            "unit": "MB",
            "extra": "median of 3 samples: 715, 891, 907 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 187.9,
            "unit": "MB",
            "extra": "median of 6 samples: 250.5, 187.8, 188.8, 188.1, 182.8, 164.3 MB"
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
        "date": 1791942909071,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 572,
            "unit": "MB",
            "extra": "median of 3 samples: 486, 572, 662 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 285.7,
            "unit": "MB",
            "extra": "median of 6 samples: 283.1, 283.8, 285.3, 286.1, 287, 379.5 MB"
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
        "date": 1791945308343,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 904,
            "unit": "MB",
            "extra": "median of 3 samples: 755, 904, 964 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 176.3,
            "unit": "MB",
            "extra": "median of 6 samples: 234.2, 218, 177.7, 174.8, 173.2, 122.2 MB"
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
        "date": 1791947505783,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 811,
            "unit": "MB",
            "extra": "median of 3 samples: 725, 811, 889 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 223.3,
            "unit": "MB",
            "extra": "median of 6 samples: 213.9, 223.9, 224.6, 223.2, 223.4, 130.9 MB"
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
        "date": 1791950083192,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 818,
            "unit": "MB",
            "extra": "median of 3 samples: 818, 818, 887 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 221.8,
            "unit": "MB",
            "extra": "median of 6 samples: 215.7, 225.8, 228.3, 222, 221.6, 146.3 MB"
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
        "date": 1791966936145,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 952,
            "unit": "MB",
            "extra": "median of 2 samples: 880, 1024 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 178.7,
            "unit": "MB",
            "extra": "median of 6 samples: 203.8, 214, 177.3, 178.3, 179, 124.4 MB"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple",
            "email": "eyehollow@gmail.com"
          },
          "committer": {
            "name": "Aaron Yordanyan",
            "username": "diazoxide",
            "email": "aaron.yor@gmail.com"
          },
          "id": "9b7cc465b3e2833aacc57154b509a9b6e626ea1e",
          "message": "docs(secrets): a tool's example reads its values itself; curl expands its own\n\nThe several-values example is now `aws`, which reads AWS_ACCESS_KEY_ID and\nAWS_SECRET_ACCESS_KEY from its environment. curl reads no credential from\na variable, and a chat's arguments reach a tool with no shell between, so\nthe secrets guide shows curl's own expansion instead (curl 8.3.0 or\nlater): both values as env, then\n`curl --variable %KIBANA_USER --variable %KIBANA_PASS\n--expand-user '{{KIBANA_USER}}:{{KIBANA_PASS}}' <url>`.\nThe guide's first `secret exec ... curl` line is written the same way.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T07:36:14Z",
          "url": "https://github.com/purlis/purlis/commit/9b7cc465b3e2833aacc57154b509a9b6e626ea1e"
        },
        "date": 1791967098407,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 892,
            "unit": "MB",
            "extra": "median of 3 samples: 726, 892, 984 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 220.1,
            "unit": "MB",
            "extra": "median of 6 samples: 209.2, 220.2, 220, 220.7, 221.3, 121.5 MB"
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
        "date": 1791970070033,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 828,
            "unit": "MB",
            "extra": "median of 3 samples: 818, 828, 890 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 218.6,
            "unit": "MB",
            "extra": "median of 6 samples: 213.8, 229.1, 230.4, 218.3, 219, 136.5 MB"
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
        "date": 1791972942407,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 877,
            "unit": "MB",
            "extra": "median of 3 samples: 809, 877, 1004 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 194.2,
            "unit": "MB",
            "extra": "median of 6 samples: 246.9, 248.9, 193.1, 193.8, 194.5, 134.7 MB"
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
        "date": 1791975459932,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 814,
            "unit": "MB",
            "extra": "median of 3 samples: 773, 814, 897 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 222.9,
            "unit": "MB",
            "extra": "median of 6 samples: 208.8, 222, 223.8, 224, 224.7, 155.3 MB"
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
        "date": 1791977170321,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 518,
            "unit": "MB",
            "extra": "median of 3 samples: 516, 518, 664 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 288.7,
            "unit": "MB",
            "extra": "median of 6 samples: 287.3, 287, 288.4, 289, 290, 395.6 MB"
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
        "date": 1791980934819,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 813,
            "unit": "MB",
            "extra": "median of 3 samples: 759, 813, 916 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 241.7,
            "unit": "MB",
            "extra": "median of 6 samples: 237.1, 244.4, 246, 241.3, 242.1, 128.8 MB"
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
        "date": 1791984641998,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 896,
            "unit": "MB",
            "extra": "median of 3 samples: 755, 896, 997 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 185.3,
            "unit": "MB",
            "extra": "median of 6 samples: 251.4, 210.1, 197.5, 172.3, 173.1, 119.2 MB"
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
        "date": 1791986784440,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 908,
            "unit": "MB",
            "extra": "median of 3 samples: 725, 908, 1091 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 224.6,
            "unit": "MB",
            "extra": "median of 6 samples: 220, 237.8, 228.8, 224.1, 225, 150.3 MB"
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
        "date": 1791991201499,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 905,
            "unit": "MB",
            "extra": "median of 3 samples: 763, 905, 1097 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 199.1,
            "unit": "MB",
            "extra": "median of 6 samples: 218.7, 213.7, 205.9, 191.8, 192.4, 128.9 MB"
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
        "date": 1792006868332,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 932,
            "unit": "MB",
            "extra": "median of 3 samples: 773, 932, 1012 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 210.6,
            "unit": "MB",
            "extra": "median of 6 samples: 275.9, 210.8, 209.4, 210.5, 211.3, 157.7 MB"
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
        "date": 1792009529638,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 835,
            "unit": "MB",
            "extra": "median of 3 samples: 770, 835, 1005 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 203.6,
            "unit": "MB",
            "extra": "median of 6 samples: 237.5, 233.1, 213, 194.2, 194.3, 124 MB"
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
        "date": 1792013740753,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 832,
            "unit": "MB",
            "extra": "median of 3 samples: 786, 832, 943 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 186.7,
            "unit": "MB",
            "extra": "median of 6 samples: 206.1, 226.8, 190.6, 182.7, 177.4, 134.8 MB"
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
        "date": 1792016110194,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 850,
            "unit": "MB",
            "extra": "median of 3 samples: 730, 850, 1032 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 212.7,
            "unit": "MB",
            "extra": "median of 6 samples: 203.8, 228.5, 225.3, 212.4, 213.1, 135.4 MB"
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
        "date": 1792018451169,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 846,
            "unit": "MB",
            "extra": "median of 3 samples: 742, 846, 980 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 196.8,
            "unit": "MB",
            "extra": "median of 6 samples: 257.7, 255.1, 196.9, 196, 196.8, 179.9 MB"
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
        "date": 1792020910547,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 830,
            "unit": "MB",
            "extra": "median of 3 samples: 724, 830, 870 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 221.1,
            "unit": "MB",
            "extra": "median of 6 samples: 225.3, 223.3, 224.8, 218.2, 219, 143.8 MB"
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
        "date": 1792025251552,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 531,
            "unit": "MB",
            "extra": "median of 3 samples: 515, 531, 602 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 287,
            "unit": "MB",
            "extra": "median of 6 samples: 286.8, 285.5, 286, 287.3, 288, 394.5 MB"
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
        "date": 1792027243793,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 824,
            "unit": "MB",
            "extra": "median of 3 samples: 771, 824, 913 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 190.6,
            "unit": "MB",
            "extra": "median of 6 samples: 211.6, 231.2, 198.8, 181.7, 182.5, 125.3 MB"
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
        "date": 1792029469855,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 858,
            "unit": "MB",
            "extra": "median of 3 samples: 778, 858, 972 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 213.5,
            "unit": "MB",
            "extra": "median of 6 samples: 224.4, 238.3, 232.2, 201.9, 202.6, 125.2 MB"
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
        "date": 1792033439371,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 907,
            "unit": "MB",
            "extra": "median of 3 samples: 763, 907, 937 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 210.2,
            "unit": "MB",
            "extra": "median of 6 samples: 217.6, 214.6, 214.6, 204.9, 205.8, 119.9 MB"
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
        "date": 1792036172310,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 769,
            "unit": "MB",
            "extra": "median of 3 samples: 626, 769, 899 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 220.7,
            "unit": "MB",
            "extra": "median of 6 samples: 262.4, 258.8, 244.5, 196.2, 196.9, 169.9 MB"
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
        "date": 1792040665971,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 824,
            "unit": "MB",
            "extra": "median of 3 samples: 669, 824, 953 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 200.7,
            "unit": "MB",
            "extra": "median of 6 samples: 274.7, 262.7, 201.6, 198.9, 199.7, 159.1 MB"
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
        "date": 1792043118622,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 876,
            "unit": "MB",
            "extra": "median of 3 samples: 785, 876, 940 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 227.1,
            "unit": "MB",
            "extra": "median of 6 samples: 215.4, 233.9, 235.8, 228, 226.2, 132.9 MB"
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
        "date": 1792045557418,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 828,
            "unit": "MB",
            "extra": "median of 3 samples: 726, 828, 909 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 201,
            "unit": "MB",
            "extra": "median of 6 samples: 274.8, 206.7, 205.8, 195.6, 196.2, 169.9 MB"
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
          "id": "bfff74f567cb7fe921c218984a85ea7e33908434",
          "message": "Bound how many secret execs the window's relay tracks per link (#2571) (#2576)\n\n* Bound how many secret execs the window's relay tracks per link (#2571)\n\nThe window's Heeding bounded the bytes it keeps for secret execs not\nbegun, but not how many entries a link tracks: an end of input for a\nnumber that never runs, an exec past the byte cap or a drop mark stayed\nuntil the host's end or the link's end. A link now tracks at most 256\nexecs; a line that would add one past that keeps nothing, and an exec\nthat begins then reads no input. A tracked exec is never evicted.\n\nThe bytes kept are a running total instead of a sum over every entry\non each line, and each cap is warned of once per link, never with the\ninput's content.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M538BFC8ZDFGABYNBRP8913C\nPurlis-Persona: steward\n\n* Track a secret exec from when its line is taken, bounded per chat (#2571)\n\nReview of #2576: the per-link cap was shared by every chat on the link,\nand input lines carry no chat, so one chat holding many execs open\ncould fill it; another chat's exec then lost its input, or its end, and\nwaited until the link ended.\n\nThe window's link now takes each line in order before a thread of its\nown hears it (Heeding::admit, called from Relayed::take): a secret exec\nis tracked from there, with its chat, so input that comes before it\nbegins always finds its entry and an end is never refused. A line for a\nnumber not tracked is of no exec here and is dropped, so no entry is\nmade from a line that carries no chat, and no drop marks are needed.\nAn exec is untracked when it ends. A chat has at most 32 tracked on a\nlink and a link 256; one past either is not run, and its host is told\nit ended.\n\nEvery entry goes in through Inputs::track and out through\nInputs::untrack, which keep the byte total and each chat's count;\nthe subtraction is debug-asserted. The exec's end goes through\nInputs::finished.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M538BFC8ZDFGABYNBRP8913C\nPurlis-Persona: steward\n\n* Hear lines that waited for the board first; end a panicked exec; say why one is refused (#2571)\n\nReview of #2576:\n\n- Relayed::keep_with gave up the waiting lines' lock before hearing\n  them, so a line the host handed on meanwhile was heard first: a\n  secret exec's input could find its exec not yet taken and be dropped,\n  and the exec waited for good. They are now heard with the lock held,\n  through Relayed::hand_on, the path that skips the waiting list.\n- A secret exec's run that panics skipped its end: it stayed tracked\n  and its host was never told. A drop guard (Finishing) untracks it and\n  tells the host it ended however the run returns.\n- An exec not run for want of room now tells its asker why, in a\n  refusal frame, before it ends.\n- One chat may have 16 secret execs tracked on a link, not 32, so\n  filling a link's 256 takes 16 chats.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M538BFC8ZDFGABYNBRP8913C\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-18T12:30:16+04:00",
          "tree_id": "ac4cc7a82f91f40b30719298d6fcb3e808c8e70a",
          "url": "https://github.com/purlis/purlis/commit/bfff74f567cb7fe921c218984a85ea7e33908434"
        },
        "date": 1792315045027,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 894,
            "unit": "MB",
            "extra": "median of 3 samples: 706, 894, 1196 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 194.6,
            "unit": "MB",
            "extra": "median of 6 samples: 201.2, 199.8, 193.5, 194.2, 195, 176.9 MB"
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
          "id": "9c6e6872b81f085a642fa1632cc1501ecc4d28be",
          "message": "Linux wrap: starts with a fixed environment purlis builds (#2548)\n\n* Linux wrap: starts with a fixed environment purlis builds\n\npurlis's own wrap (purlis _sandbox) and the chat's side it starts begin\nwith an environment purlis builds. The program's environment is carried\nin the wrap's own under names of the wrap's own (purlis_linux_sandbox::\ncarried), read back whole and given to the program alone. A wrap whose\nown environment is anything else starts nothing.\n\nA chat's terminal (chatspec::spec, in the window and in a session host)\nand a brokered secret exec start the wrap this way. The teller reads the\nchat's hook settings from the carried environment.\n\nCloses #2460\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Carry what a profile hands a chat past the Linux wrap\n\nThe window sets a profile's handed variables after the chat's spec is\nbuilt. chatspec::with_handed sets them last in the program's environment\nand carries them with the rest when the chat runs on purlis's own wrap.\ncarried::read builds on every system.\n\nRefs #2460\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Let the terminal's SHELL be beside what the Linux wrap carries\n\nThe pseudo-terminal sets SHELL on every program it starts, as the session\nsets TERM. The wrap lets both be and hands neither on; a terminal test\npins what a session sets.\n\nRefs #2460\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Sort the Linux wrap's refusals by the project and temp folder it carries\n\nThe supervisor starts with none of the chat's environment, so it sorts\neach refusal against the project the carried PURLIS_ROOT names (held to\nthe fence as a resolve is; the walk from where the wrap started only\nwithout one) and the carried TMPDIR. sorter_in is the seam the teller\ntells through. with_handed decides by the chat's line, as spec does.\n\nRefs #2460\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Clippy: the chat's temp folder is already a reference\n\nRefs #2460\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-18T19:57:59+04:00",
          "tree_id": "0dd88d39086e2fd9cd16470cc9982e0c6ce6470d",
          "url": "https://github.com/purlis/purlis/commit/9c6e6872b81f085a642fa1632cc1501ecc4d28be"
        },
        "date": 1792341760317,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 875,
            "unit": "MB",
            "extra": "median of 3 samples: 646, 875, 1156 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 196.5,
            "unit": "MB",
            "extra": "median of 6 samples: 207.2, 203.9, 195.5, 196.1, 197, 178.9 MB"
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
          "id": "23d8a6fd941adb5d4de61fdefca98777c431f793",
          "message": "A theme controls workspace tint (#2509)\n\nA theme's `tint` key is read (#2329): on or off, which tokens a\nworkspace's colour turns (from a closed list), and its own hues for the\neight workspace colours. The engine resolves the recipe on every\nappearance, the check and the consent floors measure every hue of it,\nand the window takes the recipe with the resolved theme and keeps only\nthe arithmetic (#2393). A tinted accent is held apart from the danger\ncolour: the tint keeps the hue and gives less colour, and draws untinted\nwhen no amount does. A token whose tint still breaks a consent floor is\ndrawn untinted, and the Notice says so.\n\n\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\nAssisted-by: claude-code\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-18T20:42:05+04:00",
          "tree_id": "a78856afc7d1be6833ad08dfae9b6e9add228f4c",
          "url": "https://github.com/purlis/purlis/commit/23d8a6fd941adb5d4de61fdefca98777c431f793"
        },
        "date": 1792344780933,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 938,
            "unit": "MB",
            "extra": "median of 3 samples: 758, 938, 1236 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 372.9,
            "unit": "MB",
            "extra": "median of 6 samples: 326, 379.5, 366.3, 380.3, 381.2, 284.3 MB; window 196.3 MB, session host 174.7 MB"
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
          "id": "0c95fe5420c56d2e3cf037a3f0228292bd964a76",
          "message": "fix(sandbox): give a run's named link back by its own name (#2617) (#2632)\n\n* fix(sandbox): give a run's named link back by its own name (#2617)\n\nOn macOS a vault tool declared under another name than its program's file\n(#1977) failed to start through the run's sandbox: `sandbox-exec` could not\nexec the run's named link (\"Operation not permitted\").\n\nCause: `profile_on` gave each of the run's files back under\n`seatbelt::kernel_strings`, which resolves the path whole. For the named link\nthat resolved to the program it points at, so the allow named the program and\nthe link itself stayed under the vaults denial. The run's files are now given\nback by their own name, the folder resolved and the last part never followed\n(`sandbox::kernel_names_itself`), under both firmlink names as before (#2489).\nA credential file is a plain file, so its grant is unchanged; a link is\ngranted as itself and nothing it points at is.\n\nThe macOS test also declared `zcat`, which macOS's gzip treats as\n`compress`'s zcat (it reads only `<file>.Z`); it now declares `gzcat`.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* style: rustfmt the named-link test (#2617)\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* docs: plainer changelog wording (#2617)\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-18T22:24:09+04:00",
          "tree_id": "67d88b07311f9351408b1c55483831f07a6e4520",
          "url": "https://github.com/purlis/purlis/commit/0c95fe5420c56d2e3cf037a3f0228292bd964a76"
        },
        "date": 1792350832535,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 873,
            "unit": "MB",
            "extra": "median of 3 samples: 654, 873, 1028 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 331.4,
            "unit": "MB",
            "extra": "median of 6 samples: 405.1, 407.3, 335, 327, 327.9, 270.1 MB; window 156.6 MB, session host 174.9 MB"
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
          "id": "77f672e0a701a018e9bcc3e4b35e1e556cfd27de",
          "message": "A theme folder's fonts are read through its checked handle (#2603)\n\n* A theme folder's fonts are read through its checked handle\n\nA face is opened from the handle of the theme folder's fonts/, which is\nopened from the handle its theme.json was read through, each open\nrefusing a link; serving a face reads it from the same handles. A\nproject's and a workspace's theme folders bring their fonts only through\nFolders::opened, for a project the window opened through the trust ask\n(ADR 0035); the person's own folders always do.\n\nCloses #2555\nPart of #2313\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* A project's font files are approved with the project\n\nEach font file in a theme folder at the plane root or in a workspace is\npart of what the project contributes, by its path and sha-256, so the\ntrust ask lists it and a new or changed one asks again. The theme engine\nreads this machine's approval of the project itself and draws a face\nfrom the project only when the approval holds its file with that digest.\nA folder named to the library brings its fonts only when asked to.\nRecorded as an amendment of ADR 0098 (operator's ruling, 2026-10-18).\n\nPart of #2313\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* A project's font files are listed under one budget\n\nA project's font files are listed under one budget for the whole plane:\nat most 64 files and 32 MiB, 24 in one theme folder, the theme folders of\n64 workspaces, 256 folders and 256 files opened. A file past it is not\nlisted, so it is never approved or drawn, and the complaint names the\nlimit. A vouch after a chat's record write reads the record's half alone.\nThe trust ask groups font files by theme folder with their size and a\nshort digest, and counts those past the first twelve.\n\nPart of #2313\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* Every trust-ask fixture says what font files a project brings\n\nThe two fixtures that described a project's contribution without its\nfont files are typed as the contribution now, so the ask they draw has\nevery field and a fixture that leaves one out does not type-check.\n\nPart of #2313\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-18T23:50:35+04:00",
          "tree_id": "085cc6c0cbc915f719d0973b25df60150dab3a9b",
          "url": "https://github.com/purlis/purlis/commit/77f672e0a701a018e9bcc3e4b35e1e556cfd27de"
        },
        "date": 1792356150671,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 897,
            "unit": "MB",
            "extra": "median of 3 samples: 758, 897, 1221 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 379.2,
            "unit": "MB",
            "extra": "median of 6 samples: 307.9, 389.5, 374.4, 384.1, 384.9, 273.6 MB; window 194.3 MB, session host 184.2 MB"
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
          "id": "7e739c9ddd673300a34f93e3e518df5f95889dea",
          "message": "Linux: a refused file no longer linked is sorted by the name it had (#2630)\n\n* Linux: a refused file no longer linked is sorted by the name it had (#2574)\n\nThe wrap's supervisor sorts a refusal whose target ends in the kernel's\n\" (deleted)\" by the path before it: its kind, and what Allow would name.\nA protected file that was unlinked is a protected file, with no Allow.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Linux: the name-lookup check reads the name a socket had (#2574)\n\nA resolver socket with \" (deleted)\" after its name is told as a lookup\nwith nothing to allow, as under the name it had.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T00:33:15+04:00",
          "tree_id": "632b3ee916ebf51445b62ce411b3655e44086d3e",
          "url": "https://github.com/purlis/purlis/commit/7e739c9ddd673300a34f93e3e518df5f95889dea"
        },
        "date": 1792357997684,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 947,
            "unit": "MB",
            "extra": "median of 3 samples: 700, 947, 1053 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 372.5,
            "unit": "MB",
            "extra": "median of 6 samples: 362.5, 373, 377.5, 372.1, 372.8, 317.6 MB; window 197.8 MB, session host 175.1 MB"
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
          "id": "67dfd81deb9b6953db74d2decdec129eec1b4187",
          "message": "Theme import and export: follow-ups (#2587, #2588, #2612) (#2660)\n\n- A staged import is read only to check (Library::checking, Staged::read;\n  Staged::path is private): no font of the stage is registered to be\n  served. Folders::find_at and every_at read only to check too.\n- One rule for names Windows cannot hold (theme::portable, on contain's\n  device table, now with CONIN$ and CONOUT$): device names and names\n  ending in a dot or a space are refused with a sentence in import,\n  export, the stage, the level reader and the files a theme names.\n- purlis theme export takes a level id and finds it with Folders::find_at,\n  never a farther level's folder. A level id takes the name grammar, so\n  the engine says why a folder like `con` is refused.\n\nCloses #2587\nCloses #2588\nCloses #2612\n\n\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T01:12:28+04:00",
          "tree_id": "5610e224f7b53899c4e6c990c5ebbeea3de25cf4",
          "url": "https://github.com/purlis/purlis/commit/67dfd81deb9b6953db74d2decdec129eec1b4187"
        },
        "date": 1792360221713,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 905,
            "unit": "MB",
            "extra": "median of 3 samples: 753, 905, 1282 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 386.9,
            "unit": "MB",
            "extra": "median of 6 samples: 302, 397.6, 396.3, 386.5, 387.3, 309.9 MB; window 200 MB, session host 190.5 MB"
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
          "id": "efb9dfd6b7a9dbb980224ee6bfb28a827d1dd23f",
          "message": "The guards follow a style wherever the file hands it on (#2659)\n\n* The guards follow a style wherever the file hands it on (#2566)\n\nThe geometry and type guards read a component's or a script's style\nthrough one follower, and the controls guard reads a control's style\nthrough the same one. From a style={…} expression, a spread, the objects\nObject.assign writes and a style typed as CSSProperties, it follows a\nheld variable, a member of a local object or a destructuring, each\nbranch of a ternary and each side of ||, ?? and &&, what a local\nfunction or useMemo returns, and what a relative import gives. In\nscript it reads an element's style under any name the file holds it by\n(const st = el.style, const { style } = el), cssText += and a\ntemplate-literal index. Each spelling has a planted-literal test through\nthe real tree; docs/design-system.md lists what the guards read and what\nthey do not.\n\nPart of #2313\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* The style follower reads each file once and lazily (#2566)\n\nA variable's initializer and a function's body are found where they end\nonly when a style is followed into them; a file that holds no element\nstyle skips alias detection, and one that names no CSSProperties skips\nthe typed pass. Alias names are escaped as a whole pattern. The geometry\nguard reads each source's declarations once per run.\n\nPart of #2313\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* The style follower fails closed and follows the review's misses (#2566)\n\nA style handed on past FOLLOW_LIMIT hops, and a style from a relative\nimport the guard cannot open (above src, not found, not script), now\nfail the guard with a sentence naming the file and the style. A script\nextension resolves as TypeScript resolves it (./s.js is s.ts).\n\nThe follower now also follows a default-exported function, a call on a\n`* as` import, a written-key member (styles[\"box\"]), a function with an\nobject return type, useCallback, and el[\"style\"] and (el.style)\naliases. Each imported file is read and parsed once per guard pass. The\nreview's planted cases are tests, and \"What the guards read in script\"\nlists what is still not followed.\n\nPart of #2313\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T01:39:41+04:00",
          "tree_id": "58741fe41322c7c1392521ebbbc2160bbc8e1d2e",
          "url": "https://github.com/purlis/purlis/commit/efb9dfd6b7a9dbb980224ee6bfb28a827d1dd23f"
        },
        "date": 1792362824933,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 891,
            "unit": "MB",
            "extra": "median of 3 samples: 823, 891, 980 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 325.6,
            "unit": "MB",
            "extra": "median of 6 samples: 251, 352.1, 347.4, 325.2, 326, 278.8 MB; window 152.2 MB, session host 173.4 MB"
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
          "id": "0221bf611ca897387a2bbc713b6b604df2822cf0",
          "message": "Theme: a window's first frame draws the theme the levels choose (#2631)\n\n* Theme: a window's first frame draws the theme the levels choose\n\nThe core resolves the theme levels once as a window is created, for the\nproject and workspace it opens on, and the creation script, the native\nwindow and the page's first frame take that one answer (#2602). The\nsystem's contrast is read natively at creation: Increase contrast on\nmacOS, the desktop portal on Linux; elsewhere the page picks among the\nframes it is handed for both (#2582). An appearance a theme does not\nhave draws its first for the system's contrast, and the page's shortcut\nto the person's own theme gives way to the levels with no project in\nfront.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* Theme: first-frame tests hold, and rustfmt\n\nThe floors test compares the theme and its floors, the remembered\nstore is a seam so no test writes machine.json, and the page test\nexpects the new first frame field.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* Theme: macOS contrast is not read natively yet\n\nThe read needs two objc2-app-kit features the tree does not build\ntoday, which is the operator's call. Until then macOS, like Windows,\nhands the page a frame for each contrast and the page picks with\nprefers-contrast.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* Theme: macOS reads Increase contrast as a window is created\n\nNSWorkspace.accessibilityDisplayShouldIncreaseContrast, with two more\nfeatures of objc2-app-kit (NSWorkspace, NSAccessibility); Cargo.lock is\nunchanged. The decision from the platform's answer is tested.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* Theme: a first frame draws a project's theme only once it is trusted\n\nA window's first frame draws a project's theme only once the project is\ntrusted on this machine: a remembered project is used only while the\nstore's approval of it holds for what it contributes now. Linux asks\nthe portal once per process, and more contrast is drawn when either\nthe portal or the page asks for it.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* Theme: the first frame's trust test reads the faces through the store\n\nA window created on a project whose approval in this machine's store\ndoes not hold its faces is handed none. The contrast test no longer asks\nthe real platform, so no portal is started by a test.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* Theme: name the approved store in the trust test plainly\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T02:16:08+04:00",
          "tree_id": "e94e156ad3e69157f370bbc5096aebc6d4c90b03",
          "url": "https://github.com/purlis/purlis/commit/0221bf611ca897387a2bbc713b6b604df2822cf0"
        },
        "date": 1792365906211,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 904,
            "unit": "MB",
            "extra": "median of 3 samples: 687, 904, 1099 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 381.1,
            "unit": "MB",
            "extra": "median of 6 samples: 388.8, 386.2, 374.4, 380.6, 381.5, 310.2 MB; window 199.4 MB, session host 181.7 MB"
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
          "id": "d44a455cd5381ceefde692f7ff27dc1aa05c0a77",
          "message": "Sandbox: a socket grant never names the system's name-lookup sockets (#2675)\n\n* Sandbox: a socket grant never names the system's name lookup (#2657)\n\nservices::socket refuses each socket of RESOLVER_SOCKETS, written whole\nor as a unix:// address, before it looks at what is there, in one\nsentence (NAME_LOOKUP). Every Allow is judged through it, so a block's\nNotice, the centralized Ask and Settings refuse it at every level, and\nthe one keep (kept_for) refuses it too. A kept one is never compiled in\n(still_sockets), and the Granted list marks it as reaching no chat.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Sandbox: the Mac's own name lookup is refused too (#2657)\n\nReview of #2675. The name lookup a grant never names is Linux's\n(RESOLVER_SOCKETS, which still sorts a refusal as a lookup) and the\nMac's own. services::socket and services::service refuse it, kept_for\nrefuses it, a start never compiles one in, the Granted list marks a\nkept one and Doctor counts it as held. A path is read with . and ..\ntaken lexically. NAME_LOOKUP_REFUSED says the rule, then why. Tests\nloop over every path and level, for this project and for every\nproject. ADR 0093 amended, to be confirmed.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Sandbox: every name of the Mac's lookup socket is refused (#2657)\n\nis_name_lookup compares every name a path goes by (as written, as the\nkernel names it, across a firmlink) and, where something is there, the\nfile itself. Tests cover the socket's names on the data volume for a\ngrant, a start and the Granted list.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T03:09:06+04:00",
          "tree_id": "f76612a1dfbee6ef686a9236293d063fdf8c9602",
          "url": "https://github.com/purlis/purlis/commit/d44a455cd5381ceefde692f7ff27dc1aa05c0a77"
        },
        "date": 1792368018793,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 926,
            "unit": "MB",
            "extra": "median of 3 samples: 735, 926, 1050 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 381.4,
            "unit": "MB",
            "extra": "median of 6 samples: 354.3, 389.6, 402, 381, 381.7, 287.4 MB; window 200.9 MB, session host 184.7 MB"
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
          "id": "5eded4f658ffb97ff0fdd6154d5589813b7fae3e",
          "message": "Browsers: idle stop and the memory budget (#2573)\n\n* Browsers: tests for a stuck browser retried off the call's path\n\nA browser call answers at once while a browser of the project is stuck,\nand a stuck browser being retried still counts toward the project's three.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Browsers: a stuck browser is retried off the call's path\n\nA browser purlis could not end yet is retried by one thread per project,\non a backoff from 1 s to 60 s, never inside a browser call. It moves from\nits chat to the ending list under both locks, so the project's count\nnever drops below the browsers still running. The window is told when\nthe retry ends one; Stop all wakes the retry at once.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Browsers: idle stop and the memory budget\n\nA chat's browser unused for ten minutes stops, and the chat's next call\nis told; one the person drives or that an Ask waits on never stops so.\nThe device runs at most a budget of browsers from its memory class\n(16 GB: 4, 32 GB: 8, 64 GB: 16), counted from the session host's own\nlist and checked one start at a time, beside the three per project.\nA refusal past either says which limit it met.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Memory pressure: its tests move to a file of their own\n\nMove only: the tests that were inline before #2023 go to\nmemorypressure/tests.rs unchanged. The test #2023 added comes back in\nthe next commit.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Machine store: its tests move to a file of their own\n\nMove only: the inline test block goes to machine/tests.rs unchanged,\nunder the same cfg attributes.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: the limits in Settings, and the idle stop follows the host\n\nThe person's ruling of 2026-10-18: browsers.per_project,\nbrowsers.on_this_device and browsers.idle_minutes in this machine's\nstore, set in Settings > This machine > Browsers and read at each\nbrowser call. A refusal past a limit names the setting.\n\nThe idle sweep never stops a browser the session host lists with a\ndriver, and stops nothing when the host cannot say. A login, and an Ask\nraised about a browser, count as a use. Test placement as the\ncontributing guide says.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: bindings for main's commands and the limits together\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: the palette's rows count the Browsers settings group\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T03:34:29+04:00",
          "tree_id": "a90575df1fd116c735be9a2d43c2c9b8d86dad39",
          "url": "https://github.com/purlis/purlis/commit/5eded4f658ffb97ff0fdd6154d5589813b7fae3e"
        },
        "date": 1792370223248,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 866,
            "unit": "MB",
            "extra": "median of 3 samples: 788, 866, 1062 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 352.5,
            "unit": "MB",
            "extra": "median of 6 samples: 355.2, 374.5, 371.2, 349, 349.8, 251 MB; window 152.3 MB, session host 201 MB"
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
          "id": "262caecea78dc489f89ccb192ddcf66d133d2858",
          "message": "Browsers: pin, hand over, and reopen after quit (#2586)\n\nThe person pins a chat's browser from its row's menu or the palette: it\noutlives its chat, listed under Pinned browsers, until they stop it, Stop\nall, or a quit. A pinned browser whose chat closed, or one the person\nopened with no chat, is handed to an open chat, which gets that chat's\nreach at once; one signed in with a login goes only to a chat that may\nlog in with that login now, and a failed hand-over gives the browser\nback the reach it had or ends it. As purlis quits it keeps where the\nproject's browsers were in app/browsers-at-quit.json, addresses only,\nand the next start offers them once.\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T04:37:13+04:00",
          "tree_id": "6f21763bb6e69cb331d2e4bf988064cfb2f3a1a9",
          "url": "https://github.com/purlis/purlis/commit/262caecea78dc489f89ccb192ddcf66d133d2858"
        },
        "date": 1792372558483,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 945,
            "unit": "MB",
            "extra": "median of 3 samples: 823, 945, 975 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 371.2,
            "unit": "MB",
            "extra": "median of 6 samples: 281.8, 391.9, 377.4, 370.8, 371.6, 240.4 MB; window 153 MB, session host 217.9 MB"
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
          "id": "0c16f80e506f6edb0c6feff8874cf89995feb086",
          "message": "A runner chat composed by its names is asked about a service allowed for it alone (#2673)\n\n* A runner chat composed by its names is asked about a service allowed for it alone\n\nSession protocol 1.26: a start by names that names its confinement may name the services the\nwindow allowed the chat alone (`alone`). The runner leaves out of the confinement it composes\neach one its own sandbox does not give, names them in `confined.left_out`, and asks the window\nabout each, as for a plan made elsewhere.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\nAssisted-by: claude-code:claude-opus-5-5\n\n* A composed plan may leave a service out of its proxy, and a test starts one\n\nPlan::starting no longer holds that only a plan made elsewhere leaves a service out: a plan\nthe host composed for a chat started by its names does too. The egress docs say both.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* The runner's policy binds what a composed chat is asked about\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T05:10:51+04:00",
          "tree_id": "2d49aa1d6bbd04406217132c7e15fef1480e6e4d",
          "url": "https://github.com/purlis/purlis/commit/0c16f80e506f6edb0c6feff8874cf89995feb086"
        },
        "date": 1792374982258,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 926,
            "unit": "MB",
            "extra": "median of 3 samples: 807, 926, 1031 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 345.9,
            "unit": "MB",
            "extra": "median of 6 samples: 245.2, 347.2, 346.1, 345.8, 346.6, 257 MB; window 152.2 MB, session host 193.1 MB"
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
          "id": "7ba7867a0db1d5cc584b7ce88549ab48d9ec65a3",
          "message": "Browsers: a profile keeps its browser logins sealed on this device (#2021) (#2654)\n\nA browser that performed a declared login keeps it, sealed under this\ndevice's key, once it ends; the next chat of that profile whose login is\nallowed starts signed in, held to the login's site from the first moment.\nThe store is bound to the login as declared and its generation, denied to\nchats, left out of backups, one writer at a time, and wiped with its\nprofile. Session protocol 1.27.\n\n\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T05:53:08+04:00",
          "tree_id": "eba756b48a6d3919fefc5b615522c9423549a7d3",
          "url": "https://github.com/purlis/purlis/commit/7ba7867a0db1d5cc584b7ce88549ab48d9ec65a3"
        },
        "date": 1792377445633,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 977,
            "unit": "MB",
            "extra": "median of 3 samples: 847, 977, 1056 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 318.2,
            "unit": "MB",
            "extra": "median of 6 samples: 299.9, 310.3, 326.1, 328.7, 329.5, 259.5 MB; window 154.7 MB, session host 165.2 MB"
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
          "id": "f454c0ea985727e1c369438c4263d00e13f765e6",
          "message": "Sandbox: a chat is judged sandboxed or not by the record of how its run started (#2688)\n\n* The session host records how each run is sandboxed\n\nThe host reads it from what it starts, never from the start: the program\nthat wraps it (Seatbelt, purlis's Linux wrap, Claude Code's own sandbox\nin its settings) and whether a confinement holds it. A confined run never\nreads as off. It is kept per program, on the host record's started line,\nand answered by programs_of (session protocol 1.28, sandbox).\n\nPart of #2636\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* WIP: the window judges a run by the record of how it started\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* A chat is judged by the record of how its run started\n\nThe window keeps how each run is sandboxed: its own record for a run it\nstarts, the session host's for one it reattaches. A browser login reads\nit. A run recorded as off is reattached with no sandbox of now standing\nin for it; a run with no record is taken as sandboxed and offered a\nrestart. ADR 0068 amendment, plane-format's host record row, and a\nchangelog fragment.\n\nCloses #2636\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* A runner chat started without the sandbox keeps its logins; only logins read the record\n\nA harness chat on a runner that the person started without the sandbox,\nwhich the runner composed with no confinement, is recorded off, so its\nbrowser logins are its persona's. Logins are the one path that reads the\nrecord of how a run started; the paths that grant a sandboxed chat more\nkeep the window's compiled record. A composed Claude Code line is read as\nits own sandbox.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T06:30:34+04:00",
          "tree_id": "6a3f62c5b47289e21c2301e42822fef812023ecd",
          "url": "https://github.com/purlis/purlis/commit/f454c0ea985727e1c369438c4263d00e13f765e6"
        },
        "date": 1792379733413,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 820,
            "unit": "MB",
            "extra": "median of 3 samples: 679, 820, 971 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 355.7,
            "unit": "MB",
            "extra": "median of 6 samples: 401.9, 399, 369.8, 340.8, 341.7, 243.3 MB; window 161.4 MB, session host 194.4 MB"
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
          "id": "a4d3afcfd392248b9d8eb42e6da2fc74bbed5257",
          "message": "Browsers: a browser the person opens reaches the project's sites and goes to an address they type (#2643)\n\n* Browsers: a browser the person opens reaches the project's sites and goes to an address they type\n\nA browser opened from the window starts with what the project's sandbox\nsettings list for a chat on no persona and the browser sites allowed\nhere, and its view tab has an address field. Only http and https\naddresses go; a site new to the project asks once, under the field.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Browsers: a person's browser may allow a site for everyone in the project\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Browsers: bindings, formatting and palette tests for a person's browser\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: the reconnecting view test names its project\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: the address tests get a loaded runner's budget\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: the address box is named where the views' native controls are\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: a person's browser asks in the sites ask's words, and goes after what they typed\n\nIts sites ask offers the registry's answers at the scopes policy leaves\nopen, and for everyone in the project only where the sandbox is on. A\npublic site on another port asks. The address goes through the view's\nqueue, handed back first where the person drives. Tests move to files of\ntheir own.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: bindings for the sites ask's words\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: a person's browser that cannot start says why in the device budget's words\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: an address kept at quit opens a browser there on the person's press\n\nThe quit Notice's buttons open a browser of the person's own, which goes\nto the address once its view is live, as a typed one does. A pinned\nbrowser past its chat has no address field.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: formatting after kept logins\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T06:58:34+04:00",
          "tree_id": "cd864a55bd8813b43aab72a318fd6948872b63dd",
          "url": "https://github.com/purlis/purlis/commit/a4d3afcfd392248b9d8eb42e6da2fc74bbed5257"
        },
        "date": 1792381608548,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 616,
            "unit": "MB",
            "extra": "median of 3 samples: 506, 616, 645 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 484,
            "unit": "MB",
            "extra": "median of 6 samples: 467.2, 479.6, 482.9, 485, 486, 832.9 MB; window 226.8 MB, session host 257.7 MB"
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
          "id": "cb31e95f40b6ebd89f76b20d4562c1e05e38889c",
          "message": "Browsers: a chat without the sandbox is given what a sandboxed chat of its persona lists (#2677)\n\n* Browsers: a chat without the sandbox is given what a sandboxed chat of its persona lists\n\nA chat started without the sandbox has no record of a compiled sandbox,\nso its browser's listing is compiled at each call from the project's\nsettings for its persona, never the floor's reach. Its sites ask as any\nchat's do, and its posture Ask says what each answer gives it.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\nAssisted-by: claude-code:claude-opus-5-5\n\n* Browsers: a chat without the sandbox is one with none compiled and none holding it\n\nA sandboxed chat reattached under a sandbox whose compile was not\nrecorded is given nothing listed, and its Ask is a sandboxed chat's.\nWhether a chat started without the sandbox is the chat's, not its\npersona's.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: a chat's sites ask offers everyone in the project only where the sandbox is on\n\nA committed browser site reaches a browser only where the project's\nsandbox is in force, so a chat's sites ask offers it only there, and an\nAllow at that level is refused elsewhere, as for the person's own\nbrowser.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T07:31:54+04:00",
          "tree_id": "957b790ddd50ff509b46bca1376baef954a5bcdd",
          "url": "https://github.com/purlis/purlis/commit/cb31e95f40b6ebd89f76b20d4562c1e05e38889c"
        },
        "date": 1792384217299,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 880,
            "unit": "MB",
            "extra": "median of 3 samples: 741, 880, 1045 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 339.2,
            "unit": "MB",
            "extra": "median of 6 samples: 342.7, 334.2, 344.6, 338.8, 339.6, 282.9 MB; window 163.9 MB, session host 175.5 MB"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "name": "Aaron Yordanyan",
            "username": "diazoxide",
            "email": "aaron.yor@gmail.com"
          },
          "committer": {
            "name": "GitHub",
            "username": "web-flow",
            "email": "noreply@github.com"
          },
          "id": "cb31e95f40b6ebd89f76b20d4562c1e05e38889c",
          "message": "Browsers: a chat without the sandbox is given what a sandboxed chat of its persona lists (#2677)\n\n* Browsers: a chat without the sandbox is given what a sandboxed chat of its persona lists\n\nA chat started without the sandbox has no record of a compiled sandbox,\nso its browser's listing is compiled at each call from the project's\nsettings for its persona, never the floor's reach. Its sites ask as any\nchat's do, and its posture Ask says what each answer gives it.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\nAssisted-by: claude-code:claude-opus-5-5\n\n* Browsers: a chat without the sandbox is one with none compiled and none holding it\n\nA sandboxed chat reattached under a sandbox whose compile was not\nrecorded is given nothing listed, and its Ask is a sandboxed chat's.\nWhether a chat started without the sandbox is the chat's, not its\npersona's.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: a chat's sites ask offers everyone in the project only where the sandbox is on\n\nA committed browser site reaches a browser only where the project's\nsandbox is in force, so a chat's sites ask offers it only there, and an\nAllow at that level is refused elsewhere, as for the person's own\nbrowser.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T03:31:54Z",
          "url": "https://github.com/purlis/purlis/commit/cb31e95f40b6ebd89f76b20d4562c1e05e38889c"
        },
        "date": 1792399661844,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 925,
            "unit": "MB",
            "extra": "median of 3 samples: 820, 925, 1013 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 324.2,
            "unit": "MB",
            "extra": "median of 6 samples: 285.3, 356.3, 348.5, 324.1, 324.3, 204 MB; window 153.2 MB, session host 174.7 MB"
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
          "id": "23855fecf4c5607c66f61363ef92e225af663d84",
          "message": "The theme check holds the remaining pairs and geometry across tokens (#2700)\n\nEvery syntax role is held on the editor's current line, and the window's\nsecondary, muted, accent and state words on a theme's selection while\nselected text is the platform's. A line of words has to fit the control\nor strip it is drawn in, in both densities: the check names the sizes\nof one that does not (`line-does-not-fit`). Each chart point is its own\nseries, the chart palette in turn, so every chart colour is drawn.\nPurlis Classic light's property and attribute colours are a shade\ndarker so they read on the current line.\n\nCloses #2522\n\n\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T21:46:47+04:00",
          "tree_id": "896380d54561775f0fe75f21502dbbf40c0ccf0a",
          "url": "https://github.com/purlis/purlis/commit/23855fecf4c5607c66f61363ef92e225af663d84"
        },
        "date": 1792435071768,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 997,
            "unit": "MB",
            "extra": "median of 3 samples: 852, 997, 1123 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 312.8,
            "unit": "MB",
            "extra": "median of 6 samples: 351.8, 303.2, 312.7, 312.9, 313.8, 262.4 MB; window 141.1 MB, session host 166.5 MB"
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
          "id": "6fb516d20de78e65177a29cda211aaf88d74ea82",
          "message": "Theme: every theme file is read through a checked handle; names every platform holds (#2704)\n\nEvery theme file is read from a handle on the folder that was opened\n(theme/opened.rs): a folder is opened from the one before it refusing a\nlink, and each file from that handle, a plain file under the cap.\n\n- purlis theme check <folder> opens the folder once (theme::Named) and\n  reads theme.json, its icon set, its fonts and the folders its extends\n  chain names from those handles (#2604).\n- The person's own theme.json, its icon set and fonts are read through a\n  handle on their config folder (#2604).\n- purlis theme export <level id> packs every file from the handle the\n  level reader found the folder through (Folders::folder_at,\n  FoundFolder::export); export by path packs from a handle too (#2670).\n- A theme folder's name and every name inside it hold none of\n  < > : \" / \\ | ? * nor a control character, beside the device-name\n  rule (contain::unspellable, portable::not_held), so import, export and\n  the folder reader refuse such a name in one sentence (#2671).\n\nCloses #2604\nCloses #2670\nCloses #2671\nPart of #2313\n\n\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T22:44:58+04:00",
          "tree_id": "0532d74562814e2258384395b4114666e6d31c26",
          "url": "https://github.com/purlis/purlis/commit/6fb516d20de78e65177a29cda211aaf88d74ea82"
        },
        "date": 1792437840141,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 1036,
            "unit": "MB",
            "extra": "median of 3 samples: 856, 1036, 1067 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 307.5,
            "unit": "MB",
            "extra": "median of 6 samples: 310.2, 295.1, 304.9, 310.4, 311.3, 183.6 MB; window 138.9 MB, session host 162.4 MB"
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
          "id": "840d889d8c9a38fc265c31eea166ed1c48af8e8e",
          "message": "The person sets a theme's option values per project and workspace (#2702)\n\n* Theme: a choice carries option values and names its folder's level\n\nA theme choice now holds the value of each of its theme's options the\nperson chose (#2527) and, for a theme folder, the level of the folder it\npicked (#2516). The theme engine builds the values onto the theme with\nTheme::with before drawing it, so the consent floors hold over every\nvalue, and a choice that names a level draws that level's folder and\nnever another of its name. Choices written before keep resolving as they\ndid and gain their level the next time they are written.\n\nSettings › Theme lists a card for each folder of one name, each with its\nlevel, and its options become choosable at You, Project and Workspace\n(theme_choose_option, window-only), each value badged with the level\nwhose choice made it.\n\nCloses #2527\nCloses #2516\nPart of #2313\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* Theme: regenerate the bindings and badge each option value with its level\n\nThe window's bindings carry the new command and the card's level. The\noptions list names the level whose choice made each value, and the\nstanding-lines guard counts the gallery's line for values an option's\npick dropped.\n\nPart of #2527\nPart of #2516\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* Theme: review follow-ups for option values and folder levels\n\n- choices::choose compares both choices as written, so an old choice\n  with no level re-chosen as another level's folder starts from its\n  defaults, and keeps only the values its theme still has.\n- Each option's radio group is named by the option alone and described\n  by its line, and has a Theme default entry that chooses no value.\n- A card's pictures are drawn with the option values chosen at the\n  level, as pointing at it draws it.\n\nPart of #2527\nPart of #2516\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T23:39:56+04:00",
          "tree_id": "2a9bfbbf51d05c00606698d8467939523195116a",
          "url": "https://github.com/purlis/purlis/commit/840d889d8c9a38fc265c31eea166ed1c48af8e8e"
        },
        "date": 1792441210926,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 1048,
            "unit": "MB",
            "extra": "median of 3 samples: 832, 1048, 1104 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 317.9,
            "unit": "MB",
            "extra": "median of 6 samples: 370, 302.8, 316.9, 319, 320, 260.5 MB; window 146.3 MB, session host 171.6 MB"
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
          "id": "f235d3c25fa16b2ae346f01f536d2f7dd6c4ee9a",
          "message": "The consent floors hold exactly under every workspace tint (#2703)\n\n* The consent floors hold exactly under every workspace tint\n\nA tinted shade rounds each channel toward the luminance it had, on its\nown side of WCAG's middle, so it keeps every contrast floor it kept\nuntinted at every hue. The accent-hued tokens drawn beside danger\n(focus ring, selected-row edge, accent surface, active tab) are held\napart from danger by the tint like the accent. The floors measure every\napart rule as written and per tint, measure the pairs a tint can move\nbetween hues at every whole degree, and draw a theme that falls under a\nfloor only in a tinted workspace as written with those tokens untinted.\nThe window does the same arithmetic, held by the shared tint fixture.\n\nCloses #2526\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* The tint's rounding compares as clippy asks, and the window's tolerances follow it\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* The floors sweep every pair with a see-through colour, and keep each degree's chroma\n\nA pair with a colour written see-through is mixed on screen channel by\nchannel, so the floors measure it at every whole degree when the recipe\nturns one of its tokens, and read each side's luminance as drawn. The\nsweep finds each whole degree's chroma limit once across the floors'\nrounds while the rules' colours are unchanged, and turns only the tokens\nof the swept pairs.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* A see-through ask on a tinted control is measured at every hue\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* The Chats list fixture draws persona marks in the tint's rounded colours\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-20T02:06:13+04:00",
          "tree_id": "10ff46f3f689670ecc3df34d31bebaf166445a89",
          "url": "https://github.com/purlis/purlis/commit/f235d3c25fa16b2ae346f01f536d2f7dd6c4ee9a"
        },
        "date": 1792450094884,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (footprint peak)",
            "value": 918,
            "unit": "MB",
            "extra": "median of 3 samples: 786, 918, 1046 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 361.4,
            "unit": "MB",
            "extra": "median of 6 samples: 314, 365.8, 368.8, 361, 361.9, 249.9 MB; window 153.1 MB, session host 208.3 MB"
          }
        ]
      }
    ],
    "stress (ubuntu-24.04)": [
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
        "date": 1791771767130,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1490.9,
            "unit": "MB",
            "extra": "median of 3 samples: 1306.6, 1490.9, 1655.7 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 419.8,
            "unit": "MB",
            "extra": "median of 6 samples: 385.2, 404.7, 415.1, 424.5, 425, 536.3 MB"
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
        "date": 1791777578098,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1482.9,
            "unit": "MB",
            "extra": "median of 3 samples: 1358.3, 1482.9, 1541.8 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 417.2,
            "unit": "MB",
            "extra": "median of 6 samples: 389.8, 405.4, 417.8, 416.9, 417.5, 532.7 MB"
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
        "date": 1791779279168,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1398.9,
            "unit": "MB",
            "extra": "median of 3 samples: 1283.3, 1398.9, 1672.8 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 427.8,
            "unit": "MB",
            "extra": "median of 6 samples: 386.1, 417.4, 432.5, 427.6, 427.9, 542.4 MB"
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
        "date": 1791780736815,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1448,
            "unit": "MB",
            "extra": "median of 3 samples: 1370.3, 1448, 1661.5 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 440,
            "unit": "MB",
            "extra": "median of 6 samples: 396.3, 420, 430.9, 449.1, 449.4, 545.3 MB"
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
        "date": 1791791144513,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1503.3,
            "unit": "MB",
            "extra": "median of 3 samples: 1334.7, 1503.3, 1610.1 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 418,
            "unit": "MB",
            "extra": "median of 6 samples: 392.7, 401.9, 414, 422, 422.6, 515.6 MB"
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
        "date": 1791793734364,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1485.9,
            "unit": "MB",
            "extra": "median of 3 samples: 1317.9, 1485.9, 1559.5 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 423.5,
            "unit": "MB",
            "extra": "median of 6 samples: 387.7, 411.4, 419.5, 427.4, 427.9, 503.6 MB"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "name": "Aaron Yordanyan",
            "username": "diazoxide",
            "email": "aaron.yor@gmail.com"
          },
          "committer": {
            "name": "Aaron Yordanyan",
            "username": "diazoxide",
            "email": "aaron.yor@gmail.com"
          },
          "id": "40f2ac86b9471cfb153825eda867a96f8b0bbac3",
          "message": "A Go tool's refused certificate check is read from a failed command\n\nA sandboxed chat's `glab api` failed with `x509: OSStatus -26276` and no\nNotice came up. A Go program refused the system's certificate check exits\nnon-zero, so its words reach the block hook only in PostToolUseFailure's\n`error`, which is read as standard output: a line counts only where it\nends as a refusal, and no ending matched the certificate check. The one\nexisting test fed the line as standard error of a command that succeeded,\na shape real Go CLIs never produce for this failure.\n\n`ends_as_a_refusal` now ends on `x509: OSStatus -26276`, so the existing\ncertificate-check block, and its Notice, comes up. A line that only\nquotes the words is still not a block. Nothing is granted.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-12T06:59:36Z",
          "url": "https://github.com/purlis/purlis/commit/40f2ac86b9471cfb153825eda867a96f8b0bbac3"
        },
        "date": 1791800589397,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1544.3,
            "unit": "MB",
            "extra": "median of 3 samples: 1337, 1557.8, 1544.3 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 422.8,
            "unit": "MB",
            "extra": "median of 6 samples: 394.7, 404.8, 410.8, 434.7, 435.1, 520.6 MB"
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
        "date": 1791803058333,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1420.1,
            "unit": "MB",
            "extra": "median of 3 samples: 1276.2, 1420.1, 1427.3 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 417.9,
            "unit": "MB",
            "extra": "median of 6 samples: 383.5, 397.7, 410.1, 425.6, 426.4, 517.4 MB"
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
        "date": 1791806354542,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1368.4,
            "unit": "MB",
            "extra": "median of 3 samples: 1225.1, 1368.4, 1472.4 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 409.1,
            "unit": "MB",
            "extra": "median of 6 samples: 383.7, 409.4, 414.7, 408.2, 408.8, 513 MB"
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
        "date": 1791810499200,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1406.6,
            "unit": "MB",
            "extra": "median of 3 samples: 1216.3, 1406.6, 1544.1 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 425.6,
            "unit": "MB",
            "extra": "median of 6 samples: 395.8, 414.6, 421, 430.2, 430.7, 533.4 MB"
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
        "date": 1791813983583,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1499.7,
            "unit": "MB",
            "extra": "median of 3 samples: 1286.7, 1499.7, 1622.2 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 429,
            "unit": "MB",
            "extra": "median of 6 samples: 389.4, 418.5, 424, 434, 433.9, 504.8 MB"
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
        "date": 1791816348640,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1445.3,
            "unit": "MB",
            "extra": "median of 3 samples: 1263.4, 1445.3, 1649.4 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 421.1,
            "unit": "MB",
            "extra": "median of 6 samples: 391.9, 411.4, 415, 427.3, 427.2, 517.7 MB"
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
        "date": 1791833974894,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1434.1,
            "unit": "MB",
            "extra": "median of 3 samples: 1247.5, 1434.1, 1515.7 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 454.9,
            "unit": "MB",
            "extra": "median of 6 samples: 406.9, 429.9, 445.1, 464.6, 465.1, 545.4 MB"
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
        "date": 1791843623481,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1363.3,
            "unit": "MB",
            "extra": "median of 3 samples: 1286.7, 1363.3, 1530.5 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 454.3,
            "unit": "MB",
            "extra": "median of 6 samples: 402.9, 422.2, 446.5, 462, 462.4, 542.6 MB"
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
        "date": 1791845986376,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1478.4,
            "unit": "MB",
            "extra": "median of 3 samples: 1344.4, 1478.4, 1575.8 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 445.4,
            "unit": "MB",
            "extra": "median of 6 samples: 416.6, 432.3, 437.5, 453.3, 453.8, 539.3 MB"
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
        "date": 1791848546735,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1407.2,
            "unit": "MB",
            "extra": "median of 3 samples: 1336.4, 1407.2, 1594 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 428.7,
            "unit": "MB",
            "extra": "median of 6 samples: 404.9, 415.3, 417.4, 439.9, 440.4, 513.1 MB"
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
        "date": 1791850885723,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1462.7,
            "unit": "MB",
            "extra": "median of 3 samples: 1310.1, 1462.7, 1530 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 452.1,
            "unit": "MB",
            "extra": "median of 6 samples: 396.4, 417.2, 450.8, 453.4, 453.9, 554.2 MB"
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
        "date": 1791855458399,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1498.2,
            "unit": "MB",
            "extra": "median of 3 samples: 1357.7, 1519.9, 1498.2 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 454.2,
            "unit": "MB",
            "extra": "median of 6 samples: 407.7, 430, 450.1, 458.3, 458.7, 533 MB"
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
        "date": 1791858204089,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1490.7,
            "unit": "MB",
            "extra": "median of 3 samples: 1334.8, 1490.7, 1528.8 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 435.7,
            "unit": "MB",
            "extra": "median of 6 samples: 401, 417.5, 427.2, 444.1, 444.8, 543.6 MB"
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
        "date": 1791862958544,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1456.1,
            "unit": "MB",
            "extra": "median of 3 samples: 1317.7, 1471.5, 1456.1 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 428.3,
            "unit": "MB",
            "extra": "median of 6 samples: 397.3, 415.5, 424.8, 431.8, 432.1, 525.8 MB"
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
        "date": 1791866653190,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1506.9,
            "unit": "MB",
            "extra": "median of 3 samples: 1338, 1506.9, 1560.3 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 434.5,
            "unit": "MB",
            "extra": "median of 6 samples: 404.8, 420.3, 429.2, 439.7, 440.4, 540.1 MB"
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
        "date": 1791882718614,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1407.6,
            "unit": "MB",
            "extra": "median of 3 samples: 1296.8, 1407.6, 1530.7 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 446.8,
            "unit": "MB",
            "extra": "median of 6 samples: 406.2, 419.8, 446, 447.5, 448.1, 549.2 MB"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple",
            "email": "eyehollow@gmail.com"
          },
          "committer": {
            "name": "Aaron Yordanyan",
            "username": "diazoxide",
            "email": "aaron.yor@gmail.com"
          },
          "id": "d7d7e7d62ced63b2b57d1b7c5d9848f410828f9d",
          "message": "sandbox: a docker run down a pipe names its socket, so its Notice offers Allow\n\nA sandboxed Claude Code chat ran `docker run … 2>&1 | tail`, the sandbox\nrefused Docker its socket, and the pipe ended with tail's status. Claude\nCode handed the result over as one that came back, with Docker's words on\nstandard output and no failure beside them. A socket was named only from a\nfailed command (ADR 0093, D-sock-5), so the block reached the app with no\nsocket and the Notice said \"The sandbox did not say which socket it\nrefused\", with Start without the sandbox as its only way out.\n\nNo harness says how each program of a line ended, so the line is the one\nfailure signal left in the payload: where one of its own programs is a\ncontainer engine's client (docker, docker-compose, podman, podman-compose,\nnerdctl), by name after the shell's plumbing, the socket its output says it\nwas refused is named, on either stream and in Codex's and opencode's one\nstring. A line that only reads (cat, grep, git show, curl) still names\nnone, and neither does a violation line in a one-string result. Only a\nsocket is named this way; the certificate check keeps to a failed command.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4AREQXXRGBAEWM0SZ0N0WBN\nPurlis-Persona: steward",
          "timestamp": "2026-10-13T07:24:31Z",
          "url": "https://github.com/purlis/purlis/commit/d7d7e7d62ced63b2b57d1b7c5d9848f410828f9d"
        },
        "date": 1791887385008,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1521.1,
            "unit": "MB",
            "extra": "median of 3 samples: 1332.1, 1525.9, 1521.1 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 433.6,
            "unit": "MB",
            "extra": "median of 6 samples: 399.8, 409.5, 426.4, 440.8, 441, 530.8 MB"
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
        "date": 1791896338966,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1429.8,
            "unit": "MB",
            "extra": "median of 3 samples: 1340.1, 1429.8, 1587.4 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 453,
            "unit": "MB",
            "extra": "median of 6 samples: 422.4, 447.1, 450, 455.9, 456.3, 542.2 MB"
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
        "date": 1791899025536,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1444.7,
            "unit": "MB",
            "extra": "median of 3 samples: 1348.2, 1455.4, 1444.7 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 423.2,
            "unit": "MB",
            "extra": "median of 6 samples: 406, 418.4, 418.6, 427.8, 428.4, 525.6 MB"
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
        "date": 1791901543548,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1493.6,
            "unit": "MB",
            "extra": "median of 3 samples: 1289.6, 1511.4, 1493.6 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 451.3,
            "unit": "MB",
            "extra": "median of 6 samples: 404.5, 432.1, 453.5, 451.1, 451.5, 542.6 MB"
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
        "date": 1791904128744,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1538.2,
            "unit": "MB",
            "extra": "median of 3 samples: 1364.1, 1538.2, 1645.9 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 434.9,
            "unit": "MB",
            "extra": "median of 6 samples: 404, 417, 422.3, 447.4, 447.8, 557.7 MB"
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
        "date": 1791906490860,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1410.2,
            "unit": "MB",
            "extra": "median of 3 samples: 1337.6, 1410.2, 1619.6 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 457.8,
            "unit": "MB",
            "extra": "median of 6 samples: 407.4, 439.7, 452, 463.6, 463.9, 542 MB"
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
        "date": 1791910637294,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1459.2,
            "unit": "MB",
            "extra": "median of 3 samples: 1361.3, 1459.2, 1595.1 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 448.2,
            "unit": "MB",
            "extra": "median of 6 samples: 411.8, 429.4, 437.5, 458.9, 459.5, 533.8 MB"
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
        "date": 1791913088890,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1399.2,
            "unit": "MB",
            "extra": "median of 3 samples: 1341.7, 1399.2, 1551.9 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 448.6,
            "unit": "MB",
            "extra": "median of 6 samples: 400.6, 427.7, 443, 454.3, 454.7, 536.3 MB"
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
        "date": 1791915359875,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1464.2,
            "unit": "MB",
            "extra": "median of 3 samples: 1327, 1464.2, 1575.4 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 437,
            "unit": "MB",
            "extra": "median of 6 samples: 402.2, 433.7, 436.2, 437.7, 437.8, 535.2 MB"
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
        "date": 1791916976549,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1441,
            "unit": "MB",
            "extra": "median of 3 samples: 1340.5, 1441, 1551.6 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 440.7,
            "unit": "MB",
            "extra": "median of 6 samples: 398.5, 420.6, 436.4, 445, 445.4, 536.4 MB"
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
        "date": 1791920920788,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1483.7,
            "unit": "MB",
            "extra": "median of 3 samples: 1323.4, 1483.7, 1567.8 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 445.6,
            "unit": "MB",
            "extra": "median of 6 samples: 411.1, 426.4, 441.1, 450.1, 450.6, 515.1 MB"
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
        "date": 1791922713642,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1530.5,
            "unit": "MB",
            "extra": "median of 3 samples: 1350.4, 1530.5, 1573.1 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 442.5,
            "unit": "MB",
            "extra": "median of 6 samples: 402.7, 420.2, 438.6, 446.4, 447.2, 542.3 MB"
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
        "date": 1791924931671,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1465.2,
            "unit": "MB",
            "extra": "median of 3 samples: 1291.5, 1465.2, 1665.4 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 446.6,
            "unit": "MB",
            "extra": "median of 6 samples: 401.8, 427.2, 433.7, 459.5, 459.5, 550.3 MB"
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
        "date": 1791927317239,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1551.7,
            "unit": "MB",
            "extra": "median of 3 samples: 1352.7, 1551.7, 1622.1 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 441,
            "unit": "MB",
            "extra": "median of 6 samples: 403.7, 423.1, 436.1, 446, 446.4, 546.1 MB"
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
        "date": 1791929813531,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1457.4,
            "unit": "MB",
            "extra": "median of 3 samples: 1297.5, 1457.4, 1566.6 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 432.9,
            "unit": "MB",
            "extra": "median of 6 samples: 408, 416.3, 429, 436.7, 437.4, 544.6 MB"
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
        "date": 1791932298813,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1545.5,
            "unit": "MB",
            "extra": "median of 3 samples: 1356.8, 1545.5, 1578 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 440.2,
            "unit": "MB",
            "extra": "median of 6 samples: 402.3, 426.1, 437.2, 443.2, 443.7, 519.3 MB"
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
        "date": 1791935042652,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1518.9,
            "unit": "MB",
            "extra": "median of 3 samples: 1371.5, 1518.9, 1621.1 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 437.6,
            "unit": "MB",
            "extra": "median of 6 samples: 399.8, 415, 432.9, 442.3, 442.7, 532.6 MB"
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
        "date": 1791937221676,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1456.7,
            "unit": "MB",
            "extra": "median of 3 samples: 1354.7, 1456.7, 1663.9 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 440.3,
            "unit": "MB",
            "extra": "median of 6 samples: 396.1, 421.6, 445.2, 440.1, 440.4, 543 MB"
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
        "date": 1791939158100,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1490.8,
            "unit": "MB",
            "extra": "median of 3 samples: 1328.4, 1499.4, 1490.8 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 439.6,
            "unit": "MB",
            "extra": "median of 6 samples: 401.7, 402.9, 437, 442.3, 442.7, 528 MB"
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
        "date": 1791941354585,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1548.7,
            "unit": "MB",
            "extra": "median of 3 samples: 1404.9, 1548.7, 1652.4 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 445.9,
            "unit": "MB",
            "extra": "median of 6 samples: 411, 422.9, 443.5, 448.4, 448.7, 535.3 MB"
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
        "date": 1791942924035,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1510.4,
            "unit": "MB",
            "extra": "median of 3 samples: 1326.5, 1510.4, 1667.3 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 449.5,
            "unit": "MB",
            "extra": "median of 6 samples: 406.8, 423.8, 442.5, 456.7, 456.5, 535.3 MB"
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
        "date": 1791945326456,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1514.6,
            "unit": "MB",
            "extra": "median of 3 samples: 1321.1, 1514.6, 1570.9 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 432.3,
            "unit": "MB",
            "extra": "median of 6 samples: 409.2, 418.6, 431.6, 432.9, 433.4, 532.3 MB"
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
        "date": 1791947519946,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1461.1,
            "unit": "MB",
            "extra": "median of 3 samples: 1298.4, 1461.1, 1573.8 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 435.9,
            "unit": "MB",
            "extra": "median of 6 samples: 409.3, 425.3, 432.1, 439.7, 440.1, 508.6 MB"
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
        "date": 1791950098990,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1515.4,
            "unit": "MB",
            "extra": "median of 3 samples: 1337.8, 1515.4, 1517.9 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 444.3,
            "unit": "MB",
            "extra": "median of 6 samples: 409.1, 426.1, 437.9, 450.7, 451, 528.4 MB"
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
        "date": 1791966956377,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1457.5,
            "unit": "MB",
            "extra": "median of 3 samples: 1303.3, 1457.5, 1546.3 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 435.8,
            "unit": "MB",
            "extra": "median of 6 samples: 409.4, 420.3, 433, 438.7, 439.1, 531 MB"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "name": "Aaron Yordanyan",
            "username": "dbrownsimple",
            "email": "eyehollow@gmail.com"
          },
          "committer": {
            "name": "Aaron Yordanyan",
            "username": "diazoxide",
            "email": "aaron.yor@gmail.com"
          },
          "id": "9b7cc465b3e2833aacc57154b509a9b6e626ea1e",
          "message": "docs(secrets): a tool's example reads its values itself; curl expands its own\n\nThe several-values example is now `aws`, which reads AWS_ACCESS_KEY_ID and\nAWS_SECRET_ACCESS_KEY from its environment. curl reads no credential from\na variable, and a chat's arguments reach a tool with no shell between, so\nthe secrets guide shows curl's own expansion instead (curl 8.3.0 or\nlater): both values as env, then\n`curl --variable %KIBANA_USER --variable %KIBANA_PASS\n--expand-user '{{KIBANA_USER}}:{{KIBANA_PASS}}' <url>`.\nThe guide's first `secret exec ... curl` line is written the same way.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward",
          "timestamp": "2026-10-14T07:36:14Z",
          "url": "https://github.com/purlis/purlis/commit/9b7cc465b3e2833aacc57154b509a9b6e626ea1e"
        },
        "date": 1791967116523,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1487.2,
            "unit": "MB",
            "extra": "median of 3 samples: 1336.9, 1487.2, 1513.3 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 453,
            "unit": "MB",
            "extra": "median of 6 samples: 408.4, 431.4, 442, 463.9, 464.6, 563.2 MB"
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
        "date": 1791970089305,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1496.3,
            "unit": "MB",
            "extra": "median of 3 samples: 1287.2, 1496.3, 1529.9 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 459.1,
            "unit": "MB",
            "extra": "median of 6 samples: 417.7, 440.1, 455.7, 462.4, 462.7, 535.5 MB"
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
        "date": 1791975476907,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1466.7,
            "unit": "MB",
            "extra": "median of 3 samples: 1264.5, 1466.7, 1561.8 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 446.8,
            "unit": "MB",
            "extra": "median of 6 samples: 406.9, 430.2, 439.7, 453.9, 454.5, 536.9 MB"
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
        "date": 1791980952340,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1417.2,
            "unit": "MB",
            "extra": "median of 3 samples: 1256.9, 1417.2, 1590.4 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 448.8,
            "unit": "MB",
            "extra": "median of 6 samples: 408.7, 427.3, 441.7, 456.1, 456, 543.2 MB"
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
        "date": 1791984660243,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1421,
            "unit": "MB",
            "extra": "median of 3 samples: 1296, 1421, 1529.7 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 431.4,
            "unit": "MB",
            "extra": "median of 6 samples: 408.6, 417.2, 423.4, 439.3, 440, 527.1 MB"
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
        "date": 1791986799657,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1448.7,
            "unit": "MB",
            "extra": "median of 3 samples: 1308.2, 1448.7, 1458.6 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 447.3,
            "unit": "MB",
            "extra": "median of 6 samples: 398.6, 423.3, 438.5, 456.1, 456.6, 581.5 MB"
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
        "date": 1791991218431,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1370.9,
            "unit": "MB",
            "extra": "median of 3 samples: 1320.7, 1370.9, 1538.7 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 437.8,
            "unit": "MB",
            "extra": "median of 6 samples: 408.5, 427.9, 431, 444.6, 445, 538.5 MB"
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
        "date": 1792006884436,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1379.1,
            "unit": "MB",
            "extra": "median of 3 samples: 1264.9, 1379.1, 1498.2 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 437.8,
            "unit": "MB",
            "extra": "median of 6 samples: 410.4, 427.6, 429.8, 445.8, 446.3, 520.1 MB"
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
        "date": 1792009545556,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1415.8,
            "unit": "MB",
            "extra": "median of 3 samples: 1308.8, 1415.8, 1530.1 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 452.9,
            "unit": "MB",
            "extra": "median of 6 samples: 408.1, 440, 448.5, 457.2, 457.8, 552.3 MB"
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
        "date": 1792013756287,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1492.1,
            "unit": "MB",
            "extra": "median of 3 samples: 1279.5, 1492.1, 1618 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 437.9,
            "unit": "MB",
            "extra": "median of 6 samples: 411.4, 427.4, 434.8, 441, 441.3, 531.9 MB"
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
        "date": 1792016126343,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1462.9,
            "unit": "MB",
            "extra": "median of 3 samples: 1297.3, 1462.9, 1572.6 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 446.7,
            "unit": "MB",
            "extra": "median of 6 samples: 409, 420.1, 442, 451.4, 452, 557.9 MB"
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
        "date": 1792018465397,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1459.2,
            "unit": "MB",
            "extra": "median of 3 samples: 1249.3, 1459.2, 1510.8 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 439.9,
            "unit": "MB",
            "extra": "median of 6 samples: 403.8, 421.8, 438.2, 441.5, 442.1, 529.7 MB"
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
        "date": 1792020924875,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1429.9,
            "unit": "MB",
            "extra": "median of 3 samples: 1323.3, 1429.9, 1498 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 439.2,
            "unit": "MB",
            "extra": "median of 6 samples: 411.6, 419.9, 429.5, 449, 449.2, 554.1 MB"
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
        "date": 1792025268352,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1396.6,
            "unit": "MB",
            "extra": "median of 3 samples: 1291.2, 1396.6, 1476.6 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 455.7,
            "unit": "MB",
            "extra": "median of 6 samples: 423.3, 440.4, 451.9, 459.6, 460.1, 592.8 MB"
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
        "date": 1792027257490,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1431.5,
            "unit": "MB",
            "extra": "median of 3 samples: 1238.1, 1431.5, 1487.7 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 441.9,
            "unit": "MB",
            "extra": "median of 6 samples: 406.6, 434.6, 441.2, 442.6, 442.9, 545.8 MB"
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
        "date": 1792029484940,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1499.5,
            "unit": "MB",
            "extra": "median of 3 samples: 1322.2, 1499.5, 1596.1 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 455.3,
            "unit": "MB",
            "extra": "median of 6 samples: 412.1, 430.6, 449.8, 460.8, 461.2, 530.1 MB"
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
        "date": 1792033456697,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1433.1,
            "unit": "MB",
            "extra": "median of 3 samples: 1255.7, 1433.1, 1508.7 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 445,
            "unit": "MB",
            "extra": "median of 6 samples: 410.7, 424.8, 442.6, 447.5, 447.8, 518 MB"
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
        "date": 1792036184959,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1408.5,
            "unit": "MB",
            "extra": "median of 3 samples: 1248, 1408.5, 1568.4 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 440.6,
            "unit": "MB",
            "extra": "median of 6 samples: 415.7, 427.7, 440.2, 441, 441.5, 530.9 MB"
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
        "date": 1792040679421,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1485.1,
            "unit": "MB",
            "extra": "median of 3 samples: 1344.5, 1485.1, 1610.5 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 446.2,
            "unit": "MB",
            "extra": "median of 6 samples: 417, 432.3, 441.1, 451.3, 451.5, 532.6 MB"
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
        "date": 1792043134348,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1401.9,
            "unit": "MB",
            "extra": "median of 3 samples: 1306, 1401.9, 1640.6 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 445.7,
            "unit": "MB",
            "extra": "median of 6 samples: 408.7, 429.9, 442.6, 448.8, 449.2, 529.6 MB"
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
        "date": 1792045573574,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1475.2,
            "unit": "MB",
            "extra": "median of 3 samples: 1278.4, 1475.2, 1584.8 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 450.8,
            "unit": "MB",
            "extra": "median of 6 samples: 402.6, 431, 448.6, 453.2, 453.1, 540.1 MB"
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
        "date": 1792048235612,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1488.9,
            "unit": "MB",
            "extra": "median of 3 samples: 1296.9, 1492.6, 1488.9 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 440.5,
            "unit": "MB",
            "extra": "median of 6 samples: 407.5, 420.7, 436.8, 444.3, 445, 552.7 MB"
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
        "date": 1792051397365,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1410.1,
            "unit": "MB",
            "extra": "median of 3 samples: 1306.9, 1410.1, 1525.1 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 439.9,
            "unit": "MB",
            "extra": "median of 6 samples: 416.4, 429.1, 440, 439.9, 440.4, 588.5 MB"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "name": "Aaron Yordanyan",
            "username": "diazoxide",
            "email": "aaron.yor@gmail.com"
          },
          "committer": {
            "name": "GitHub",
            "username": "web-flow",
            "email": "noreply@github.com"
          },
          "id": "ba10b852611f6a9d3e4dd30e203ae9da8f327edb",
          "message": "Profiles: create, import and remove profiles (#2096)\n\n* Profiles: create, import and remove profiles (#2034)\n\nA profile of this machine's is made in Settings > You > Profiles or with\npurlis profile create, kept in purlis's data home and available in every\nproject. Its folder is 0700, its settings are imported from the machine's\nharness settings by a list of settings files per harness and never a\nsign-in, and it records when and offers Import again (purlis profile\nimport). A chat on it runs its harness with the declared config_env and\ndata_env set to its folder. Remove names the conversations that go and\nasks first (--yes on the command line); it deletes only a folder directly\nunder the store, and an adopted harness folder is only forgotten.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Profiles: the window's commands typed, the group in the catalogue\n\nThe generated bindings and UI RPC client carry the five machine_profile\ncommands; You > Profiles is in the Settings catalogue, the palette and\nthe guards' lists, its row buttons wear ui-button, and its Remove\nquestion has a way in for the dialog guard.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2034): a stand-in harness on the start test's PATH, no old file name in the window\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2034): the records are #2033's file and shape\n\nThis machine's profiles are now recorded in <config>/profiles.json, the\nfile #2033's adoption writes: version 1, and each record its name,\nharness, title and folders, with adopted for a folder adopted in place\nand imported for one purlis made. A record or key this purlis cannot\nread is kept as written. A made profile's folder stays under\n<data>/profiles/<name>/, and an adopted record starts as its built-in\ndid.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2096): an import copies no link and no file with another name; chats are denied the profiles\n\n- An import never follows a link inside the settings folder, and never\n  copies a file that has another name on the disk; a file is opened\n  without following a link and judged by what was opened.\n- A chat's sandbox denies <data>/profiles/, reading and writing, with the\n  person's other powers; the records are in the config home it may not\n  write. plane-format.md says so for both.\n- control-plane.md: Profiles of this machine is a section of its own.\n- The import's words say the settings files are copied as they are.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2096): made profiles are records of #2033's store\n\nThe profiles purlis makes are now MachineProfile records with an\nimported field, changed through profilestore::update under its one\nlock; the duplicate store, its reader and its lock are gone. Making,\nimporting and removing live in profilestore::made, which reads where a\nharness keeps its settings from the store's table of default folders.\nRemoving an adopted profile forgets its folder, so the next launch does\nnot adopt it again. plane-format.md has one row for the store.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2096): the git hooks test asks that no denied read covers the hooks, and the merged tab test is formatted\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2096): an import reads the settings folder by descriptor; one data-home ladder\n\n- The import opens the settings folder once and each name in it, and in\n  each listed folder, relative to the descriptor of the folder it was\n  listed in, with O_NOFOLLOW and O_NONBLOCK, judged by fstat. A test\n  swaps a listed folder for a link while imports run.\n- A chat's sandbox finds the data home by the store's own ladder, so a\n  relative $PURLIS_DATA_HOME names one folder for both.\n- The profiles deny test covers opencode's wrap too.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* review(2096): the start test's stand-in harnesses answer their version, as a sandboxed start asks\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-15T07:27:07Z",
          "url": "https://github.com/purlis/purlis/commit/ba10b852611f6a9d3e4dd30e203ae9da8f327edb"
        },
        "date": 1792053218506,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1469.9,
            "unit": "MB",
            "extra": "median of 3 samples: 1264.4, 1469.9, 1493.7 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 449.8,
            "unit": "MB",
            "extra": "median of 6 samples: 416.2, 431.4, 431.5, 468, 468.4, 585.4 MB"
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
        "date": 1792054514379,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1392.3,
            "unit": "MB",
            "extra": "median of 3 samples: 1246.5, 1392.3, 1499.9 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 469.7,
            "unit": "MB",
            "extra": "median of 6 samples: 446.3, 459.7, 466.9, 472.6, 473.3, 606.5 MB"
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
        "date": 1792057762015,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1417.5,
            "unit": "MB",
            "extra": "median of 3 samples: 1276.4, 1417.5, 1490.8 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 440.2,
            "unit": "MB",
            "extra": "median of 6 samples: 412.1, 427.2, 432, 448.3, 448.7, 593.6 MB"
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
          "id": "bfff74f567cb7fe921c218984a85ea7e33908434",
          "message": "Bound how many secret execs the window's relay tracks per link (#2571) (#2576)\n\n* Bound how many secret execs the window's relay tracks per link (#2571)\n\nThe window's Heeding bounded the bytes it keeps for secret execs not\nbegun, but not how many entries a link tracks: an end of input for a\nnumber that never runs, an exec past the byte cap or a drop mark stayed\nuntil the host's end or the link's end. A link now tracks at most 256\nexecs; a line that would add one past that keeps nothing, and an exec\nthat begins then reads no input. A tracked exec is never evicted.\n\nThe bytes kept are a running total instead of a sum over every entry\non each line, and each cap is warned of once per link, never with the\ninput's content.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M538BFC8ZDFGABYNBRP8913C\nPurlis-Persona: steward\n\n* Track a secret exec from when its line is taken, bounded per chat (#2571)\n\nReview of #2576: the per-link cap was shared by every chat on the link,\nand input lines carry no chat, so one chat holding many execs open\ncould fill it; another chat's exec then lost its input, or its end, and\nwaited until the link ended.\n\nThe window's link now takes each line in order before a thread of its\nown hears it (Heeding::admit, called from Relayed::take): a secret exec\nis tracked from there, with its chat, so input that comes before it\nbegins always finds its entry and an end is never refused. A line for a\nnumber not tracked is of no exec here and is dropped, so no entry is\nmade from a line that carries no chat, and no drop marks are needed.\nAn exec is untracked when it ends. A chat has at most 32 tracked on a\nlink and a link 256; one past either is not run, and its host is told\nit ended.\n\nEvery entry goes in through Inputs::track and out through\nInputs::untrack, which keep the byte total and each chat's count;\nthe subtraction is debug-asserted. The exec's end goes through\nInputs::finished.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M538BFC8ZDFGABYNBRP8913C\nPurlis-Persona: steward\n\n* Hear lines that waited for the board first; end a panicked exec; say why one is refused (#2571)\n\nReview of #2576:\n\n- Relayed::keep_with gave up the waiting lines' lock before hearing\n  them, so a line the host handed on meanwhile was heard first: a\n  secret exec's input could find its exec not yet taken and be dropped,\n  and the exec waited for good. They are now heard with the lock held,\n  through Relayed::hand_on, the path that skips the waiting list.\n- A secret exec's run that panics skipped its end: it stayed tracked\n  and its host was never told. A drop guard (Finishing) untracks it and\n  tells the host it ended however the run returns.\n- An exec not run for want of room now tells its asker why, in a\n  refusal frame, before it ends.\n- One chat may have 16 secret execs tracked on a link, not 32, so\n  filling a link's 256 takes 16 chats.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M538BFC8ZDFGABYNBRP8913C\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-18T12:30:16+04:00",
          "tree_id": "ac4cc7a82f91f40b30719298d6fcb3e808c8e70a",
          "url": "https://github.com/purlis/purlis/commit/bfff74f567cb7fe921c218984a85ea7e33908434"
        },
        "date": 1792315062742,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1456.3,
            "unit": "MB",
            "extra": "median of 3 samples: 1355.3, 1456.3, 1487.7 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 392.5,
            "unit": "MB",
            "extra": "median of 6 samples: 385, 391.7, 390.7, 393.4, 394.1, 412 MB"
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
          "id": "9c6e6872b81f085a642fa1632cc1501ecc4d28be",
          "message": "Linux wrap: starts with a fixed environment purlis builds (#2548)\n\n* Linux wrap: starts with a fixed environment purlis builds\n\npurlis's own wrap (purlis _sandbox) and the chat's side it starts begin\nwith an environment purlis builds. The program's environment is carried\nin the wrap's own under names of the wrap's own (purlis_linux_sandbox::\ncarried), read back whole and given to the program alone. A wrap whose\nown environment is anything else starts nothing.\n\nA chat's terminal (chatspec::spec, in the window and in a session host)\nand a brokered secret exec start the wrap this way. The teller reads the\nchat's hook settings from the carried environment.\n\nCloses #2460\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Carry what a profile hands a chat past the Linux wrap\n\nThe window sets a profile's handed variables after the chat's spec is\nbuilt. chatspec::with_handed sets them last in the program's environment\nand carries them with the rest when the chat runs on purlis's own wrap.\ncarried::read builds on every system.\n\nRefs #2460\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Let the terminal's SHELL be beside what the Linux wrap carries\n\nThe pseudo-terminal sets SHELL on every program it starts, as the session\nsets TERM. The wrap lets both be and hands neither on; a terminal test\npins what a session sets.\n\nRefs #2460\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Sort the Linux wrap's refusals by the project and temp folder it carries\n\nThe supervisor starts with none of the chat's environment, so it sorts\neach refusal against the project the carried PURLIS_ROOT names (held to\nthe fence as a resolve is; the walk from where the wrap started only\nwithout one) and the carried TMPDIR. sorter_in is the seam the teller\ntells through. with_handed decides by the chat's line, as spec does.\n\nRefs #2460\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Clippy: the chat's temp folder is already a reference\n\nRefs #2460\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-18T19:57:59+04:00",
          "tree_id": "0dd88d39086e2fd9cd16470cc9982e0c6ce6470d",
          "url": "https://github.com/purlis/purlis/commit/9c6e6872b81f085a642fa1632cc1501ecc4d28be"
        },
        "date": 1792341788050,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1464.7,
            "unit": "MB",
            "extra": "median of 3 samples: 1292.2, 1464.7, 1638.4 MB"
          },
          {
            "name": "M3 native side with no chats (resident)",
            "value": 380.2,
            "unit": "MB",
            "extra": "median of 6 samples: 372.5, 374.1, 379.4, 380.9, 381.3, 396.8 MB"
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
          "id": "23d8a6fd941adb5d4de61fdefca98777c431f793",
          "message": "A theme controls workspace tint (#2509)\n\nA theme's `tint` key is read (#2329): on or off, which tokens a\nworkspace's colour turns (from a closed list), and its own hues for the\neight workspace colours. The engine resolves the recipe on every\nappearance, the check and the consent floors measure every hue of it,\nand the window takes the recipe with the resolved theme and keeps only\nthe arithmetic (#2393). A tinted accent is held apart from the danger\ncolour: the tint keeps the hue and gives less colour, and draws untinted\nwhen no amount does. A token whose tint still breaks a consent floor is\ndrawn untinted, and the Notice says so.\n\n\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\nAssisted-by: claude-code\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-18T20:42:05+04:00",
          "tree_id": "a78856afc7d1be6833ad08dfae9b6e9add228f4c",
          "url": "https://github.com/purlis/purlis/commit/23d8a6fd941adb5d4de61fdefca98777c431f793"
        },
        "date": 1792344795676,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1462,
            "unit": "MB",
            "extra": "median of 3 samples: 1340.4, 1462, 1650.3 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 677.2,
            "unit": "MB",
            "extra": "median of 6 samples: 630.1, 660.5, 671, 683.3, 683.8, 765.5 MB; window 396 MB, session host 281.1 MB"
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
          "id": "9b8f84e74d43f5c1ea8e469e7d1202f549eb60d8",
          "message": "Runners: after a re-link the window takes back its own runner chats' sandboxes (#2593)\n\n* Runners: after a re-link the window takes back its own runner chats' sandboxes (#2511)\n\nSession protocol 1.19: take_back, the desktop relay's own word on each\nlink it makes to a runner for one window, names that window; the runner\nanswers with the confinements of the chats that window started there,\nmade or composed for it, and names them to the new link. It never names\none the runner's own window made, or one made or composed for another\nwindow's link. A chat started on either of two overlapping links of one\nwindow is named to both.\n\nFD-27's grants table gains a take_back row (remote-link only) and its row\ntest; ADR 0078 and ADR 0068 amended with the person's ruling of\n2026-10-17.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Runners: take_back is session protocol 1.22; the ADRs mark what is to be confirmed (#2511)\n\n1.19 was reserved and released: 1.21 landed first. The version notes and\nthe fixture keep protocol order. The ADR amendments say only the ruling\nis ruled, and the changelog fragment says what the host does.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-18T21:26:07+04:00",
          "tree_id": "c2d8f2bd8dace064f3605452258825be92344865",
          "url": "https://github.com/purlis/purlis/commit/9b8f84e74d43f5c1ea8e469e7d1202f549eb60d8"
        },
        "date": 1792347874886,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1538,
            "unit": "MB",
            "extra": "median of 3 samples: 1363.5, 1557, 1538 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 640.5,
            "unit": "MB",
            "extra": "median of 6 samples: 616.4, 632.4, 639.5, 641.6, 641.8, 734.8 MB; window 382.1 MB, session host 258.4 MB"
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
          "id": "0c95fe5420c56d2e3cf037a3f0228292bd964a76",
          "message": "fix(sandbox): give a run's named link back by its own name (#2617) (#2632)\n\n* fix(sandbox): give a run's named link back by its own name (#2617)\n\nOn macOS a vault tool declared under another name than its program's file\n(#1977) failed to start through the run's sandbox: `sandbox-exec` could not\nexec the run's named link (\"Operation not permitted\").\n\nCause: `profile_on` gave each of the run's files back under\n`seatbelt::kernel_strings`, which resolves the path whole. For the named link\nthat resolved to the program it points at, so the allow named the program and\nthe link itself stayed under the vaults denial. The run's files are now given\nback by their own name, the folder resolved and the last part never followed\n(`sandbox::kernel_names_itself`), under both firmlink names as before (#2489).\nA credential file is a plain file, so its grant is unchanged; a link is\ngranted as itself and nothing it points at is.\n\nThe macOS test also declared `zcat`, which macOS's gzip treats as\n`compress`'s zcat (it reads only `<file>.Z`); it now declares `gzcat`.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* style: rustfmt the named-link test (#2617)\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n* docs: plainer changelog wording (#2617)\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n\n---------\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-18T22:24:09+04:00",
          "tree_id": "67d88b07311f9351408b1c55483831f07a6e4520",
          "url": "https://github.com/purlis/purlis/commit/0c95fe5420c56d2e3cf037a3f0228292bd964a76"
        },
        "date": 1792350849580,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1401.1,
            "unit": "MB",
            "extra": "median of 3 samples: 1289.6, 1401.1, 1562.5 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 635.2,
            "unit": "MB",
            "extra": "median of 6 samples: 602.1, 622.4, 631.1, 639.3, 639.5, 738.6 MB; window 391.8 MB, session host 243.4 MB"
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
          "id": "813ab9a8fe4805a34a76b7611e2a620c4362f511",
          "message": "Skip the floor compile's second covering pass when the harness adds no denials (#2570) (#2635)\n\nFor Claude Code, Form::denied() is None, so the second covering pass\nre-checked exactly the list the first pass had just checked against the\nsame ground. It is now skipped there; Codex and opencode, whose compilers\nadd denials, are still held to the ground a second time. A test pins the\ncompiled profile, and every refusal, byte-identical to the two-pass path.\n\n\nAssisted-by: claude-code\nPurlis-Chat: 01M538BFC8ZDFGABYNBRP8913C\nPurlis-Persona: steward\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-18T23:11:17+04:00",
          "tree_id": "9865547fe79974d1b8d864fc34b63e0a3a84bd59",
          "url": "https://github.com/purlis/purlis/commit/813ab9a8fe4805a34a76b7611e2a620c4362f511"
        },
        "date": 1792353757038,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1447.1,
            "unit": "MB",
            "extra": "median of 3 samples: 1278.7, 1447.1, 1493.3 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 630.6,
            "unit": "MB",
            "extra": "median of 6 samples: 592.5, 618.7, 626.7, 634.5, 634.8, 711 MB; window 390.4 MB, session host 239.2 MB"
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
          "id": "77f672e0a701a018e9bcc3e4b35e1e556cfd27de",
          "message": "A theme folder's fonts are read through its checked handle (#2603)\n\n* A theme folder's fonts are read through its checked handle\n\nA face is opened from the handle of the theme folder's fonts/, which is\nopened from the handle its theme.json was read through, each open\nrefusing a link; serving a face reads it from the same handles. A\nproject's and a workspace's theme folders bring their fonts only through\nFolders::opened, for a project the window opened through the trust ask\n(ADR 0035); the person's own folders always do.\n\nCloses #2555\nPart of #2313\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* A project's font files are approved with the project\n\nEach font file in a theme folder at the plane root or in a workspace is\npart of what the project contributes, by its path and sha-256, so the\ntrust ask lists it and a new or changed one asks again. The theme engine\nreads this machine's approval of the project itself and draws a face\nfrom the project only when the approval holds its file with that digest.\nA folder named to the library brings its fonts only when asked to.\nRecorded as an amendment of ADR 0098 (operator's ruling, 2026-10-18).\n\nPart of #2313\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* A project's font files are listed under one budget\n\nA project's font files are listed under one budget for the whole plane:\nat most 64 files and 32 MiB, 24 in one theme folder, the theme folders of\n64 workspaces, 256 folders and 256 files opened. A file past it is not\nlisted, so it is never approved or drawn, and the complaint names the\nlimit. A vouch after a chat's record write reads the record's half alone.\nThe trust ask groups font files by theme folder with their size and a\nshort digest, and counts those past the first twelve.\n\nPart of #2313\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* Every trust-ask fixture says what font files a project brings\n\nThe two fixtures that described a project's contribution without its\nfont files are typed as the contribution now, so the ask they draw has\nevery field and a fixture that leaves one out does not type-check.\n\nPart of #2313\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-18T23:50:35+04:00",
          "tree_id": "085cc6c0cbc915f719d0973b25df60150dab3a9b",
          "url": "https://github.com/purlis/purlis/commit/77f672e0a701a018e9bcc3e4b35e1e556cfd27de"
        },
        "date": 1792356168762,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1506,
            "unit": "MB",
            "extra": "median of 3 samples: 1372.1, 1506, 1657.5 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 650.3,
            "unit": "MB",
            "extra": "median of 6 samples: 611.2, 640, 648, 652.5, 652.8, 758.1 MB; window 401.7 MB, session host 248.6 MB"
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
          "id": "7e739c9ddd673300a34f93e3e518df5f95889dea",
          "message": "Linux: a refused file no longer linked is sorted by the name it had (#2630)\n\n* Linux: a refused file no longer linked is sorted by the name it had (#2574)\n\nThe wrap's supervisor sorts a refusal whose target ends in the kernel's\n\" (deleted)\" by the path before it: its kind, and what Allow would name.\nA protected file that was unlinked is a protected file, with no Allow.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Linux: the name-lookup check reads the name a socket had (#2574)\n\nA resolver socket with \" (deleted)\" after its name is told as a lookup\nwith nothing to allow, as under the name it had.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T00:33:15+04:00",
          "tree_id": "632b3ee916ebf51445b62ce411b3655e44086d3e",
          "url": "https://github.com/purlis/purlis/commit/7e739c9ddd673300a34f93e3e518df5f95889dea"
        },
        "date": 1792358014606,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1547.1,
            "unit": "MB",
            "extra": "median of 3 samples: 1334.7, 1547.1, 1591.4 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 666.3,
            "unit": "MB",
            "extra": "median of 6 samples: 622.1, 650.5, 664.1, 668.5, 668.7, 747.5 MB; window 401.3 MB, session host 265.1 MB"
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
          "id": "67dfd81deb9b6953db74d2decdec129eec1b4187",
          "message": "Theme import and export: follow-ups (#2587, #2588, #2612) (#2660)\n\n- A staged import is read only to check (Library::checking, Staged::read;\n  Staged::path is private): no font of the stage is registered to be\n  served. Folders::find_at and every_at read only to check too.\n- One rule for names Windows cannot hold (theme::portable, on contain's\n  device table, now with CONIN$ and CONOUT$): device names and names\n  ending in a dot or a space are refused with a sentence in import,\n  export, the stage, the level reader and the files a theme names.\n- purlis theme export takes a level id and finds it with Folders::find_at,\n  never a farther level's folder. A level id takes the name grammar, so\n  the engine says why a folder like `con` is refused.\n\nCloses #2587\nCloses #2588\nCloses #2612\n\n\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T01:12:28+04:00",
          "tree_id": "5610e224f7b53899c4e6c990c5ebbeea3de25cf4",
          "url": "https://github.com/purlis/purlis/commit/67dfd81deb9b6953db74d2decdec129eec1b4187"
        },
        "date": 1792360238310,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1551.6,
            "unit": "MB",
            "extra": "median of 3 samples: 1377.7, 1551.6, 1648.4 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 674.5,
            "unit": "MB",
            "extra": "median of 6 samples: 621.5, 652.2, 666.8, 682.2, 682.9, 763 MB; window 399.8 MB, session host 274.5 MB"
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
          "id": "efb9dfd6b7a9dbb980224ee6bfb28a827d1dd23f",
          "message": "The guards follow a style wherever the file hands it on (#2659)\n\n* The guards follow a style wherever the file hands it on (#2566)\n\nThe geometry and type guards read a component's or a script's style\nthrough one follower, and the controls guard reads a control's style\nthrough the same one. From a style={…} expression, a spread, the objects\nObject.assign writes and a style typed as CSSProperties, it follows a\nheld variable, a member of a local object or a destructuring, each\nbranch of a ternary and each side of ||, ?? and &&, what a local\nfunction or useMemo returns, and what a relative import gives. In\nscript it reads an element's style under any name the file holds it by\n(const st = el.style, const { style } = el), cssText += and a\ntemplate-literal index. Each spelling has a planted-literal test through\nthe real tree; docs/design-system.md lists what the guards read and what\nthey do not.\n\nPart of #2313\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* The style follower reads each file once and lazily (#2566)\n\nA variable's initializer and a function's body are found where they end\nonly when a style is followed into them; a file that holds no element\nstyle skips alias detection, and one that names no CSSProperties skips\nthe typed pass. Alias names are escaped as a whole pattern. The geometry\nguard reads each source's declarations once per run.\n\nPart of #2313\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* The style follower fails closed and follows the review's misses (#2566)\n\nA style handed on past FOLLOW_LIMIT hops, and a style from a relative\nimport the guard cannot open (above src, not found, not script), now\nfail the guard with a sentence naming the file and the style. A script\nextension resolves as TypeScript resolves it (./s.js is s.ts).\n\nThe follower now also follows a default-exported function, a call on a\n`* as` import, a written-key member (styles[\"box\"]), a function with an\nobject return type, useCallback, and el[\"style\"] and (el.style)\naliases. Each imported file is read and parsed once per guard pass. The\nreview's planted cases are tests, and \"What the guards read in script\"\nlists what is still not followed.\n\nPart of #2313\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T01:39:41+04:00",
          "tree_id": "58741fe41322c7c1392521ebbbc2160bbc8e1d2e",
          "url": "https://github.com/purlis/purlis/commit/efb9dfd6b7a9dbb980224ee6bfb28a827d1dd23f"
        },
        "date": 1792362840994,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1504.6,
            "unit": "MB",
            "extra": "median of 3 samples: 1373, 1504.6, 1515 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 661.8,
            "unit": "MB",
            "extra": "median of 6 samples: 611.5, 646.7, 660.5, 663.1, 663.4, 759.6 MB; window 398.6 MB, session host 263.2 MB"
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
          "id": "0221bf611ca897387a2bbc713b6b604df2822cf0",
          "message": "Theme: a window's first frame draws the theme the levels choose (#2631)\n\n* Theme: a window's first frame draws the theme the levels choose\n\nThe core resolves the theme levels once as a window is created, for the\nproject and workspace it opens on, and the creation script, the native\nwindow and the page's first frame take that one answer (#2602). The\nsystem's contrast is read natively at creation: Increase contrast on\nmacOS, the desktop portal on Linux; elsewhere the page picks among the\nframes it is handed for both (#2582). An appearance a theme does not\nhave draws its first for the system's contrast, and the page's shortcut\nto the person's own theme gives way to the levels with no project in\nfront.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* Theme: first-frame tests hold, and rustfmt\n\nThe floors test compares the theme and its floors, the remembered\nstore is a seam so no test writes machine.json, and the page test\nexpects the new first frame field.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* Theme: macOS contrast is not read natively yet\n\nThe read needs two objc2-app-kit features the tree does not build\ntoday, which is the operator's call. Until then macOS, like Windows,\nhands the page a frame for each contrast and the page picks with\nprefers-contrast.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* Theme: macOS reads Increase contrast as a window is created\n\nNSWorkspace.accessibilityDisplayShouldIncreaseContrast, with two more\nfeatures of objc2-app-kit (NSWorkspace, NSAccessibility); Cargo.lock is\nunchanged. The decision from the platform's answer is tested.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* Theme: a first frame draws a project's theme only once it is trusted\n\nA window's first frame draws a project's theme only once the project is\ntrusted on this machine: a remembered project is used only while the\nstore's approval of it holds for what it contributes now. Linux asks\nthe portal once per process, and more contrast is drawn when either\nthe portal or the page asks for it.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* Theme: the first frame's trust test reads the faces through the store\n\nA window created on a project whose approval in this machine's store\ndoes not hold its faces is handed none. The contrast test no longer asks\nthe real platform, so no portal is started by a test.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* Theme: name the approved store in the trust test plainly\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T02:16:08+04:00",
          "tree_id": "e94e156ad3e69157f370bbc5096aebc6d4c90b03",
          "url": "https://github.com/purlis/purlis/commit/0221bf611ca897387a2bbc713b6b604df2822cf0"
        },
        "date": 1792365922643,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1482.1,
            "unit": "MB",
            "extra": "median of 3 samples: 1319.3, 1482.1, 1565.8 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 632.2,
            "unit": "MB",
            "extra": "median of 6 samples: 592.9, 620.9, 628.7, 635.7, 636.1, 725.7 MB; window 393.1 MB, session host 239.1 MB"
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
          "id": "d44a455cd5381ceefde692f7ff27dc1aa05c0a77",
          "message": "Sandbox: a socket grant never names the system's name-lookup sockets (#2675)\n\n* Sandbox: a socket grant never names the system's name lookup (#2657)\n\nservices::socket refuses each socket of RESOLVER_SOCKETS, written whole\nor as a unix:// address, before it looks at what is there, in one\nsentence (NAME_LOOKUP). Every Allow is judged through it, so a block's\nNotice, the centralized Ask and Settings refuse it at every level, and\nthe one keep (kept_for) refuses it too. A kept one is never compiled in\n(still_sockets), and the Granted list marks it as reaching no chat.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Sandbox: the Mac's own name lookup is refused too (#2657)\n\nReview of #2675. The name lookup a grant never names is Linux's\n(RESOLVER_SOCKETS, which still sorts a refusal as a lookup) and the\nMac's own. services::socket and services::service refuse it, kept_for\nrefuses it, a start never compiles one in, the Granted list marks a\nkept one and Doctor counts it as held. A path is read with . and ..\ntaken lexically. NAME_LOOKUP_REFUSED says the rule, then why. Tests\nloop over every path and level, for this project and for every\nproject. ADR 0093 amended, to be confirmed.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Sandbox: every name of the Mac's lookup socket is refused (#2657)\n\nis_name_lookup compares every name a path goes by (as written, as the\nkernel names it, across a firmlink) and, where something is there, the\nfile itself. Tests cover the socket's names on the data volume for a\ngrant, a start and the Granted list.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T03:09:06+04:00",
          "tree_id": "f76612a1dfbee6ef686a9236293d063fdf8c9602",
          "url": "https://github.com/purlis/purlis/commit/d44a455cd5381ceefde692f7ff27dc1aa05c0a77"
        },
        "date": 1792368036338,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1472.9,
            "unit": "MB",
            "extra": "median of 3 samples: 1361.8, 1472.9, 1616.2 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 631.8,
            "unit": "MB",
            "extra": "median of 6 samples: 593.4, 619.4, 625.5, 638.2, 638.6, 737 MB; window 385.8 MB, session host 246.6 MB"
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
          "id": "5eded4f658ffb97ff0fdd6154d5589813b7fae3e",
          "message": "Browsers: idle stop and the memory budget (#2573)\n\n* Browsers: tests for a stuck browser retried off the call's path\n\nA browser call answers at once while a browser of the project is stuck,\nand a stuck browser being retried still counts toward the project's three.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Browsers: a stuck browser is retried off the call's path\n\nA browser purlis could not end yet is retried by one thread per project,\non a backoff from 1 s to 60 s, never inside a browser call. It moves from\nits chat to the ending list under both locks, so the project's count\nnever drops below the browsers still running. The window is told when\nthe retry ends one; Stop all wakes the retry at once.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Browsers: idle stop and the memory budget\n\nA chat's browser unused for ten minutes stops, and the chat's next call\nis told; one the person drives or that an Ask waits on never stops so.\nThe device runs at most a budget of browsers from its memory class\n(16 GB: 4, 32 GB: 8, 64 GB: 16), counted from the session host's own\nlist and checked one start at a time, beside the three per project.\nA refusal past either says which limit it met.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Memory pressure: its tests move to a file of their own\n\nMove only: the tests that were inline before #2023 go to\nmemorypressure/tests.rs unchanged. The test #2023 added comes back in\nthe next commit.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Machine store: its tests move to a file of their own\n\nMove only: the inline test block goes to machine/tests.rs unchanged,\nunder the same cfg attributes.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: the limits in Settings, and the idle stop follows the host\n\nThe person's ruling of 2026-10-18: browsers.per_project,\nbrowsers.on_this_device and browsers.idle_minutes in this machine's\nstore, set in Settings > This machine > Browsers and read at each\nbrowser call. A refusal past a limit names the setting.\n\nThe idle sweep never stops a browser the session host lists with a\ndriver, and stops nothing when the host cannot say. A login, and an Ask\nraised about a browser, count as a use. Test placement as the\ncontributing guide says.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: bindings for main's commands and the limits together\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: the palette's rows count the Browsers settings group\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T03:34:29+04:00",
          "tree_id": "a90575df1fd116c735be9a2d43c2c9b8d86dad39",
          "url": "https://github.com/purlis/purlis/commit/5eded4f658ffb97ff0fdd6154d5589813b7fae3e"
        },
        "date": 1792370241543,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1536,
            "unit": "MB",
            "extra": "median of 3 samples: 1331.1, 1536, 1655.2 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 648.8,
            "unit": "MB",
            "extra": "median of 6 samples: 613.8, 636.9, 646.6, 650.9, 651.3, 734.1 MB; window 389.3 MB, session host 259.4 MB"
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
          "id": "262caecea78dc489f89ccb192ddcf66d133d2858",
          "message": "Browsers: pin, hand over, and reopen after quit (#2586)\n\nThe person pins a chat's browser from its row's menu or the palette: it\noutlives its chat, listed under Pinned browsers, until they stop it, Stop\nall, or a quit. A pinned browser whose chat closed, or one the person\nopened with no chat, is handed to an open chat, which gets that chat's\nreach at once; one signed in with a login goes only to a chat that may\nlog in with that login now, and a failed hand-over gives the browser\nback the reach it had or ends it. As purlis quits it keeps where the\nproject's browsers were in app/browsers-at-quit.json, addresses only,\nand the next start offers them once.\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T04:37:13+04:00",
          "tree_id": "6f21763bb6e69cb331d2e4bf988064cfb2f3a1a9",
          "url": "https://github.com/purlis/purlis/commit/262caecea78dc489f89ccb192ddcf66d133d2858"
        },
        "date": 1792372574151,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1455.6,
            "unit": "MB",
            "extra": "median of 3 samples: 1310.7, 1455.6, 1488.4 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 629.9,
            "unit": "MB",
            "extra": "median of 6 samples: 592, 616.6, 628.7, 631.2, 631.5, 726.5 MB; window 388.9 MB, session host 241 MB"
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
          "id": "0c16f80e506f6edb0c6feff8874cf89995feb086",
          "message": "A runner chat composed by its names is asked about a service allowed for it alone (#2673)\n\n* A runner chat composed by its names is asked about a service allowed for it alone\n\nSession protocol 1.26: a start by names that names its confinement may name the services the\nwindow allowed the chat alone (`alone`). The runner leaves out of the confinement it composes\neach one its own sandbox does not give, names them in `confined.left_out`, and asks the window\nabout each, as for a plan made elsewhere.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\nAssisted-by: claude-code:claude-opus-5-5\n\n* A composed plan may leave a service out of its proxy, and a test starts one\n\nPlan::starting no longer holds that only a plan made elsewhere leaves a service out: a plan\nthe host composed for a chat started by its names does too. The egress docs say both.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* The runner's policy binds what a composed chat is asked about\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T05:10:51+04:00",
          "tree_id": "2d49aa1d6bbd04406217132c7e15fef1480e6e4d",
          "url": "https://github.com/purlis/purlis/commit/0c16f80e506f6edb0c6feff8874cf89995feb086"
        },
        "date": 1792374996468,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1545.3,
            "unit": "MB",
            "extra": "median of 3 samples: 1368.4, 1545.3, 1583.4 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 646.7,
            "unit": "MB",
            "extra": "median of 6 samples: 603.3, 635.3, 644.8, 648.7, 649.8, 747 MB; window 399.7 MB, session host 247 MB"
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
          "id": "7ba7867a0db1d5cc584b7ce88549ab48d9ec65a3",
          "message": "Browsers: a profile keeps its browser logins sealed on this device (#2021) (#2654)\n\nA browser that performed a declared login keeps it, sealed under this\ndevice's key, once it ends; the next chat of that profile whose login is\nallowed starts signed in, held to the login's site from the first moment.\nThe store is bound to the login as declared and its generation, denied to\nchats, left out of backups, one writer at a time, and wiped with its\nprofile. Session protocol 1.27.\n\n\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T05:53:08+04:00",
          "tree_id": "eba756b48a6d3919fefc5b615522c9423549a7d3",
          "url": "https://github.com/purlis/purlis/commit/7ba7867a0db1d5cc584b7ce88549ab48d9ec65a3"
        },
        "date": 1792377460522,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1537.6,
            "unit": "MB",
            "extra": "median of 3 samples: 1368.4, 1537.6, 1588.1 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 649.7,
            "unit": "MB",
            "extra": "median of 6 samples: 614.4, 636.8, 648.4, 651, 652.7, 742.5 MB; window 405.4 MB, session host 244.3 MB"
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
          "id": "f454c0ea985727e1c369438c4263d00e13f765e6",
          "message": "Sandbox: a chat is judged sandboxed or not by the record of how its run started (#2688)\n\n* The session host records how each run is sandboxed\n\nThe host reads it from what it starts, never from the start: the program\nthat wraps it (Seatbelt, purlis's Linux wrap, Claude Code's own sandbox\nin its settings) and whether a confinement holds it. A confined run never\nreads as off. It is kept per program, on the host record's started line,\nand answered by programs_of (session protocol 1.28, sandbox).\n\nPart of #2636\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* WIP: the window judges a run by the record of how it started\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* A chat is judged by the record of how its run started\n\nThe window keeps how each run is sandboxed: its own record for a run it\nstarts, the session host's for one it reattaches. A browser login reads\nit. A run recorded as off is reattached with no sandbox of now standing\nin for it; a run with no record is taken as sandboxed and offered a\nrestart. ADR 0068 amendment, plane-format's host record row, and a\nchangelog fragment.\n\nCloses #2636\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* A runner chat started without the sandbox keeps its logins; only logins read the record\n\nA harness chat on a runner that the person started without the sandbox,\nwhich the runner composed with no confinement, is recorded off, so its\nbrowser logins are its persona's. Logins are the one path that reads the\nrecord of how a run started; the paths that grant a sandboxed chat more\nkeep the window's compiled record. A composed Claude Code line is read as\nits own sandbox.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T06:30:34+04:00",
          "tree_id": "6a3f62c5b47289e21c2301e42822fef812023ecd",
          "url": "https://github.com/purlis/purlis/commit/f454c0ea985727e1c369438c4263d00e13f765e6"
        },
        "date": 1792379750236,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1523.5,
            "unit": "MB",
            "extra": "median of 3 samples: 1278.9, 1523.5, 1551.6 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 635.8,
            "unit": "MB",
            "extra": "median of 6 samples: 596.4, 622.2, 634.8, 636.8, 637.5, 736.1 MB; window 389.4 MB, session host 246.4 MB"
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
          "id": "a4d3afcfd392248b9d8eb42e6da2fc74bbed5257",
          "message": "Browsers: a browser the person opens reaches the project's sites and goes to an address they type (#2643)\n\n* Browsers: a browser the person opens reaches the project's sites and goes to an address they type\n\nA browser opened from the window starts with what the project's sandbox\nsettings list for a chat on no persona and the browser sites allowed\nhere, and its view tab has an address field. Only http and https\naddresses go; a site new to the project asks once, under the field.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Browsers: a person's browser may allow a site for everyone in the project\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M4XB4GEA0C3CMJM6MM7Y5XXB\nPurlis-Persona: steward\n\n* Browsers: bindings, formatting and palette tests for a person's browser\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: the reconnecting view test names its project\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: the address tests get a loaded runner's budget\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: the address box is named where the views' native controls are\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: a person's browser asks in the sites ask's words, and goes after what they typed\n\nIts sites ask offers the registry's answers at the scopes policy leaves\nopen, and for everyone in the project only where the sandbox is on. A\npublic site on another port asks. The address goes through the view's\nqueue, handed back first where the person drives. Tests move to files of\ntheir own.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: bindings for the sites ask's words\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: a person's browser that cannot start says why in the device budget's words\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: an address kept at quit opens a browser there on the person's press\n\nThe quit Notice's buttons open a browser of the person's own, which goes\nto the address once its view is live, as a typed one does. A pinned\nbrowser past its chat has no address field.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: formatting after kept logins\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T06:58:34+04:00",
          "tree_id": "cd864a55bd8813b43aab72a318fd6948872b63dd",
          "url": "https://github.com/purlis/purlis/commit/a4d3afcfd392248b9d8eb42e6da2fc74bbed5257"
        },
        "date": 1792381626965,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1518.4,
            "unit": "MB",
            "extra": "median of 3 samples: 1390.8, 1555.2, 1518.4 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 652.2,
            "unit": "MB",
            "extra": "median of 6 samples: 606, 637.3, 645.4, 658.9, 659.2, 755.8 MB; window 402.8 MB, session host 249.8 MB"
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
          "id": "cb31e95f40b6ebd89f76b20d4562c1e05e38889c",
          "message": "Browsers: a chat without the sandbox is given what a sandboxed chat of its persona lists (#2677)\n\n* Browsers: a chat without the sandbox is given what a sandboxed chat of its persona lists\n\nA chat started without the sandbox has no record of a compiled sandbox,\nso its browser's listing is compiled at each call from the project's\nsettings for its persona, never the floor's reach. Its sites ask as any\nchat's do, and its posture Ask says what each answer gives it.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\nAssisted-by: claude-code:claude-opus-5-5\n\n* Browsers: a chat without the sandbox is one with none compiled and none holding it\n\nA sandboxed chat reattached under a sandbox whose compile was not\nrecorded is given nothing listed, and its Ask is a sandboxed chat's.\nWhether a chat started without the sandbox is the chat's, not its\npersona's.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: a chat's sites ask offers everyone in the project only where the sandbox is on\n\nA committed browser site reaches a browser only where the project's\nsandbox is in force, so a chat's sites ask offers it only there, and an\nAllow at that level is refused elsewhere, as for the person's own\nbrowser.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T07:31:54+04:00",
          "tree_id": "957b790ddd50ff509b46bca1376baef954a5bcdd",
          "url": "https://github.com/purlis/purlis/commit/cb31e95f40b6ebd89f76b20d4562c1e05e38889c"
        },
        "date": 1792384232019,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1561.1,
            "unit": "MB",
            "extra": "median of 3 samples: 1323.8, 1561.1, 1623.9 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 652.6,
            "unit": "MB",
            "extra": "median of 6 samples: 614.1, 640.3, 649.4, 655.7, 655.9, 742 MB; window 411.7 MB, session host 240.9 MB"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "name": "Aaron Yordanyan",
            "username": "diazoxide",
            "email": "aaron.yor@gmail.com"
          },
          "committer": {
            "name": "GitHub",
            "username": "web-flow",
            "email": "noreply@github.com"
          },
          "id": "cb31e95f40b6ebd89f76b20d4562c1e05e38889c",
          "message": "Browsers: a chat without the sandbox is given what a sandboxed chat of its persona lists (#2677)\n\n* Browsers: a chat without the sandbox is given what a sandboxed chat of its persona lists\n\nA chat started without the sandbox has no record of a compiled sandbox,\nso its browser's listing is compiled at each call from the project's\nsettings for its persona, never the floor's reach. Its sites ask as any\nchat's do, and its posture Ask says what each answer gives it.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\nAssisted-by: claude-code:claude-opus-5-5\n\n* Browsers: a chat without the sandbox is one with none compiled and none holding it\n\nA sandboxed chat reattached under a sandbox whose compile was not\nrecorded is given nothing listed, and its Ask is a sandboxed chat's.\nWhether a chat started without the sandbox is the chat's, not its\npersona's.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n* Browsers: a chat's sites ask offers everyone in the project only where the sandbox is on\n\nA committed browser site reaches a browser only where the project's\nsandbox is in force, so a chat's sites ask offers it only there, and an\nAllow at that level is refused elsewhere, as for the person's own\nbrowser.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code:claude-opus-5-5\nPurlis-Chat: 01M5853N5JVZQCP5AEZR3XDC61\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T03:31:54Z",
          "url": "https://github.com/purlis/purlis/commit/cb31e95f40b6ebd89f76b20d4562c1e05e38889c"
        },
        "date": 1792399682705,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1433.9,
            "unit": "MB",
            "extra": "median of 3 samples: 1344.5, 1433.9, 1582.6 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 648.8,
            "unit": "MB",
            "extra": "median of 6 samples: 607.6, 641.1, 647.7, 649.8, 650.1, 742.3 MB; window 393.4 MB, session host 255.4 MB"
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
          "id": "23855fecf4c5607c66f61363ef92e225af663d84",
          "message": "The theme check holds the remaining pairs and geometry across tokens (#2700)\n\nEvery syntax role is held on the editor's current line, and the window's\nsecondary, muted, accent and state words on a theme's selection while\nselected text is the platform's. A line of words has to fit the control\nor strip it is drawn in, in both densities: the check names the sizes\nof one that does not (`line-does-not-fit`). Each chart point is its own\nseries, the chart palette in turn, so every chart colour is drawn.\nPurlis Classic light's property and attribute colours are a shade\ndarker so they read on the current line.\n\nCloses #2522\n\n\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T21:46:47+04:00",
          "tree_id": "896380d54561775f0fe75f21502dbbf40c0ccf0a",
          "url": "https://github.com/purlis/purlis/commit/23855fecf4c5607c66f61363ef92e225af663d84"
        },
        "date": 1792435090907,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1498.8,
            "unit": "MB",
            "extra": "median of 3 samples: 1379.3, 1498.8, 1623.3 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 653.5,
            "unit": "MB",
            "extra": "median of 6 samples: 621.3, 646.8, 650.8, 656.3, 656.5, 745 MB; window 408.1 MB, session host 246.1 MB"
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
          "id": "6fb516d20de78e65177a29cda211aaf88d74ea82",
          "message": "Theme: every theme file is read through a checked handle; names every platform holds (#2704)\n\nEvery theme file is read from a handle on the folder that was opened\n(theme/opened.rs): a folder is opened from the one before it refusing a\nlink, and each file from that handle, a plain file under the cap.\n\n- purlis theme check <folder> opens the folder once (theme::Named) and\n  reads theme.json, its icon set, its fonts and the folders its extends\n  chain names from those handles (#2604).\n- The person's own theme.json, its icon set and fonts are read through a\n  handle on their config folder (#2604).\n- purlis theme export <level id> packs every file from the handle the\n  level reader found the folder through (Folders::folder_at,\n  FoundFolder::export); export by path packs from a handle too (#2670).\n- A theme folder's name and every name inside it hold none of\n  < > : \" / \\ | ? * nor a control character, beside the device-name\n  rule (contain::unspellable, portable::not_held), so import, export and\n  the folder reader refuse such a name in one sentence (#2671).\n\nCloses #2604\nCloses #2670\nCloses #2671\nPart of #2313\n\n\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T22:44:58+04:00",
          "tree_id": "0532d74562814e2258384395b4114666e6d31c26",
          "url": "https://github.com/purlis/purlis/commit/6fb516d20de78e65177a29cda211aaf88d74ea82"
        },
        "date": 1792437858662,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1480.7,
            "unit": "MB",
            "extra": "median of 3 samples: 1336.1, 1480.7, 1636.9 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 647.2,
            "unit": "MB",
            "extra": "median of 6 samples: 604.8, 633, 641.3, 653.1, 653.5, 734.1 MB; window 401.6 MB, session host 245.6 MB"
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
          "id": "840d889d8c9a38fc265c31eea166ed1c48af8e8e",
          "message": "The person sets a theme's option values per project and workspace (#2702)\n\n* Theme: a choice carries option values and names its folder's level\n\nA theme choice now holds the value of each of its theme's options the\nperson chose (#2527) and, for a theme folder, the level of the folder it\npicked (#2516). The theme engine builds the values onto the theme with\nTheme::with before drawing it, so the consent floors hold over every\nvalue, and a choice that names a level draws that level's folder and\nnever another of its name. Choices written before keep resolving as they\ndid and gain their level the next time they are written.\n\nSettings › Theme lists a card for each folder of one name, each with its\nlevel, and its options become choosable at You, Project and Workspace\n(theme_choose_option, window-only), each value badged with the level\nwhose choice made it.\n\nCloses #2527\nCloses #2516\nPart of #2313\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* Theme: regenerate the bindings and badge each option value with its level\n\nThe window's bindings carry the new command and the card's level. The\noptions list names the level whose choice made each value, and the\nstanding-lines guard counts the gallery's line for values an option's\npick dropped.\n\nPart of #2527\nPart of #2516\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n* Theme: review follow-ups for option values and folder levels\n\n- choices::choose compares both choices as written, so an old choice\n  with no level re-chosen as another level's folder starts from its\n  defaults, and keeps only the values its theme still has.\n- Each option's radio group is named by the option alone and described\n  by its line, and has a Theme default entry that chooses no value.\n- A card's pictures are drawn with the option values chosen at the\n  level, as pointing at it draws it.\n\nPart of #2527\nPart of #2516\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nAssisted-by: claude-code\nPurlis-Chat: 01M4V610KJH75YKMQHN124WK1Y\nPurlis-Persona: steward\n\n---------\n\nCo-authored-by: Aaron Yordanyan <eyehollow@gmail.com>\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>",
          "timestamp": "2026-10-19T23:39:56+04:00",
          "tree_id": "2a9bfbbf51d05c00606698d8467939523195116a",
          "url": "https://github.com/purlis/purlis/commit/840d889d8c9a38fc265c31eea166ed1c48af8e8e"
        },
        "date": 1792441229064,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "M2 web content at fifty hot chats (resident)",
            "value": 1486.7,
            "unit": "MB",
            "extra": "median of 3 samples: 1377.3, 1486.7, 1606.7 MB"
          },
          {
            "name": "M3 native side with no chats (window + session host, resident)",
            "value": 640.8,
            "unit": "MB",
            "extra": "median of 6 samples: 606.9, 632.2, 638.5, 643.1, 643.6, 721.5 MB; window 398.7 MB, session host 242.1 MB"
          }
        ]
      }
    ]
  }
}