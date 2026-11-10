#!/usr/bin/env python3
"""adx app pipeline: build, check, test, deploy and smoke-test an app on an emulator.

  adx pipeline run APP [--serial S] [--stages build,check,...] [--json]
  adx pipeline last APP [--json]       the latest run (summary, or run.json)
  adx pipeline list APP [--json]       runs, newest first

APP is a folder name under ADX_APPS_DIR or a path. Every run is a folder
$ADX_PIPELINE_DIR/<app>/<run-id>/ (run id: UTC time) with run.json (rewritten
after each stage, so a reader sees progress), summary.md, one log per stage and
the evidence (lint.xml, screenshot diffs, crash.txt, the phone's screen and
logcat); <app>/latest points at the newest. Made for Claude Code: run it, then
read the summary it prints (or <app>/latest/summary.md).

Exit codes: 0 passed, 1 a stage failed (or nothing ran), 2 usage or setup,
3 another run of this app is in progress, 130/143 interrupted. Progress and
errors go to stderr and $ADX_LOG_DIR/pipeline.log. Run it through
`adx pipeline`, which passes the configuration (config/defaults.env).
Standard library only.
"""
import fcntl
import glob
import hashlib
import json
import logging
import os
import re
import shutil
import signal
import subprocess
import sys
import time
import xml.etree.ElementTree as ET

STAGES = ["build", "check", "test", "deploy", "smoke"]
RUN_ID_RE = re.compile(r"^\d{8}-\d{6}(-\d+)?$")
log = logging.getLogger("pipeline")
C = {}   # configuration from the environment (adx passes config/defaults.env)


class Usage(Exception):
    """Bad arguments or configuration (exit 2)."""


class Failed(Exception):
    """A stage failed; the message is its summary."""


def load_config():
    def need(name, num=False):
        v = os.environ.get(name, "")
        if not v or (num and not v.isdigit()):
            raise Usage(f"{name}={v!r}: {'a number' if num else 'not set'} (run it as: adx pipeline …)")
        return int(v) if num else v
    C.update(apps=need("ADX_APPS_DIR"), dir=need("ADX_PIPELINE_DIR"), logs=need("ADX_LOG_DIR"),
             keep=need("ADX_PIPELINE_KEEP", True), smoke=need("ADX_PIPELINE_SMOKE_SECS", True),
             gradle_timeout=need("ADX_PIPELINE_GRADLE_TIMEOUT", True), image=need("ADX_PIPELINE_IMAGE"),
             sdk=need("ANDROID_HOME"))
    C["adb"] = os.path.join(C["sdk"], "platform-tools", "adb")


def setup_logging():
    os.makedirs(C["logs"], exist_ok=True)
    fmt = logging.Formatter("%(asctime)s %(levelname)-5s [%(name)s] %(message)s", "%Y-%m-%d %H:%M:%S")
    for h in (logging.StreamHandler(sys.stderr), logging.FileHandler(os.path.join(C["logs"], "pipeline.log"))):
        h.setFormatter(fmt)
        log.addHandler(h)
    log.setLevel(logging.INFO)


def sh(args, logfile, cwd=None, timeout=600, check=True, keep_output=True):
    """Run a command and append it to logfile (output too, unless keep_output is False);
    returns (code, output)."""
    t0 = time.time()
    try:
        p = subprocess.run(args, cwd=cwd, capture_output=True, text=True, errors="replace",
                           timeout=timeout, stdin=subprocess.DEVNULL)
        code, out = p.returncode, p.stdout + p.stderr
    except subprocess.TimeoutExpired as e:
        code, out = 124, f"{e.stdout or ''}{e.stderr or ''}\ntimed out after {timeout} s"
    except OSError as e:
        code, out = 127, str(e)
    with open(logfile, "a", encoding="utf-8") as fh:
        body = out if keep_output else f"({len(out)} bytes, saved separately)"
        fh.write(f"$ {' '.join(map(str, args))}\n{body}\n[exit {code}, {time.time() - t0:.1f} s]\n")
    log.info("%s -> exit %s in %.1f s", " ".join(map(str, args[:6])), code, time.time() - t0)
    if check and code != 0:
        raise Failed(f"`{' '.join(map(str, args[-3:]))}` exited {code}")
    return code, out


def hold(path, what):
    """Blocking exclusive lock (kept until the process ends); says when it waits."""
    fh = open(path, "w")
    try:
        fcntl.flock(fh, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        log.info("waiting for %s (another pipeline run uses it)", what)
        fcntl.flock(fh, fcntl.LOCK_EX)
    return fh


# ------------------------------------------------------------------ helpers ---
def app_root(app):
    root = os.path.realpath(app if os.sep in app else os.path.join(C["apps"], app))
    if not os.path.isfile(os.path.join(root, "gradlew")):
        raise Usage(f"{root} is not an app (no gradlew)")
    return root


def package_from_gradle(root):
    try:
        with open(os.path.join(root, "app", "build.gradle.kts"), encoding="utf-8") as fh:
            text = re.sub(r"//.*", "", fh.read())
        m = re.search(r'applicationId\s*=\s*"([^"]+)"', text)
        return m.group(1) if m else ""
    except OSError:
        return ""


def package_from_apk(apk):
    """The package the APK really installs as (suffixes and flavors included)."""
    tools = sorted(glob.glob(os.path.join(C["sdk"], "build-tools", "*", "aapt2")))
    if not tools or not apk:
        return ""
    p = subprocess.run([tools[-1], "dump", "packagename", apk], capture_output=True, text=True)
    return p.stdout.strip() if p.returncode == 0 else ""


def git_state(root):
    def g(*a):
        p = subprocess.run(["git", "-C", root, *a], capture_output=True, text=True)
        return p.stdout.strip() if p.returncode == 0 else ""
    return {"commit": g("rev-parse", "--short", "HEAD") or "none", "dirty": bool(g("status", "--porcelain"))}


def gradle_errors(out):
    """The useful lines of a failed Gradle run: every 'What went wrong', then compiler errors."""
    wrong = [w.strip() for w in re.findall(r"\* What went wrong:\n(.+?)(?=\n\* Try:|\n\* Exception|\Z)", out, re.S)]
    errs = [ln for ln in out.splitlines() if re.match(r"^(e: |.*\berror: )", ln)]
    seen, lines = set(), []
    for ln in wrong + errs[:15]:
        if ln not in seen:
            seen.add(ln)
            lines.append(ln)
    return "\n".join(lines)[:6000]


def fresh(path, t0):
    return os.path.isfile(path) and os.path.getmtime(path) >= t0


def running_emulators():
    _c, out = sh([C["adb"], "devices"], os.devnull, check=False, timeout=15)
    return [ln.split()[0] for ln in out.splitlines() if re.match(r"^emulator-\d+\s+device$", ln)]


def image_of(serial):
    _c, out = sh([C["adb"], "-s", serial, "emu", "avd", "name"], os.devnull, check=False, timeout=10)
    return (out.strip().splitlines() or ["?"])[0].strip()


def newest_source(root):
    t = 0.0
    for dp, _dn, fn in os.walk(os.path.join(root, "app", "src")):
        for f in fn:
            t = max(t, os.path.getmtime(os.path.join(dp, f)))
    return t


def apk_info(apk):
    with open(apk, "rb") as fh:
        digest = hashlib.sha256(fh.read()).hexdigest()
    return {"path": apk, "bytes": os.path.getsize(apk), "sha256": digest,
            "built": time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(os.path.getmtime(apk)))}


# ------------------------------------------------------------------- stages ---
def st_build(ctx):
    t0 = time.time()
    code, out = sh(["./gradlew", "--console=plain", "assembleDebug"], ctx["log"], cwd=ctx["root"],
                   timeout=C["gradle_timeout"], check=False)
    if code != 0:
        raise Failed("build failed:\n" + (gradle_errors(out) or "see build.log"))
    apks = [a for a in glob.glob(os.path.join(ctx["root"], "app/build/outputs/apk/debug/*.apk")) if fresh(a, t0 - 1)]
    apks = apks or sorted(glob.glob(os.path.join(ctx["root"], "app/build/outputs/apk/debug/*.apk")), key=os.path.getmtime)
    if not apks:
        raise Failed("build passed but no APK in app/build/outputs/apk/debug/")
    ctx["apk"] = sorted(apks, key=os.path.getmtime)[-1]
    ctx["run"]["apk"] = apk_info(ctx["apk"])
    return f"{os.path.basename(ctx['apk'])}, {ctx['run']['apk']['bytes'] / 1048576:.1f} MB"


def st_check(ctx):
    t0 = time.time()
    code, out = sh(["./gradlew", "--console=plain", "lintDebug"], ctx["log"], cwd=ctx["root"],
                   timeout=C["gradle_timeout"], check=False)
    xml = os.path.join(ctx["root"], "app/build/reports/lint-results-debug.xml")
    # Gradle skips lint when nothing changed (the report stays valid), so an old report
    # only counts when Gradle succeeded; after a failure only one written by this run.
    if not os.path.isfile(xml) or (code != 0 and not fresh(xml, t0 - 1)):
        if code != 0:
            raise Failed("lint did not run:\n" + (gradle_errors(out) or "see check.log"))
        raise Failed("lint passed but wrote no report (app/build/reports/lint-results-debug.xml): is xmlReport off?")
    shutil.copy(xml, os.path.join(ctx["dir"], "lint.xml"))
    counts, errors = {"error": 0, "warning": 0}, []
    for issue in ET.parse(xml).getroot().iter("issue"):
        kind = "error" if issue.get("severity", "").lower() in ("error", "fatal") else "warning"
        counts[kind] += 1
        if kind == "error" and len(errors) < 20:
            loc = issue.find("location")
            where = (f"{os.path.relpath(loc.get('file'), ctx['root'])}:{loc.get('line', '?')}"
                     if loc is not None and loc.get("file") else "")
            errors.append(f"{issue.get('id')}: {issue.get('message')} ({where})")
    summary = f"{counts['error']} errors, {counts['warning']} warnings (lint.xml)"
    if code != 0 or counts["error"]:
        raise Failed(summary + ("\n" + "\n".join(errors) if errors else "\n" + gradle_errors(out)))
    return summary


def tracked(root, path):
    return subprocess.run(["git", "-C", root, "ls-files", "--error-unmatch", path],
                          capture_output=True).returncode == 0


def st_test(ctx):
    root = ctx["root"]
    # References: committed in app/src/test/screenshots/ (current template) or, in older
    # apps, kept in app/build/outputs/roborazzi/. Diffs land next to them or in the latter.
    dirs = [os.path.join(root, "app/src/test/screenshots"), os.path.join(root, "app/build/outputs/roborazzi")]
    pngs = lambda pat: [p for d in dirs for p in glob.glob(os.path.join(d, pat))]
    refs = [p for p in pngs("*.png") if not re.search(r"_(compare|actual)\.png$", p)]
    for p in pngs("*_compare.png") + pngs("*_actual.png"):
        if not tracked(root, p):   # last run's diffs; never a committed file
            os.remove(p)
    t0 = time.time()
    task = "verifyRoborazziDebug" if refs else "recordRoborazziDebug"
    code, out = sh(["./gradlew", "--console=plain", task], ctx["log"], cwd=root, timeout=C["gradle_timeout"], check=False)
    tests = fails = 0
    failing = []
    for x in glob.glob(os.path.join(root, "app/build/test-results/testDebugUnitTest/*.xml")):
        if code != 0 and not fresh(x, t0 - 1):
            continue   # a failed run: only results it wrote (Gradle reuses them when nothing changed)
        r = ET.parse(x).getroot()
        tests += int(r.get("tests", 0))
        fails += int(r.get("failures", 0)) + int(r.get("errors", 0))
        for tc in r.iter("testcase"):
            f = tc.find("failure") if tc.find("failure") is not None else tc.find("error")
            if f is not None and len(failing) < 20:
                msg = (f.get("message") or "").strip().splitlines()
                failing.append(f"{tc.get('classname', '').rsplit('.', 1)[-1]}.{tc.get('name')}: {msg[0] if msg else ''}"[:300])
    diffs = sorted(p for p in pngs("*_compare.png") if fresh(p, t0 - 1))
    for d in diffs:
        shutil.copy(d, os.path.join(ctx["dir"], "diff-" + os.path.basename(d)))
    shot_note = (f"screenshots verified against {len(refs)} references" if refs
                 else f"{len(pngs('*.png'))} reference screenshots recorded (first run)")
    summary = f"{tests} tests, {fails} failed; {shot_note}"
    if diffs:
        summary += (f"; {len(diffs)} visual change(s): diff-*.png. If intended, re-record: "
                    "./gradlew recordRoborazziDebug")
    if code != 0 and not tests:
        raise Failed("tests did not run:\n" + (gradle_errors(out) or "see test.log"))
    if code != 0 or fails or diffs:
        raise Failed(summary + ("\n" + "\n".join(failing) if failing else ("\n" + gradle_errors(out) if not diffs else "")))
    return summary


def st_deploy(ctx):
    serial = ctx["serial"]
    phones = running_emulators()
    if serial and serial not in phones:
        raise Failed(f"{serial} is not a running emulator (running: {', '.join(phones) or 'none'})")
    if not serial:
        if len(phones) > 1:
            raise Failed(f"several phones run ({', '.join(phones)}): choose one with --serial")
        if not phones:
            ctx["locks"].append(hold(os.path.join(C["dir"], ".start.lock"), "the phone start"))
            phones = running_emulators()   # another run may have started it meanwhile
            if not phones:
                aodm = shutil.which("aodm")
                if not aodm:
                    raise Failed(f"no phone runs and aodm is not installed (adx devices install), or start one: adx emulator start")
                log.info("no phone running: starting %s", C["image"])
                sh([aodm, "start", C["image"], "--window"], ctx["log"], timeout=600)
                phones = running_emulators()
            if not phones:
                raise Failed(f"started {C['image']} but adb sees no phone")
        serial = phones[0]
    # One app at a time on a phone: deploy and smoke of another run would mix the evidence.
    ctx["locks"].append(hold(os.path.join(C["dir"], f".device-{serial}.lock"), serial))
    ctx["serial"] = serial
    image = image_of(serial)
    ctx["run"]["target"] = {"serial": serial, "image": image}
    note = ""
    if not ctx["built"]:   # deploying an earlier build
        ctx["run"]["apk"] = apk_info(ctx["apk"])
        if os.path.getmtime(ctx["apk"]) < newest_source(ctx["root"]):
            note = f"; WARNING: APK built {ctx['run']['apk']['built']}, older than the sources: run the build stage"
    sh([C["adb"], "-s", serial, "install", "-r", "-t", ctx["apk"]], ctx["log"], timeout=300)
    return f"installed {os.path.basename(ctx['apk'])} on {serial} ({image}){note}"


def crash_block(crash, pkg):
    """The crash buffer's entries for this package, whole (cause and library frames kept)."""
    entries = re.split(r"(?m)^(?=.*FATAL EXCEPTION)", crash)
    own = [e for e in entries if re.search(rf"Process: {re.escape(pkg)}(?=[\s,:]|$)", e)]
    return "\n".join(own).strip()


def st_smoke(ctx):
    s, d = ctx["serial"], ctx["dir"]
    pkg = package_from_apk(ctx["apk"]) or ctx["package"]
    if not pkg:
        raise Failed("unknown package (no applicationId in app/build.gradle.kts, no APK)")
    adb = lambda *a, **k: sh([C["adb"], "-s", s, *a], ctx["log"], **k)
    adb("shell", "input", "keyevent", "KEYCODE_WAKEUP", check=False, timeout=15)
    adb("shell", "wm", "dismiss-keyguard", check=False, timeout=15)
    _c, since = adb("shell", "date", "+%s", timeout=15)
    since = since.strip().splitlines()[0] + ".000"
    adb("shell", "am", "force-stop", pkg, timeout=30)
    _c, comp = adb("shell", "cmd", "package", "resolve-activity", "--brief", "-c",
                   "android.intent.category.LAUNCHER", pkg, timeout=30)
    comp = comp.strip().splitlines()[-1] if comp.strip() else ""
    if "/" not in comp:
        raise Failed(f"no launcher activity for {pkg}")
    _c, out = adb("shell", "am", "start", "-W", "-n", comp, timeout=60)
    m = re.search(r"TotalTime: (\d+)", out)
    launched = bool(m) and "Error" not in out
    time.sleep(C["smoke"] if launched else 2)   # a failed launch: just long enough for the crash log
    _c, ps = adb("shell", "ps", "-A", "-o", "NAME", check=False, timeout=15)
    alive = any(re.fullmatch(rf"{re.escape(pkg)}(:.*)?", ln.strip()) for ln in ps.splitlines())
    _c, crash = adb("logcat", "-d", "-b", "crash", "-T", since, check=False, timeout=30, keep_output=False)
    _c, events = adb("logcat", "-d", "-b", "main,system", "-T", since, check=False, timeout=30, keep_output=False)
    with open(os.path.join(d, "logcat.txt"), "w", encoding="utf-8") as fh:
        fh.write(events)
    p = subprocess.run([C["adb"], "-s", s, "exec-out", "screencap", "-p"], capture_output=True, timeout=30)
    if p.returncode == 0 and p.stdout.startswith(b"\x89PNG"):
        with open(os.path.join(d, "screen.png"), "wb") as fh:
            fh.write(p.stdout)
    else:
        log.warning("screencap failed (exit %s, %d bytes): no screen.png", p.returncode, len(p.stdout))
    problems = [] if launched else ["launch did not complete (" + (re.findall(r"(?:Status|LaunchState|Error): .*", out) or ["no status"])[-1] + ")"]
    block = crash_block(crash, pkg)
    if block:
        with open(os.path.join(d, "crash.txt"), "w", encoding="utf-8") as fh:
            fh.write(block + "\n")
        problems.append("crashed (crash.txt)")
    if re.search(rf"ANR in {re.escape(pkg)}(?=[\s,:(]|$)", events):
        problems.append("ANR (app not responding)")
    if launched and not alive and not block:
        swiped = re.search(rf"Killing \d+:{re.escape(pkg)}/\S+ .*: remove task", events)
        problems.append(f"closed from Recents during the {C['smoke']} s wait (someone swiped it away, not an app fault): run again"
                        if swiped else f"not running {C['smoke']} s after launch")
    summary = (f"launched in {m.group(1)} ms; " if launched else "") + (
        "; ".join(problems) if problems else f"running after {C['smoke']} s, no crash or ANR (screen.png, logcat.txt)")
    if problems:
        raise Failed(summary + ("\n" + "\n".join(block.splitlines()[:40]) if block else ""))
    return summary


RUNNERS = {"build": st_build, "check": st_check, "test": st_test, "deploy": st_deploy, "smoke": st_smoke}


# ---------------------------------------------------------------------- run ---
def write(ctx):
    path = os.path.join(ctx["dir"], "run.json")
    with open(path + ".tmp", "w", encoding="utf-8") as fh:
        json.dump(ctx["run"], fh, indent=2)
    os.replace(path + ".tmp", path)


def summary_md(run):
    icon = {"passed": "PASS", "failed": "FAIL", "skipped": "skip", "running": "....", "pending": "    ",
            "interrupted": "STOP"}
    tgt = run.get("target") or {}
    out = [f"# {run['app']} pipeline run {run['id']}: {run['status'].upper()}", "",
           f"- commit {run['git']['commit']}{' + uncommitted changes' if run['git']['dirty'] else ''}",
           f"- started {run['started']}, {run.get('secs', 0):.0f} s",
           f"- target {tgt.get('serial', '-')} ({tgt.get('image', '-')})",
           f"- folder {run['dir']}", ""]
    if run.get("note"):
        out += [run["note"], ""]
    for st in run["stages"]:
        first, *rest = (st.get("summary") or "").splitlines() or [""]
        out.append(f"{icon.get(st['status'], st['status'])}  {st['name']:<7} {st.get('secs', 0):5.0f} s  {first}")
        out += [f"               {ln}" for ln in rest]
    return "\n".join(out) + "\n"


def prune(app_dir):
    runs = sorted(d for d in os.listdir(app_dir) if RUN_ID_RE.match(d))
    for d in runs[:-C["keep"]] if C["keep"] > 0 else []:
        shutil.rmtree(os.path.join(app_dir, d), ignore_errors=True)
        log.info("pruned old run %s (ADX_PIPELINE_KEEP=%d)", d, C["keep"])


def new_run_dir(app_dir):
    base = time.strftime("%Y%m%d-%H%M%S", time.gmtime())   # UTC: ordered across DST changes
    for n in range(1, 100):
        rid = base if n == 1 else f"{base}-{n}"
        try:
            os.makedirs(os.path.join(app_dir, rid))
            return rid
        except FileExistsError:
            continue
    raise Usage("could not create a run folder")


def cmd_run(app, serial, stages, as_json):
    root = app_root(app)
    name = os.path.basename(root)
    app_dir = os.path.join(C["dir"], name)
    os.makedirs(app_dir, exist_ok=True)
    lock = open(os.path.join(app_dir, ".lock"), "w")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        print(f"pipeline: a run of {name} is already in progress", file=sys.stderr)
        return 3
    rid = new_run_dir(app_dir)
    d = os.path.join(app_dir, rid)
    t0 = time.time()
    run = {"schema": 1, "app": name, "id": rid, "dir": d, "root": root, "pid": os.getpid(),
           "package": package_from_gradle(root), "git": git_state(root),
           "started": time.strftime("%Y-%m-%d %H:%M:%S"), "status": "running",
           "stages": [{"name": s, "status": "pending"} if s in stages
                      else {"name": s, "status": "skipped", "summary": "not requested"} for s in STAGES]}
    ctx = {"root": root, "dir": d, "run": run, "serial": serial, "package": run["package"], "apk": None,
           "built": "build" in stages, "locks": []}
    log.info("run %s/%s: stages %s, folder %s", name, rid, ",".join(stages), d)
    if not ctx["built"]:   # deploy/smoke the last build
        apks = sorted(glob.glob(os.path.join(root, "app/build/outputs/apk/debug/*.apk")), key=os.path.getmtime)
        ctx["apk"] = apks[-1] if apks else None
    failed, current = False, None
    try:
        for st in run["stages"]:
            if st["status"] == "skipped":
                continue
            if failed:
                st["status"], st["summary"] = "skipped", "an earlier stage failed"
                continue
            if st["name"] == "smoke" and not ctx["serial"]:
                st["status"], st["summary"] = "skipped", "needs the deploy stage or --serial"
                continue
            if st["name"] in ("deploy", "smoke") and not ctx["apk"]:
                st["status"], st["summary"], failed = "failed", "no APK (run the build stage)", True
                continue
            current = st
            st["status"], st["log"] = "running", f"{st['name']}.log"
            ctx["log"] = os.path.join(d, st["log"])
            write(ctx)
            s0 = time.time()
            logger = logging.getLogger(f"pipeline.{st['name']}")
            logger.info("start")
            try:
                st["summary"] = RUNNERS[st["name"]](ctx)
                st["status"] = "passed"
            except Failed as e:
                st["status"], st["summary"], failed = "failed", str(e), True
            except Exception as e:   # recorded in the run, never lost
                st["status"], st["summary"], failed = "failed", f"internal error: {type(e).__name__}: {e}", True
                logger.exception("internal error")
            st["secs"] = round(time.time() - s0, 1)
            current = None
            logger.log(logging.WARNING if failed else logging.INFO, "%s in %.1f s: %s",
                       st["status"], st["secs"], (st.get("summary") or "").splitlines()[0] if st.get("summary") else "")
            write(ctx)
        if not failed and not any(s["status"] == "passed" for s in run["stages"]):
            failed, run["note"] = True, "Nothing ran: every requested stage was skipped."
        run["status"] = "failed" if failed else "passed"
    except BaseException:   # Ctrl-C, SIGTERM, a closed terminal: the run must not stay "running"
        run["status"] = "interrupted"
        if current:
            current["status"], current["summary"] = "interrupted", "the pipeline was stopped"
        for st in run["stages"]:
            if st["status"] == "pending":
                st["status"], st["summary"] = "skipped", "the pipeline was stopped"
        log.warning("run %s/%s interrupted", name, rid)
        raise
    finally:
        run["secs"] = round(time.time() - t0, 1)
        run["ended"] = time.strftime("%Y-%m-%d %H:%M:%S")
        write(ctx)
        with open(os.path.join(d, "summary.md"), "w", encoding="utf-8") as fh:
            fh.write(summary_md(run))
        latest = os.path.join(app_dir, "latest")
        if os.path.islink(latest) or not os.path.exists(latest):
            tmp = latest + ".tmp"
            if os.path.lexists(tmp):
                os.remove(tmp)
            os.symlink(rid, tmp)
            os.replace(tmp, latest)
        prune(app_dir)
    log.info("run %s/%s %s in %.0f s", name, rid, run["status"], run["secs"])
    print(json.dumps(run, indent=2) if as_json else summary_md(run), end="\n" if as_json else "")
    return 0 if not failed else 1


def lock_free(app_dir):
    """No run of this app is in progress (its lock can be taken)."""
    try:
        with open(os.path.join(app_dir, ".lock"), "a") as fh:
            fcntl.flock(fh, fcntl.LOCK_EX | fcntl.LOCK_NB)
            return True
    except OSError:
        return False


def runs_of(app):
    app_dir = os.path.join(C["dir"], os.path.basename(app_root(app)))
    out = []
    names = sorted((d for d in (os.listdir(app_dir) if os.path.isdir(app_dir) else []) if RUN_ID_RE.match(d)), reverse=True)
    for rid in names:
        try:
            with open(os.path.join(app_dir, rid, "run.json"), encoding="utf-8") as fh:
                run = json.load(fh)
        except (OSError, ValueError):
            continue
        if run.get("status") == "running" and lock_free(app_dir):
            run["status"] = "interrupted"   # its process died without recording the end
        out.append(run)
    return out


def main(argv):
    os.umask(0o077)   # runs hold app logs and screens: this user only
    try:
        load_config()
        setup_logging()
        if len(argv) < 2 or argv[0] not in ("run", "last", "list"):
            raise Usage("usage: adx pipeline run|last|list <App> [...] (see --help in tools/pipeline/pipeline.py)")
        cmd, app, rest = argv[0], argv[1], argv[2:]
        as_json = "--json" in rest
        rest = [a for a in rest if a != "--json"]
        if cmd == "run":
            serial, stages = None, list(STAGES)
            while rest:
                opt, val = rest[0], (rest[1] if len(rest) > 1 else "")
                if opt == "--serial" and re.fullmatch(r"emulator-\d{4,5}", val):
                    serial = val
                elif opt == "--stages" and val and all(s in STAGES for s in val.split(",")):
                    stages = [s for s in STAGES if s in val.split(",")]
                else:
                    raise Usage(f"bad option {opt} {val} (--serial emulator-NNNN, --stages {','.join(STAGES)}, --json)")
                rest = rest[2:]
            signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
            return cmd_run(app, serial, stages, as_json)
        runs = runs_of(app)
    except Usage as e:
        print(f"pipeline: {e}", file=sys.stderr)
        return 2
    except KeyboardInterrupt:
        return 130
    if not runs:
        print(f"pipeline: no runs of {app} yet (adx pipeline run {app})", file=sys.stderr)
        return 1
    if cmd == "last":
        print(json.dumps(runs[0], indent=2) if as_json else summary_md(runs[0]), end="\n" if as_json else "")
        return 0 if runs[0]["status"] == "passed" else 1
    if as_json:
        print(json.dumps([{k: r.get(k) for k in ("id", "status", "started", "secs", "git", "dir")} for r in runs], indent=2))
    else:
        for r in runs:
            stages = " ".join(f"{s['name']}:{s['status']}" for s in r["stages"])
            print(f"{r['id']:<17} {r['status']:<11} {r['git']['commit']:<8} {stages}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
