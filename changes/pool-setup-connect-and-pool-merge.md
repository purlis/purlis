### Added

- For Docker on another machine, Connect and Pool are one press. Naming the pool joins the page that trusts the machine, with who can sign in there, and Trust and create the pool runs every check from first contact to a linked runner. A full disk or a missing image is found before the machine is called connected (#7191).
- Every doctor and Connect check line carries how long it took, and a check running past 30 seconds says what it is waiting on and why, in the window and in `purlis pool doctor` (#7191).
- A failed add keeps the pool, as Not added, with its reason, on Docker and on a cluster. A machine is trusted only when the pool is made: an add that is turned away, or makes nothing, puts Connect back as it was, so it never says Connected beside it. Making another pool clears nothing of the refused one (#7191).
- The status bar's Try now tries the pool's forward again, and Check pool runs doctor's checks on it, not only a read (#7191).
