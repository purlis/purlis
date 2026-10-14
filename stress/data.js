window.BENCHMARK_DATA = {
  "lastUpdate": 1791970090580,
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
      }
    ]
  }
}