# Android Studio (dormant)

Android Studio 2026.2.2.10 ("Rabbit 2 Patch 2") is installed locally by `adx up` but is
**not part of the default workflow**. The framework doesn't depend on it:
the JDK, SDK and emulator are all provisioned by `adx`. It stays available
for the jobs it does best, such as the layout inspector, the profiler and
interactive debugging.

| Item | Value |
| --- | --- |
| Location | `~/.local/opt/android-studio` (user-owned, so in-IDE patch updates need no root) |
| Source | Official tarball from developer.android.com/studio: `adx studio install` (part of `adx up`) downloads `ADX_STUDIO_URL` and checks `ADX_STUDIO_SHA256` (`config/defaults.env`; 2026.2.2.10, "Rabbit 2 Patch 2", read 2026-11-24). `adx studio status` compares the installed build with `ADX_STUDIO_BUILD`; an install of another build is kept beside it as `android-studio.previous-<build>-<time>` (`unknown` when it has no `product-info.json`), never deleted. The tarball stays in `~/Downloads/adx` |
| Launcher | none on PATH and no menu entry (deliberately); `adx studio launch` (output in `~/.local/state/adx/studio.log`) |
| SDK | point it at `~/Android/Sdk`, the one shared with `adx` |

## When you need it

1. Run `adx studio launch`.
2. In the setup wizard choose **Custom**, set the SDK location to
   `~/Android/Sdk`, and skip any component downloads that are already there.
3. Optional: use Tools → Create Desktop Entry for a menu entry.
4. Optional: install the Claude Code JetBrains plugin, which shows diffs in
   the IDE.

The full install guide, with system checks and best practices, is in the
Claude doc *Installing Android Studio on Linux Mint (Debian-based)*.
