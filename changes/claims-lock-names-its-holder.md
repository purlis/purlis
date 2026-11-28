### Fixed

- Removing a branch folder no longer keeps every other chat's commit, push or removal in the
  workspace waiting while git and the forge answer: only the claim check holds the lock. A
  removal that finds the lock taken tries once more, and a refusal names the chat or process
  that holds it and for how long (#8024).
