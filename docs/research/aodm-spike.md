# aodm P0 spike: what the emulator really allows (2026-11-06)

Run on the hybrid-GPU laptop: emulator 37.2.13, `system-images;android-37.0;google_apis_playstore;x86_64`
(user build, no root), AVD `adx_api37` on port 5554 and a copied AVD `aodm_spike` on port 5556,
both `-gpu software`. Every result below was observed, not assumed.

## Runtime settings over adb (no root)

| Setting | Command | Result |
| --- | --- | --- |
| Dark mode | `cmd uimode night yes\|no`; read `cmd uimode night` | works |
| Font scale | `settings put system font_scale 1.15` | works |
| Animations | `settings put global window_animation_scale 0` (+ transition/animator) | works |
| Screen timeout | `settings put system screen_off_timeout <ms>` | works |
| Stay awake | `svc power stayon true` | works |
| Android ID | `settings put secure android_id <16 hex>` | **works** (read back) |
| System locale | `settings put system system_locales de-DE` | accepted and read back; UI effect to confirm after restart |

## Boot flags

| Flag | Result |
| --- | --- |
| `-port 5556` | works; serial `emulator-5556` |
| `-timezone Europe/Berlin` | **works** (`persist.sys.timezone`, `date` shows CET) |
| `-change-locale de-DE` | **fails on Play images**: the emulator runs `su 0 setprop …`, no `su` on user builds. Use the adb setting above |
| `-prop ro.serialno=…` | **no effect**: serial stays `EMULATOR37X2X13X0` on every emulator |
| `-phone-number 15550001234` | accepted; Android masks the number in `dumpsys isub`, not readable without privileges |
| `-dns-server 1.1.1.1` | accepted; the guest always sees the emulator DNS proxy `10.0.2.3`, so it is applied host-side and can't be confirmed from the guest |
| `-id aodm-spike` | accepted |

## Identity fields

| Field | Finding | Class for identity policies |
| --- | --- | --- |
| IMEI | `867400022047199` compiled into the emulator's modem simulator: identical on every emulator | fixed (report) |
| Serial | `EMULATOR37X2X13X0`, `-prop` ignored | fixed (report) |
| ICCID / IMSI | Stored in `modem_simulator/iccprofile_for_sim0.xml` (`CCID`, `CIMI`), but editing the copy's file did **not** change what Android reports after a cold boot | fixed (report) for now; revisit |
| Android ID | Copied AVDs keep the **same** Android ID as the source; settable over adb | controllable, **must regenerate on clone/open-as-new** |
| Time zone | `-timezone` | controllable |
| Locale | adb `system_locales` | controllable (runtime) |
| Phone number | `-phone-number` (masked by Android) | controllable |
| Wi-Fi MAC | `02:15:b2:00:00:00` on every emulator | fixed (report) |

## Copies, locks, process detection

- `cp --archive --sparse=always --reflink=auto` of a 6.4 GB AVD on ext4: **6.2 s**, 6.3 GB on disk.
- A copy with `avd.id`/`avd.name` set, a new `.ini`, transient files removed and
  `snapshots/default_boot` deleted **cold-boots fine** (28 s, software GPU) next to the original.
- `multiinstance.lock`: `flock -n` fails while the emulator runs and succeeds after it stops,
  so it is an **authoritative running check**. `hardware-qemu.ini.lock` holds the PID.
- `adb -s emulator-<port> emu avd name` returns the AVD name: serial ↔ image mapping works.
- `pgrep -f 'qemu-system.*-avd NAME'` from a shell **matches the shell's own command line**;
  detection must read `/proc/*/cmdline` (or `ps -eo comm,args`) and match the qemu process name.

## Calls and SMS

| Test | Result |
| --- | --- |
| 5554 dials `5556` (built-in routing) | no call on 5556 |
| 5554 dials `15550001234` (5556's `-phone-number`) | no call on 5556 |
| Console `gsm call 15550009999` on 5556 | **rings** (`mCallState=1`); `gsm cancel` ends it |
| Console `sms send 15550009999 "…"` on 5556 | **OK** |

So the switchboard must deliver the inbound side through the target's console
(`gsm call <from>` / `sms send <from> <text>`), as planned.

## Consequences for the plan

- Identity policies cover: phone number, time zone, locale, Android ID, console id, DNS (host-side).
  IMEI, serial, ICCID/IMSI and MAC are reported as fixed by the emulator, never faked.
- Clone and open-as-new must regenerate the Android ID.
- Locale is a runtime setting (adb), not a launch flag, on Play images.
- Running-state detection: flock on `multiinstance.lock` + `/proc` scan by process name.
