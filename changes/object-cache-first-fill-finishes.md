### Fixed

- The first read of a large repo from the forge, which lets a chat's push through, now finishes. It is cut only when git has written nothing for three minutes, not after a fixed fifteen, and it is staged (the newest commits first, then the rest), so a read that is interrupted, by a stall or by closing purlis, picks up at the stage it reached instead of starting again. A push refused while it runs now says the read resumes.
