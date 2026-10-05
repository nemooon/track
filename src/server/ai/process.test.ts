import { afterEach, describe, expect, test } from "bun:test";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { newestCodexExecutable, resolveExecutable } from "./process";

const directories: string[] = [];
afterEach(async () => {
  await Promise.all(directories.splice(0).map((dir) => rm(dir, { recursive: true, force: true })));
});

describe("Codex executable discovery", () => {
  test("npmがPATH先頭に置くSDKのCLIより通常のCLIを優先する", async () => {
    const dir = await mkdtemp(path.join(tmpdir(), "track-executable-"));
    directories.push(dir);
    const bundled = path.join(dir, "node_modules/.bin");
    const external = path.join(dir, "bin");
    for (const directory of [bundled, external]) {
      await mkdir(directory, { recursive: true });
      for (const executable of ["codex", "custom-cli"]) {
        await writeFile(path.join(directory, executable), "#!/bin/sh\necho codex-cli 999.0.0\n", { mode: 0o755 });
      }
    }
    const originalPath = process.env.PATH;
    try {
      process.env.PATH = [bundled, external].join(path.delimiter);
      expect(resolveExecutable("codex", dir)).toBe(path.join(external, "codex"));
      // Explicit paths remain authoritative; custom commands retain PATH order.
      expect(resolveExecutable(path.join(bundled, "codex"), dir)).toBe(path.join(bundled, "codex"));
      expect(resolveExecutable("custom-cli", dir)).toBe(path.join(bundled, "custom-cli"));
    } finally {
      if (originalPath === undefined) delete process.env.PATH;
      else process.env.PATH = originalPath;
    }
  });

  test("デスクトップ同梱版が新しければPATHのCLIより優先する", async () => {
    const dir = await mkdtemp(path.join(tmpdir(), "track-codex-version-"));
    directories.push(dir);
    const older = path.join(dir, "homebrew-codex");
    const newer = path.join(dir, "desktop-codex");
    await writeFile(older, "#!/bin/sh\necho codex-cli 0.154.0\n", { mode: 0o755 });
    await writeFile(newer, "#!/bin/sh\necho codex-cli 0.160.0\n", { mode: 0o755 });
    expect(newestCodexExecutable([older, newer])).toBe(newer);
    expect(newestCodexExecutable([newer, older])).toBe(newer);
    expect(newestCodexExecutable([path.join(dir, "missing"), older])).toBe(older);
  });
});
