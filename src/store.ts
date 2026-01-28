import { createStore } from 'zustand/vanilla';
import type {
  DownloadDoneEvent,
  DownloadProgressEvent,
  GenDoneEvent,
  GenState,
  GenTokenEvent,
  HfFile,
  HfModelSummary,
  HistoryItemDetail,
  HistoryItemSummary,
  InstalledModel,
  ModelKey,
  ModelState,
  SamplingParams,
  Settings,
  ToastEvent,
} from './tauri/client';
import { api } from './tauri/client';

export type Screen = 'home' | 'history' | 'settings';

export interface DownloadStatus {
  state: ModelState;
  jobId?: string;
  repoId?: string;
  filename?: string;
  bytesDownloaded?: number;
  totalBytes?: number | null;
  percent?: number | null;
}

export interface GenerationStatus {
  state: GenState;
  jobId?: string;
  totalChars: number;
  elapsedMs?: number;
  promptTokens?: number | null;
  completionTokens?: number | null;
  totalTokens?: number | null;
}

export interface LandryState {
  screen: Screen;

  hfQuery: string;
  hfModels: HfModelSummary[];
  hfSelectedRepoId?: string;
  hfFiles: HfFile[];
  hfSelectedFilename?: string;

  installed: InstalledModel[];
  selectedModelKey?: ModelKey;

  prompt: string;
  output: string;

  download: DownloadStatus;
  gen: GenerationStatus;

  history: HistoryItemSummary[];
  historySelected?: HistoryItemDetail | null;

  settings: Settings;

  toastQueue: ToastEvent[];

  actions: {
    setScreen: (screen: Screen) => void;
    init: () => Promise<void>;
    searchModels: (query?: string) => Promise<void>;
    selectRepo: (repoId: string) => Promise<void>;
    selectFile: (filename: string) => void;
    refreshInstalled: () => Promise<void>;
    setSelectedModelKey: (key: ModelKey | undefined) => void;
    setPrompt: (text: string) => void;
    clearPrompt: () => void;
    startDownload: () => Promise<void>;
    cancelDownload: () => Promise<void>;
    startGeneration: (params: SamplingParams) => Promise<void>;
    cancelGeneration: () => Promise<void>;
    loadHistory: () => Promise<void>;
    loadHistoryDetail: (id: string) => Promise<void>;
    saveSettings: (settings: Settings) => Promise<void>;
    pushToast: (toast: ToastEvent) => void;
    consumeToast: () => ToastEvent | undefined;
    applyDownloadProgress: (e: DownloadProgressEvent) => void;
    applyDownloadDone: (e: DownloadDoneEvent) => void;
    applyGenToken: (e: GenTokenEvent) => void;
    applyGenDone: (e: GenDoneEvent) => void;
  };
}

const defaultBeginner: SamplingParams = {
  temperature: 0.2,
  top_p: 0.9,
  top_k: 40,
  repeat_penalty: 1.05,
  max_tokens: 512,
  stop_sequences: [],
};

export const store = createStore<LandryState>((set, get) => ({
  screen: 'home',

  hfQuery: '',
  hfModels: [],
  hfSelectedRepoId: undefined,
  hfFiles: [],
  hfSelectedFilename: undefined,

  installed: [],
  selectedModelKey: undefined,

  prompt: '',
  output: '',

  download: { state: 'NotInstalled' },
  gen: { state: 'Idle', totalChars: 0 },

  history: [],
  historySelected: null,

  settings: {
    hf_token: null,
    default_repo_id: 'bartowski/Phi-3-mini-4k-instruct-v0.3-GGUF',
    default_quant_hint: 'Q5',
    beginner_params: defaultBeginner,
  },

  toastQueue: [],

  actions: {
    setScreen: (screen) => set({ screen }),

    init: async () => {
      const settings = await api.getSettings();
      set({ settings });

      await get().actions.searchModels('');
      await get().actions.refreshInstalled();

      // Pick default repo if present.
      const repo = settings.default_repo_id;
      if (repo) {
        await get().actions.selectRepo(repo);
      }

      // Default selected model = most recent installed.
      const installed = get().installed;
      if (installed.length > 0) {
        get().actions.setSelectedModelKey(installed[0].model_key);
      }
    },

    searchModels: async (query) => {
      const q = query ?? get().hfQuery;
      set({ hfQuery: q });
      const models = await api.hfSearchModels(q);
      set({ hfModels: models });

      // Auto-select first result if nothing selected
      const sel = get().hfSelectedRepoId;
      if (!sel && models.length > 0) {
        await get().actions.selectRepo(models[0].repo_id);
      }
    },

    selectRepo: async (repoId) => {
      set({ hfSelectedRepoId: repoId, hfFiles: [], hfSelectedFilename: undefined });
      const files = await api.hfListGgufFiles(repoId);

      // Sort by size (descending), then name.
      files.sort((a, b) => (b.size ?? 0) - (a.size ?? 0));

      // Pick default file: prefer Q5_K_M, then Q5, then first.
      const hint = (get().settings.default_quant_hint || 'Q5').toUpperCase();
      const pick =
        files.find((f) => f.rfilename.toUpperCase().includes('Q5_K_M')) ??
        files.find((f) => f.rfilename.toUpperCase().includes(hint)) ??
        files[0];

      set({ hfFiles: files, hfSelectedFilename: pick?.rfilename });
    },

    selectFile: (filename) => set({ hfSelectedFilename: filename }),

    refreshInstalled: async () => {
      const installed = await api.listInstalledModels();
      // Most recent first
      installed.sort((a, b) => b.installed_at - a.installed_at);
      set({ installed });

      // Keep selection if possible
      const sel = get().selectedModelKey;
      if (!sel && installed.length > 0) {
        set({ selectedModelKey: installed[0].model_key });
      }
    },

    setSelectedModelKey: (key) => set({ selectedModelKey: key }),

    setPrompt: (text) => set({ prompt: text }),

    clearPrompt: () => set({ prompt: '' }),

    startDownload: async () => {
      const repoId = get().hfSelectedRepoId;
      const filename = get().hfSelectedFilename;
      if (!repoId || !filename) throw new Error('Select a repo and GGUF file first.');

      set({ download: { state: 'Downloading', repoId, filename, percent: 0 } });
      const jobId = await api.downloadModel(repoId, filename);
      set((s) => ({ download: { ...s.download, jobId } }));
    },

    cancelDownload: async () => {
      const jobId = get().download.jobId;
      if (!jobId) return;
      await api.cancelJob(jobId);
    },

    startGeneration: async (params) => {
      const modelKey = get().selectedModelKey;
      if (!modelKey) throw new Error('Select an installed model first.');
      const prompt = get().prompt.trim();
      if (!prompt) throw new Error('Write a prompt first.');

      set({ gen: { state: 'Preparing', jobId: undefined, totalChars: 0 }, output: '' });
      const jobId = await api.startGeneration({ model_key: modelKey, prompt, params });
      set((s) => ({ gen: { ...s.gen, jobId, state: 'Generating' } }));
    },

    cancelGeneration: async () => {
      const jobId = get().gen.jobId;
      if (!jobId) return;
      await api.cancelJob(jobId);
    },

    loadHistory: async () => {
      const items = await api.listHistory(200);
      set({ history: items, historySelected: null });
    },

    loadHistoryDetail: async (id) => {
      const item = await api.getHistoryItem(id);
      set({ historySelected: item });
    },

    saveSettings: async (settings) => {
      await api.setSettings(settings);
      set({ settings });
    },

    pushToast: (toast) => set((s) => ({ toastQueue: [...s.toastQueue, toast] })),

    consumeToast: () => {
      const q = get().toastQueue;
      if (q.length === 0) return undefined;
      const [first, ...rest] = q;
      set({ toastQueue: rest });
      return first;
    },

    applyDownloadProgress: (e) => {
      set((s) => ({
        download: {
          ...s.download,
          state: e.state,
          jobId: e.job_id,
          repoId: e.repo_id,
          filename: e.filename,
          bytesDownloaded: e.bytes_downloaded,
          totalBytes: e.total_bytes,
          percent: e.percent,
        },
      }));
    },

    applyDownloadDone: (e) => {
      set((s) => ({
        download: {
          ...s.download,
          state: 'Installed',
          jobId: e.job_id,
          repoId: e.repo_id,
          filename: e.filename,
          percent: 100,
        },
      }));
      void get().actions.refreshInstalled();
    },

    applyGenToken: (e) => {
      set((s) => ({
        gen: {
          ...s.gen,
          state: e.state,
          jobId: e.job_id,
          totalChars: e.total_chars,
          elapsedMs: e.elapsed_ms,
        },
        output: s.output + e.chunk,
      }));
    },

    applyGenDone: (e) => {
      set((s) => ({
        gen: {
          ...s.gen,
          state: e.state,
          jobId: e.job_id,
          totalChars: e.total_chars,
          promptTokens: e.prompt_tokens ?? null,
          completionTokens: e.completion_tokens ?? null,
          totalTokens: e.total_tokens ?? null,
        },
      }));
      void get().actions.loadHistory();
    },
  },
}));
