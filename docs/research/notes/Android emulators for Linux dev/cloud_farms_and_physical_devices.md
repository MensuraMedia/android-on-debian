# Cloud device farms and physical Android devices for a solo Linux developer (late 2026)

Research date: 2026-11-05. All prices are in USD and were seen on 2026-11-05 unless marked otherwise. Context: one developer on Linux Mint 22.3 (Ubuntu 24.04 "noble" base) who builds from the CLI with Claude Code.

## Firebase Test Lab: free quota, pricing, gcloud CLI, Robo tests

### Takeaway
Firebase Test Lab (FTL) has the most generous free tier of any cloud farm and is the only one with first-party Google CLI and Gradle integration (`gcloud firebase test android run`, Gradle Managed Devices FTL plugin). On the Spark (free) plan you get 10 virtual-device and 5 physical-device test runs per day. On Blaze you get 60 virtual and 30 physical minutes per day free, then $1/hr virtual and $5/hr physical, billed per minute. It runs batch tests (Robo, Instrumentation, Game Loop), not interactive sessions. Interactive use goes through the separate Android Device Streaming product: 30 free minutes per project per month, then $0.15/min.

### Cited Findings
- Spark (no-cost) plan: 10 test runs/day on virtual devices and 5 test runs/day on physical devices, so up to 15/day. — [Firebase: Test Lab usage, quotas, pricing](https://firebase.google.com/docs/test-lab/usage-quotas-pricing)
- Blaze plan: the first 30 min/day on physical devices and 60 min/day on virtual devices cost nothing. After that, physical is $5/device-hour and virtual is $1/device-hour. — [Firebase pricing doc](https://firebase.google.com/docs/test-lab/usage-quotas-pricing)
- Billing is per minute, rounded up. Only test run time is billed; app install and result collection are not. — [Firebase pricing doc](https://firebase.google.com/docs/test-lab/usage-quotas-pricing)
- Android Device Streaming (interactive remote devices from Android Studio, served from Test Lab infrastructure): 30 free minutes per project per month, then $0.15 per extra minute. — [Firebase pricing doc](https://firebase.google.com/docs/test-lab/usage-quotas-pricing)
- The pricing page carries no date stamp. The figures were read on 2026-11-05. — [Firebase pricing doc](https://firebase.google.com/docs/test-lab/usage-quotas-pricing)
- CLI: `gcloud firebase test android run --type robo|instrumentation|game-loop --app X.apk [--test Y-test.apk] --device model=…,version=…,locale=…,orientation=…`. You can pass several `--device` flags to build a matrix. `--type` defaults to Robo, and passing `--test` implies instrumentation. — [Firebase: Start testing with gcloud CLI](https://firebase.google.com/docs/test-lab/android/command-line)
- Device discovery: `gcloud firebase test android models list` and `models describe MODEL_ID` show brand, OS versions, API levels, ABIs, and whether a device is physical or virtual. `versions list` and `locales list` are also available. — [Firebase gcloud doc](https://firebase.google.com/docs/test-lab/android/command-line)
- Other useful flags: `--use-orchestrator`, `--test-targets`, `--num-flaky-test-attempts`, `--timeout`, `--directories-to-pull`. Pulled files count toward Cloud Storage charges on Blaze. — [Firebase gcloud doc](https://firebase.google.com/docs/test-lab/android/command-line)
- Results: a summary prints in the CLI after a few minutes, with a link to the Firebase console. Full artifacts go to a Cloud Storage bucket, with coverage at `artifacts/coverage.ec`. — [Firebase gcloud doc](https://firebase.google.com/docs/test-lab/android/command-line)
- Robo crawls the UI with no test code. It signs in to Google accounts automatically (`--no-auto-google-login` turns this off). `--robo-directives` can fill `EditText` fields by resource name or make Robo ignore elements. It cannot type into WebView fields. — [Firebase gcloud doc](https://firebase.google.com/docs/test-lab/android/command-line)
- Gradle integration: Gradle Managed Devices can target FTL through the Firebase Test Lab Gradle plugin (`com.google.firebase.testlab.gradle.ManagedDevice`). You run it with `./gradlew <device-name><Variant>AndroidTest`. Sharding uses `android.experimental.androidTest.numManagedDeviceShards`, and smart sharding is available. Auth uses gcloud credentials. — [developer.android.com: build-managed devices](https://developer.android.com/studio/test/managed-devices); [Firebase: Run tests with Android Studio](https://firebase.google.com/docs/test-lab/android-studio)
- Android Studio's device mirroring always redirects audio for FTL devices running Android 12+. — [developer.android.com: Run apps on a hardware device](https://developer.android.com/studio/run/device)

### Inferences
- For a solo developer, the Spark quota (15 runs/day) or the Blaze daily free minutes cover most nightly or pre-release smoke tests at no cost. This makes FTL the default cloud choice to sit next to local emulators.
- FTL is a batch service: each run takes minutes, and results arrive asynchronously. That suits CI and agent-driven CLI loops (Claude Code can call `gcloud` and parse the output), but it is a poor fit for an inner edit-run loop.
- The Gradle Managed Devices FTL plugin lets the same Gradle DSL drive local headless emulators and remote FTL devices. That fits a CLI-only workflow well.

### Gaps
- I did not fetch the exact current FTL device and API-level catalogue. Run `gcloud firebase test android models list` to see it live.
- I did not verify the current FTL Gradle plugin version or its exact DSL.
- I did not check whether Spark run limits also cap duration per run (the per-run timeout defaults and maximums).

## AWS Device Farm, BrowserStack, Sauce Labs, LambdaTest/TestMu AI, Genymotion SaaS: pricing, free tiers, CLI/API

### Takeaway
All of these are paid services aimed at teams. For an individual, AWS Device Farm's pay-as-you-go rate ($0.17 per device-minute, with a one-time 1,000-minute trial) is the cheapest way to get occasional real-device time without a subscription. TestMu AI (formerly LambdaTest) has the only permanent free tier, and it is tiny. BrowserStack App Live and Sauce Labs Live cost about $29–49/month for one user doing interactive real-device sessions. Automation plans with real devices cost about $175–249/month. Genymotion SaaS sells cloud emulators (not real devices) by the minute.

### Cited Findings
**AWS Device Farm** (real devices)
- Pay-as-you-go: $0.17 per device-minute, covering both automated tests and remote-access sessions. — [AWS Device Farm pricing](https://aws.amazon.com/device-farm/pricing/)
- Free trial: one-time 1,000 free device-minutes, offered "for a limited time" instead of the standard 250. No end date is given. — [AWS Device Farm pricing](https://aws.amazon.com/device-farm/pricing/)
- Unmetered: from $250/month per device slot. Slots are bought separately for automation or remote access and for Android or iOS. Each slot equals one concurrent device, not a specific model. — [AWS Device Farm pricing](https://aws.amazon.com/device-farm/pricing/)
- Private devices: from $200/month (contact AWS). Desktop browser testing costs $0.005 per instance-minute. — [AWS Device Farm pricing](https://aws.amazon.com/device-farm/pricing/)

**BrowserStack** (real devices)
- The official pricing page, as fetched, showed: Freelancer plan $12.50/month billed annually or $19/month month-to-month, with 100 minutes of Live (desktop and mobile *browsers*) for one user. Live Desktop & Mobile is $39/month (annual). Automate Desktop & Mobile is $175/month and Automate Desktop & Mobile Pro is $225/month (both annual). — [BrowserStack pricing](https://www.browserstack.com/pricing?product=app-live)
- I could not extract **App Live** or **App Automate** prices from the official page, which renders them dynamically. Third-party listings conflict for App Live Individual: $39/month billed annually ([G2](https://www.g2.com/products/browserstack/pricing)); $29 annual and $39 monthly ([TrustRadius](https://www.trustradius.com/products/browserstack/pricing)); $49/month ([Capterra](https://www.capterra.com/p/162900/BrowserStack/pricing/)). G2 lists App Live Team at $150 for 5 users per month (annual). — sources as linked
- The official page also showed a "Free 3-Months Live Team – 5 Users (new customers)" promotion attached to an Automate Desktop plan. That is not an App product. — [BrowserStack pricing](https://www.browserstack.com/pricing?product=app-live)

**Sauce Labs**
- Live Testing: $39/month billed annually, $49 month-to-month, 1 parallel session. Virtual Device Cloud (emulators/simulators): $149 annual / $199 monthly. Real Device Cloud: $199 annual / $249 monthly, 1–5 concurrent sessions. Enterprise pricing is by quote. — [Sauce Labs pricing](https://saucelabs.com/pricing)
- A free trial exists ("Try for free"), but the page does not state its length or minutes. — [Sauce Labs pricing](https://saucelabs.com/pricing)

**LambdaTest, now TestMu AI**
- lambdatest.com/pricing now 301-redirects to testmuai.com/pricing, which brands itself "TestMu AI (Formerly LambdaTest)". — [TestMu AI pricing](https://www.testmuai.com/pricing/)
- Free forever plan: 2-minute live sessions (a limited number each month), including 5 real-device sessions of up to 2 minutes per month and 2 native-app emulator sessions. Automation gets 100 lifetime minutes (not monthly) with 2 parallel sessions. — [TestMu AI pricing](https://www.testmuai.com/pricing/)
- Paid plans (annual billing; monthly prices are about 20% higher and were not shown): Virtual Live $15/month (emulators/simulators); Real Device Plus Live $39/month; Virtual Automation Cloud $139/month; Real Device Plus Automation Cloud $199/month. — [TestMu AI pricing](https://www.testmuai.com/pricing/)

**Genymotion SaaS** (cloud Android *emulators*, brief)
- Official docs: $0.05 per minute per running virtual device, billed from the moment the device starts. — [Genymotion SaaS docs: pricing](https://docs.genymotion.com/saas/03_1_Pricing)
- Conflicting third-party reports say $0.06/min in 2026 ([Codersera, Apr 2026](https://codersera.com/blog/virtual-android-emulator-online/amp/); [Tech Insider 2026](https://tech-insider.org/appetize-io-vs-genymotion-cloud-vs-browserstack-2026/)). This is unverified against Genymotion itself.
- Unlimited flat plan: about $186/month per device monthly or $149/month annual, according to a Genymotion-hosted Q&A page whose official status is unclear. — [rozz.genymotion.com](https://rozz.genymotion.com/qna/what-are-the-costs-for-using-genymotion-saas.html)

### Inferences
- Rough cost comparison for 10 hours of real-device time per month: AWS pay-as-you-go is 600 min × $0.17 ≈ $102. BrowserStack App Live or Sauce Live, at about $39–49/month, is cheaper and unlimited for interactive use. FTL physical on Blaze is (600 − 30 × days used) min at $5/hr, which is at most $50 and often much less because of the daily free 30 minutes. FTL is therefore the cheapest for automated runs.
- Genymotion SaaS at about $3/hr per emulator only makes sense if the local machine cannot run KVM emulators, for example on a weak laptop or ARM-only images.
- Interactivity: BrowserStack App Live, Sauce Live, TestMu Real Device Live, AWS remote access and FTL Device Streaming stream video over the internet. Expect noticeably more input latency than local USB or scrcpy (35–70 ms, see below). I found no published latency figures for these services.
- CLI/CI: BrowserStack, Sauce and TestMu all offer REST APIs and Appium/Espresso runners, and AWS offers `aws devicefarm` in the AWS CLI. I did not verify their specific CLI tools in this pass (see Gaps). Only FTL has a first-party Gradle path.

### Gaps
- Official BrowserStack App Live and App Automate prices: the official page did not render them for the fetcher, so the third-party figures above conflict.
- Trial lengths and minutes for BrowserStack App products and Sauce Labs were not stated on the pages I fetched. (BrowserStack has historically offered short free App Live trials; unverified for 2026.)
- I did not verify device catalogue sizes (Android model and API coverage) for AWS, BrowserStack, Sauce or TestMu. TestMu claims "50,000+" real devices on its pricing page, which is a marketing figure.
- I did not fetch CLI/API docs for AWS Device Farm (`aws devicefarm create-upload / schedule-run`), BrowserStack (REST app upload, `browserstack-cli`), Sauce (`saucectl`) or TestMu. Treat specifics as unverified.
- I found no published latency measurements for any remote interactive service.

## Physical devices on Linux: udev/plugdev, adb wireless pairing, scrcpy

### Takeaway
On Ubuntu 24.04 or Mint 22.3, USB debugging needs `sudo apt install android-sdk-platform-tools-common` (community udev rules) and membership in the `plugdev` group, followed by a re-login. Wireless debugging works on Android 11+ with `adb pair ip:port` plus a 6-digit code, and later connects automatically via mDNS. Android 17 with adb 37.0.0 adds "adb Wi-Fi 2.0", which auto-connects on trusted networks. scrcpy is at v5.0.1 (October 2026) and offers 35–70 ms mirroring, audio (Android 11+), camera (Android 12+), virtual displays, OTG/HID, and recording. The noble apt package is the ancient v1.25, so install from GitHub releases instead.

### Cited Findings
**USB setup (Ubuntu/Debian)**
- Each ADB user must be in the `plugdev` group: `sudo usermod -aG plugdev $LOGNAME`, then log out and back in and check with `id`. — [developer.android.com: Run apps on a hardware device](https://developer.android.com/studio/run/device)
- Udev rules: `apt-get install android-sdk-platform-tools-common` provides a community-maintained default set of udev rules for Android devices. — [developer.android.com: Run apps on a hardware device](https://developer.android.com/studio/run/device)
- On the device, enable Developer options and then USB debugging. Accept the RSA key prompt (Android 4.2.2+). Target the device with `adb -d`. — [developer.android.com](https://developer.android.com/studio/run/device)

**Wireless debugging**
- Android 11 (API 30)+ phones, or Android 13+ for TV and Wear OS, support pairing without USB. On the device, open Wireless debugging and choose "Pair using pairing code", which shows an IP:port and code. On the workstation, run `adb pair ipaddr:port` and enter the code. Pairing happens once; afterwards the device and workstation reconnect automatically on the same network. — [developer.android.com: adb](https://developer.android.com/tools/adb)
- Android 17 with adb 37.0.0 introduces "adb Wi-Fi 2.0". When the device joins a network marked "always allow on this network" (a trusted network), it connects to the workstation automatically. — [developer.android.com: adb](https://developer.android.com/tools/adb); [developer.android.com: hardware device](https://developer.android.com/studio/run/device)
- Troubleshooting: `adb server-status` should show version 37.0.0 or higher and `mdns_enabled: true`. Otherwise set `ADB_MDNS=1` and restart the server. `adb mdns track-services --proto-text` checks mDNS on the network; `mdns_service_version: "2.0"` means Wi-Fi 2.0 is supported. As of adb v37, the default mDNS backend is `libadbmdns` (switch with `ADB_MDNS_OPENSCREEN`). — [developer.android.com: adb](https://developer.android.com/tools/adb)
- Legacy method (the only option on Android 10 and lower): over USB run `adb tcpip 5555`, unplug, then `adb connect <ip>:5555`. Some access points or firewalls block this. — [developer.android.com: adb](https://developer.android.com/tools/adb)
- A Wireless debugging Quick Settings developer tile toggles the feature quickly. — [developer.android.com: adb](https://developer.android.com/tools/adb)
- Security note: devices connected via `adb tcpip` pass mirroring data over an unencrypted channel, and Studio talks to the adb server unencrypted. — [developer.android.com: hardware device](https://developer.android.com/studio/run/device)
- Google now also offers an "Android CLI" (`android run`, `screen capture`) as a non-Studio command-line way to deploy to and interact with devices. — [developer.android.com: adb](https://developer.android.com/tools/adb)

**scrcpy**
- Current version is v5.0.1 (released 08 Oct; the GitHub page shows no year, but inferred 2026). It fixed hardware decoding on NVIDIA VA-API on Linux and multi-GPU Linux setups. v5.0 (05 Oct) added hardware video decoding, enabled by default (`--hwdec`), captures all audio usages in playback-capture mode, and supports ANDROID_SERIAL in OTG mode. — [scrcpy releases](https://github.com/Genymobile/scrcpy/releases)
- v4.1 (12 Jul) added VP8/VP9 encoders for devices without H.264/H.265/AV1 encoders. v4.0 (12 May) moved from SDL2 to SDL3 and added flex display (`--flex-display`/`-x`), camera torch and zoom, `--keep-active`, and F11 fullscreen. v3.3.4 (17 Dec, presumably 2025) was a bug-fix release. — [scrcpy releases](https://github.com/Genymobile/scrcpy/releases)
- Capabilities: the device needs API 21+ (Android 5.0). It works over USB or TCP/IP and claims 35–70 ms latency, 30–120 fps, 1920×1080+ quality, and about 1 s to first image. Audio forwarding needs Android 11+ and camera mirroring needs Android 12+. Virtual display via `--new-display[=WxH]`. HID keyboard, mouse and gamepad via UHID (`-K`, `-G`). OTG mode (`--otg`) controls the device without USB debugging. Recording via `--record=file.mp4`. Some Xiaomi devices need "USB debugging (Security settings)". — [scrcpy README](https://github.com/Genymobile/scrcpy)
- Ubuntu packaging: noble (24.04, the base of Mint 22.x) ships scrcpy **1.25**. Resolute (26.04 LTS) ships 3.3.4 and 26.10 ships 4.1. None ships 5.x. — [packages.ubuntu.com](https://packages.ubuntu.com/search?keywords=scrcpy)
- The README warns to download only from the official GitHub repo. — [scrcpy README](https://github.com/Genymobile/scrcpy)
- Android Studio also has built-in device mirroring (Running Devices window), with optional audio redirection, over USB or wireless. — [developer.android.com: hardware device](https://developer.android.com/studio/run/device)

### Inferences
- On Mint 22.3, `apt install scrcpy` gives v1.25 (about early 2023), which lacks audio, camera, virtual display, UHID and SDL3. Use the official static Linux release from GitHub, or build from source, to get v5.0.1.
- A physical phone over USB or wireless adb plus scrcpy is the lowest-latency, zero-recurring-cost "real device" option. It is ideal for checking real GPU, sensors, camera and performance. It is also fully scriptable from Claude Code (`adb install`, `adb shell`, `scrcpy --record`, `--new-display` for headless-ish testing).
- Inner loop on a single Linux developer machine: KVM emulator or physical device. Real-hardware and OEM checks: own phone plus scrcpy. Pre-release matrix across API levels and OEMs: FTL (free quota). Ad-hoc interactive testing on a specific model you don't own: FTL Device Streaming (30 free min/month), AWS pay-as-you-go, or one month of BrowserStack App Live or Sauce Live.

### Gaps
- I did not verify whether the Debian/Ubuntu `android-sdk-platform-tools-common` package in noble is current enough for the newest OEM USB vendor IDs.
- Release years for scrcpy 4.x and 5.x are inferred from release ordering, because the page showed no year.
- I did not verify whether the platform-tools version in Ubuntu's apt (`adb` package) reaches 37.x. Install SDK platform-tools from Google for Wi-Fi 2.0.

## Developer survey data on emulator vs device vs cloud testing

### Takeaway
I found no recent (2024–2026) primary survey that splits Android testing between emulator, physical device and cloud farm. The only concrete split I found dates from 2015 and is a small Sauce Labs poll.

### Cited Findings
- 2015 Sauce Labs global poll of 153 mobile developers and testers: 29% used only simulators or emulators, 37% only real devices, 34% a mix. — [Route Fifty / DigitalGov (Jun 2015)](https://www.route-fifty.com/digital-government/2015/06/testing-options-for-app-developers/287199/)
- Sauce Labs survey (May 2015): 81% planned to do at least 50% of testing on emulators, and 50% planned to do at least 75% on emulators. — [Sauce Labs slides (2015)](https://www.slideshare.net/slideshow/real-devices-or-emulators-wen-to-use-what-for-automated-testing/52901100)
- A "67% of developers prefer real devices" claim circulates in a Xamarin guide but has no traceable primary source. — [Moldstud](https://moldstud.com/articles/p-testing-xamarin-apps-real-devices-vs-emulators-which-to-choose) (unreliable)
- Google's own guidance: always test on a real device before release, use the emulator to cover Android versions and screen sizes, and consider Firebase Test Lab for a wide variety of real devices. — [developer.android.com: hardware device](https://developer.android.com/studio/run/device)

### Inferences
- In practice the official recommendation is hybrid: emulator for breadth and the inner loop, a real device for final checks, and a cloud farm for coverage. The 2015 data, though old, also shows that most developers use a mix.

### Gaps
- I did not search the Stack Overflow Developer Survey 2025/2026 or the JetBrains State of Developer Ecosystem 2025/2026 in depth. As far as I know, neither asks about emulator vs device vs cloud testing, but this is unverified.
