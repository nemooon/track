import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import path from "node:path";

const TAURI_DIR = path.resolve(import.meta.dir, "..");
const read = (file: string) => readFileSync(path.join(TAURI_DIR, file), "utf8");

// Hardened Runtime (codesign --options runtime) で署名した Bun 実行ファイルは、
// allow-jit が無いと JavaScriptCore の JIT が使えずインタプリタで動く。
// 待機中でも CPU を 10% 以上使い、リクエスト処理も約2倍遅くなる。
describe("Bun 実行ファイルの署名", () => {
  test("エンタイトルメントで JIT を許可する", () => {
    expect(read("Entitlements.plist")).toMatch(
      /<key>com\.apple\.security\.cs\.allow-jit<\/key>\s*<true\/>/,
    );
  });

  test("sidecar は Hardened Runtime とエンタイトルメントを一緒に指定する", () => {
    const script = read("scripts/build-sidecar.ts");
    expect(script).toContain('"--options",\n      "runtime"');
    expect(script).toContain('path.join(TAURI_DIR, "Entitlements.plist")');
  });

  test("CLI も Hardened Runtime で署名するならエンタイトルメントを渡す", () => {
    const script = read("scripts/build-cli.ts");
    expect(script).toContain('"--options", "runtime"');
    expect(script).toContain('"--entitlements"');
    expect(script).toContain("Entitlements.plist");
  });
});
