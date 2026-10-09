# Meetphant

**The elephant never forgets.** Turn meetings into notes you can act on.

[中文说明](./README.zh-CN.md)

<p align="center">
  <img src="docs/screenshots/home.png" alt="Meetphant home" width="860" />
</p>

A local-first desktop meeting assistant: capture mic + system audio (or import a file), then go from **transcription → structured summary** (key points / action items / decisions).

## Why Meetphant

| | |
|---|---|
| **One-click capture** | Mic + system speaker loopback; stop → auto-create meeting & transcribe |
| **Or import** | Drop in a local file — uploaded to TOS, then transcribed by Doubao Seed-ASR 2.0 |
| **Notes you can share** | An LLM of your choice (Qwen by default) turns transcripts into key points, action items, and decisions |
| **Local-first** | Meetings & settings in SQLite; secrets stay in the OS keyring |
| **Hotwords** | Product names and jargon boost ASR accuracy |

<p align="center">
  <img src="docs/screenshots/workspace.png" alt="Transcript and summary workspace" width="860" />
</p>

## Features

- **Recording** — WASAPI loopback mix → M4A when FFmpeg is ready, otherwise WAV without blocking
- **Transcription** — Doubao Seed-ASR 2.0 (`volc.seedasr.auc`) · every file is uploaded to TOS, then async submit/query (45-minute poll window)
- **Summary** — any OpenAI-compatible LLM (presets: DashScope `qwen3.7-plus` by default, DeepSeek, OpenAI, Moonshot, Zhipu, Volcengine Ark, or a custom base URL) → key points / action items / decisions
- **Workspace** — meeting sidebar + split transcript/summary (tabs on narrow windows)
- **Settings** — credentials, hotwords, summary context, recording folder, FFmpeg status

<p align="center">
  <img src="docs/screenshots/settings.png" alt="Settings" width="860" />
</p>

## Quick start

**Prerequisites**

- [Node.js](https://nodejs.org/) 20+ (npm)
- [Rust](https://rustup.rs/) stable (**MSVC** on Windows: `rustup default stable-x86_64-pc-windows-msvc`)
- Windows: [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the “Desktop development with C++” workload, plus WebView2

```bash
npm install
npm run tauri dev
```

Vite serves at `http://localhost:1420` and opens the Meetphant window.

Configure in **Settings**:

| Role | Provider | What you enter |
|------|----------|----------------|
| Transcription | Doubao | API Key (new speech console) |
| Audio upload for ASR (required) | Volcengine TOS | AK/SK in keyring + region / bucket |
| Summary | DashScope (default) or another OpenAI-compatible provider | Provider, Base URL, API Key, model |

Secrets live in the OS credential store only — never in SQLite, never returned by `settings_get`.

Step-by-step Chinese guide (Doubao ASR, Volcengine TOS, Qwen / Bailian API Key): **[凭证申请图文指南](./docs/credentials-guide.zh-CN.md)**.

## Typical flow

1. Pick a mic → **Start recording** (or **Import audio & transcribe**)
2. Stop → meeting is created and transcription starts
3. When the transcript is ready → **Generate summary** and copy it out

## Windows installers

Two NSIS installers, same app id (installing one replaces the other):

| Artifact | Contents | When to use |
|----------|----------|-------------|
| `Meetphant_<ver>_x64-setup.exe` (**lean**, default) | App only | Normal installs; FFmpeg downloads on first need (~80–100 MiB); **in-app updater** uses this channel |
| `Meetphant_<ver>_x64-offline-setup.exe` | App + bundled FFmpeg | Slow / offline networks (manual install only) |

```bash
npm run ffmpeg:prepare
npm run pack:lean
npm run pack:offline
npm run pack:all        # → dist-installers/ (+ latest.json when signed)
```

Push a version tag (or run **Actions → Release**) to build and attach both `.exe` files plus `latest.json` for the updater.

### Auto-update (Windows)

- Apps check `https://github.com/leoshcn/meetphant/releases/latest/download/latest.json` on launch and from **Settings → About**.
- Only the **lean** installer is published on the updater channel.
- Release builds must sign artifacts with a Tauri updater key:
  - Local: set `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH` (optional `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`).
  - CI: repository secrets `TAURI_SIGNING_PRIVATE_KEY` and optional `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
- Generate keys once with `npm run tauri signer generate -- -w ~/.tauri/meetphant.key` and keep the **private** key offline; the public key is embedded in `src-tauri/tauri.conf.json`.
- Windows **Authenticode** / SmartScreen code signing is still not configured (separate from Tauri update signatures).

Bundled FFmpeg is the Gyan **essentials** build (GPLv3). See [GyanD/codexffmpeg](https://github.com/GyanD/codexffmpeg).

## Transcription notes

| Size | Path |
|------|------|
| ≤ 512 MiB (≤ 5 h) | Upload to TOS → pre-signed GET → Doubao Seed-ASR 2.0 submit/query (`volc.seedasr.auc`) |
| > 512 MiB | `ASR_PAYLOAD_TOO_LARGE` |

Hotwords go to ASR. `context_text` is for summaries only — **not** sent to Doubao.

## Develop & test

```bash
npm test
npm run typecheck
cd src-tauri && cargo test
```

```
src/          React UI (app / pages / features / ipc / shared)
src-tauri/    Tauri commands, services, SQLite, providers
```

## Stack

Tauri 2 · React 19 · TypeScript · Vite · SQLite · Doubao ASR · Qwen · OS keyring

---

<p align="center">
  <sub>UI previews in <code>docs/screenshots/</code> · v0.2.0</sub>
</p>
