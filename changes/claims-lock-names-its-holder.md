### Fixed

- Removing a branch folder no longer keeps every other chat's commit, push or removal in the
  workspace waiting while git and the forge answer: only the claim check holds the lock. A
  removal that finds the lock taken tries once more, and a refusal names the last chat or
  process recorded holding it and when it took it (#8024).
- A chat's branch folder removal that was refused no longer leaves the folder claimed by that
  chat, which kept the folder from a prune and refused a task dispatched into it later (#8024).
- A squash-landed branch is deleted with its folder only at the commit it was judged at: a
  commit made in the folder during the removal keeps the branch (#8024).
