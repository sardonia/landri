import './styles.css';

import { open } from '@tauri-apps/plugin-opener';

import { events as tauriEvents, type SamplingParams } from './tauri/client';
import { store } from './store';

const $ = <T extends HTMLElement>(sel: string) => document.querySelector(sel) as T;

function fmtBytes(bytes: number | undefined | null): string {
  if (bytes == null) return '—';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

function setActiveScreen(screen: 'home' | 'history' | 'settings') {
  const sHome = $('#screen-home');
  const sHist = $('#screen-history');
  const sSet = $('#screen-settings');

  sHome.classList.toggle('screen--active', screen === 'home');
  sHist.classList.toggle('screen--active', screen === 'history');
  sSet.classList.toggle('screen--active', screen === 'settings');

  const navHome = $('#nav-home');
  const navHist = $('#nav-history');
  const navSet = $('#nav-settings');

  navHome.setAttribute('aria-current', screen === 'home' ? 'page' : 'false');
  navHist.setAttribute('aria-current', screen === 'history' ? 'page' : 'false');
  navSet.setAttribute('aria-current', screen === 'settings' ? 'page' : 'false');
}

function toast(title: string, message: string) {
  const root = $('#toast-root');
  const el = document.createElement('div');
  el.className = 'toast frame';
  el.innerHTML = `
    <div class="toast__title">${title}</div>
    <div class="toast__msg">${message}</div>
  `;
  root.appendChild(el);
  setTimeout(() => {
    el.style.opacity = '0';
    el.style.transition = 'opacity 220ms ease';
    setTimeout(() => el.remove(), 260);
  }, 4200);
}

function bindUI() {
  // Top nav
  $('#nav-home').addEventListener('click', () => store.getState().actions.setScreen('home'));
  $('#nav-history').addEventListener('click', async () => {
    store.getState().actions.setScreen('history');
    await store.getState().actions.loadHistory();
  });
  $('#nav-settings').addEventListener('click', () => store.getState().actions.setScreen('settings'));

  // Model browser
  $('#hf-search').addEventListener('input', (e) => {
    const v = (e.target as HTMLInputElement).value;
    store.setState({ hfQuery: v });
  });
  $('#btn-browse').addEventListener('click', async () => {
    try {
      await store.getState().actions.searchModels(store.getState().hfQuery);
      toast('Models', 'Updated search results from Hugging Face.');
    } catch (err: any) {
      toast('Error', err?.message ?? String(err));
    }
  });

  $('#hf-model-select').addEventListener('change', async (e) => {
    const repoId = (e.target as HTMLSelectElement).value;
    try {
      await store.getState().actions.selectRepo(repoId);
    } catch (err: any) {
      toast('Model list failed', err?.message ?? String(err));
    }
  });

  $('#hf-file-select').addEventListener('change', (e) => {
    const filename = (e.target as HTMLSelectElement).value;
    store.getState().actions.selectFile(filename);
  });

  $('#btn-download').addEventListener('click', async () => {
    try {
      await store.getState().actions.startDownload();
    } catch (err: any) {
      toast('Download error', err?.message ?? String(err));
    }
  });

  $('#btn-cancel-download').addEventListener('click', async () => {
    try {
      await store.getState().actions.cancelDownload();
    } catch (err: any) {
      toast('Cancel error', err?.message ?? String(err));
    }
  });

  // Installed models
  $('#installed-model-select').addEventListener('change', (e) => {
    const v = (e.target as HTMLSelectElement).value;
    store.getState().actions.setSelectedModelKey(v || undefined);
  });

  // Prompt & generation
  const prompt = $('#prompt') as HTMLTextAreaElement;
  prompt.addEventListener('input', () => store.getState().actions.setPrompt(prompt.value));

  $('#btn-clear').addEventListener('click', () => {
    store.getState().actions.clearPrompt();
    store.setState({ output: '' });
  });

  $('#btn-generate').addEventListener('click', async () => {
    try {
      const params = readSamplingParams();
      await store.getState().actions.startGeneration(params);
    } catch (err: any) {
      toast('Generation error', err?.message ?? String(err));
    }
  });

  $('#btn-stop').addEventListener('click', async () => {
    try {
      await store.getState().actions.cancelGeneration();
    } catch (err: any) {
      toast('Stop error', err?.message ?? String(err));
    }
  });

  // Output tools
  $('#btn-copy').addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(store.getState().output);
      toast('Copied', 'Response copied to clipboard.');
    } catch {
      toast('Copy failed', 'Clipboard permission denied.');
    }
  });

  $('#btn-download-output').addEventListener('click', async () => {
    const content = store.getState().output;
    if (!content.trim()) {
      toast('Nothing to export', 'Generate a response first.');
      return;
    }
    const stamp = new Date().toISOString().replace(/[:.]/g, '-');
    const filename = `landry-response-${stamp}.txt`;
    try {
      const path = await (await import('./tauri/client')).api.exportText(filename, content);
      toast('Exported', path);
      // Try opening it for convenience.
      try {
        await open(path);
      } catch {
        // ignore
      }
    } catch (err: any) {
      toast('Export failed', err?.message ?? String(err));
    }
  });

  // History interactions
  $('#history-list').addEventListener('click', async (e) => {
    const target = e.target as HTMLElement;
    const itemEl = target.closest('[data-history-id]') as HTMLElement | null;
    if (!itemEl) return;
    const id = itemEl.dataset.historyId!;
    await store.getState().actions.loadHistoryDetail(id);
  });

  // Settings
  $('#settings-hf-token').addEventListener('input', (e) => {
    const v = (e.target as HTMLInputElement).value;
    store.setState((s) => ({ settings: { ...s.settings, hf_token: v || null } }));
  });

  $('#btn-save-settings').addEventListener('click', async () => {
    try {
      await store.getState().actions.saveSettings(store.getState().settings);
      toast('Saved', 'Settings saved locally.');
    } catch (err: any) {
      toast('Save failed', err?.message ?? String(err));
    }
  });

  $('#btn-reset-settings').addEventListener('click', async () => {
    const s = store.getState().settings;
    const reset = {
      ...s,
      hf_token: null,
      default_repo_id: 'bartowski/Phi-3-mini-4k-instruct-v0.3-GGUF',
      default_quant_hint: 'Q5',
    };
    await store.getState().actions.saveSettings(reset);
    toast('Reset', 'Defaults restored.');
  });

  // Sampling controls
  for (const id of ['temp', 'topP', 'topK', 'maxTokens']) {
    $(`#${id}`).addEventListener('input', () => {
      // no-op; read on generate
    });
  }
}

function readSamplingParams(): SamplingParams {
  const s = store.getState().settings.beginner_params;
  const temp = parseFloat(($('#temp') as HTMLInputElement).value);
  const topP = parseFloat(($('#topP') as HTMLInputElement).value);
  const topK = parseInt(($('#topK') as HTMLInputElement).value, 10);
  const maxTokens = parseInt(($('#maxTokens') as HTMLInputElement).value, 10);

  return {
    ...s,
    temperature: Number.isFinite(temp) ? temp : s.temperature,
    top_p: Number.isFinite(topP) ? topP : s.top_p,
    top_k: Number.isFinite(topK) ? topK : s.top_k,
    max_tokens: Number.isFinite(maxTokens) ? maxTokens : s.max_tokens,
  };
}

function render() {
  const state = store.getState();

  setActiveScreen(state.screen);

  // Left panel: HF model list
  const modelSelect = $('#hf-model-select') as HTMLSelectElement;
  modelSelect.innerHTML = state.hfModels
    .map((m) => `<option value="${m.repo_id}">${m.repo_id}</option>`)
    .join('');
  if (state.hfSelectedRepoId) {
    modelSelect.value = state.hfSelectedRepoId;
  }

  const fileSelect = $('#hf-file-select') as HTMLSelectElement;
  fileSelect.innerHTML = state.hfFiles
    .map((f) => {
      const size = f.size ? ` • ${fmtBytes(f.size)}` : '';
      return `<option value="${f.rfilename}">${f.rfilename}${size}</option>`;
    })
    .join('');
  if (state.hfSelectedFilename) {
    fileSelect.value = state.hfSelectedFilename;
  }

  // Download status
  const pct = state.download.percent ?? 0;
  ($('#download-pct') as HTMLElement).textContent = `${Math.round(pct)}%`;
  ($('#download-bytes') as HTMLElement).textContent = `${fmtBytes(state.download.bytesDownloaded)} / ${fmtBytes(
    state.download.totalBytes,
  )}`;
  ($('#download-state') as HTMLElement).textContent = state.download.state;
  ($('#download-bar') as HTMLElement).setAttribute('style', `width:${Math.max(0, Math.min(100, pct))}%`);

  const cancelDownload = $('#btn-cancel-download') as HTMLButtonElement;
  cancelDownload.disabled = !(state.download.state === 'Downloading' || state.download.state === 'Verifying');

  // Installed models
  const installedSelect = $('#installed-model-select') as HTMLSelectElement;
  installedSelect.innerHTML = state.installed
    .map((m) => {
      const label = `${m.repo_id} — ${m.filename}`;
      return `<option value="${m.model_key}">${label}</option>`;
    })
    .join('');

  if (state.selectedModelKey && state.installed.find((m) => m.model_key === state.selectedModelKey)) {
    installedSelect.value = state.selectedModelKey;
  }

  const selName = state.installed.find((m) => m.model_key === state.selectedModelKey);
  ($('#selected-model-name') as HTMLElement).textContent =
    selName ? `${selName.repo_id} • ${selName.filename}` : 'No model selected';

  // Prompt + output
  const promptEl = $('#prompt') as HTMLTextAreaElement;
  if (promptEl.value !== state.prompt) promptEl.value = state.prompt;

  ($('#response') as HTMLElement).textContent = state.output || 'Your response will appear here…';

  // Response metrics
  ($('#metric-tokens') as HTMLElement).textContent =
    state.gen.totalTokens != null ? `${state.gen.totalTokens}` : '—';

  // Buttons
  const btnGen = $('#btn-generate') as HTMLButtonElement;
  const btnStop = $('#btn-stop') as HTMLButtonElement;
  btnGen.disabled = state.gen.state === 'Generating' || state.gen.state === 'Preparing';
  btnStop.disabled = !(state.gen.state === 'Generating' || state.gen.state === 'Preparing');

  // History list
  const histList = $('#history-list') as HTMLElement;
  histList.innerHTML = state.history
    .map(
      (h) => `
      <div class="history-item" data-history-id="${h.id}">
        <div class="history-item__title">${h.prompt_preview}</div>
        <div class="history-item__meta">${new Date(h.created_at).toLocaleString()} • ${h.total_tokens ?? '—'} tok</div>
      </div>
    `,
    )
    .join('');

  const histDetail = $('#history-detail') as HTMLElement;
  if (!state.historySelected) {
    histDetail.innerHTML = `<div class="muted">Select a run to view details.</div>`;
  } else {
    const d = state.historySelected;
    histDetail.innerHTML = `
      <div class="panel__title">${new Date(d.created_at).toLocaleString()}</div>
      <div class="muted">Model: <span style="color: rgba(255,211,138,.9)">${d.model_key}</span></div>
      <div class="divider"></div>
      <div class="label">Prompt</div>
      <div class="response" style="min-height: 120px">${escapeHtml(d.prompt)}</div>
      <div class="divider"></div>
      <div class="label">Output</div>
      <div class="response" style="min-height: 180px">${escapeHtml(d.output)}</div>
    `;
  }

  // Settings
  const tokenEl = $('#settings-hf-token') as HTMLInputElement;
  tokenEl.value = state.settings.hf_token ?? '';

  // Sampling defaults
  const p = state.settings.beginner_params;
  setIfEmpty($('#temp') as HTMLInputElement, String(p.temperature));
  setIfEmpty($('#topP') as HTMLInputElement, String(p.top_p));
  setIfEmpty($('#topK') as HTMLInputElement, String(p.top_k));
  setIfEmpty($('#maxTokens') as HTMLInputElement, String(p.max_tokens));
}

function setIfEmpty(el: HTMLInputElement, value: string) {
  if (!el.value) el.value = value;
}

function escapeHtml(str: string): string {
  return str
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#039;');
}

async function main() {
  bindUI();

  // Hook Tauri events → store
  await tauriEvents.onDownloadProgress((e) => store.getState().actions.applyDownloadProgress(e));
  await tauriEvents.onDownloadDone((e) => store.getState().actions.applyDownloadDone(e));
  await tauriEvents.onGenToken((e) => store.getState().actions.applyGenToken(e));
  await tauriEvents.onGenDone((e) => store.getState().actions.applyGenDone(e));
  await tauriEvents.onToastError((e) => store.getState().actions.pushToast(e));
  await tauriEvents.onToastInfo((e) => store.getState().actions.pushToast(e));

  // Render loop on store changes
  store.subscribe(() => render());

  // Toast consumer loop
  setInterval(() => {
    const t = store.getState().actions.consumeToast();
    if (t) toast(t.title, t.message);
  }, 250);

  try {
    await store.getState().actions.init();
    render();
  } catch (err: any) {
    toast('Startup error', err?.message ?? String(err));
  }
}

void main();
