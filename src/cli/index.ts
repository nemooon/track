#!/usr/bin/env bun
import { readFileSync, readdirSync, statSync } from "node:fs";
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
      `Trackへ接続できません。Trackアプリを起動してください (${api}): ${String(error)}`,
    );
  }
  const data = await response.json().catch(() => null);
  if (!response.ok) fail("http_error", `HTTP ${response.status}`, data);
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

function newest(files: string[]): string | undefined {
  return files.sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
}

function findSession(source: Source): string {
  const cwd = resolve(process.cwd());
  if (source === "claude") {
    const key = cwd.replaceAll("/", "-");
    return (
      newest(walk(join(homedir(), ".claude", "projects", key))) ??
      fail("session_not_found", "Claude Codeセッションを特定できませんでした。")
    );
  }

  const root = process.env.CODEX_SESSIONS_DIR ?? join(homedir(), ".codex", "sessions");
  const files = walk(root);
  const threadId = process.env.CODEX_THREAD_ID;
  const byId = threadId
    ? newest(files.filter((file) => basename(file).includes(threadId)))
    : undefined;
  if (byId) return byId;

  return (
    newest(
      files.filter((file) => {
        const meta = jsonLines(file).find((item) => item.type === "session_meta")
          ?.payload as Json | undefined;
        return meta?.cwd === cwd;
      }),
    ) ?? fail("session_not_found", "Codexセッションを特定できませんでした。")
  );
}

function jstIso(date: Date): string {
  return `${JST.format(date).replace(" ", "T")}+09:00`;
}

export function activityWindow(
  file: string,
  idleMinutes = 30,
  now = new Date(),
): Json {
  const times = jsonLines(file)
    .flatMap((item) => {
      const time = typeof item.timestamp === "string" ? Date.parse(item.timestamp) : NaN;
      return Number.isNaN(time) ? [] : [time];
    })
    .sort((a, b) => a - b);
  if (!times.length) {
    fail("session_has_no_timestamps", "セッションに時刻情報がありません。");
  }
  if (now.getTime() >= times.at(-1)!) times.push(now.getTime());

  let index = times.length - 1;
  while (index > 0 && times[index] - times[index - 1] < idleMinutes * 60_000) {
    index -= 1;
  }
  const start = new Date(Math.floor(times[index] / QUARTER_MS) * QUARTER_MS);
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

async function prepare(args: string[]): Promise<Json> {
  const source = option(args, "--source") as Source | undefined;
  if (source !== "claude" && source !== "codex") {
    fail("invalid_source", "--source claude|codex が必要です。");
  }
  const idleMinutes = Number(option(args, "--idle-minutes") ?? 30);
  if (!Number.isFinite(idleMinutes) || idleMinutes < 1) {
    fail("invalid_input", "--idle-minutesは1以上の数値にしてください。");
  }
  const window = activityWindow(findSession(source), idleMinutes);
  const projects = await request("GET", "projects");
  if (!Array.isArray(projects)) {
    fail("unexpected_response", "projectsが配列ではありません。");
  }
  return {
    source,
    window,
    projects,
    overlaps: await overlaps(String(window.start), String(window.end)),
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
  if (title.length > 100) fail("title_too_long", "titleは100文字以内です。");

  const existing = await overlaps(start, end);
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
  })) as Json;
  return { created: entrySummary(created) };
}

function usage(): string {
  return `Track CLI

使い方:
  track-cli status
  track-cli projects
  track-cli list --from <ISO日時> --to <ISO日時>
  track-cli prepare --source codex|claude [--idle-minutes 30]
  track-cli create --start <ISO日時> --end <ISO日時> --title <タイトル>
                   [--project-id <ID>] [--note <メモ>] [--tag-id <ID> ...]
                   [--break-minutes <分>] --confirmed [--allow-overlap]

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
