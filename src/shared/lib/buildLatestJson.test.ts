import { describe, expect, it } from "vitest";
import { buildLatestJson } from "./buildLatestJson";

describe("buildLatestJson", () => {
  it("points windows-x86_64 at lean asset URL", () => {
    const json = buildLatestJson({
      version: "0.3.0",
      signature: "sig-line\n",
      pubDate: "2026-07-29T00:00:00.000Z",
      notes: "hi",
    });
    expect(json.version).toBe("0.3.0");
    expect(json.notes).toBe("hi");
    expect(json.platforms["windows-x86_64"]).toEqual({
      signature: "sig-line",
      url: "https://github.com/leoshcn/meetphant/releases/download/v0.3.0/Meetphant_0.3.0_x64-setup.exe",
    });
  });
});
