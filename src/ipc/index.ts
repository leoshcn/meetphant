export { invokeCommand, normalizeError, isAppError } from "./client";
export type { AppError } from "./client";
export type {
  ThemePreference,
  Settings,
  SettingsUpdate,
  SettingsTestDoubaoOverrides,
  SettingsTestTosOverrides,
  SettingsTestDashscopeOverrides,
  SettingsTestResult,
  HealthResponse,
  Meeting,
  Transcript,
  TranscriptSegment,
  Job,
  JobStatus,
  Summary,
  SummaryLanguage,
  InputDevice,
  DevicesResponse,
  RecordStartResponse,
  RecordStopResponse,
  RecordStatusResponse,
  FfmpegStatus,
  FfmpegProgressEvent,
} from "./types";
export {
  settingsGet,
  settingsUpdate,
  settingsClearDoubaoCredentials,
  settingsClearDashscopeCredentials,
  settingsClearTosCredentials,
  settingsTestDoubao,
  settingsTestTos,
  settingsTestDashscope,
} from "./commands/settings";
export {
  meetingsCreate,
  meetingsCreateFromFile,
  meetingsAttachSource,
  meetingsList,
  meetingsGet,
  meetingsRename,
  meetingsDelete,
  meetingsGetTranscript,
  meetingsUpdateSpeakers,
} from "./commands/meetings";
export { jobsStartTranscription, jobsGet } from "./commands/jobs";
export { summaryGenerate, summaryGet } from "./commands/summary";
export { appHealth, logsOpenDir } from "./commands/health";
export {
  recordListInputDevices,
  recordStart,
  recordStop,
  recordStatus,
} from "./commands/recording";
export {
  recordingHideToTray,
  recordingRestoreFromTray,
  recordingHideTray,
} from "./commands/tray";
export { ffmpegStatus, ffmpegDownload } from "./commands/ffmpeg";
export {
  appGetVersion,
  updaterCheck,
  appRelaunch,
} from "./commands/updater";
export type {
  AvailableUpdate,
  UpdateDownloadEvent,
} from "./commands/updater";
