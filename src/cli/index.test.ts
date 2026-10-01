import { afterEach, describe, expect, test } from "bun:test";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { activityWindow, findSession, registrationSegments, collectDailySessions, dailyWindows, dayBounds, subtractCoverage } from "./index";

const directories: string[] = [];
const original = { root: process.env.CODEX_SESSIONS_DIR, thread: process.env.CODEX_THREAD_ID, claude: process.env.TRACK_CLAUDE_PROJECTS_DIR };
afterEach(() => {
  for (const dir of directories.splice(0)) rmSync(dir, { recursive: true, force: true });
  for (const [key, value] of Object.entries({ CODEX_SESSIONS_DIR: original.root, CODEX_THREAD_ID: original.thread, TRACK_CLAUDE_PROJECTS_DIR: original.claude })) {
    if (value === undefined) delete process.env[key]; else process.env[key] = value;
  }
});
function fixture(id = "session-a", timestamps: string[] = []) {
  const dir = mkdtempSync(join(tmpdir(), "track-session-"));
  directories.push(dir);
  const file = join(dir, `${id}.jsonl`);
  writeFileSync(file, [JSON.stringify({ type: "session_meta", payload: { id, cwd: process.cwd() } }),
    ...timestamps.map((timestamp) => JSON.stringify({ timestamp }))].join("\n"));
  return { dir, file };
}
describe("AI activity candidates", () => {
  test("登録済み時刻より後の区間だけを提案し、丸めても前回区間へ戻らない", () => {
    const { file } = fixture("a", ["2026-10-01T10:00:00+09:00", "2026-10-01T10:20:00+09:00", "2026-10-01T10:35:00+09:00"]);
    expect(activityWindow(file, 30, new Date("2026-10-01T10:40:00+09:00"), "2026-10-01T10:30:00+09:00"))
      .toMatchObject({ start: "2026-10-01T10:30:00+09:00", end: "2026-10-01T10:45:00+09:00" });
  });
  test("新しい活動がなければ現在時刻だけで工数を作らない", () => {
    const { file } = fixture("a", ["2026-10-01T10:20:00+09:00"]);
    expect(activityWindow(file, 30, new Date("2026-10-01T12:00:00+09:00"), "2026-10-01T10:30:00+09:00")).toBeNull();
  });
  test("古いログの末尾から現在までを活動として追加しない", () => {
    const { file } = fixture("a", ["2026-10-01T10:05:00+09:00", "2026-10-01T10:20:00+09:00"]);
    expect(activityWindow(file, 30, new Date("2026-10-01T12:00:00+09:00"))).toMatchObject({
      start: "2026-10-01T10:00:00+09:00", end: "2026-10-01T10:30:00+09:00",
    });
  });
  test("日付またぎを別の要求IDで分割する", () => {
    const parts = registrationSegments({ start: "2026-10-01T23:45:00+09:00", end: "2026-10-02T00:30:00+09:00" });
    expect(parts).toHaveLength(2);
    expect(parts[0].end).toBe("2026-10-02T00:00:00+09:00");
    expect(parts[1].start).toBe(parts[0].end);
    expect(parts[0].requestId).not.toBe(parts[1].requestId);
    expect(registrationSegments(null)).toEqual([]);
  });
});
describe("session selection", () => {
  test("現在のIDがなければ別のログへフォールバックしない", () => {
    const { dir } = fixture();
    process.env.CODEX_SESSIONS_DIR = dir;
    process.env.CODEX_THREAD_ID = "missing";
    expect(() => findSession("codex")).toThrow("セッションを特定できません");
  });
  test("同じフォルダの複数セッションを最新時刻で選ばない", () => {
    const { dir } = fixture();
    writeFileSync(join(dir, "session-b.jsonl"), JSON.stringify({ type: "session_meta", payload: { id: "session-b", cwd: process.cwd() } }));
    process.env.CODEX_SESSIONS_DIR = dir;
    delete process.env.CODEX_THREAD_ID;
    expect(() => findSession("codex")).toThrow("複数のセッション");
    expect(findSession("codex", ["--session-id", "session-b"]).id).toBe("session-b");
  });
  test("明示したファイルのIDも照合する", () => {
    const { file } = fixture();
    expect(findSession("codex", ["--session-file", file, "--session-id", "session-a"]).file).toBe(file);
    expect(() => findSession("codex", ["--session-file", file, "--session-id", "other"])).toThrow("一致しません");
  });
  test("Claudeの明示ログからセッションIDを取得する", () => {
    const { file } = fixture();
    writeFileSync(file, JSON.stringify({ sessionId: "claude-id", timestamp: "2026-10-01T10:00:00+09:00" }));
    expect(findSession("claude", ["--session-file", file, "--session-id", "claude-id"]).id).toBe("claude-id");
  });
});


describe("daily conversation candidates", () => {
  const now = new Date("2026-10-01T16:00:00+09:00");
  const stamp = (clock: string) => Date.parse(`2026-10-01T${clock}:00+09:00`);
  test("当日の複数の活動区間を取り、長い休憩をつなげない", () => {
    const windows = dailyWindows([stamp("10:04"), stamp("10:20"), stamp("14:10"), stamp("14:20")], dayBounds("2026-10-01"), now);
    expect(windows).toEqual([{ start: stamp("10:00"), end: stamp("10:30") }, { start: stamp("14:00"), end: stamp("14:30") }]);
  });
  test("日本時間の日付で絞り、前日・翌日・未来のログは含めない", () => {
    const windows = dailyWindows([
      Date.parse("2026-09-30T15:05:00Z"), // JSTの当日00:05
      Date.parse("2026-09-30T14:55:00Z"),
      Date.parse("2026-10-01T15:05:00Z"),
      stamp("18:00"),
    ], dayBounds("2026-10-01"), now);
    expect(windows).toEqual([{ start: stamp("00:00"), end: stamp("00:15") }]);
    expect(() => dayBounds("2026-02-30")).toThrow("不正");
  });
  test("登録済みの中央区間だけを除き、前後の未記録分を失わない", () => {
    expect(subtractCoverage([{ start: stamp("10:00"), end: stamp("11:00") }], [{ start: stamp("10:15"), end: stamp("10:30") }]))
      .toEqual([{ start: stamp("10:00"), end: stamp("10:15") }, { start: stamp("10:30"), end: stamp("11:00") }]);
    expect(subtractCoverage([{ start: stamp("10:00"), end: stamp("11:00") }], [{ start: stamp("09:00"), end: stamp("12:00") }])).toEqual([]);
  });
  test("登録済み境界が15分単位でなくても候補は15分単位を保つ", () => {
    expect(subtractCoverage([{ start: stamp("10:00"), end: stamp("11:00") }], [{ start: stamp("10:17"), end: stamp("10:28") }]))
      .toEqual([{ start: stamp("10:00"), end: stamp("10:15") }, { start: stamp("10:30"), end: stamp("11:00") }]);
  });
  test("異なる作業フォルダのCodexとClaudeを集め、子エージェントを個別計上しない", () => {
    const { dir, file } = fixture("day-codex", []);
    writeFileSync(file, [
      { type: "session_meta", payload: { id: "day-codex", cwd: "/other-project" } },
      { timestamp: "2026-10-01T10:04:00+09:00", type: "event_msg", payload: { type: "user_message", message: "検索画面を修正" } },
      { timestamp: "2026-10-01T10:20:00+09:00", type: "event_msg", payload: { type: "agent_message", message: "検索を実装しました" } },
    ].map((row) => JSON.stringify(row)).join("\n"));
    const claude = join(dir, "claude-root");
    mkdirSync(join(claude, "project", "subagents"), { recursive: true });
    const transcript = [
      { timestamp: "2026-10-01T11:05:00+09:00", sessionId: "day-claude", cwd: "/third-project", type: "user", message: { content: "レポートを修正" } },
      { timestamp: "2026-10-01T11:15:00+09:00", sessionId: "day-claude", type: "assistant", message: { content: [{ type: "text", text: "修正完了" }] } },
    ].map((row) => JSON.stringify(row)).join("\n");
    writeFileSync(join(claude, "project", "day-claude.jsonl"), transcript);
    writeFileSync(join(claude, "project", "subagents", "agent.jsonl"), transcript.replaceAll("day-claude", "agent"));
    process.env.CODEX_SESSIONS_DIR = dir;
    process.env.TRACK_CLAUDE_PROJECTS_DIR = claude;
    process.env.CODEX_THREAD_ID = "unrelated-current-thread";
    // Codex専用ルートでClaudeログを見つけても、Codexログとしては認識しない。
    const result = collectDailySessions(["codex", "claude"], "2026-10-01", now);
    expect(result.sessions.map((item) => item.id)).toEqual(["day-codex", "day-claude"]);
    expect(result.sessions[0].cwd).toBe("/other-project");
    expect(result.sessions[1].messages[0].text).toBe("レポートを修正");
    expect(collectDailySessions(["codex"], "2026-09-30", now).sessions).toEqual([]);
  });
});
