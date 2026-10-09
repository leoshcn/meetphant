import { useState } from "react";
import { SettingsHotwordsPanel } from "../../features/settings-hotwords";
import { SettingsCredentialsPanel } from "../../features/settings-credentials";
import { SettingsRecordingPanel } from "../../features/settings-recording";
import { SettingsFfmpegPanel } from "../../features/settings-ffmpeg";
import { SettingsAppearancePanel } from "../../features/settings-appearance";
import { SettingsAboutPanel } from "../../features/settings-about";
import styles from "./SettingsPage.module.css";

export type SettingsTab =
  | "credentials"
  | "transcription"
  | "recording"
  | "appearance"
  | "about";

const TABS: { id: SettingsTab; label: string }[] = [
  { id: "credentials", label: "凭证" },
  { id: "transcription", label: "转写与摘要" },
  { id: "recording", label: "录音与编码" },
  { id: "appearance", label: "外观" },
  { id: "about", label: "关于" },
];

type Props = {
  initialTab?: SettingsTab;
};

export function SettingsPage({ initialTab = "credentials" }: Props) {
  const [tab, setTab] = useState<SettingsTab>(initialTab);

  return (
    <div className={`${styles.page} meetphant-fade-up`}>
      <header className={styles.header}>
        <h1>设置</h1>
        <p>
          配置转写与摘要所需的凭证、热词与上下文，以及录音保存位置与编码工具。密钥仅保存在本机。
        </p>
      </header>

      <div className={styles.tabs} role="tablist" aria-label="设置分类">
        {TABS.map((item) => {
          const selected = tab === item.id;
          return (
            <button
              key={item.id}
              type="button"
              role="tab"
              id={`settings-tab-${item.id}`}
              aria-selected={selected}
              aria-controls={`settings-panel-${item.id}`}
              className={selected ? styles.tabActive : styles.tab}
              onClick={() => setTab(item.id)}
            >
              {item.label}
            </button>
          );
        })}
      </div>

      <div
        className={styles.stack}
        role="tabpanel"
        id={`settings-panel-${tab}`}
        aria-labelledby={`settings-tab-${tab}`}
      >
        {tab === "credentials" && <SettingsCredentialsPanel />}
        {tab === "transcription" && <SettingsHotwordsPanel />}
        {tab === "recording" && (
          <>
            <SettingsRecordingPanel />
            <SettingsFfmpegPanel />
          </>
        )}
        {tab === "appearance" && <SettingsAppearancePanel />}
        {tab === "about" && <SettingsAboutPanel />}
      </div>
    </div>
  );
}
