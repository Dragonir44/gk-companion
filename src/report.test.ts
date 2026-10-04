import { describe, expect, it } from "vitest";
import { diagnosticsText, issueUrl, type ReportContext } from "./report";

const ctx: ReportContext = {
  diag: {
    appVersion: "0.1.4",
    os: "CachyOS Linux Rolling [64-bit]",
    arch: "x86_64",
    games: [
      { game: "gk1", found: true, manual: false, buildId: "22583570", saveDirs: 1 },
      { game: "gk2", found: false, manual: false, buildId: null, saveDirs: 0 },
    ],
  },
  lang: "fr",
  data: { gk1: { freshness: "cached", save: "followed" } },
};

describe("report", () => {
  it("summarizes the environment without paths", () => {
    const text = diagnosticsText(ctx);
    expect(text).toContain("GK Companion v0.1.4 · CachyOS Linux Rolling [64-bit] · x86_64 · lang fr");
    expect(text).toContain("Graveyard Keeper: build 22583570 · data cached · save followed");
    expect(text).toContain("Graveyard Keeper 2: not found · save no save folder");
    expect(text).not.toMatch(/\/home|C:\\\\/);
  });

  it("opens the matching issue form, prefilled", () => {
    const url = new URL(issueUrl("feature", ctx));
    expect(url.pathname).toBe("/Dragonir44/gk-companion/issues/new");
    expect(url.searchParams.get("template")).toBe("feature_request.yml");
    expect(url.searchParams.get("diagnostics")).toContain("v0.1.4");
  });
});
