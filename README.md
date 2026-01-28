# Landry (Tauri 2 MVP)

**Landry** is an ethereal, futuristic-luxury desktop **LLM/prompt playground for beginners**.

- **Frontend:** Vite + TypeScript (no heavy UI framework)
- **Backend:** Tauri 2 + Rust
- **Inference:** `mistralrs` (mistral.rs) running locally
- **Model source:** Hugging Face (GGUF-first)

## What works end-to-end

- Browse/search Hugging Face models (curated defaults + real HF search)
- List GGUF files for a model
- Download GGUF with **progress events** + **cancellation**
- Persist installed models registry (SQLite)
- Select an installed model, enter a prompt, and generate a **streamed response** (chunked)
- Cancel generation quickly
- Persist history (SQLite) + view history screen
- Settings screen for HF token (for gated models)
- Export the current response to a `.txt` file

## Quick start

### Prerequisites

- Rust (stable)
- Node 18+ / npm
- Tauri prerequisites for your platform

### Install

```bash
npm install
```

### Run (dev)

```bash
npm run tauri dev
```

### Build (release)

```bash
npm run tauri build
```

## Models

Downloaded models are stored under:

- `{app_data_dir}/models/{repo_id_sanitized}/{filename}.gguf`

For beginners, we default to:

- `bartowski/Phi-3-mini-4k-instruct-v0.3-GGUF`
- Prefer a **Q5** GGUF (Q5_K_M if available)

## Manual test checklist

### Model search / list
- [ ] Click **Browse Models**
- [ ] Verify curated models appear immediately
- [ ] Type a query like `phi-3 gguf` and click **Browse Models**
- [ ] Select a repo and verify GGUF files list loads

### Download
- [ ] Click **Download Selected**
- [ ] Verify progress bar updates (bytes + percent)
- [ ] Click **Cancel** mid-download and confirm it stops and the partial file is removed
- [ ] Download completes and the model shows up in **Installed Model** dropdown

### Inference (streaming)
- [ ] Select the installed model
- [ ] Enter a prompt
- [ ] Click **Generate Response**
- [ ] Confirm tokens stream into the response area (not per-token spam)
- [ ] Click **Stop** and confirm it cancels quickly
- [ ] Click **Generate Response** again to retry

### Output tools
- [ ] Click **Copy** and confirm clipboard contains the response
- [ ] Click **Download** to export a `.txt` file and open it

### History
- [ ] Go to **History** tab
- [ ] Confirm runs are listed
- [ ] Click a run and confirm prompt/output details render

### Settings
- [ ] Go to **Settings** tab
- [ ] Enter HF token (optional)
- [ ] Save and restart app; confirm token persists

## Architecture notes

Rust backend uses Clean / Hexagonal (Ports & Adapters) with:

- `app/` orchestrators: `ModelOrchestrator`, `GenerationOrchestrator`
- `managers/` actor-ish managers: `JobManager`, `ModelManager`, `InferenceManager`
- `ports/` traits for HF, registry, history, inference, settings, events
- `adapters/` implementations (HF, SQLite, JSON settings, mistral helpers, Tauri events)

Events streamed over Tauri events with chunking/backpressure:

- `landry://download/progress`
- `landry://download/done`
- `landry://gen/token`
- `landry://gen/done`
- `landry://toast/error`
- `landry://toast/info`
