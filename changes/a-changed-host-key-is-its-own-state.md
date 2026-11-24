### Changed

- A pool's machine that presents another host key, CA or engine than the one you trusted is now its own state, "Host key changed", not a refusal. purlis never retries it on its own and offers no Try now; its notice says so and offers Check and trust pool, which opens the pool's settings where you can trust it again if the change is expected.
