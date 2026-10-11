### Fixed

- **A workspace purlis cannot read is not called behind the layout.** The status line no longer
  shows its `reinit` tip for a workspace whose folder or layout stamp purlis cannot read, and the
  `reinit` alert no longer counts one, since `purlis ws reinit` could not reach it either. The
  doctor's `workspace layout` row already left them out (#1289).
