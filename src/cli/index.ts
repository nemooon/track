#!/usr/bin/env bun
import { readFileSync, readdirSync } from "node:fs";
import { randomUUID } from "node:crypto";
import { homedir } from "node:os";
import { basename, join, resolve } from "node:path";

type Json = Record<string, unknown>;
type Source = "claude" | "codex";

const QUARTER_MS = 15 * 60_000;
const RUNTIME_FILE =
  process.env.TRACK_RUNTIME_FILE ?? join(homedir(), ".track", "runtime.json");
const JST = new Intl.DateTimeFormat("sv-SE", {
  timeZone: "Asia/Tokyo",
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hourCycle: "h23",
});

class CliError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly details?: unknown,
  ) {
    super(message);
  }
}

function fail(code: string, message: string, details?: unknown): never {
  throw new CliError(code, message, details);
}

function option(args: string[], name: string): string | undefined {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : undefined;
}

function options(args: string[], name: string): string[] {
  return args.flatMap((value, index) =>
    value === name && args[index + 1] ? [args[index + 1]] : [],
  );
}

function apiBase(): string {
  const configured = process.env.TRACK_API_BASE;
  if (configured) return validateApiBase(configured, "TRACK_API_BASE");

  try {
    const runtime = JSON.parse(readFileSync(RUNTIME_FILE, "utf8")) as Json;
    if (typeof runtime.apiBase !== "string") {
      fail("invalid_runtime", `apiBaseがありません: ${RUNTIME_FILE}`);
    }
    return validateApiBase(runtime.apiBase, RUNTIME_FILE);
  } catch (error) {
    if (error instanceof CliError) throw error;
    // 開発サーバーと旧バージョンのTrackに対する後方互換。
    return "http://127.0.0.1:8787/api";
  }
}

function validateApiBase(value: string, source: string): string {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    return fail("invalid_api_base", `接続先がURLではありません: ${source}`);
  }
  if (
    url.protocol !== "http:" ||
    (url.hostname !== "127.0.0.1" && url.hostname !== "localhost")
  ) {
    return fail("invalid_api_base", `ローカル以外の接続先は使用できません: ${source}`);
  }
  return value.replace(/\/+$/, "");
}

async function request(method: string, route: string, body?: Json): Promise<unknown> {
  const api = apiBase();
  let response: Response;
  try {
    response = await fetch(`${api}/${route}`, {
      method,
      headers: body ? { "Content-Type": "application/json" } : undefined,
      body: body ? JSON.stringify(body) : undefined,
    });
  } catch (error) {
    return fail(
      "network_error",
      `Trackへ接続できません。Trackアプリの起動とローカル通信の権限を確認してください (${api}): ${String(error)}`,
    );
  }
  const data = await response.json().catch(() => null);
  if (!response.ok) {
    const code = data && typeof data === "object" && typeof data.error === "string"
      ? data.error : "http_error";
    fail(code, `HTTP ${response.status}`, data);
  }
  return data;
}

function jsonLines(file: string): Json[] {
  return readFileSync(file, "utf8")
    .split("\n")
    .flatMap((line) => {
      try {
        return line ? [JSON.parse(line) as Json] : [];
      } catch {
        return [];
      }
    });
}

function walk(dir: string): string[] {
  try {
    return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
      const child = join(dir, entry.name);
      return entry.isDirectory()
        ? walk(child)
        : entry.name.endsWith(".jsonl")
          ? [child]
          : [];
    });
  } catch {
    return [];
  }
}

type Session = { id: string; file: string };

function sessionFromFile(source: Source, file: string): Session {
  const records = jsonLines(file);
  const metadata = records.find((item) => item.type === "session_meta")?.payload as Json | undefined;
  const id = source === "codex" ? metadata?.id :
    records.find((item) => typeof item.sessionId === "string")?.sessionId ?? basename(file, ".jsonl");
  if (typeof id !== "string" || !id || id.length > 200) {
    fail("invalid_session", "ログからセッションIDを確認できません。", { file });
  }
  return { id, file };
}

export function findSession(source: Source, args: string[] = []): Session {
  const explicitFile = option(args, "--session-file");
  const explicitId = option(args, "--session-id") ??
    (source === "codex" ? process.env.CODEX_THREAD_ID : undefined);
  if (explicitFile) {
    const session = sessionFromFile(source, resolve(explicitFile));
    if (explicitId && session.id !== explicitId) {
      fail("session_id_mismatch", "指定したIDとログのセッションIDが一致しません。");
    }
    return session;
  }
  const cwd = resolve(process.cwd());
  const root = source === "codex"
    ? process.env.CODEX_SESSIONS_DIR ?? join(homedir(), ".codex", "sessions")
    : process.env.TRACK_CLAUDE_PROJECTS_DIR ?? join(homedir(), ".claude", "projects");
  const files = walk(source === "claude" && !explicitId ? join(root, cwd.replaceAll("/", "-")) : root);
  const candidates = files.flatMap((file) => {
    if (explicitId && !basename(file).includes(explicitId)) return [];
    try {
      const session = sessionFromFile(source, file);
      if (explicitId) return session.id === explicitId ? [session] : [];
      if (source === "codex") {
        const meta = jsonLines(file).find((item) => item.type === "session_meta")?.payload as Json | undefined;
        if (meta?.cwd !== cwd) return [];
      }
      return [session];
    } catch { return []; }
  });
  if (!candidates.length) fail("session_not_found", "セッションを特定できません。--session-idまたは--session-fileを指定してください。");
  if (candidates.length !== 1) {
    fail("ambiguous_session", "複数のセッションがあります。--session-idまたは--session-fileを指定してください。", candidates.slice(0, 10));
  }
  return candidates[0];
}

function jstIso(date: Date): string {
  return `${JST.format(date).replace(" ", "T")}+09:00`;
}

export function activityWindow(
  file: string,
  idleMinutes = 30,
  now = new Date(),
  recordedUntil?: string | null,
): Json | null {
  let times = jsonLines(file)
    .flatMap((item) => {
      const time = typeof item.timestamp === "string" ? Date.parse(item.timestamp) : NaN;
      return Number.isNaN(time) ? [] : [time];
    })
    .sort((a, b) => a - b);
  if (!times.length) {
    fail("session_has_no_timestamps", "セッションに時刻情報がありません。");
  }
  const cutoff = recordedUntil ? Date.parse(recordedUntil) : undefined;
  if (cutoff !== undefined && !Number.isFinite(cutoff)) fail("invalid_progress", "登録済み時刻が不正です。");
  if (cutoff !== undefined) times = times.filter((time) => time > cutoff);
  if (!times.length) return null;
  const lastActivity = times.at(-1)!;
  if (now.getTime() >= lastActivity && now.getTime() - lastActivity < idleMinutes * 60_000) {
    times.push(now.getTime());
  }

  let index = times.length - 1;
  while (index > 0 && times[index] - times[index - 1] < idleMinutes * 60_000) {
    index -= 1;
  }
  const start = new Date(Math.max(Math.floor(times[index] / QUARTER_MS) * QUARTER_MS, cutoff ?? -Infinity));
  let end = new Date(Math.ceil(times.at(-1)! / QUARTER_MS) * QUARTER_MS);
  if (end <= start) end = new Date(start.getTime() + QUARTER_MS);

  return {
    start: jstIso(start),
    end: jstIso(end),
    durationMinutes: (end.getTime() - start.getTime()) / 60_000,
    crossesJstMidnight:
      jstIso(start).slice(0, 10) !==
      jstIso(new Date(end.getTime() - 1)).slice(0, 10),
    sessionFile: file,
    recordedUntil: recordedUntil ?? null,
  };
}

function entrySummary(entry: Json): Json {
  const project = entry.project as Json | null;
  const client = project?.client as Json | undefined;
  return {
    id: entry.id,
    start: entry.start,
    end: entry.end,
    title: entry.title,
    project: project?.name ?? null,
    client: client?.name ?? null,
  };
}

async function overlaps(start: string, end: string): Promise<Json[]> {
  const query = new URLSearchParams({ from: start, to: end });
  const data = await request("GET", `entries?${query}`);
  return Array.isArray(data)
    ? data.map((entry) => entrySummary(entry as Json))
    : fail("unexpected_response", "entriesが配列ではありません。");
}

export function registrationSegments(window: Json | null): Json[] {
  if (!window) return [];
  const segments: Json[] = [];
  let start = Date.parse(String(window.start));
  const end = Date.parse(String(window.end));
  while (start < end) {
    const dayStart = Date.parse(`${jstIso(new Date(start)).slice(0, 10)}T00:00:00+09:00`);
    const segmentEnd = Math.min(end, dayStart + 24 * 60 * 60_000);
    segments.push({ requestId: randomUUID(), start: jstIso(new Date(start)), end: jstIso(new Date(segmentEnd)) });
    start = segmentEnd;
  }
  return segments;
}

type Interval = { start: number; end: number };
type DailySession = Session & { source: Source; cwd: string | null; messages: Json[]; windows: Interval[] };

function sessionRoot(source: Source): string {
  return source === "codex"
    ? process.env.CODEX_SESSIONS_DIR ?? join(homedir(), ".codex", "sessions")
    : process.env.TRACK_CLAUDE_PROJECTS_DIR ?? join(homedir(), ".claude", "projects");
}

export function dayBounds(day: string): Interval {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(day)) fail("invalid_day", "--dateはYYYY-MM-DDで指定してください。");
  const start = Date.parse(`${day}T00:00:00+09:00`);
  if (!Number.isFinite(start) || jstIso(new Date(start)).slice(0, 10) !== day) {
    fail("invalid_day", "指定した日付が不正です。");
  }
  return { start, end: start + 24 * 60 * 60_000 };
}

export function dailyWindows(times: number[], bounds: Interval, now: Date, idleMinutes = 30): Interval[] {
  const sorted = [...new Set(times)].filter((time) => time >= bounds.start && time < bounds.end && time <= now.getTime()).sort((a, b) => a - b);
  const groups: number[][] = [];
  for (const time of sorted) {
    const last = groups.at(-1);
    if (!last || time - last.at(-1)! >= idleMinutes * 60_000) groups.push([time]);
    else last.push(time);
  }
  return groups.map((group) => {
    const first = group[0];
    const last = group.at(-1)!;
    const until = now.getTime() >= last && now.getTime() - last < idleMinutes * 60_000
      ? Math.min(now.getTime(), bounds.end) : last;
    const start = Math.max(bounds.start, Math.floor(first / QUARTER_MS) * QUARTER_MS);
    const end = Math.min(bounds.end, Math.max(start + QUARTER_MS, Math.ceil(until / QUARTER_MS) * QUARTER_MS));
    return { start, end };
  });
}

function conversationMessage(item: Json): Json | null {
  const payload = item.payload as Json | undefined;
  let role: unknown;
  let content: unknown;
  if (item.type === "event_msg" && payload && (payload.type === "user_message" || payload.type === "agent_message")) {
    role = payload.type === "user_message" ? "user" : "assistant";
    content = payload.message;
  } else if (item.type === "response_item" && payload?.type === "message") {
    role = payload.role;
    content = payload.content;
  } else if (item.type === "user" || item.type === "assistant") {
    const message = item.message as Json | undefined;
    role = item.type;
    content = message?.content;
  }
  if (role !== "user" && role !== "assistant") return null;
  const text = typeof content === "string" ? content : Array.isArray(content)
    ? content.flatMap((part: Json) => typeof part.text === "string" ? [part.text] : []).join("\n") : "";
  if (!text.trim()) return null;
  return { timestamp: item.timestamp, role, text: text.slice(0, 800) };
}

export function collectDailySessions(sources: Source[], day: string, now = new Date(), idleMinutes = 30): { sessions: DailySession[]; warnings: Json[] } {
  const bounds = dayBounds(day);
  const sessions = new Map<string, { session: DailySession; times: number[] }>();
  const warnings: Json[] = [];
  for (const source of sources) {
    const root = sessionRoot(source);
    const files = source === "codex" && !process.env.CODEX_SESSIONS_DIR
      ? [...walk(root), ...walk(join(homedir(), ".codex", "archived_sessions"))]
      : walk(root);
    for (const file of files) {
      // Claudeの子エージェントログは親会話と同じ作業を含むため個別計上しない。
      if (file.split("/").includes("subagents")) continue;
      try {
        const records = jsonLines(file);
        const today = records.filter((item) => {
          const time = typeof item.timestamp === "string" ? Date.parse(item.timestamp) : NaN;
          return time >= bounds.start && time < bounds.end && time <= now.getTime();
        });
        if (!today.length) continue;
        const metadata = records.find((item) => item.type === "session_meta")?.payload as Json | undefined;
        const origin = metadata?.source;
        if (source === "codex" && origin && typeof origin === "object" && "subagent" in origin) continue;
        const found = sessionFromFile(source, file);
        const key = `${source}:${found.id}`;
        const existing = sessions.get(key);
        const messages = today.map(conversationMessage).filter((item): item is Json => item !== null);
        const times = today.map((item) => Date.parse(String(item.timestamp)));
        if (existing) {
          existing.times.push(...times);
          existing.session.messages.push(...messages);
        } else {
          const cwd = metadata?.cwd ?? records.find((item) => typeof item.cwd === "string")?.cwd;
          sessions.set(key, { session: { ...found, source, cwd: typeof cwd === "string" ? cwd : null, messages, windows: [] }, times });
        }
      } catch (error) { warnings.push({ file, error: String(error) }); }
    }
  }
  return { sessions: [...sessions.values()].map(({ session, times }) => ({
    ...session,
    messages: (() => {
      const messages = [...new Map(session.messages.map((message) => [JSON.stringify(message), message])).values()]
        .sort((a, b) => Date.parse(String(a.timestamp)) - Date.parse(String(b.timestamp)));
      return messages.length <= 20 ? messages : [...messages.slice(0, 3), ...messages.slice(-17)];
    })(),
    windows: dailyWindows(times, bounds, now, idleMinutes),
  })).sort((a, b) => a.windows[0].start - b.windows[0].start), warnings };
}

export function subtractCoverage(windows: Interval[], covered: Interval[]): Interval[] {
  let remaining = windows;
  for (const taken of covered) {
    remaining = remaining.flatMap((window) => {
      if (taken.end <= window.start || taken.start >= window.end) return [window];
      const pieces: Interval[] = [];
      const before = Math.floor(taken.start / QUARTER_MS) * QUARTER_MS;
      const after = Math.ceil(taken.end / QUARTER_MS) * QUARTER_MS;
      if (before > window.start) pieces.push({ start: window.start, end: before });
      if (after < window.end) pieces.push({ start: after, end: window.end });
      return pieces;
    });
  }
  return remaining;
}

async function prepareDay(args: string[], idleMinutes: number): Promise<Json> {
  const selected = option(args, "--source") ?? "all";
  if (selected !== "all" && selected !== "codex" && selected !== "claude") fail("invalid_source", "--source all|codex|claudeを指定してください。");
  const now = new Date();
  const day = option(args, "--date") ?? jstIso(now).slice(0, 10);
  const bounds = dayBounds(day);
  const sources: Source[] = selected === "all" ? ["codex", "claude"] : [selected];
  const collected = collectDailySessions(sources, day, now, idleMinutes);
  const existing = await overlaps(jstIso(new Date(bounds.start)), jstIso(new Date(bounds.end)));
  const projects = await request("GET", "projects");
  if (!Array.isArray(projects)) fail("unexpected_response", "projectsが配列ではありません。");
  const segments: Json[] = [];
  for (const session of collected.sessions) {
    const progress = await request("GET", `entries/ai-progress?${new URLSearchParams({ source: session.source, sessionId: session.id, coverage: "1" })}`) as Json;
    if (!Array.isArray(progress.registrations)) fail("unexpected_response", "Trackを最新版へ更新してください。");
    const covered = [
      ...existing.map((entry) => ({ start: Date.parse(String(entry.start)), end: Date.parse(String(entry.end)) })),
      ...progress.registrations.map((row: Json) => ({ start: row.recordedFrom ? Date.parse(String(row.recordedFrom)) : bounds.start, end: Date.parse(String(row.recordedUntil)) })),
    ];
    const remaining = subtractCoverage(session.windows, covered);
    segments.push(...remaining.map((window) => ({
      requestId: randomUUID(), source: session.source, sessionId: session.id,
      start: jstIso(new Date(window.start)), end: jstIso(new Date(window.end)),
      durationMinutes: (window.end - window.start) / 60_000,
    })));
  }
  segments.sort((a, b) => Date.parse(String(a.start)) - Date.parse(String(b.start)));
  for (const segment of segments) {
    segment.parallelWith = segments.filter((other) => other !== segment &&
      Date.parse(String(other.start)) < Date.parse(String(segment.end)) &&
      Date.parse(String(other.end)) > Date.parse(String(segment.start))).map((other) => other.requestId);
  }
  const union: Interval[] = [];
  for (const segment of segments) {
    const start = Date.parse(String(segment.start));
    const end = Date.parse(String(segment.end));
    const prior = union.at(-1);
    if (prior && start <= prior.end) prior.end = Math.max(prior.end, end);
    else union.push({ start, end });
  }
  return {
    scope: "day", date: day, timezone: "Asia/Tokyo", source: selected, roundingMinutes: 15,
    segments, totalCandidateMinutes: union.reduce((sum, interval) => sum + (interval.end - interval.start) / 60_000, 0),
    sessions: collected.sessions.map(({ id, source, file, cwd, messages }) => ({ sessionId: id, source, sessionFile: file, cwd, messages })),
    projects, existing, warnings: collected.warnings,
  };
}

async function prepare(args: string[]): Promise<Json> {
  const idleMinutes = Number(option(args, "--idle-minutes") ?? 30);
  if (!Number.isFinite(idleMinutes) || idleMinutes < 1) {
    fail("invalid_input", "--idle-minutesは1以上の数値にしてください。");
  }
  const scope = option(args, "--scope") ?? (option(args, "--session-id") || option(args, "--session-file") ? "session" : "day");
  if (scope === "day") return prepareDay(args, idleMinutes);
  if (scope !== "session") fail("invalid_scope", "--scope day|sessionを指定してください。");
  const source = option(args, "--source") as Source | undefined;
  if (source !== "codex" && source !== "claude") fail("invalid_source", "会話単位では--source codex|claudeが必要です。");
  const session = findSession(source, args);
  const progress = await request("GET", `entries/ai-progress?${new URLSearchParams({ source, sessionId: session.id })}`) as Json;
  const window = activityWindow(session.file, idleMinutes, new Date(), progress.recordedUntil as string | null);

  const projects = await request("GET", "projects");
  if (!Array.isArray(projects)) {
    fail("unexpected_response", "projectsが配列ではありません。");
  }
  return {
    scope: "session", roundingMinutes: 15,
    source,
    sessionId: session.id,
    segments: registrationSegments(window),
    window,
    projects,
    overlaps: window ? await overlaps(String(window.start), String(window.end)) : [],
  };
}

async function create(args: string[]): Promise<Json> {
  if (!args.includes("--confirmed")) {
    fail("confirmation_required", "登録前に内容を提示し、利用者の確認を得てください。");
  }
  const start = option(args, "--start") ?? fail("invalid_input", "--startが必要です。");
  const end = option(args, "--end") ?? fail("invalid_input", "--endが必要です。");
  const title = option(args, "--title") ?? fail("invalid_input", "--titleが必要です。");
  if (!(Date.parse(end) > Date.parse(start))) {
    fail("invalid_interval", "endはstartより後にしてください。");
  }
  const startDay = jstIso(new Date(start)).slice(0, 10);
  const endDay = jstIso(new Date(Date.parse(end) - 1)).slice(0, 10);
  if (startDay !== endDay) {
    fail("crosses_jst_midnight", "JSTの同一日内に分けてください。");
  }
  if (Date.parse(start) % QUARTER_MS !== 0 || Date.parse(end) % QUARTER_MS !== 0) {
    fail("invalid_time_step", "開始・終了時刻は15分単位で指定してください。");
  }
  if (title.length > 100) fail("title_too_long", "titleは100文字以内です。");

  const source = option(args, "--source");
  const sessionId = option(args, "--session-id");
  const requestId = option(args, "--request-id");
  if ((source || sessionId || requestId) &&
    ((source !== "codex" && source !== "claude") || !sessionId || !requestId)) {
    fail("invalid_input", "--source、--session-id、--request-idを一緒に指定してください。");
  }
  // AI登録の重複確認はAPIのトランザクション内で実施。再送は成功済み結果を返す。
  const existing = requestId ? [] : await overlaps(start, end);
  const duplicate = existing.some(
    (entry) =>
      Date.parse(String(entry.start)) === Date.parse(start) &&
      Date.parse(String(entry.end)) === Date.parse(end) &&
      entry.title === title,
  );
  if (duplicate) fail("exact_duplicate", "同一エントリが登録済みです。");
  if (existing.length > 0 && !args.includes("--allow-overlap")) {
    fail(
      "overlap_requires_confirmation",
      "既存エントリと重複します。重複を提示し、許可後に--allow-overlapを付けてください。",
      existing,
    );
  }

  const created = (await request("POST", "entries", {
    start,
    end,
    title,
    projectId: option(args, "--project-id") ?? null,
    note: option(args, "--note") ?? null,
    tagIds: options(args, "--tag-id"),
    breakMinutes: Number(option(args, "--break-minutes") ?? 0),
    ...(requestId ? { aiRegistration: {
      requestId, source, sessionId, allowOverlap: args.includes("--allow-overlap"),
    } } : {}),
  })) as Json;
  return { created: entrySummary(created) };
}

function usage(): string {
  return `Track CLI

使い方:
  track-cli status
  track-cli projects
  track-cli list --from <ISO日時> --to <ISO日時>
  track-cli prepare [--source all|codex|claude] [--date YYYY-MM-DD]
                    [--scope day|session] [--idle-minutes 30]
                    [--session-id <ID>] [--session-file <ログパス>]
  track-cli create --start <ISO日時> --end <ISO日時> --title <タイトル>
                   [--project-id <ID>] [--note <メモ>] [--tag-id <ID> ...]
                   [--break-minutes <分>] --confirmed [--allow-overlap]
                   [--source codex|claude --session-id <ID> --request-id <UUID>]

createは利用者へ登録内容を提示し、明示的な確認を得た後に実行してください。
接続先は~/.track/runtime.jsonから自動検出します。`;
}

async function run(args = process.argv.slice(2)): Promise<Json | string> {
  const [command, ...rest] = args;
  if (!command || command === "help" || command === "--help" || command === "-h") {
    return usage();
  }
  if (command === "status") {
    const projects = await request("GET", "projects");
    return {
      connected: true,
      apiBase: apiBase(),
      projectCount: Array.isArray(projects) ? projects.length : null,
    };
  }
  if (command === "projects") {
    return { projects: await request("GET", "projects") };
  }
  if (command === "list") {
    const from = option(rest, "--from") ?? fail("invalid_input", "--fromが必要です。");
    const to = option(rest, "--to") ?? fail("invalid_input", "--toが必要です。");
    return { entries: await overlaps(from, to) };
  }
  if (command === "prepare") return prepare(rest);
  if (command === "create") return create(rest);
  return fail("invalid_command", `未対応のコマンドです: ${command}`);
}

async function main(): Promise<void> {
  try {
    const result = await run();
    console.log(typeof result === "string" ? result : JSON.stringify({ ok: true, ...result }, null, 2));
  } catch (error) {
    const cliError =
      error instanceof CliError
        ? error
        : new CliError("unexpected_error", String(error));
    console.log(
      JSON.stringify(
        {
          ok: false,
          error: cliError.code,
          message: cliError.message,
          details: cliError.details,
        },
        null,
        2,
      ),
    );
    process.exitCode = 1;
  }
}

if (import.meta.main) await main();
