import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type ModelKey = string;

export interface HfModelSummary {
  repo_id: string;
  description?: string | null;
  likes?: number | null;
  downloads?: number | null;
}

export interface HfFile {
  rfilename: string;
  size?: number | null;
  sha256?: string | null;
}

export type ModelState =
  | 'NotInstalled'
  | 'Downloading'
  | 'Verifying'
  | 'Installed'
  | 'Corrupt';

export interface InstalledModel {
  model_key: ModelKey;
  repo_id: string;
  filename: string;
  local_path: string;
  size_bytes: number;
  verified: boolean;
  installed_at: number;
}

export type GenState =
  | 'Idle'
  | 'Preparing'
  | 'Generating'
  | 'Done'
  | 'Failed'
  | 'Cancelled';

export interface SamplingParams {
  temperature: number;
  top_p: number;
  top_k: number;
  repeat_penalty: number;
  max_tokens: number;
  stop_sequences: string[];
}

export interface GenerateRequest {
  model_key: ModelKey;
  prompt: string;
  params: SamplingParams;
}

export interface Settings {
  hf_token?: string | null;
  default_repo_id: string;
  default_quant_hint: string;
  beginner_params: SamplingParams;
}

export interface HistoryItemSummary {
  id: string;
  created_at: number;
  model_key: string;
  prompt_preview: string;
  output_preview: string;
  total_tokens?: number | null;
}

export interface HistoryItemDetail {
  id: string;
  created_at: number;
  model_key: string;
  prompt: string;
  output: string;
  params_json: string;
  prompt_tokens?: number | null;
  completion_tokens?: number | null;
  total_tokens?: number | null;
}

// Events
export interface DownloadProgressEvent {
  job_id: string;
  repo_id: string;
  filename: string;
  state: ModelState;
  bytes_downloaded: number;
  total_bytes?: number | null;
  percent?: number | null;
}

export interface DownloadDoneEvent {
  job_id: string;
  model_key: string;
  repo_id: string;
  filename: string;
  local_path: string;
  size_bytes: number;
  verified: boolean;
}

export interface GenTokenEvent {
  job_id: string;
  state: GenState;
  chunk: string;
  total_chars: number;
  elapsed_ms: number;
}

export interface GenDoneEvent {
  job_id: string;
  state: GenState;
  session_id?: string | null;
  total_chars: number;
  prompt_tokens?: number | null;
  completion_tokens?: number | null;
  total_tokens?: number | null;
}

export type ToastKind = 'error' | 'info';

export interface ToastEvent {
  title: string;
  message: string;
  detail?: string | null;
  remediation?: string | null;
  kind: ToastKind;
}

// Invokes
export const api = {
  hfSearchModels: (query?: string) => invoke<HfModelSummary[]>('hf_search_models', { query }),
  hfListGgufFiles: (repoId: string) => invoke<HfFile[]>('hf_list_gguf_files', { repoId }),
  listInstalledModels: () => invoke<InstalledModel[]>('list_installed_models'),
  downloadModel: (repoId: string, filename: string) =>
    invoke<string>('download_model', { repoId, filename }),
  cancelJob: (jobId: string) => invoke<boolean>('cancel_job', { jobId }),
  startGeneration: (req: GenerateRequest) => invoke<string>('start_generation', { req }),
  getSettings: () => invoke<Settings>('get_settings'),
  setSettings: (settings: Settings) => invoke<boolean>('set_settings', { settings }),
  listHistory: (limit = 100) => invoke<HistoryItemSummary[]>('list_history', { limit }),
  getHistoryItem: (id: string) => invoke<HistoryItemDetail | null>('get_history_item', { id }),
  exportText: (filename: string, content: string) => invoke<string>('export_text', { filename, content }),
};

export const events = {
  onDownloadProgress: (handler: (e: DownloadProgressEvent) => void) =>
    listen<DownloadProgressEvent>('landry://download/progress', (event) => handler(event.payload)),
  onDownloadDone: (handler: (e: DownloadDoneEvent) => void) =>
    listen<DownloadDoneEvent>('landry://download/done', (event) => handler(event.payload)),
  onGenToken: (handler: (e: GenTokenEvent) => void) =>
    listen<GenTokenEvent>('landry://gen/token', (event) => handler(event.payload)),
  onGenDone: (handler: (e: GenDoneEvent) => void) =>
    listen<GenDoneEvent>('landry://gen/done', (event) => handler(event.payload)),
  onToastError: (handler: (e: ToastEvent) => void) =>
    listen<ToastEvent>('landry://toast/error', (event) => handler(event.payload)),
  onToastInfo: (handler: (e: ToastEvent) => void) =>
    listen<ToastEvent>('landry://toast/info', (event) => handler(event.payload)),
};
