import { invokeCommand } from "../client";
import type { HealthResponse } from "../types";

export function appHealth(): Promise<HealthResponse> {
  return invokeCommand<HealthResponse>("app_health");
}

/** Open the app log directory in the system file manager. */
export function logsOpenDir(): Promise<void> {
  return invokeCommand<void>("logs_open_dir");
}
