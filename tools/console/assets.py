"""Assets and phone features for the adx console (Python standard library only).

An asset library (~/.local/share/adx/assets/<kind>/) whose files can be loaded
into a running emulator phone and removed again, plus quick phone features
(location, dark mode, battery, connectivity, display, clean status bar).

Safety: actions only target emulators aodm reports as running (emulator-NNNN
serials, never a physical phone), and removal only deletes files from the
phone folders the console itself loads into (<Dir>/adx/<name>).
"""
import glob
import os
import re
import subprocess
import time

ANDROID_HOME = os.environ.get("ANDROID_HOME") or os.path.expanduser("~/Android/Sdk")
ASSETS_DIR = os.environ.get("ADX_CONSOLE_ASSETS_DIR") or os.path.join(
    os.environ.get("XDG_DATA_HOME") or os.path.expanduser("~/.local/share"), "adx", "assets")
UPLOAD_MAX = int(os.environ.get("ADX_CONSOLE_UPLOAD_MAX_MB") or "200") * 1024 * 1024
SERIAL_RE = re.compile(r"^emulator-\d{4,5}$")
SAFE_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._ -]{0,119}$")

# kind -> allowed extensions, phone folders (None: not pushed), label
KINDS = {
    "wallpapers": {"ext": (".jpg", ".jpeg", ".png", ".webp"), "phone": ["Pictures/adx-wallpapers"]},
    "media": {"ext": (".jpg", ".jpeg", ".png", ".webp", ".gif", ".mp4", ".webm", ".mkv", ".3gp"),
              "phone": ["DCIM/adx"]},
    "sounds": {"ext": (".mp3", ".ogg", ".wav", ".m4a", ".flac", ".aac"),
               "phone": ["Ringtones/adx", "Notifications/adx", "Alarms/adx"]},
    "apps": {"ext": (".apk",), "phone": None},
    "screenshots": {"ext": (".png",), "phone": None},
}
MAGIC = {".png": [b"\x89PNG"], ".jpg": [b"\xff\xd8\xff"], ".jpeg": [b"\xff\xd8\xff"],
         ".gif": [b"GIF8"], ".webp": [b"RIFF"], ".apk": [b"PK\x03\x04"]}
MIME = {".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".webp": "image/webp",
        ".gif": "image/gif", ".mp4": "video/mp4", ".webm": "video/webm", ".mkv": "video/x-matroska",
        ".3gp": "video/3gpp", ".mp3": "audio/mpeg", ".ogg": "audio/ogg", ".wav": "audio/wav",
        ".m4a": "audio/mp4", ".flac": "audio/flac", ".aac": "audio/aac", ".apk": "application/vnd.android.package-archive"}

LOCATIONS = {   # name -> (lat, lon)
    "Tokyo": (35.6895, 139.6917), "New York": (40.7128, -74.0060), "London": (51.5074, -0.1278),
    "Paris": (48.8566, 2.3522), "Sydney": (-33.8688, 151.2093), "São Paulo": (-23.5505, -46.6333),
    "Cape Town": (-33.9249, 18.4241), "Mumbai": (19.0760, 72.8777), "Reykjavík": (64.1466, -21.9426),
    "Mountain View": (37.3861, -122.0839),
}
FONT_SCALES = ("0.85", "1.0", "1.15", "1.3", "1.5", "1.8", "2.0")
STATE = {}   # serial -> features adb cannot read back (demo mode)


def adb_path():
    p = os.path.join(ANDROID_HOME, "platform-tools", "adb")
    return p if os.access(p, os.X_OK) else "adb"


def aapt2():
    found = sorted(glob.glob(os.path.join(ANDROID_HOME, "build-tools", "*", "aapt2")))
    return found[-1] if found else None


class Log:
    """Collects every command and its output; becomes the job log."""
    def __init__(self):
        self.lines, self.ok = [], True

    def run(self, args, timeout=60, check=True, binary=False):
        self.lines.append("$ " + " ".join(args))
        try:
            p = subprocess.run(args, capture_output=True, timeout=timeout)
            out = p.stdout if binary else (p.stdout + p.stderr).decode("utf-8", "replace").strip()
            if not binary and out:
                self.lines.append(out)
            if check and p.returncode != 0:
                self.ok = False
                if binary:
                    self.lines.append(p.stderr.decode("utf-8", "replace").strip())
            return p.returncode, out
        except (OSError, subprocess.SubprocessError) as e:
            self.lines.append(str(e))
            self.ok = False
            return 127, b"" if binary else str(e)

    def note(self, text):
        self.lines.append(text)

    def fail(self, text):
        self.lines.append("error: " + text)
        self.ok = False

    def text(self):
        return "\n".join(self.lines)


def adb(log, serial, *args, **kw):
    return log.run([adb_path(), "-s", serial, *args], **kw)


# ------------------------------------------------------------- library ---
def kind_dir(kind):
    return os.path.join(ASSETS_DIR, kind)


def clean_name(name):
    base = os.path.basename(str(name or "")).strip()
    stem, ext = os.path.splitext(base)
    stem = re.sub(r"[^A-Za-z0-9._ -]+", "_", stem).strip(" .-_")[:100] or "file"
    return f"{stem}{ext.lower()}"


def valid_file(kind, name):
    return (kind in KINDS and SAFE_NAME.match(name or "") and name.lower().endswith(KINDS[kind]["ext"])
            and os.path.isfile(os.path.join(kind_dir(kind), name)))


def library(kind):
    d = kind_dir(kind)
    out = []
    for f in sorted(os.listdir(d)) if os.path.isdir(d) else []:
        p = os.path.join(d, f)
        if os.path.isfile(p) and f.lower().endswith(KINDS[kind]["ext"]) and SAFE_NAME.match(f):
            st = os.stat(p)
            out.append({"name": f, "size": st.st_size, "mtime": int(st.st_mtime),
                        "mime": MIME.get(os.path.splitext(f)[1].lower(), "")})
    return out


def save_upload(kind, name, data):
    """Store an uploaded file in the library. Returns (ok, message, stored name)."""
    if kind not in KINDS or kind == "screenshots":
        return False, "uploads go to wallpapers, media, sounds or apps", ""
    name = clean_name(name)
    ext = os.path.splitext(name)[1]
    if ext not in KINDS[kind]["ext"]:
        return False, f"{kind} accepts {', '.join(KINDS[kind]['ext'])}", ""
    if not data:
        return False, "empty file", ""
    if len(data) > UPLOAD_MAX:
        return False, f"larger than {UPLOAD_MAX // (1024 * 1024)} MB", ""
    if ext in MAGIC and not any(data.startswith(m) for m in MAGIC[ext]):
        return False, f"not a real {ext} file", ""
    if ext == ".webp" and data[8:12] != b"WEBP":
        return False, "not a real .webp file", ""
    os.makedirs(kind_dir(kind), exist_ok=True)
    tmp = os.path.join(kind_dir(kind), f".upload-{os.getpid()}-{time.time_ns()}")
    with open(tmp, "wb") as fh:
        fh.write(data)
    os.replace(tmp, os.path.join(kind_dir(kind), name))
    return True, "stored", name


def delete_local(kind, name):
    if not valid_file(kind, name):
        return False
    os.remove(os.path.join(kind_dir(kind), name))
    return True


# ------------------------------------------------------- phone: files ---
def phone_path(folder, name):
    return f"/sdcard/{folder}/{name}"


def scan(log, serial):
    """Ask MediaStore to rescan shared storage (adds new files, forgets removed ones)."""
    adb(log, serial, "shell", "content", "call", "--uri", "content://media/", "--method",
        "scan_volume", "--arg", "external_primary", check=False, timeout=60)


def on_phone(serial, kind):
    """Names in the console's phone folder for this kind (read-only)."""
    folders = KINDS[kind]["phone"]
    if not folders:
        return set()
    log = Log()
    _c, out = adb(log, serial, "shell", f"ls /sdcard/{folders[0]} 2>/dev/null", check=False, timeout=15)
    return {ln.strip() for ln in out.splitlines() if ln.strip()}


def installed_packages(serial):
    log = Log()
    _c, out = adb(log, serial, "shell", "pm", "list", "packages", "-3", check=False, timeout=20)
    return {ln.split(":", 1)[1].strip() for ln in out.splitlines() if ln.startswith("package:")}


def apk_package(name):
    tool = aapt2()
    if not tool:
        return ""
    try:
        p = subprocess.run([tool, "dump", "packagename", os.path.join(kind_dir("apps"), name)],
                           capture_output=True, text=True, timeout=30)
        pkg = p.stdout.strip()
        return pkg if p.returncode == 0 and re.match(r"^[A-Za-z][\w]*(\.[A-Za-z][\w]*)+$", pkg) else ""
    except (OSError, subprocess.SubprocessError):
        return ""


def load(log, serial, kind, name):
    if kind == "apps":
        return install_apk(log, serial, name)
    src = os.path.join(kind_dir(kind), name)
    for folder in KINDS[kind]["phone"]:
        adb(log, serial, "shell", "mkdir", "-p", f"/sdcard/{folder}")
        adb(log, serial, "push", src, phone_path(folder, name), timeout=300)
    scan(log, serial)


def remove(log, serial, kind, name):
    if kind == "apps":
        return uninstall_apk(log, serial, name)
    for folder in KINDS[kind]["phone"]:
        # name is SAFE_NAME-checked (no quotes, slashes or shell characters)
        adb(log, serial, "shell", f"rm -f '{phone_path(folder, name)}'")
    scan(log, serial)


def media_id(log, serial, folder, name):
    where = f"_data='/storage/emulated/0/{folder}/{name}'"
    _c, out = adb(log, serial, "shell", "content", "query", "--uri", "content://media/external/images/media",
                  "--projection", "_id", "--where", f'"{where}"', check=False, timeout=20)
    m = re.search(r"_id=(\d+)", out)
    return m.group(1) if m else ""


def set_wallpaper(log, serial, name):
    """Load the image (if needed), then open the phone's own set-wallpaper preview on it.
    Android has no adb command that sets a wallpaper; the user confirms on the phone."""
    folder = KINDS["wallpapers"]["phone"][0]
    if name not in on_phone(serial, "wallpapers"):
        load(log, serial, "wallpapers", name)
    mid = ""
    for _ in range(5):
        mid = media_id(log, serial, folder, name)
        if mid:
            break
        time.sleep(1)
    if not mid:
        return log.fail("the phone did not index the image yet; try again")
    mime = MIME.get(os.path.splitext(name)[1].lower(), "image/*")
    _c, acts = adb(log, serial, "shell", "cmd", "package", "query-activities", "--brief", "-a",
                   "android.intent.action.ATTACH_DATA", "-t", mime, check=False, timeout=20)
    comp = next((ln.strip() for ln in acts.splitlines() if "wallpaper" in ln.lower() and "/" in ln), "")
    args = ["shell", "am", "start", "-a", "android.intent.action.ATTACH_DATA",
            "-d", f"content://media/external/images/media/{mid}", "-t", mime, "--grant-read-uri-permission"]
    if comp:
        args[3:3] = ["-n", comp]   # after "shell am start"
    adb(log, serial, *args)
    log.note("Confirm on the phone: choose Home screen, Lock screen or both.")


def install_apk(log, serial, name):
    adb(log, serial, "install", "-r", "-t", os.path.join(kind_dir("apps"), name), timeout=300)


def uninstall_apk(log, serial, name):
    pkg = apk_package(name)
    if not pkg:
        return log.fail("could not read the package name from the APK (aapt2)")
    adb(log, serial, "uninstall", pkg, timeout=120)


def screenshot(log, serial):
    os.makedirs(kind_dir("screenshots"), exist_ok=True)
    name = f"{serial}-{time.strftime('%Y%m%d-%H%M%S')}.png"
    code, data = adb(log, serial, "exec-out", "screencap", "-p", binary=True, timeout=30)
    if code != 0 or not data.startswith(b"\x89PNG"):
        return log.fail("screencap returned no PNG")
    with open(os.path.join(kind_dir("screenshots"), name), "wb") as fh:
        fh.write(data)
    log.note(f"saved {name} ({len(data) // 1024} kB) to {kind_dir('screenshots')}")


# ---------------------------------------------------- phone: features ---
DEMO = "com.android.systemui.demo"


def feature(log, serial, name, value):
    """Apply one phone feature. value is validated per feature."""
    v = str(value if value is not None else "")
    if name == "location":
        if v in LOCATIONS:
            lat, lon = LOCATIONS[v]
        else:
            m = re.match(r"^\s*(-?\d{1,2}(?:\.\d+)?)\s*,\s*(-?\d{1,3}(?:\.\d+)?)\s*$", v)
            if not m or abs(float(m.group(1))) > 90 or abs(float(m.group(2))) > 180:
                return log.fail("location: a preset or 'lat, lon'")
            lat, lon = float(m.group(1)), float(m.group(2))
        adb(log, serial, "emu", "geo", "fix", str(lon), str(lat))   # emulator console: lon first
    elif name == "dark":
        if v not in ("yes", "no"):
            return log.fail("dark: yes or no")
        adb(log, serial, "shell", "cmd", "uimode", "night", v)
    elif name == "battery_level":
        if not v.isdigit() or not 0 <= int(v) <= 100:
            return log.fail("battery level: 0-100")
        adb(log, serial, "emu", "power", "capacity", v)
    elif name == "charging":
        if v not in ("on", "off"):
            return log.fail("charging: on or off")
        adb(log, serial, "emu", "power", "ac", v)
        adb(log, serial, "emu", "power", "status", "charging" if v == "on" else "discharging")
    elif name in ("airplane", "wifi", "data"):
        if v not in ("on", "off"):
            return log.fail(f"{name}: on or off")
        if name == "airplane":
            adb(log, serial, "shell", "cmd", "connectivity", "airplane-mode", "enable" if v == "on" else "disable")
        else:
            adb(log, serial, "shell", "svc", name, "enable" if v == "on" else "disable")
    elif name == "font_scale":
        if v not in FONT_SCALES:
            return log.fail("font scale: " + ", ".join(FONT_SCALES))
        adb(log, serial, "shell", "settings", "put", "system", "font_scale", v)
    elif name == "animations":
        if v not in ("on", "off"):
            return log.fail("animations: on or off")
        for key in ("window_animation_scale", "transition_animation_scale", "animator_duration_scale"):
            adb(log, serial, "shell", "settings", "put", "global", key, "1" if v == "on" else "0")
    elif name == "touches":
        if v not in ("on", "off"):
            return log.fail("touches: on or off")
        adb(log, serial, "shell", "settings", "put", "system", "show_touches", "1" if v == "on" else "0")
    elif name == "demo":
        if v not in ("on", "off"):
            return log.fail("clean status bar: on or off")
        if v == "on":
            adb(log, serial, "shell", "settings", "put", "global", "sysui_demo_allowed", "1")
            for extra in (["-e", "command", "enter"], ["-e", "command", "clock", "-e", "hhmm", "1200"],
                          ["-e", "command", "battery", "-e", "level", "100", "-e", "plugged", "false"],
                          ["-e", "command", "network", "-e", "wifi", "show", "-e", "level", "4"],
                          ["-e", "command", "network", "-e", "mobile", "show", "-e", "datatype", "none", "-e", "level", "4"],
                          ["-e", "command", "notifications", "-e", "visible", "false"]):
                adb(log, serial, "shell", "am", "broadcast", "-a", DEMO, *extra)
        else:
            adb(log, serial, "shell", "am", "broadcast", "-a", DEMO, "-e", "command", "exit")
        STATE.setdefault(serial, {})["demo"] = v
    elif name == "screenshot":
        screenshot(log, serial)
    else:
        log.fail("unknown feature " + name)


def phone_state(serial):
    """Current values of the features (read-only)."""
    log = Log()
    g = lambda *a: adb(log, serial, "shell", *a, check=False, timeout=15)[1].strip()   # noqa: E731
    night = g("cmd", "uimode", "night").replace("Night mode:", "").strip()
    battery = g("dumpsys", "battery")
    level = re.search(r"^\s*level:\s*(\d+)", battery, re.M)
    ac = re.search(r"AC powered:\s*(\w+)", battery)
    return {
        "dark": "yes" if night == "yes" else "no",
        "battery_level": int(level.group(1)) if level else None,
        "charging": "on" if ac and ac.group(1) == "true" else "off",
        "airplane": "on" if g("cmd", "connectivity", "airplane-mode") == "enabled" else "off",
        "wifi": "off" if g("settings", "get", "global", "wifi_on") in ("0", "") else "on",
        "data": "on" if g("settings", "get", "global", "mobile_data") == "1" else "off",
        "font_scale": g("settings", "get", "system", "font_scale") or "1.0",
        "animations": "off" if g("settings", "get", "global", "window_animation_scale") in ("0", "0.0") else "on",
        "touches": "on" if g("settings", "get", "system", "show_touches") == "1" else "off",
        "demo": STATE.get(serial, {}).get("demo", "off"),
    }
