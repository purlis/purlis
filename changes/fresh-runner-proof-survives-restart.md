### Changed

- A task's own cluster runner also stays while the task saved a file anywhere else on it that its user owns, outside its home, such as beside the home on its volume; system and temporary folders are left out. A runner whose host started again still finds what the task saved before (#4933).
