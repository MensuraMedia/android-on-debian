#!/usr/bin/env python3
"""adx console: local web GUI for android-on-debian (Python standard library only).

Pages: Images (emulator phones via aodm, SDK system images), Apps (render, run,
open), Modules (adx list, install, doctor), Skills, Activity (jobs + adx.log)
and Settings. Binds to 127.0.0.1. Reads are free; every action is a POST that
needs a same-origin request and the per-process token, and runs as a job whose
output goes to a log file.

Started by `adx console start` (which passes the configuration in the
environment); run directly with:  python3 tools/console/server.py
"""
import json
import os
import re
import secrets
import shutil
import signal
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, unquote, urlparse

import assets

HERE = os.path.dirname(os.path.abspath(__file__))
STATIC = os.path.join(HERE, "static")
ADX_ROOT = os.environ.get("ADX_ROOT") or os.path.dirname(os.path.dirname(HERE))
ADX = os.path.join(ADX_ROOT, "adx")
APPS_DIR = os.environ.get("ADX_APPS_DIR") or os.path.expanduser("~/projects/android-apps")
LOG_DIR = os.environ.get("ADX_LOG_DIR") or os.path.expanduser("~/.local/state/adx")
JOB_DIR = os.path.join(LOG_DIR, "console")
ANDROID_HOME = os.environ.get("ANDROID_HOME") or os.path.expanduser("~/Android/Sdk")
AVD_HOME = os.environ.get("ANDROID_AVD_HOME") or os.path.expanduser("~/.android/avd")
HOST = "127.0.0.1"
PORT = int(os.environ.get("ADX_CONSOLE_PORT") or "8766")
TOKEN = secrets.token_urlsafe(24)
NAME_RE = re.compile(r"^[A-Za-z0-9._-]+$")


def run(cmd, timeout=60, cwd=None):
    """Run a command; return (exit code, stdout+stderr). Never raises."""
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout, cwd=cwd)
        return p.returncode, p.stdout + p.stderr
    except (OSError, subprocess.SubprocessError) as e:
        return 127, str(e)


def aodm():
    return shutil.which("aodm") or os.path.expanduser("~/.local/bin/aodm")


def adb():
    p = os.path.join(ANDROID_HOME, "platform-tools", "adb")
    return p if os.access(p, os.X_OK) else (shutil.which("adb") or "adb")


def read_props(path, sep="="):
    out = {}
    try:
        with open(path, encoding="utf-8") as fh:
            for line in fh:
                k, s, v = line.partition(sep)
                if s and not k.lstrip().startswith("#"):
                    out[k.strip()] = v.strip()
    except OSError:
        pass
    return out


# ---------------------------------------------------------------- jobs ---
JOBS = {}
JOBS_LOCK = threading.Lock()


def start_job(kind, target, cmd, cwd=None):
    """Run cmd in the background; one running job per (kind, target)."""
    with JOBS_LOCK:
        for j in JOBS.values():
            if j["kind"] == kind and j["target"] == target and j["status"] == "running":
                return j, False
        jid = f"{int(time.time())}-{len(JOBS) + 1}"
        os.makedirs(JOB_DIR, exist_ok=True)
        log = os.path.join(JOB_DIR, f"{jid}-{kind}-{target}.log")
        job = {"id": jid, "kind": kind, "target": target, "cmd": " ".join(cmd),
               "status": "running", "exit": None, "started": time.strftime("%Y-%m-%d %H:%M:%S"),
               "ended": None, "log": log}
        JOBS[jid] = job

    def worker():
        with open(log, "ab") as fh:
            fh.write(f"$ {job['cmd']}\n".encode())
            fh.flush()
            try:
                p = subprocess.Popen(cmd, cwd=cwd, stdin=subprocess.DEVNULL, stdout=fh,
                                     stderr=subprocess.STDOUT, start_new_session=True)
                code = p.wait()
            except OSError as e:
                fh.write(f"{e}\n".encode())
                code = 127
        job.update(status="ok" if code == 0 else "failed", exit=code,
                   ended=time.strftime("%Y-%m-%d %H:%M:%S"))

    threading.Thread(target=worker, daemon=True).start()
    return job, True


def sync_job(kind, target, fn):
    """Run a quick action now (fn gets an assets.Log) and record it as a finished job."""
    log = assets.Log()
    started = time.strftime("%Y-%m-%d %H:%M:%S")
    try:
        fn(log)
    except Exception as e:   # recorded in the job, never a 500 without a log
        log.fail(f"{type(e).__name__}: {e}")
    with JOBS_LOCK:
        jid = f"{int(time.time())}-{len(JOBS) + 1}"
        os.makedirs(JOB_DIR, exist_ok=True)
        path = os.path.join(JOB_DIR, f"{jid}-{kind}-{re.sub(r'[^A-Za-z0-9._-]', '_', target)[:60]}.log")
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(log.text() + "\n")
        JOBS[jid] = {"id": jid, "kind": kind, "target": target, "cmd": kind, "log": path,
                     "status": "ok" if log.ok else "failed", "exit": 0 if log.ok else 1,
                     "started": started, "ended": time.strftime("%Y-%m-%d %H:%M:%S")}
    return {"ok": log.ok, "job": jid, "output": log.text()[-4000:]}


def tail(path, lines=80):
    try:
        with open(path, "rb") as fh:
            fh.seek(0, 2)
            size = fh.tell()
            fh.seek(max(0, size - 64 * 1024))
            data = fh.read().decode("utf-8", "replace")
    except OSError:
        return ""
    data = re.sub(r"\x1b\[[0-9;]*m", "", data)
    return "\n".join(data.splitlines()[-lines:])


def jobs_view():
    with JOBS_LOCK:
        jobs = sorted(JOBS.values(), key=lambda j: j["id"], reverse=True)
    return [{**j, "tail": tail(j["log"], 40)} for j in jobs]


def busy(kind, target):
    with JOBS_LOCK:
        return any(j["kind"] == kind and j["target"] == target and j["status"] == "running"
                   for j in JOBS.values())


# -------------------------------------------------------------- images ---
def emulator_phones():
    code, out = run([aodm(), "list", "--json"], timeout=30)
    try:
        rows = json.loads(out[out.index("{"):]).get("results", []) if code == 0 else []
    except ValueError:
        rows = []
    phones = []
    for r in rows:
        name = r.get("name", "")
        cfg = read_props(os.path.join(AVD_HOME, f"{name}.avd", "config.ini"))
        device = r.get("device") or cfg.get("hw.device.name", "")
        model = " ".join(w.capitalize() for w in device.split("_")) or "Generic phone"
        maker = cfg.get("hw.device.manufacturer", "")
        state = r.get("state", "")
        phones.append({
            "name": name, "phone": f"{maker} {model}".strip() if maker and not model.startswith(maker) else model,
            "api": r.get("api", ""), "abi": r.get("abi", ""), "variant": r.get("tag", "").replace("_", " "),
            "ram": r.get("ram", ""), "size": r.get("size", ""), "saves": r.get("saves", ""),
            "groups": r.get("groups", ""), "state": state, "running": state.startswith("running"),
            "screen": f'{cfg.get("hw.lcd.width", "?")}×{cfg.get("hw.lcd.height", "?")}',
            "keyboard": cfg.get("hw.keyboard", ""),
            "busy": next((k for k in ("start", "stop", "save") if busy(k, name)), ""),
        })
    return phones, (code == 0)


def running_emulators():
    """Running emulator phones as adb serials (the only targets of asset/feature actions)."""
    out = []
    for p in emulator_phones()[0]:
        m = re.search(r":(\d+)", p["state"])
        if p["running"] and m:
            out.append({"serial": f"emulator-{m.group(1)}", "name": p["name"], "phone": p["phone"]})
    return out


def assets_view(serial):
    phones = running_emulators()
    serials = [p["serial"] for p in phones]
    target = serial if serial in serials else (serials[0] if serials else None)
    kinds = {}
    for kind in assets.KINDS:
        items = assets.library(kind)
        if kind == "apps":
            installed = assets.installed_packages(target) if target else set()
            for i in items:
                i["package"] = assets.apk_package(i["name"])
                i["on_phone"] = i["package"] in installed
        else:
            present = assets.on_phone(target, kind) if target else set()
            for i in items:
                i["on_phone"] = i["name"] in present
        kinds[kind] = items
    return {"phones": phones, "target": target, "kinds": kinds, "dir": assets.ASSETS_DIR,
            "locations": list(assets.LOCATIONS), "font_scales": list(assets.FONT_SCALES),
            "max_mb": assets.UPLOAD_MAX // (1024 * 1024),
            "ext": {k: list(v["ext"]) for k, v in assets.KINDS.items()},
            "phone_dirs": {k: v["phone"] for k, v in assets.KINDS.items()}}


def act_asset(kind, action, body):
    serial = str(body.get("serial", ""))
    if kind not in assets.KINDS:
        return 404, {"ok": False, "reason": "unknown asset kind"}
    if action == "delete":
        name = str(body.get("name", ""))
        return (200, {"ok": True}) if assets.delete_local(kind, name) else (404, {"ok": False, "reason": "no such file"})
    if action == "open":
        os.makedirs(assets.kind_dir(kind), exist_ok=True)
        job, _new = start_job("open", kind, ["xdg-open", assets.kind_dir(kind)])
        return 200, {"ok": True, "job": job["id"]}
    if action not in ("load", "remove", "set") or (action == "set" and kind != "wallpapers"):
        return 404, {"ok": False, "reason": "unknown action"}
    if assets.KINDS[kind]["phone"] is None and kind != "apps":
        return 400, {"ok": False, "reason": f"{kind} stay on this computer"}
    if not assets.SERIAL_RE.match(serial) or serial not in {p["serial"] for p in running_emulators()}:
        return 400, {"ok": False, "reason": "no running emulator phone with that serial"}
    names = [i["name"] for i in assets.library(kind)] if body.get("all") else [str(body.get("name", ""))]
    names = [n for n in names if assets.valid_file(kind, n)]
    if not names:
        return 404, {"ok": False, "reason": "no such file in the library"}

    def fn(log):
        for n in names:
            if action == "set":
                assets.set_wallpaper(log, serial, n)
            else:
                (assets.load if action == "load" else assets.remove)(log, serial, kind, n)
    target = f"{serial}:{names[0] if len(names) == 1 else str(len(names)) + ' files'}"
    return 200, sync_job(f"{action}-{kind}", target, fn)


def act_feature(body):
    serial, name = str(body.get("serial", "")), str(body.get("name", ""))
    if not assets.SERIAL_RE.match(serial) or serial not in {p["serial"] for p in running_emulators()}:
        return 400, {"ok": False, "reason": "no running emulator phone with that serial"}
    return 200, sync_job(f"phone-{name}", serial, lambda log: assets.feature(log, serial, name, body.get("value")))


def system_images():
    base = os.path.join(ANDROID_HOME, "system-images")
    used = {}
    for f in os.listdir(AVD_HOME) if os.path.isdir(AVD_HOME) else []:
        if f.endswith(".avd"):
            d = read_props(os.path.join(AVD_HOME, f, "config.ini")).get("image.sysdir.1", "").strip("/")
            used.setdefault(d, []).append(f[:-4])
    out = []
    for root, _dirs, files in os.walk(base):
        if "source.properties" not in files:
            continue
        p = read_props(os.path.join(root, "source.properties"))
        rel = os.path.relpath(root, ANDROID_HOME)
        api = p.get("AndroidVersion.ApiLevel", "")
        out.append({
            "title": f"Android API {api}", "variant": p.get("SystemImage.TagDisplay", ""),
            "abi": p.get("SystemImage.Abi", ""), "revision": p.get("Pkg.Revision", ""),
            "extension": p.get("AndroidVersion.ExtensionLevel", ""), "desc": p.get("Pkg.Desc", ""),
            "package": rel.replace(os.sep, ";"), "path": rel, "used_by": used.get(rel, []),
        })
    return sorted(out, key=lambda i: i["package"], reverse=True)


# ---------------------------------------------------------------- apps ---
def app_dirs():
    if not os.path.isdir(APPS_DIR):
        return []
    return sorted(d for d in os.listdir(APPS_DIR)
                  if NAME_RE.match(d) and os.path.isfile(os.path.join(APPS_DIR, d, "gradlew")))


def app_shots(d):
    for sub in ("app/src/test/screenshots", "app/build/outputs/roborazzi", "docs/mockups"):
        p = os.path.join(APPS_DIR, d, sub)
        if os.path.isdir(p):
            pngs = sorted(f for f in os.listdir(p) if f.endswith(".png"))
            if pngs:
                return p, pngs
    return None, []


def app_package(d):
    try:
        with open(os.path.join(APPS_DIR, d, "app", "build.gradle.kts"), encoding="utf-8") as fh:
            m = re.search(r'applicationId\s*=\s*"([^"]+)"', fh.read())
            return m.group(1) if m else ""
    except OSError:
        return ""


def apps():
    out = []
    for d in app_dirs():
        root = os.path.join(APPS_DIR, d)
        shot_dir, shots = app_shots(d)
        bc, branch = run(["git", "-C", root, "rev-parse", "--abbrev-ref", "HEAD"], timeout=5)
        lc, last = run(["git", "-C", root, "log", "-1", "--format=%h %s (%cr)"], timeout=5)
        skills = os.path.join(root, ".claude", "skills")
        out.append({
            "name": d, "package": app_package(d), "path": root,
            "shots": len(shots), "shot_dir": os.path.relpath(shot_dir, root) if shot_dir else "",
            "branch": branch.strip() if bc == 0 else "", "last_commit": last.strip() if lc == 0 else "",
            "skills": sorted(s for s in os.listdir(skills) if os.path.isdir(os.path.join(skills, s)))
                      if os.path.isdir(skills) else [],
            "has_claude_md": os.path.isfile(os.path.join(root, "CLAUDE.md")),
            "busy": next((k for k in ("render", "run") if busy(k, d)), ""),
        })
    return out


# ------------------------------------------------------------- modules ---
def modules():
    code, out = run([ADX, "list"], timeout=120)
    rows = []
    for line in re.sub(r"\x1b\[[0-9;]*m", "", out).splitlines()[1:]:
        m = re.match(r"^(\S+)\s+(\S+)\s+(.*)$", line)
        if m:
            rows.append({"name": m.group(1), "status": m.group(2), "desc": m.group(3),
                         "busy": busy("install", m.group(1))})
    return rows


def skills():
    rows = []
    try:
        with open(os.path.join(ADX_ROOT, "config", "skills.manifest"), encoding="utf-8") as fh:
            comment = ""
            for line in fh:
                line = line.strip()
                if line.startswith("#"):
                    comment = line.lstrip("# ")
                    continue
                parts = [p.strip() for p in line.split("|")]
                if len(parts) == 4:
                    rows.append({"name": parts[0], "repo": parts[1], "commit": parts[2],
                                 "path": parts[3], "note": comment,
                                 "apps": [a for a in app_dirs() if os.path.isdir(
                                     os.path.join(APPS_DIR, a, ".claude", "skills", parts[0]))]})
    except OSError:
        pass
    return rows


def settings():
    values = []
    for src in ("defaults.env", "local.env"):
        path = os.path.join(ADX_ROOT, "config", src)
        try:
            with open(path, encoding="utf-8") as fh:
                for line in fh:
                    m = re.match(r'^\s*(ADX_[A-Z0-9_]+)=("([^"]*)"|\S*)\s*(#\s*(.*))?$', line)
                    if m:
                        values = [v for v in values if v["key"] != m.group(1)]
                        values.append({"key": m.group(1), "value": m.group(3) if m.group(3) is not None else m.group(2),
                                       "note": m.group(5) or "", "source": src})
        except OSError:
            continue
    _c, env = run([ADX, "env"], timeout=60)
    return {"values": values, "env": re.sub(r"\x1b\[[0-9;]*m", "", env)}


def devices():
    _c, out = run([adb(), "devices"], timeout=10)
    return [ln.split()[0] for ln in out.splitlines()[1:] if ln.strip() and ln.split()[-1] == "device"]


# ------------------------------------------------------------- actions ---
def act_image(name, action):
    names = {p["name"] for p in emulator_phones()[0]}
    if name not in names:
        return 404, {"ok": False, "reason": "unknown image"}
    cmd = {"start": [aodm(), "start", name, "--window"],
           "stop": [aodm(), "stop", name],
           "save": [aodm(), "save", "create", name]}.get(action)
    if not cmd:
        return 404, {"ok": False, "reason": "unknown action"}
    job, new = start_job(action, name, cmd)
    return 200, {"ok": True, "job": job["id"], "new": new}


def act_app(name, action):
    if name not in app_dirs():
        return 404, {"ok": False, "reason": "unknown app"}
    root = os.path.join(APPS_DIR, name)
    if action == "render":
        job, new = start_job("render", name, ["./gradlew", "recordRoborazziDebug"], cwd=root)
    elif action == "run":
        pkg = app_package(name)
        if not pkg:
            return 400, {"ok": False, "reason": "no applicationId in app/build.gradle.kts"}
        script = ('./gradlew installDebug && "$1" shell monkey -p "$2" '
                  '-c android.intent.category.LAUNCHER 1')
        job, new = start_job("run", name, ["bash", "-c", script, "run", adb(), pkg], cwd=root)
    elif action == "open":
        job, new = start_job("open", name, ["xdg-open", root])
    else:
        return 404, {"ok": False, "reason": "unknown action"}
    return 200, {"ok": True, "job": job["id"], "new": new}


def act_module(name, action):
    if action == "doctor" and name == "all":
        job, new = start_job("doctor", "all", [ADX, "doctor"])
        return 200, {"ok": True, "job": job["id"], "new": new}
    if action != "install" or name not in {m["name"] for m in modules()}:
        return 404, {"ok": False, "reason": "unknown module or action"}
    job, new = start_job("install", name, [ADX, name, "install"])
    return 200, {"ok": True, "job": job["id"], "new": new}


def act_open_sysimage(path):
    base = os.path.realpath(os.path.join(ANDROID_HOME, "system-images"))
    target = os.path.realpath(os.path.join(ANDROID_HOME, path))
    if not target.startswith(base + os.sep) or not os.path.isdir(target):
        return 400, {"ok": False, "reason": "not a system-images folder"}
    job, new = start_job("open", os.path.basename(target), ["xdg-open", target])
    return 200, {"ok": True, "job": job["id"], "new": new}


# ---------------------------------------------------------------- HTTP ---
CTYPES = {".html": "text/html; charset=utf-8", ".svg": "image/svg+xml", ".png": "image/png",
          ".css": "text/css", ".js": "text/javascript", ".txt": "text/plain; charset=utf-8"}


class Handler(BaseHTTPRequestHandler):
    server_version = "adx-console"

    def log_message(self, fmt, *args):   # quiet: actions are logged as jobs
        pass

    def send(self, code, body, ctype="application/json"):
        data = json.dumps(body).encode() if ctype == "application/json" else body
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.end_headers()
        self.wfile.write(data)

    def host_ok(self):
        """Refuse other Host names (DNS rebinding)."""
        host = (self.headers.get("Host") or "").rsplit(":", 1)[0].strip("[]").lower()
        return host in ("127.0.0.1", "localhost", "::1")

    def origin_ok(self):
        origin = self.headers.get("Origin") or self.headers.get("Referer")
        if not origin:
            return True
        return (urlparse(origin).hostname or "").lower() in ("127.0.0.1", "localhost", "::1")

    def file(self, path):
        try:
            with open(path, "rb") as fh:
                data = fh.read()
        except OSError:
            return self.send(404, {"error": "not found"})
        ext = os.path.splitext(path)[1].lower()
        self.send(200, data, CTYPES.get(ext) or assets.MIME.get(ext, "application/octet-stream"))

    def do_GET(self):
        if not self.host_ok():
            return self.send(403, {"error": "bad host"})
        path = unquote(urlparse(self.path).path)
        if path in ("/", "/index.html"):
            return self.file(os.path.join(STATIC, "index.html"))
        if path.startswith("/static/"):
            target = os.path.realpath(os.path.join(STATIC, path[len("/static/"):]))
            if not target.startswith(os.path.realpath(STATIC) + os.sep):
                return self.send(404, {"error": "not found"})
            return self.file(target)
        m = re.match(r"^/api/apps/([^/]+)/shot/([^/]+\.png)$", path)
        if m:
            name, png = m.groups()
            if name not in app_dirs():
                return self.send(404, {"error": "not found"})
            d, shots = app_shots(name)
            return self.file(os.path.join(d, png)) if png in shots else self.send(404, {"error": "not found"})
        routes = {
            "/api/health": lambda: {"ok": True, "app": "adx console", "root": ADX_ROOT},
            "/api/token": lambda: {"token": TOKEN},
            "/api/images": lambda: (lambda p: {"phones": p[0], "aodm": p[1], "system_images": system_images()})(emulator_phones()),
            "/api/apps": lambda: {"apps": apps(), "dir": APPS_DIR},
            "/api/modules": lambda: {"modules": modules()},
            "/api/skills": lambda: {"skills": skills()},
            "/api/jobs": lambda: {"jobs": jobs_view()},
            "/api/log": lambda: {"path": os.path.join(LOG_DIR, "adx.log"), "text": tail(os.path.join(LOG_DIR, "adx.log"), 300)},
            "/api/settings": settings,
            "/api/devices": lambda: {"devices": devices()},
        }
        if path in routes:
            return self.send(200, routes[path]())
        q = parse_qs(urlparse(self.path).query)
        if path == "/api/assets":
            return self.send(200, assets_view((q.get("serial") or [""])[0]))
        if path == "/api/phone/state":
            serial = (q.get("serial") or [""])[0]
            if serial not in {p["serial"] for p in running_emulators()}:
                return self.send(400, {"ok": False, "reason": "no running emulator phone with that serial"})
            return self.send(200, assets.phone_state(serial))
        m = re.match(r"^/api/assets/([a-z]+)/file/([^/]+)$", path)
        if m:
            kind, name = m.groups()
            if not assets.valid_file(kind, name):
                return self.send(404, {"error": "not found"})
            return self.file(os.path.join(assets.kind_dir(kind), name))
        m = re.match(r"^/api/apps/([^/]+)/shots$", path)
        if m and m.group(1) in app_dirs():
            return self.send(200, {"shots": app_shots(m.group(1))[1]})
        self.send(404, {"error": "not found"})

    def do_POST(self):
        if not self.host_ok() or not self.origin_ok() or self.headers.get("X-Adx-Token") != TOKEN:
            return self.send(403, {"ok": False, "reason": "missing or invalid token"})
        path = unquote(urlparse(self.path).path)
        n = int(self.headers.get("Content-Length") or 0)
        m = re.match(r"^/api/assets/([a-z]+)/upload$", path)
        if m:
            if n > assets.UPLOAD_MAX:
                return self.send(413, {"ok": False, "reason": f"larger than {assets.UPLOAD_MAX // (1024 * 1024)} MB"})
            data = self.rfile.read(n) if n else b""
            name = (parse_qs(urlparse(self.path).query).get("name") or [""])[0]
            ok, msg, stored = assets.save_upload(m.group(1), name, data)
            return self.send(200 if ok else 400, {"ok": ok, "reason": msg, "name": stored})
        if n > 64 * 1024:
            return self.send(413, {"ok": False, "reason": "request too large"})
        try:
            body = json.loads(self.rfile.read(n) or b"{}") if n else {}
        except ValueError:
            body = {}
        if not isinstance(body, dict):
            body = {}
        m = re.match(r"^/api/assets/([a-z]+)/([a-z]+)$", path)
        if m:
            code, res = act_asset(m.group(1), m.group(2), body)
            return self.send(code, res)
        if path == "/api/phone/feature":
            code, res = act_feature(body)
            return self.send(code, res)
        m = re.match(r"^/api/(images|apps|modules)/([^/]+)/([a-z]+)$", path)
        if m and NAME_RE.match(m.group(2)):
            kind, name, action = m.groups()
            code, res = {"images": act_image, "apps": act_app, "modules": act_module}[kind](name, action)
            return self.send(code, res)
        if path == "/api/system-images/open":
            code, res = act_open_sysimage(str(body.get("path", "")))
            return self.send(code, res)
        if path == "/api/shutdown":
            self.send(200, {"stopping": True})
            threading.Thread(target=lambda: (time.sleep(0.2), os.kill(os.getpid(), signal.SIGTERM)), daemon=True).start()
            return None
        self.send(404, {"ok": False, "reason": "not found"})


def main():
    srv = ThreadingHTTPServer((HOST, PORT), Handler)
    signal.signal(signal.SIGTERM, lambda *_: threading.Thread(target=srv.shutdown, daemon=True).start())
    print(f"adx console: http://{HOST}:{PORT}", flush=True)
    srv.serve_forever()


if __name__ == "__main__":
    sys.exit(main())
