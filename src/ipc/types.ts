export type ThemePreference = "system" | "light" | "dark";

/** Summary LLM provider presets (all OpenAI Chat Completions compatible). */
export type SummaryLlmProvider =
  | "dashscope"
  | "deepseek"
  | "openai"
  | "moonshot"
  | "zhipu"
  | "ark"
  | "custom";

export type Settings = {
  hotwords: string[];
  context_text: string;
  doubao_configured: boolean;
  /** True when a summary LLM API key is in the OS keyring (key never returned). */
  summary_llm_configured: boolean;
  /** Summary LLM provider preset id. */
  summary_llm_provider: SummaryLlmProvider | string;
  /** Effective OpenAI-compatible base URL (preset default when not overridden). */
  summary_llm_base_url: string;
  /** Effective model name (preset recommendation when not overridden). */
  summary_llm_model: string;
  tos_configured: boolean;
  tos_region: string;
  tos_bucket: string;
  tos_endpoint: string;
  /** User override; empty means default Documents/Meetphant/Recordings. */
  recording_dir: string;
  /** Effective path after resolving the empty-default rule. */
  recording_dir_resolved: string;
  /** UI theme: system | light | dark. */
  theme_preference: ThemePreference;
};

export type SettingsUpdate = {
  hotwords?: string[];
  context_text?: string;
  /** Write-only Doubao new-console API Key; never returned by settings_get. */
  doubao_api_key?: string;
  /** Write-only summary LLM API key; never returned by settings_get. */
  summary_llm_api_key?: string;
  /** Known preset id; switching without a base URL / model resets them to preset defaults. */
  summary_llm_provider?: SummaryLlmProvider;
  /** Must start with http:// or https://. */
  summary_llm_base_url?: string;
  /** Must be non-empty. */
  summary_llm_model?: string;
  /** Write-only TOS Access Key Id; never returned by settings_get. */
  tos_access_key_id?: string;
  /** Write-only TOS Secret Access Key; never returned by settings_get. */
  tos_secret_access_key?: string;
  tos_region?: string;
  tos_bucket?: string;
  tos_endpoint?: string;
  /** Absolute path or empty string to reset to default. */
  recording_dir?: string;
  theme_preference?: ThemePreference;
};

/** Optional write-only overrides for settings_test_doubao; empty/omit → use keyring. */
export type SettingsTestDoubaoOverrides = {
  doubao_api_key?: string;
};

/** Optional write-only overrides for settings_test_tos; empty/omit → use keyring/SQLite. */
export type SettingsTestTosOverrides = {
  tos_access_key_id?: string;
  tos_secret_access_key?: string;
  tos_region?: string;
  tos_bucket?: string;
  tos_endpoint?: string;
};

/**
 * Optional overrides for settings_test_summary_llm; empty/omit → use saved.
 * When provider / base_url differ from the saved ones, api_key is required.
 */
export type SettingsTestSummaryLlmOverrides = {
  api_key?: string;
  provider?: SummaryLlmProvider;
  base_url?: string;
  model?: string;
};

export type SettingsTestResult = {
  ok: true;
};

export type HealthResponse = {
  status: "ok" | string;
  version: string;
};

export type Meeting = {
  id: string;
  source_path: string;
  title: string | null;
  created_at: string;
};

export type TranscriptSegment = {
  speaker_id: string;
  text: string;
};

export type Transcript = {
  meeting_id: string;
  text: string;
  segments: TranscriptSegment[];
  speaker_names: Record<string, string>;
};

export type JobStatus = "running" | "succeeded" | "failed" | string;

export type Job = {
  id: string;
  meeting_id: string;
  kind: "transcription" | string;
  status: JobStatus;
  error_code: string | null;
  error_message: string | null;
  created_at: string;
  updated_at: string;
};

export type SummaryLanguage = "zh-CN" | "en" | "zh-en";

export type Summary = {
  meeting_id: string;
  key_points: string[];
  action_items: string[];
  decisions: string[];
  language: SummaryLanguage | string;
  created_at: string;
};

export type InputDevice = {
  id: string;
  name: string;
  is_default: boolean;
};

export type DevicesResponse = {
  devices: InputDevice[];
  default_id: string | null;
};

export type RecordStartResponse = {
  path: string;
  device_name: string;
  output_device_name: string;
};

export type RecordStopResponse = {
  path: string;
  duration_ms: number;
};

export type RecordStatusResponse = {
  state: "idle" | "recording" | string;
  path: string | null;
  started_at: string | null;
  device_name: string | null;
  output_device_name: string | null;
  /** Smoothed microphone amplitude in [0, 1]. */
  mic_level: number;
  /** Smoothed system-loopback amplitude in [0, 1]. */
  system_level: number;
};

export type FfmpegStatus = {
  installed: boolean;
  busy: boolean;
  phase: string;
  downloaded_bytes: number;
  total_bytes: number;
  path: string | null;
  message: string | null;
};

export type FfmpegProgressEvent = {
  phase: string;
  downloaded_bytes: number;
  total_bytes: number;
  installed: boolean;
  message: string | null;
};
