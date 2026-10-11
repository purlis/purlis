### Security

- **The commit and push scan reads a change with the unchanged lines it joins.** A line that
  ends in a backslash joins the next, as a TOML `"""` string reads it. A chat's commit or push
  that completes a credential an unchanged line began, or begins one an unchanged line
  completes, is now refused as well (#1346).
- **A credential assignment or a signed token split by a line-ending backslash is caught in any
  file.** Where a `"""` string opens the value, the guards that read whole texts (a project
  save, a handoff or dispatch brief, the settings editors) now find it in a file of any name,
  not only one that parses as TOML (#1346).
