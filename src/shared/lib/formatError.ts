import type { AppError } from "../../ipc";

const FRIENDLY: Record<string, string> = {
  TOS_NOT_CONFIGURED: "转写需要火山 TOS。请先在设置中完成配置。",
  TOS_UPLOAD_ERROR: "上传到 TOS 失败，请检查 Region、Bucket 与密钥。",
  ASR_TIMEOUT: "转写超时，请稍后重试或换较小的音频文件。",
  ASR_PAYLOAD_TOO_LARGE: "文件超过 512 MiB，无法转写。",
  NOT_FOUND: "未找到该资源。",
  RECORD_NO_DEVICE: "未找到可用的音频输入设备。",
  RECORD_BUSY: "已有录音正在进行。",
  RECORD_NOT_ACTIVE: "当前没有进行中的录音。",
  RECORD_DEVICE_ERROR: "无法使用所选音频设备，请检查系统权限或更换设备。",
};

export function friendlyErrorMessage(err: AppError): string {
  if (err.code && FRIENDLY[err.code]) {
    return FRIENDLY[err.code];
  }
  return err.message || "操作失败";
}

export function errorTitle(err: AppError): string | undefined {
  return err.code || undefined;
}
