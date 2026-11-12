import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import path from "node:path";

const LIB_RS = readFileSync(path.resolve(import.meta.dir, "../src/lib.rs"), "utf8");

function functionBody(name: string): string {
  const start = LIB_RS.indexOf(`fn ${name}(`);
  if (start === -1) throw new Error(`fn ${name} が見つかりません`);
  const open = LIB_RS.indexOf("{", start);
  let depth = 0;
  for (let i = open; i < LIB_RS.length; i++) {
    if (LIB_RS[i] === "{") depth++;
    if (LIB_RS[i] === "}" && --depth === 0) return LIB_RS.slice(open, i + 1);
  }
  throw new Error(`fn ${name} の終わりが見つかりません`);
}

// Tauri の AppHandle::restart() はメインスレッドから呼ばれると RunEvent::Exit を経由せず
// exit(0) するため、Exit イベントでの stop_sidecar が走らない。アプリ内アップデート後の
// 再起動で古い sidecar が親なしで残り、ポートと CPU を使い続けていた。
describe("sidecar の終了", () => {
  test("再起動の前に sidecar を止める", () => {
    const body = functionBody("restart_track");
    const stop = body.indexOf("stop_sidecar(");
    const restart = body.indexOf(".restart()");
    expect(stop).toBeGreaterThan(-1);
    expect(restart).toBeGreaterThan(stop);
  });

  test("通常の終了でも sidecar を止める", () => {
    expect(LIB_RS).toMatch(/RunEvent::Exit \| RunEvent::ExitRequested \{ \.\. \}\) \{\s*stop_sidecar\(app_handle\);/);
  });
});
