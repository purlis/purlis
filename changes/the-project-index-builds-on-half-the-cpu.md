### Changed

- **The project index builds on about half the CPU.** Reading a memory for secrets before it is
  indexed asked each rule once instead of twice, and read personal data a line at a time instead
  of across the whole memory, where one character that is not ASCII sent those rules to their
  slowest search. A new file is read with one lookup of its path instead of two. What is masked is
  unchanged, span for span (#5672).
