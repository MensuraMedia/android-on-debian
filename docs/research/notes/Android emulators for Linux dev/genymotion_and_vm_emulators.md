# Third-party VM-based Android emulators for Linux dev hosts: Genymotion and full-VM Android-x86 distributions (state as of Nov 2026)

Source-quality note: Genymotion runs a domain, rozz.genymotion.com, that appears to be an AI-assistant/mirror knowledge base. It is Genymotion-hosted but not the canonical docs, and in places it contradicts docs.genymotion.com. Claims sourced only from it are flagged "(mirror)". tech-insider.org, filehorse, filecr and similar pages are low quality and are not relied on for facts.

## Genymotion Desktop on Linux: hypervisor, Android versions, Google Play, integration, gmtool/headless, pricing, release status

### Takeaway
Genymotion Desktop 3.11.0 (released Sep 24, 2026) is actively maintained. On Linux it runs on an embedded QEMU with KVM by default; VirtualBox is legacy only. It officially supports Ubuntu 24.04 LTS, so a Mint 22.3 host is close but not officially listed. Free personal use is restricted: no latest Android version, quick boot, full gmtool or several sensors. Commercial use requires Business at $479.99 per user per year, or Individual at $239.99 per computer per year for freelancers. Desktop does not support ARM translation according to a Genymotion-hosted mirror FAQ, so ARM-only native libraries will not run.

### Cited Findings
**Releases and maintenance**
- Release timeline: 3.6.0 (Dec 20, 2023) made QEMU the default hypervisor on Linux and macOS. 3.7.0/3.7.1 came out in May 2024 and 3.8.0 on Oct 23, 2024; 3.8.0 added gamepad support, dynamic root access and 720p camera injection for Android 12+. 3.9.0 (Apr 24, 2025) added Android 15. 3.10.0 (Apr 1, 2026, updated Jun 16, 2026) moved the Android 15/16 images to 3.6.2, took Android 16 out of beta and improved Android 15 boot time. 3.11.0 is dated Sep 24, 2026, and the changelog lists no notable details for it. — [Genymotion Desktop 3.x changelog](https://docs.genymotion.com/changelog/desktop/3_x/)
- Known issues: in 3.9 and later, Android 15/16 are slow on Windows hosts and have performance problems with the VirtualBox hypervisor, and multi-display is broken. In 3.10, full-screen may not work with cloud devices, ADB over gmsaas fails silently if gmsaas and Desktop use different accounts, and snapshot recovery may malfunction. — [changelog](https://docs.genymotion.com/changelog/desktop/3_x/)
- 3.10.0 headline: Genymotion SaaS cloud devices now appear inside Desktop and show up in ADB automatically. Developer options, dynamic rooting and gamepad support also work on SaaS instances. — [Genymotion blog: Desktop 3.10.0](https://www.genymotion.com/blog/genymotion-desktop-3-10-released/) (also summarised in the search snippet of the [3.10 announcement mirror](https://rozz.genymotion.com/pages/genymotion-desktop-3-10-0-is-out.html))
- The current download page lists 3.11.0 Stable for Linux as `genymotion-3.11.0-linux_x64.run` with SHA256 8cd727af…052f. The same page lists Ubuntu 24.04 LTS or higher, Debian stable 13 or higher and Fedora Workstation 40 or higher, and says "latest Android version is a Pro feature" that is unavailable in personal use. — [genymotion.com/download](https://www.genymotion.com/download/)

**Linux host requirements and hypervisor**
- Supported Linux distros are Ubuntu 24.04 LTS, Debian Stable (13 Trixie) and Fedora Workstation 43, on x86_64/amd64 only. Only GNOME, KDE and MATE are fully supported desktop environments. — [Linux install docs](https://docs.genymotion.com/desktop/Get_started/013_Linux_install)
- The docs say "Genymotion Desktop may crash on X11 session in Ubuntu; we recommend using Wayland instead." This matters for Mint, whose Cinnamon desktop is primarily X11. — [Linux install docs](https://docs.genymotion.com/desktop/Get_started/013_Linux_install)
- QEMU is the default hypervisor on Linux. It uses KVM, which is installed by default on the supported distributions. VirtualBox "is not recommended and only maintained for retro compatibility". It is not bundled, the docs pin VirtualBox 7.0.26, and they warn of known issues with VirtualBox on kernel 6.14 (Ubuntu 24.04.3 HWE). — [Linux install docs](https://docs.genymotion.com/desktop/Get_started/013_Linux_install)
- Installation is a `.run` installer: `chmod +x genymotion-X.Y.Z-linux_x64.run`, then run it with `-d PATH`. A global sudo install to /opt/genymotion is discouraged. — [Linux install docs](https://docs.genymotion.com/desktop/Get_started/013_Linux_install)
- The hardware minimum for PCs is a 64-bit Intel 6th gen or AMD Ryzen CPU. Linux servers and Linux arm/arm64 are listed as incompatible. — [requirements (mirror)](https://rozz.genymotion.com/pages/what-are-genymotion-desktop-requirements.html)

**Google Play / GApps and ARM**
- Genymotion images ship without Google Play. Users install Open GApps from the Device Content section or the File Upload widget. — [Genymotion docs: Open GApps](https://docs.genymotion.com/features/opengapps)
- The widget dates from Genymotion 2.10 (2017), which installed the "pico" Open GApps package on Android 4.4+. — [Genymotion blog 2.10](https://www.genymotion.com/blog/2-10-open-gapps-widget/) (dated 2017)
- A Genymotion mirror Q&A says Open GApps packages for Android 12+ are not on the official Open GApps website. — [mirror Q&A](https://rozz.genymotion.com/qna/ive-installed-genymotion-but-cant-find-the-play-store-how-do.html) (mirror, unverified)
- "On Desktop: ARM translation is not supported", and "Google Apps ARM libraries aren't provided anymore". ARM64 testing is directed to SaaS ARM64 devices, said to be available since July 2025. — [Genymotion mirror FAQ on ARM translation](https://rozz.genymotion.com/qna/what-is-genymotion-s-current-arm-translation-support-and-how.html) (mirror)
- The ARM-translation-plus-GApps flashing workaround guides date from 2015–2016 and cover Android 4.x–6.0 only. — [e.g. GitHub gist](https://gist.github.com/inix/a183a98e70fcb2bbc7a9) via search (dated)

**gmtool CLI, ADB, headless and CI**
- gmtool is in the install directory ($HOME/genymotion/ or /opt/genymotion/) and is meant to "automate series of actions". The basic `admin start|stop|list` commands carry no license badge. The whole `license`, `config` and `device` groups are paid-only, as are the advanced `admin` commands (create, clone, delete, edit, templates, osimages, hwprofiles, factoryreset, stopall, logzip, details). Error code 14: "This command can only run with Indie and Business licenses." — [GMTool docs](https://docs.genymotion.com/desktop/06_GMTool/)
- The GMTool page documents no `--no-window` or headless start option; `start` supports only `--coldboot`. There is a global `--timeout` and documented exit codes. — [GMTool docs](https://docs.genymotion.com/desktop/06_GMTool/)
- The pricing page lists "full gmtool" as missing from the Free Personal edition. — [Genymotion pricing](https://www.genymotion.com/pricing/)
- Genymotion's requirements state that Linux servers are incompatible with Desktop. That points CI toward SaaS or Device Image rather than Desktop on a headless runner. — [requirements (mirror)](https://rozz.genymotion.com/pages/what-are-genymotion-desktop-requirements.html)
- An Android Studio/IntelliJ Genymotion plugin is distributed from the plugins download page. That page also states that the Indie plan is reserved for individual workers. — [genymotion.com/download/plugins](https://www.genymotion.com/download/plugins)

**Pricing (current pricing page, fetched Nov 2026)**
- Free (Personal Use) costs $0. It lacks the latest Android version, quick boot, camera/media injection, network profiles, full gmtool and GPS routes, and has no support. Educational is $49/yr. Individual is $239.99/yr per computer and is limited to freelancers and the self-employed (not employees or contractors). Business is $479.99/yr per user and workstation. Premium is contact-sales and adds a license server. A 30-day Desktop Pro trial is available. — [Genymotion pricing](https://www.genymotion.com/pricing/)
- Mirror pages quote older or alternate figures ($206/yr Individual, $412/yr Business), which conflict with the live pricing page. — [mirror pricing](https://rozz.genymotion.com/pages/genymotion-solutions-pricing.html)
- For history, a 2020 snapshot showed Indie at $136/yr. — via [archive proxy](https://top.tyaskartini89.workers.dev/host-https-web.archive.org/web/20201014172918/https://www.genymotion.com/) (dated 2020)

### Inferences
- On the target i7-13620H / Mint 22.3 (Ubuntu 24.04 base) host, Genymotion should install and use KVM through its embedded QEMU, the same acceleration path as Google's emulator. Two caveats apply: Mint isn't officially listed, and Cinnamon/X11 carries the documented X11 crash risk.
- With no ARM translation on Desktop, any app or SDK shipping only arm64-v8a native libraries will not run. Google's emulator for x86_64 system images (API 30+) includes ARM-to-x86 translation, which gives Google's emulator an edge for such apps.
- For a commercial developer, Genymotion Desktop costs $240–$480/yr versus $0 for the official emulator. Its remaining differentiators are sensor widgets, device profiles, and the SaaS bridge added in 3.10.
- The free tier's lack of the "latest Android version" means free users cannot test on Android 16. This follows from the pricing statement; I did not verify which version is the cutoff.

### Gaps
- I could not fetch the exact list of Android versions available in 3.11 on Desktop. 3.10 confirms Android 15 and 16, but whether older images (5.x–9) are still downloadable was not verified.
- I found no 3.11.0 feature details; the changelog entry is sparse.
- The current state of Google Play on Android 13–16 Desktop images is unclear (Open GApps is abandoned upstream for 12+). Whether Genymotion ships its own GApps package for new images is unverified.
- I found no official documentation of headless or `-no-window` operation for Desktop on Linux.
- I found no reliable adoption or market-share statistics.

## Genymotion SaaS (cloud) and Device Image (AWS/GCP/Azure) for CI

### Takeaway
SaaS costs $0.06 per device-minute pay-as-you-go, or $219/month ($179/month billed annually) per device for unlimited runtime. It became arm64-only in March 2026. Device Image (PaaS) on cloud marketplaces costs $0.60 per instance-hour plus the cloud provider's fees. Both target CI without needing a local KVM runner.

### Cited Findings
- SaaS Pay-As-You-Go is $0.06/minute per running device with no recurring fee, including public cloud and CI/CD tools. Unlimited is $219/month per device or $179/month billed annually, with priority support. Premium is custom and adds private/dedicated servers, custom CPU/RAM, GPU acceleration and an SLA. — [Genymotion pricing](https://www.genymotion.com/pricing/)
- Device Image (PaaS) for AWS, GCP, Azure and Alibaba Cloud costs $0.60/hour per instance excluding cloud platform fees, with private offers for volume. Self-hosted/on-prem and an Enterprise Master Agreement are contact-sales. — [Genymotion pricing](https://www.genymotion.com/pricing/)
- SaaS "has transitioned to exclusively support arm64 architecture for all Android virtual devices", which Genymotion ties to the Google Play 64-bit requirement. Java/Kotlin apps need no change; native apps must include arm64-v8a. The change does not affect pricing. — [Genymotion support: What happened to x86/x86_64 devices in SaaS](https://support.genymotion.com/hc/en-us/articles/30419556610333-What-happened-to-x86-x86-64-devices-in-Genymotion-SaaS)
- Creating or starting x86 SaaS devices stopped after March 30, 2026, and existing x86 recipes fail. Only mirror pages give the exact date. — [mirror FAQ](https://rozz.genymotion.com/pages/what-is-happening-to-x86-devices-in-genymotion-saas.html)
- Desktop 3.10 integrates SaaS: cloud devices are listed in Desktop and connected to local ADB, via gmsaas. — [changelog](https://docs.genymotion.com/changelog/desktop/3_x/); [3.10 blog](https://www.genymotion.com/blog/genymotion-desktop-3-10-released/)

### Inferences
- Because SaaS is arm64-native, it runs ARM-only apps that Desktop cannot. That makes it the Genymotion answer for ARM-native testing.
- Cost example: at $0.06/min, 1,000 device-minutes per month costs $60, and the Unlimited plan breaks even at roughly 3,650 min/month (about 61 h). This is my arithmetic from the published rates.
- On self-hosted Linux CI runners with KVM, the free official emulator (headless `-no-window`) is the zero-cost baseline that Genymotion's cloud offerings compete against.

### Gaps
- I did not verify the gmsaas CLI version, its CI integrations (GitHub Actions or Bitrise), or the list of Android versions available on SaaS arm64.
- Device Image instance types and Android versions per marketplace were not verified.

## Android-x86 / BlissOS / PrimeOS-type full-VM distributions

### Takeaway
Android-x86 is effectively dormant. Its last stable release is 9.0-r2 (Android 9, March 2020), and the last maintenance release is 8.1-r6 (June 2021). BlissOS has paused all public images with no ETA; its existing SourceForge builds (Android 11–13 era, e.g. 16.x on Android 13) are unsupported archives. Neither is a sound primary target for app development in 2026: they lack current Android versions and CTS/Play certification, and the Bliss user support channels are paused.

### Cited Findings
- Android-x86's last stable release is 9.0-r2 (Android Pie 9.0.0), March 25, 2020. Its other branches are 8.1-r6 (June 23, 2021) and 7.1-r5 (Feb 14, 2021). Android 10 and 11 exist only as in-progress source. "As of 2025, the project is considered inactive or discontinued." — [Wikipedia: Android-x86](https://en.wikipedia.org/wiki/Android-x86)
- The SourceForge mirror's newest folder is "Release 9.0", modified 2022-03-25, and has been unchanged since. — [SourceForge android-x86 files](https://sourceforge.net/projects/android-x86/files/)
- History: in 2017 the project lead questioned whether to continue the project. — [Liliputing 2017](https://liliputing.com/2017/02/android-x86-lead-developer-may-abandon-project.html); [Phoronix 2017](https://www.phoronix.com/news/Android-x86-2017-Questioned)
- BlissOS homepage: "Public images are paused. The source is not." — [blissos.org](https://blissos.org/)
- BlissOS status page: official public image releases stopped while the next tree is built, because the old tree and docs no longer match. There is "No ETA", and the team will not invent a ship date. Older SourceForge builds are archives with no updates or official support. No in-place upgrade is possible. Telegram/Discord user support is paused. The trees still in development are BlissOS, BlissRoms-x86, Android-Generic, Ānanda Āropa and Bliss-Bass, plus Boringdroid, Smart Dock and XTMapper. — [blissos.org/status](https://blissos.org/status)
- Third-party reports say the newest widely used Bliss PC build is 16.9 (Android 13). They also say the BlissOS 17 (Android 14) and 18 (Android 15) trees existed by Aug 2025 but never shipped as ISOs. These come from low-quality sites and should be treated as indicative only. — [FileHorse](https://www.filehorse.com/download-bliss-os/52972); [tech-insider](https://tech-insider.org/how-to-install-bliss-os-pc-2026/)
- BlissOS is described as based on Android-x86. — [Wikipedia: Android-x86](https://en.wikipedia.org/wiki/Android-x86)
- Android-x86 gets QEMU GPU acceleration via virgl. — [Wikipedia](https://en.wikipedia.org/wiki/Android-x86)

### Inferences
- Under QEMU/KVM with virtio-gpu/virgl these images can boot fast, and ADB over TCP works (it is standard AOSP adbd). However, Android 9–13 are far behind the targetSdk that Google Play requires in 2026 (API 35+ for new apps). That limits them to legacy/compatibility testing.
- PrimeOS and Phoenix OS are older Android-x86 derivatives. I found no evidence of 2026 maintenance for either, but did not verify this directly.
- For a developer on the target host, the official Google emulator (current API levels, Play images, ARM translation) and Genymotion both dominate these options.

### Gaps
- I did not obtain primary-source BlissOS release history (dates and Android versions of the last SourceForge ISOs).
- I found no 2026 data on PrimeOS, Phoenix OS or other forks.
- I found no measured KVM performance data for BlissOS or Android-x86 in 2025–2026.

## Benchmarks vs the official Google Android Emulator

### Takeaway
I found no rigorous, recent (2024–2026) benchmark comparing Genymotion or Android-x86 to the official emulator on Linux/KVM. The available comparisons are anecdotal, mostly pre-2020, and predate both Genymotion's move to QEMU/KVM and the emulator's improvements. Where they exist, the claims conflict.

### Cited Findings
- Old claims put Genymotion's boot at about 5–10 s versus 30 s or more for the stock emulator. One is a Stack Overflow answer from roughly 2013–2014 that warns results vary by host. — [Stack Overflow archive](https://dev.library.kiwix.org/content/stackoverflow_en_nopic_2021-08/a/18684017); [moldstud comparison](https://moldstud.com/articles/p-android-emulator-vs-genymotion-which-is-the-best-choice-for-senior-developers) (no methodology)
- A Linux guide states, without citing data, that the Android Studio emulator with KVM "can be as fast or faster than Genymotion". — [developnsolve](https://www.developnsolve.com/linux/genymotion-alternatives-for-linux)
- An older developer blog found the official emulator beat Genymotion in most benchmarks except Geekbench 4. — [dev.to](https://dev.to/sumitgpt/genymotion-vs-android-emulator-2i6o) (date unclear, likely circa 2018–2020)

### Inferences
- Since 3.6 (Dec 2023), Genymotion on Linux uses QEMU+KVM, the same acceleration base as Google's emulator (which is QEMU-derived). Large raw CPU differences are therefore unlikely; differences would come from graphics stacks, image tuning and snapshots. This is reasoning, not measurement.
- The user should run their own cold-boot and app-install timing tests on the i7-13620H host if performance is decisive.

### Gaps
- I found no reproducible benchmark with stated Linux/KVM host specs for any of these options in 2024–2026.
