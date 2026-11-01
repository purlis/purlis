#!/usr/bin/env python3
# THROWAWAY (#4472): which step, if any, of purlis's DevTools use makes Chrome die on a page whose
# frame reloads every 100 ms. Every round takes the next arm in turn, so the arms share a runner.
import fcntl, glob, json, os, shutil, signal, subprocess, sys, tempfile, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
SITE = "crates/browser-engine/tests/fixtures/site"
PAGE = "http://fixture.test/typing-beside-a-reloading-frame.html"
OBSERVE = float(os.environ.get("OBSERVE", "5"))
OUT = os.environ.get("OUT", "probe-out")


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        rest = self.path.removeprefix("http://")
        host, _, path = rest.partition("/")
        path = path.split("?")[0].split("#")[0] or "index.html"
        try:
            body = open(os.path.join(SITE, host.split(":")[0], path), "rb").read()
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
        except OSError:
            body = b"not here"
            self.send_response(404)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_):
        pass


def flags(data, proxy, pipe, last):
    args = [CHROME]
    if pipe:
        args.append("--remote-debugging-pipe")
    return args + [
        f"--user-data-dir={data}",
        "--headless",
        "--window-size=1280,800",
        f"--proxy-server=http://127.0.0.1:{proxy}",
        "--proxy-bypass-list=<-loopback>",
        "--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE 127.0.0.1",
        "--disable-quic",
        "--force-webrtc-ip-handling-policy=disable_non_proxied_udp",
        "--disable-breakpad",
        "--disable-crash-reporter",
        "--use-mock-keychain",
        "--password-store=basic",
        "--disable-background-networking",
        "--disable-component-update",
        "--disable-sync",
        "--disable-extensions",
        "--disable-default-apps",
        "--disable-features=MediaRouter,DialMediaRouteProvider,Translate,AutofillServerCommunication",
        "--no-first-run",
        "--no-default-browser-check",
        "--noerrdialogs",
        last,
    ]


def high(fd):
    new = fcntl.fcntl(fd, fcntl.F_DUPFD, 20)
    os.close(fd)
    return new


def start(proxy, pipe, last):
    data = tempfile.mkdtemp(prefix="p4472")
    home = os.path.join(data, "home")
    os.makedirs(os.path.join(data, "Default"))
    os.makedirs(home)
    with open(os.path.join(data, "Default", "Preferences"), "w") as f:
        json.dump({"credentials_enable_service": False}, f)
    keep = ("PATH", "TMPDIR", "LANG", "LC_ALL", "LC_CTYPE", "TZ", "USER", "LOGNAME")
    env = {k: v for k, v in os.environ.items() if k in keep}
    env.update(HOME=home, DBUS_SESSION_BUS_ADDRESS="disabled:")
    err = open(os.path.join(data, "stderr"), "wb")
    kw = {}
    fds = None
    if pipe:
        r1, w1 = os.pipe()
        r2, w2 = os.pipe()
        r1, w1, r2, w2 = high(r1), high(w1), high(r2), high(w2)

        def child():
            os.dup2(r1, 3)
            os.dup2(w2, 4)

        kw = dict(preexec_fn=child, pass_fds=(3, 4))
        fds = (r1, w1, r2, w2)
    proc = subprocess.Popen(
        flags(data, proxy, pipe, last),
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=err,
        stderr=err,
        start_new_session=True,
        **kw,
    )
    conn = None
    if pipe:
        os.close(fds[0])
        os.close(fds[3])
        conn = Conn(fds[1], fds[2])
    return proc, data, conn


class Conn:
    def __init__(self, w, r):
        self.w, self.r = w, r
        self.next = 0
        self.waiting = {}
        self.lock = threading.Lock()
        self.on_event = lambda m: None
        self.gone = False
        threading.Thread(target=self.read, daemon=True).start()

    def read(self):
        buf = b""
        while True:
            try:
                got = os.read(self.r, 1 << 16)
            except OSError:
                got = b""
            if not got:
                self.gone = True
                with self.lock:
                    for ev in self.waiting.values():
                        ev[0].set()
                return
            buf += got
            while b"\0" in buf:
                one, buf = buf.split(b"\0", 1)
                m = json.loads(one)
                if "id" in m:
                    with self.lock:
                        ev = self.waiting.pop(m["id"], None)
                    if ev:
                        ev[1].append(m)
                        ev[0].set()
                else:
                    self.on_event(m)

    def call(self, method, params=None, session=None, within=30):
        with self.lock:
            self.next += 1
            i = self.next
            ev = (threading.Event(), [])
            self.waiting[i] = ev
        m = {"id": i, "method": method, "params": params or {}}
        if session:
            m["sessionId"] = session
        try:
            os.write(self.w, json.dumps(m).encode() + b"\0")
        except OSError:
            return {}
        ev[0].wait(within)
        return ev[1][0] if ev[1] else {}


def drive(arm, conn):
    """The arm's DevTools use. Each adds one thing to the arm before it."""
    if arm == "discover":
        conn.call("Target.setDiscoverTargets", {"discover": True})
        conn.call("Target.createTarget", {"url": PAGE})
        return
    steps = []
    cv = threading.Condition()

    def worker():
        while True:
            with cv:
                while not steps:
                    cv.wait()
                step = steps.pop(0)
            step()

    threading.Thread(target=worker, daemon=True).start()

    def queue(step):
        with cv:
            steps.append(step)
            cv.notify()

    def setup(session):
        if arm == "full":
            conn.call("Page.enable", session=session)
            conn.call("Runtime.enable", session=session)
            conn.call("Network.enable", session=session)
        conn.call(
            "Target.setAutoAttach",
            {"autoAttach": True, "waitForDebuggerOnStart": arm in ("wait", "full"), "flatten": True},
            session,
        )
        if arm == "full":
            conn.call("Page.getFrameTree", session=session)
        conn.call("Runtime.runIfWaitingForDebugger", session=session)

    def event(m):
        if m.get("method") == "Target.attachedToTarget":
            kind = m["params"]["targetInfo"]["type"]
            session = m["params"]["sessionId"]
            if kind == "iframe" and arm in ("wait", "full"):
                queue(lambda: setup(session) if arm == "full" else conn.call("Runtime.runIfWaitingForDebugger", session=session))

    conn.on_event = event
    conn.call("Target.setDiscoverTargets", {"discover": True})
    target = conn.call("Target.createTarget", {"url": "about:blank"}).get("result", {}).get("targetId")
    session = conn.call("Target.attachToTarget", {"targetId": target, "flatten": True}).get("result", {}).get("sessionId")
    if arm != "attach":
        setup(session)
    conn.call("Page.navigate", {"url": PAGE}, session)


def newest_report(since, label):
    time.sleep(3)
    reports = [
        r
        for r in glob.glob(os.path.expanduser("~/Library/Logs/DiagnosticReports/*.ips"))
        if os.path.getmtime(r) >= since - 1
    ]
    if not reports:
        return ""
    report = max(reports, key=os.path.getmtime)
    os.makedirs(OUT, exist_ok=True)
    shutil.copy(report, os.path.join(OUT, f"{label}.ips"))
    try:
        text = open(report).read().split("\n", 1)
        b = json.loads(text[1])
        images = b.get("usedImages", [])
        thread = next(t for t in b["threads"] if t.get("triggered"))
        frames = [
            f'{images[f["imageIndex"]].get("name", "?")[:12]}+{hex(f["imageOffset"])}:{f.get("symbol", "")}'
            for f in thread["frames"][:10]
        ]
        return f'{b.get("exception", {}).get("type")} {thread.get("name")} ' + " ; ".join(frames)
    except Exception as why:
        return f"unreadable {why}"


def chrome_round(arm, proxy, label):
    since = time.time()
    pipe = arm != "plain"
    proc, data, conn = start(proxy, pipe, PAGE if arm == "plain" else "about:blank")
    began = time.monotonic()
    result = dict(arm=arm, outcome="ok")
    try:
        if pipe:
            drive(arm, conn)
        while time.monotonic() - began < OBSERVE and proc.poll() is None:
            time.sleep(0.05)
        code = proc.poll()
        if code is not None:
            result.update(outcome="died", code=code, after=round(time.monotonic() - began, 2))
            result["stderr"] = open(os.path.join(data, "stderr"), "rb").read()[-1500:].decode("utf8", "replace")
            result["crash"] = newest_report(since, label)
    finally:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except OSError:
            pass
        proc.wait()
        shutil.rmtree(data, ignore_errors=True)
    return result


def purlis_round(binary, label):
    since = time.time()
    trace = os.path.join(tempfile.gettempdir(), f"trace-{label}")
    env = dict(os.environ, PURLIS_PROBE_TRACE=trace, RUST_BACKTRACE="0")
    began = time.monotonic()
    try:
        run = subprocess.run(
            [binary, "the_persons_keys_reach_a_field", "--nocapture", "--test-threads=1"],
            env=env,
            cwd="crates/browser-engine",
            capture_output=True,
            text=True,
            timeout=150,
        )
        out, passed = run.stdout + run.stderr, run.returncode == 0
    except subprocess.TimeoutExpired as why:
        out, passed = f"timed out {why.stdout}", False
    result = dict(arm="purlis", outcome="ok", took=round(time.monotonic() - began, 2))
    if not passed:
        said = [l for l in out.splitlines() if "panicked" in l or "the browser" in l or "Open" in l]
        result.update(outcome="failed", said=" | ".join(said)[:900])
        if "has ended" in out:
            result["crash"] = newest_report(since, label)
        os.makedirs(OUT, exist_ok=True)
        if os.path.exists(trace):
            shutil.copy(trace, os.path.join(OUT, f"{label}.trace"))
    if os.path.exists(trace):
        os.remove(trace)
    return result


def main():
    arms = sys.argv[2].split(",")
    limit = time.monotonic() + float(sys.argv[3])
    binary = sys.argv[1]
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    proxy = server.server_address[1]
    os.makedirs(OUT, exist_ok=True)
    results = open(os.path.join(OUT, "results.jsonl"), "a")
    tally, n, said = {}, 0, 0
    while time.monotonic() < limit:
        arm = arms[n % len(arms)]
        n += 1
        label = f"{arm}-{n}"
        r = purlis_round(binary, label) if arm == "purlis" else chrome_round(arm, proxy, label)
        results.write(json.dumps(r) + "\n")
        results.flush()
        t = tally.setdefault(arm, {})
        t[r["outcome"]] = t.get(r["outcome"], 0) + 1
        if r["outcome"] != "ok" and said < 8:
            said += 1
            line = json.dumps(r).replace("%", "%25").replace("\n", "%0A")[:2500]
            print(f"::warning title={label}::{line}", flush=True)
    print(f"::notice title=tally::{json.dumps(tally)}", flush=True)
    with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as f:
        f.write(json.dumps(tally) + "\n")


main()
