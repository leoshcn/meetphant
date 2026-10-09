/**
 * Demo fixtures for the website screenshot pipeline (dev-only, never bundled).
 * Shapes mirror `src/ipc/types.ts`. All content is fictional sample data:
 * no real people, no real keys.
 */
import type {
  DevicesResponse,
  FfmpegStatus,
  Job,
  Meeting,
  RecordStatusResponse,
  Settings,
  Summary,
  Transcript,
} from "../../src/ipc/types";

export type Scene = "workspace" | "recording" | "hotwords" | "llm" | "home";

const REC_DIR = "C:\\Users\\demo\\Documents\\Meetphant\\Recordings";

function isoDaysAgo(days: number, hour: number, minute: number): string {
  const d = new Date();
  d.setDate(d.getDate() - days);
  d.setHours(hour, minute, 0, 0);
  return d.toISOString();
}

export const meetings: Meeting[] = [
  {
    id: "m-roadmap",
    source_path: `${REC_DIR}\\2026-10-09_1400.m4a`,
    title: "Q4 产品路线评审",
    created_at: isoDaysAgo(0, 14, 0),
  },
  {
    id: "m-weekly",
    source_path: `${REC_DIR}\\2026-10-08_1000.m4a`,
    title: "研发周会：1.1 版本进度同步",
    created_at: isoDaysAgo(1, 10, 0),
  },
  {
    id: "m-customer",
    source_path: `${REC_DIR}\\2026-10-07_1530.m4a`,
    title: "客户访谈：华东区渠道反馈",
    created_at: isoDaysAgo(2, 15, 30),
  },
  {
    id: "m-design",
    source_path: `${REC_DIR}\\2026-10-05_1100.m4a`,
    title: "设计评审：录音浮窗交互",
    created_at: isoDaysAgo(4, 11, 0),
  },
  {
    id: "m-hiring",
    source_path: `${REC_DIR}\\2026-09-30_1600.m4a`,
    title: "招聘复盘：前端工程师岗位",
    created_at: isoDaysAgo(9, 16, 0),
  },
];

const roadmapTranscript: Transcript = {
  meeting_id: "m-roadmap",
  text: "",
  speaker_names: { "1": "林悦", "2": "周凯", "3": "陈思远" },
  segments: [
    {
      speaker_id: "1",
      text: "我们先过一下 Q4 的路线。核心目标有两个：一是把转写准确率再往上提，二是让纪要能直接发给没参会的同事。",
    },
    {
      speaker_id: "2",
      text: "准确率这块，热词表上线后专有名词的错误明显少了。下一步想支持按项目切换热词，避免不同客户的名词互相干扰。",
    },
    {
      speaker_id: "3",
      text: "设计这边建议把录音浮窗再做轻一点，开会时只保留计时和停止按钮，鼠标移上去再展开波形。",
    },
    {
      speaker_id: "1",
      text: "可以。摘要方面，好几位客户问能不能接公司内网部署的大模型，这个需求优先级要提上来。",
    },
    {
      speaker_id: "2",
      text: "摘要本来就走 OpenAI 兼容接口，填自定义 Base URL 就能接 Ollama 或者内网网关。我们补一份配置说明就行。",
    },
    {
      speaker_id: "1",
      text: "好，那就这么定：10 月底前完成浮窗改版和接入文档，11 月做按项目热词。周凯，排期周五前给我。",
    },
    {
      speaker_id: "3",
      text: "浮窗的交互稿我下周二前出两版，周三约大家一起看。",
    },
  ],
};

const roadmapSummary: Summary = {
  meeting_id: "m-roadmap",
  language: "zh-CN",
  created_at: isoDaysAgo(0, 15, 5),
  key_points: [
    "Q4 两大目标：继续提升转写准确率；让会议纪要可直接分享给未参会同事。",
    "热词表上线后专有名词识别错误明显减少，计划支持按项目切换热词。",
    "多位客户希望摘要接入内网部署的大模型；现有 OpenAI 兼容接口填写自定义 Base URL 即可支持。",
    "录音浮窗将进一步精简，默认只显示计时与停止按钮。",
  ],
  action_items: [
    "周凯：周五前提交 Q4 研发排期。",
    "陈思远：下周二前输出两版浮窗交互稿，周三组织评审。",
    "研发：补充「接入本地 / 内网大模型」配置说明文档。",
  ],
  decisions: [
    "10 月底前完成录音浮窗改版与本地模型接入文档。",
    "按项目切换热词排入 11 月迭代。",
  ],
};

function genericTranscript(meetingId: string): Transcript {
  return {
    meeting_id: meetingId,
    text: "",
    speaker_names: { "1": "林悦", "2": "周凯" },
    segments: [
      { speaker_id: "1", text: "我们开始吧，先同步一下上周的进展。" },
      { speaker_id: "2", text: "主要的功能都已经联调完成，剩下两个缺陷在修。" },
    ],
  };
}

function genericSummary(meetingId: string): Summary {
  return {
    meeting_id: meetingId,
    language: "zh-CN",
    created_at: isoDaysAgo(1, 11, 0),
    key_points: ["主要功能联调完成，剩余两个缺陷修复中。"],
    action_items: ["周凯：本周内关闭剩余缺陷。"],
    decisions: ["按原计划推进发布。"],
  };
}

export function settingsFor(theme: "light" | "dark"): Settings {
  return {
    hotwords: [
      "Meetphant",
      "林悦",
      "周凯",
      "陈思远",
      "火山引擎 TOS",
      "Ollama",
      "录音浮窗",
    ],
    context_text:
      "Meetphant 产品团队例会。参会人：产品经理林悦、研发负责人周凯、设计师陈思远。本季度重点：转写准确率、纪要分享、录音浮窗改版。",
    doubao_configured: true,
    summary_llm_configured: true,
    summary_llm_provider: "custom",
    summary_llm_base_url: "http://localhost:11434/v1",
    summary_llm_model: "qwen3:14b",
    tos_configured: true,
    tos_region: "cn-beijing",
    tos_bucket: "meetphant-demo",
    tos_endpoint: "",
    recording_dir: "",
    recording_dir_resolved: REC_DIR,
    theme_preference: theme,
  };
}

const devices: DevicesResponse = {
  devices: [
    { id: "mic-array", name: "麦克风阵列 (Realtek(R) Audio)", is_default: true },
    { id: "usb-headset", name: "USB 耳机麦克风", is_default: false },
  ],
  default_id: "mic-array",
};

const ffmpeg: FfmpegStatus = {
  installed: true,
  busy: false,
  phase: "installed",
  downloaded_bytes: 0,
  total_bytes: 0,
  path: "C:\\Users\\demo\\AppData\\Local\\Meetphant\\ffmpeg\\ffmpeg.exe",
  message: null,
};

/** Recording started 23m 41s before page load. */
const recordingStartedAt = new Date(Date.now() - (23 * 60 + 41) * 1000).toISOString();

/** Speech-like envelope: slow syllable rhythm with pauses. */
function level(t: number, phase: number, base: number): number {
  const syllable = Math.abs(Math.sin(t / 170 + phase));
  const phrase = 0.5 + 0.5 * Math.sin(t / 1300 + phase * 2);
  const jitter = 0.75 + 0.25 * Math.sin(t / 37 + phase * 5);
  return Math.min(1, base * syllable * phrase * jitter + 0.02);
}

function recordStatusFor(scene: Scene): RecordStatusResponse {
  if (scene !== "recording") {
    return {
      state: "idle",
      path: null,
      started_at: null,
      device_name: null,
      output_device_name: null,
      mic_level: 0,
      system_level: 0,
    };
  }
  const t = performance.now();
  return {
    state: "recording",
    path: `${REC_DIR}\\2026-10-09_1600.wav`,
    started_at: recordingStartedAt,
    device_name: "麦克风阵列 (Realtek(R) Audio)",
    output_device_name: "扬声器 (Realtek(R) Audio)",
    mic_level: level(t, 0, 0.55),
    system_level: level(t, 1.7, 0.45),
  };
}

function job(meetingId: string): Job {
  return {
    id: `job-${meetingId}`,
    meeting_id: meetingId,
    kind: "transcription",
    status: "succeeded",
    error_code: null,
    error_message: null,
    created_at: isoDaysAgo(0, 14, 40),
    updated_at: isoDaysAgo(0, 14, 45),
  };
}

function findMeeting(id: unknown): Meeting {
  return meetings.find((m) => m.id === id) ?? meetings[0];
}

/** Commands whose return value is irrelevant to the screenshots. */
const VOID_COMMANDS = new Set([
  "recording_hide_to_tray",
  "recording_restore_from_tray",
  "recording_hide_tray",
  "logs_open_dir",
]);

export function handle(
  scene: Scene,
  theme: "light" | "dark",
  cmd: string,
  args: Record<string, unknown> | undefined,
): unknown {
  const a = args ?? {};
  switch (cmd) {
    case "app_health":
      return { status: "ok", version: "1.0.0" };
    case "settings_get":
    case "settings_update":
    case "settings_clear_doubao_credentials":
    case "settings_clear_summary_llm_credentials":
    case "settings_clear_tos_credentials":
      return settingsFor(theme);
    case "settings_test_doubao":
    case "settings_test_tos":
    case "settings_test_summary_llm":
      return { ok: true };
    case "meetings_list":
      return meetings;
    case "meetings_get":
    case "meetings_rename":
    case "meetings_attach_source":
      return findMeeting(a.meeting_id);
    case "meetings_create":
    case "meetings_create_from_file":
      return meetings[0];
    case "meetings_delete":
      return null;
    case "meetings_get_transcript":
    case "meetings_update_speakers":
      return a.meeting_id === "m-roadmap"
        ? roadmapTranscript
        : genericTranscript(String(a.meeting_id));
    case "summary_get":
    case "summary_generate":
      return a.meeting_id === "m-roadmap"
        ? roadmapSummary
        : genericSummary(String(a.meeting_id));
    case "jobs_start_transcription":
    case "jobs_get":
      return job(String(a.meeting_id ?? "m-roadmap"));
    case "record_list_input_devices":
      return devices;
    case "record_status":
      return recordStatusFor(scene);
    case "record_start":
      return {
        path: `${REC_DIR}\\2026-10-09_1600.wav`,
        device_name: devices.devices[0].name,
        output_device_name: "扬声器 (Realtek(R) Audio)",
      };
    case "record_stop":
      return { path: `${REC_DIR}\\2026-10-09_1600.wav`, duration_ms: 1421000 };
    case "ffmpeg_status":
    case "ffmpeg_download":
      return ffmpeg;
    // Plugin commands.
    case "plugin:app|version":
      return "1.0.0";
    case "plugin:updater|check":
      return null;
    case "plugin:dialog|open":
      return null;
    default:
      if (VOID_COMMANDS.has(cmd) || cmd.startsWith("plugin:window|")) {
        return null;
      }
      console.warn(`[demo] unmocked command: ${cmd}`);
      return null;
  }
}
