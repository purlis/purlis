### Added

- **Run outcome evals from a terminal.** `purlis skill grade <name> --run --eval <id>` runs the evals
  you name, and the plan it prints marks each one on or off before you answer. It takes your yes
  only when typed at a terminal, so `--yes` and a piped answer are refused beside `--eval`. An id
  the plan cannot run stops it, with the reason. With no `--eval`, no eval runs (Skill studio
  preview, #4312).
