### Changed

- **A task's record lists the files it changed.** Once a task reports, purlis reads what it
  changed from the folder it worked in and keeps the list beside its report
  (`report.changed.files`). The Activity tab's "also changed by" mark reads that list, and
  reads file names out of a report's words only for a record that lists none (#1457).
