// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from "vitest";
import {
  ARTIFACT_KIND_LABELS,
  fileNameFromPath,
  formatBytes,
  formatCount,
  formatDuration,
  NOT_AVAILABLE,
} from "./formatters";
import type { ArtifactKind } from "../ipc/client";

describe("formatBytes", () => {
  it("renders null/undefined as 'Not available', never 0 (§9.3, §25)", () => {
    expect(formatBytes(null)).toBe(NOT_AVAILABLE);
    expect(formatBytes(undefined)).toBe(NOT_AVAILABLE);
  });

  it("renders a genuine zero-byte file as 0 B, distinct from unknown", () => {
    expect(formatBytes(0)).toBe("0 B");
  });

  it("scales through KB/MB/GB", () => {
    expect(formatBytes(500)).toBe("500 B");
    expect(formatBytes(2048)).toBe("2 KB");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5 MB");
  });
});

describe("formatCount", () => {
  it("renders null as 'Not available', never 0", () => {
    expect(formatCount(null)).toBe(NOT_AVAILABLE);
  });

  it("renders a genuine zero count as 0", () => {
    expect(formatCount(0)).toBe("0");
  });

  it("adds thousands separators", () => {
    expect(formatCount(1234567)).toBe("1,234,567");
  });
});

describe("formatDuration", () => {
  it("renders null as 'Not available'", () => {
    expect(formatDuration(null)).toBe(NOT_AVAILABLE);
  });

  it("renders seconds/minutes/hours", () => {
    expect(formatDuration(4000)).toBe("4s");
    expect(formatDuration(65_000)).toBe("1m 05s");
    expect(formatDuration(3_661_000)).toBe("1h 01m 01s");
  });
});

describe("ARTIFACT_KIND_LABELS", () => {
  it("has a label for every ArtifactKind, and never an empty label", () => {
    const kinds: ArtifactKind[] = ["uniqueGames", "duplicateGames", "reportJson", "reportText", "logText"];
    for (const kind of kinds) {
      expect(ARTIFACT_KIND_LABELS[kind]).toBeTruthy();
    }
  });
});

describe("fileNameFromPath", () => {
  it("handles Windows and POSIX separators", () => {
    expect(fileNameFromPath("C:\\games\\a.pgn")).toBe("a.pgn");
    expect(fileNameFromPath("/home/user/a.pgn")).toBe("a.pgn");
  });
});
