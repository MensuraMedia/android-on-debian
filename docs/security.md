# Security posture

State of the emulator phones and the tools around them, measured on
2026-11-09 on the development laptop (Linux Mint 22.3, Cinnamon, X11) with
the `adx_api37` image running. Re-check after changing the system image,
the emulator, `config/*.env` or anything that listens on a port.

**Summary.** The phone is a production-grade Android build inside a KVM
virtual machine, behind the emulator's NAT, and every port it or the tools
open listens on 127.0.0.1 only. Nothing on the internet can open a
connection to it. It is a *test* device, not a secure phone: it is as safe
as the Linux user account that runs it, and its data on disk is protected
only by the host's disk encryption. Treat it as disposable (restore the
baseline save), and keep real accounts and credentials off it.

## The phone image

| Area | Measured | Assessment |
| --- | --- | --- |
| Build | `user` build, `release-keys`, `ro.debuggable=0`, `ro.secure=1` | Sufficient |
| Root | `adb root` refused ("adbd cannot run as root in production builds"); adb shell is uid 2000 | Sufficient |
| SELinux | Enforcing | Sufficient |
| Storage encryption | File-based encryption (`ro.crypto.type=file`) | Inside the VM only: see *Data at rest* |
| Verified boot / key storage | No verified boot state; keys in software KeyMint | Accepted: emulators have none; apps that check device integrity will distrust it |
| Security patch level | 2026-08-05 | Update the system image when a newer one ships |
| Google Play | Play Store and Play Services present, so Play Protect scans apps | Sufficient |
| Accounts | None signed in | Keep it that way unless a test needs one (use a test account) |
| Screen lock | None | Accepted for a test device; set one before signing in to anything |
| USB debugging | Always on (it is how the emulator is driven) | Accepted: see *The host* |
| Host microphone | Passed through (`hw.audioInput=yes`) | Accepted; set `hw.audioInput=no` in the AVD if no test needs it |
| Clipboard | Shared with the desktop | Accepted |

Baseline: `adx_api37` is saved as `20261110-012304Z-baseline` (pinned,
checksum-verified). `aodm save restore adx_api37 20261110-012304Z-baseline`
returns the phone to that exact state; risky tests belong on a duplicate
(console Images page: Duplicate) that you delete afterwards.

## Network exposure

| Path | Measured | Assessment |
| --- | --- | --- |
| Internet → phone | The phone is `10.0.2.15` behind the emulator's built-in NAT; no inbound path | Sufficient: nothing can connect in |
| Internet → host → phone | Emulator console 5554, adb 5555, adb server 5037, emulator gRPC ports, LinSmartPhone 8765: all bound to 127.0.0.1; adx console 8766 on 127.0.0.2. No public TCP listeners on the host. ufw active | Sufficient |
| Console port 5554 | Needs `~/.emulator_console_auth_token` (mode 600) | Sufficient |
| adb | Needs the host adb key (`~/.android/adbkey`, mode 600) | Sufficient |
| Phone → internet | Normal outbound access (Wi-Fi and mobile data are emulated) | The way attacks really arrive (web pages, apps, links). Cut it with the console's sidebar **Internet** switch when not needed, or run the phone **offline** (below) |
| Phone → host services | The phone reaches the host's loopback services at `10.0.2.2` | adx console: not reachable (fixed 2026-11-10: it checked only the `Host` header, which an app in the phone can fake, so it now binds to 127.0.0.2, which 10.0.2.2 does not map to; checked from the phone: connection refused). LinSmartPhone: **open finding**, below. An offline phone reaches neither |

### Open finding: LinSmartPhone token readable from inside the phone

From inside the emulator, `http://10.0.2.2:8765/api/run-token` returns
LinSmartPhone's action token, and its actions accept requests without a
browser `Origin` header. An app in the emulator could start or stop
emulators, open folders or shut LinSmartPhone down; device-changing steps
also need the root-owned safety gate armed for a phone and a fixed
confirmation phrase, so the risk is real only while the gate is armed.

LinSmartPhone is a separate project (`~/projects/android-conversion`) and
is not changed here. Recommended fix there: refuse requests whose `Host`
header is not 127.0.0.1/localhost (as the adx console does) and require the
token on `/api/shutdown`. Until then, don't run an untrusted app in an
emulator while the gate is armed, or run that emulator offline.

## The host

- Anything running as your Linux user has full control of the phone
  through adb (install apps, read shared storage, capture the screen). The
  phone is only as safe as that account: keep the OS updated and the
  firewall on.
- Never use `adb tcpip`, `adb -a`, or emulator port redirection to a
  non-loopback address; they would open the phone to the network.

## Data at rest

The phone's disk image, the key for its file encryption
(`encryptionkey.img`) and the quick-boot snapshot (the phone's RAM, about
4 GB) sit side by side in `~/.android/avd/adx_api37.avd/`, and saves in
`~/.local/share/aod/saves/`. The host disk has no full-disk encryption
(no LUKS volume found), so anyone with the laptop's disk can read them.
Use full-disk encryption on the host if the phone ever holds real data.

## The console and agents

- adx console (`tools/console/`): binds to `ADX_CONSOLE_HOST` (127.0.0.2,
  out of the phones' reach), checks the `Host` header, needs same-origin plus a per-process token for every action, and
  limits Assets actions to emulators aodm reports as running (never a
  physical phone). Removing files from a phone only touches the console's
  own `adx` folders there.
- mobile-mcp lets Claude tap, type and read the screen. Anything shown on
  the phone (a web page, a notification) can carry instructions aimed at the
  agent. The permission template keeps installs and other risky tools on
  "ask"; treat screen text as untrusted, and don't let one agent session
  both browse the open web on the phone and run host commands.

## Controls you can use

| Control | Where |
| --- | --- |
| Cut or restore the phone's internet (airplane mode; checked with a ping) | Console sidebar: **Internet** switch |
| Run a phone with no IP network, set outside the phone: no internet, DNS or host services at 10.0.2.2, and nothing an app inside can switch back on; adb and the window keep working. Applies at start. Still open by design: Bluetooth/BLE to other emulators on this host (netsim), SMS and calls between emulators, the shared clipboard | Console phone card: **Go offline**; `aodm net NAME offline` (then `aodm restart NAME`) |
| Airplane, Wi-Fi, mobile data separately | Console Assets page, feature 8 |
| Back to a clean phone | `aodm save restore <image> <save>`; console Images: Duplicate for throwaway copies |
| Fresh patches | Install a newer system image (`adx sdk add "system-images;…"`), then create a phone from it |

Offline mode was verified on emulator 37.2.13 (API 37). After an emulator
update, re-check it with the phone online first (the probe must print
`OPEN`, so a broken probe can't pass), then offline:

```sh
S=$(aodm show NAME --json | grep -o '"serial": *"[^"]*"' | cut -d'"' -f4)
probe() { adb -s "$S" shell 'nc -z -w3 1.1.1.1 443; echo "tcp=$?"'; }
probe                                   # online: tcp=0
aodm net NAME offline && aodm restart NAME --window
S=$(aodm show NAME --json | grep -o '"serial": *"[^"]*"' | cut -d'"' -f4)
probe                                   # offline: tcp=1
aodm net NAME online && aodm restart NAME --window
```

DNS offline (checked 2026-11-10): names the phone had not looked up fail
("unknown host"); names already in its cache (kept in the quick-boot
snapshot) still resolve, but nothing can connect to them.

## Logs

Every start saves the phone's logcat to `~/.local/state/adx/logcat-NAME.log`
(previous starts `.1`…). It is raw app output and can hold tokens or
personal data an app logs: it stays under `~/.local/state` (mode 700), and
belongs in no bug report unread. The same goes for pipeline runs
(`~/.local/share/adx/pipeline/`, mode 700, files 600): stage logs, the
phone's logcat and screen after launch.

## Not addressed (by decision)

Items marked *Sufficient* or *Accepted* above stay as they are. Options
that were considered but not built: a filtering Private DNS setting, a
filtered network mode per image (network namespace or
`-http-proxy`), and `hw.audioInput=no` as a framework default.
