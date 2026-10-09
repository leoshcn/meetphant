import { beforeEach, describe, expect, it, vi } from "vitest";
import { __setInvokeForTests } from "../client";
import { appHealth, logsOpenDir } from "./health";

describe("health commands", () => {
  beforeEach(() => {
    __setInvokeForTests(null);
  });

  it("appHealth invokes app_health", async () => {
    const invoke = vi.fn().mockResolvedValue({
      status: "ok",
      version: "0.1.0",
    });
    __setInvokeForTests(invoke);

    const result = await appHealth();
    expect(invoke).toHaveBeenCalledWith("app_health", undefined);
    expect(result).toEqual({ status: "ok", version: "0.1.0" });
  });

  it("logsOpenDir invokes logs_open_dir", async () => {
    const invoke = vi.fn().mockResolvedValue(null);
    __setInvokeForTests(invoke);

    await logsOpenDir();
    expect(invoke).toHaveBeenCalledWith("logs_open_dir", undefined);
  });

  it("logsOpenDir surfaces IO_ERROR", async () => {
    __setInvokeForTests(
      vi.fn().mockRejectedValue({ code: "IO_ERROR", message: "Could not open the log directory" }),
    );

    await expect(logsOpenDir()).rejects.toMatchObject({ code: "IO_ERROR" });
  });
});
