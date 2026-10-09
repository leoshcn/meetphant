<div align="center">

<img src="branding/meetphant-icon.svg" alt="Meetphant" width="96" height="96" />

# Meetphant Meeting Assistant

**An elephant never forgets.** Turn meeting recordings into notes you can take away.

Record or import audio → transcribe → key points, action items & decisions.

[![Latest release](https://img.shields.io/github/v/release/leoshcn/meetphant?label=release&color=111A20)](https://github.com/leoshcn/meetphant/releases/latest)
[![Platform](https://img.shields.io/badge/platform-Windows%2010%20%2F%2011-0078D4)](https://github.com/leoshcn/meetphant/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-2EA44F)](./LICENSE)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-24C8DB)](https://tauri.app/)

[**Download for Windows**](https://github.com/leoshcn/meetphant/releases/latest) · [Website](https://leoshcn.github.io/meetphant/en/) · [中文说明](./README.zh-CN.md)

<img src="website/assets/screenshots/workspace.png" alt="Meetphant workspace: meeting list on the left, speaker-labelled transcript in the middle, key points, action items and decisions on the right" width="880" />

</div>

## Why Meetphant

Meetphant is a free, open-source, local-first desktop meeting assistant for Windows. It sits next to whatever meeting app you already use — no bots joining your call, no plugins, no host permissions.

|  |  |
|---|---|
| 🎙️ **Record any meeting** | Microphone + system audio at once — Tencent Meeting, Feishu, Zoom, Teams or an in-person room |
| 🧠 **Hotwords that learn your vocabulary** | People, customers and product names go into every transcription request |
| 🔒 **Your notes, your call** | Summarize with any OpenAI-compatible model — cloud, or a local LLM such as Ollama |
| 📋 **Notes you can act on** | Key points · action items · decisions, in Chinese, English or both |
| 💾 **Local-first** | Meetings and settings in SQLite; API keys only in the OS keychain |

## Features

### 01 · Record any meeting, no platform lock-in

<img src="website/assets/screenshots/recording.png" alt="Recording in progress: timer, dual waveform for microphone and system audio, and the active devices" width="880" />

- Capture your microphone and system audio (WASAPI loopback) in one click
- Live dual-track levels, so you know both sides are being recorded
- Already have a recording? Import `wav`, `mp3`, `m4a`, `flac`, `ogg`, `aac`
- Saves M4A when FFmpeg is available, falls back to WAV without blocking you

### 02 · Custom hotwords for your team’s vocabulary

<img src="website/assets/screenshots/hotwords.png" alt="Hotword settings with people and product names, plus summary context below" width="880" />

- Colleagues, customers, project code names — add once, use every time
- Hotwords only affect transcription; summaries get their own *context* setting
- Speakers are separated and can be renamed so notes show real names

### 03 · Connect a local LLM — your notes, your call

<img src="website/assets/screenshots/llm.png" alt="Summary model settings: custom OpenAI-compatible provider, Base URL http://localhost:11434/v1, model qwen3:14b" width="880" />

- Presets for **Alibaba Cloud DashScope (Qwen, default)**, DeepSeek, OpenAI, Moonshot (Kimi), Zhipu and Volcengine Ark (Doubao)
- Or any custom endpoint, e.g. `http://localhost:11434/v1` for Ollama or a model on your company network
- With a local model, summarizing never sends meeting content to a third-party AI service
- API keys live in the OS keychain — never in the database, never shown in plain text

> [!NOTE]
> Speech-to-text currently uses the cloud-based **Doubao Seed-ASR 2.0** file transcription service. Audio is uploaded through **your own** Volcengine TOS bucket.

### And more

- **Structured summaries** — what was discussed, who does what, what was decided
- **Workspace** — meeting sidebar with a split transcript / summary view (tabs on narrow windows)
- **Light & dark themes** — follows your system or set it yourself
- **Auto-update** — checks for new versions at launch; download, install and restart in one click

## How it works

| Step | | |
|:---:|---|---|
| **1** | **Record or import** | Capture mic + system audio during the meeting, or import an existing recording |
| **2** | **Transcribe** | Stopping a recording creates the meeting and starts transcription, with speakers separated and hotwords applied |
| **3** | **Summarize** | Get key points, action items and decisions — copy and share |

## Download

Grab the latest installer from [**Releases**](https://github.com/leoshcn/meetphant/releases/latest) (Windows 10 / 11, x64). The app interface is in Simplified Chinese.

| Installer | Contents | When to use |
|---|---|---|
| `Meetphant_<ver>_x64-setup.exe` — **lean** (recommended) | App only | Normal installs. FFmpeg (~80–100 MiB) downloads on first need. Receives in-app updates |
| `Meetphant_<ver>_x64-offline-setup.exe` | App + bundled FFmpeg | Slow or offline networks (manual updates) |

Both share one app id — installing one replaces the other.

## Setup

Open **Settings** and fill in three credentials:

| Role | Provider | What you enter |
|---|---|---|
| Transcription | Doubao Speech | API Key (new speech console) |
| Audio upload for ASR (required) | Volcengine TOS | AK / SK + region / bucket |
| Summary | DashScope (default) or any OpenAI-compatible service | Provider, Base URL, API Key, model |

Secrets are stored only in the OS credential store — never in SQLite, never returned by `settings_get`.

📖 Step-by-step guide with screenshots (Chinese): **[Credentials guide](./docs/credentials-guide.zh-CN.md)**

<details>
<summary><b>Transcription limits</b></summary>

| File size | Path |
|---|---|
| ≤ 512 MiB (≤ 5 h) | Upload to TOS → pre-signed GET → Doubao Seed-ASR 2.0 submit / query (`volc.seedasr.auc`, up to 45 min polling) |
| > 512 MiB | Rejected with `ASR_PAYLOAD_TOO_LARGE` |

Hotwords are sent to ASR. `context_text` is used for summaries only and is **not** sent to Doubao.

</details>

## Development

**Prerequisites**

- [Node.js](https://nodejs.org/) 20+
- [Rust](https://rustup.rs/) stable — on Windows use MSVC: `rustup default stable-x86_64-pc-windows-msvc`
- Windows: [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) (“Desktop development with C++”) + WebView2

```bash
npm install
npm run tauri dev        # Vite on http://localhost:1420 + the Meetphant window
```

```bash
npm test                 # frontend tests (Vitest)
npm run typecheck
cd src-tauri && cargo test
```

```
src/          React UI (app / pages / features / ipc / shared)
src-tauri/    Tauri commands, services, SQLite, providers
website/      Product site (static HTML/CSS, zh at /, en at /en/)
scripts/      Installer packaging, FFmpeg prep, screenshot capture
```

<details>
<summary><b>Packaging & releases</b></summary>

```bash
npm run ffmpeg:prepare   # cache the pinned FFmpeg essentials build
npm run pack:lean        # lean installer
npm run pack:offline     # offline installer
npm run pack:all         # both → dist-installers/ (+ latest.json when signed)
```

Push a version tag (or run **Actions → Release**) to build both `.exe` files and `latest.json` and attach them to a GitHub release.

**Auto-update**

- The app checks `https://github.com/leoshcn/meetphant/releases/latest/download/latest.json` on launch and from **Settings → About**.
- Only the **lean** installer is published on the updater channel.
- Release builds must be signed with a Tauri updater key:
  - Local: `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH` (optional `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`)
  - CI: repository secrets `TAURI_SIGNING_PRIVATE_KEY` and optional `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
- Generate keys once with `npm run tauri signer generate -- -w ~/.tauri/meetphant.key`; keep the private key offline. The public key is embedded in `src-tauri/tauri.conf.json`.
- Windows Authenticode / SmartScreen signing is not configured yet (separate from Tauri update signatures).

Bundled FFmpeg is the Gyan **essentials** build (GPLv3) — see [GyanD/codexffmpeg](https://github.com/GyanD/codexffmpeg).

</details>

<details>
<summary><b>Website & screenshots</b></summary>

The product site lives in `website/` — static HTML/CSS with no build step (Chinese at `/`, English at `/en/`).

```bash
npx serve website        # or: py -3 -m http.server -d website 8000
npm run screenshots      # re-render website/assets/screenshots/*.png and og.png
```

`npm run screenshots` renders the real React UI in headless Chromium with mocked Tauri IPC and sample data (`scripts/screenshots/`). Re-run it after UI changes and commit the images — this README uses them too.

Pushes to `main` that touch `website/**` deploy via `.github/workflows/pages.yml`. One-time setup: **Settings → Pages → Source = GitHub Actions**.

</details>

## Tech stack

Tauri 2 · React 19 · TypeScript · Vite · Rust · SQLite · Doubao Seed-ASR 2.0 · OpenAI-compatible LLMs · OS keyring

## License

[MIT](./LICENSE) © 2026 Leo Li — free to use, modify and distribute.

<div align="center">
<sub>🐘 Let the elephant remember your next meeting.</sub>
</div>
