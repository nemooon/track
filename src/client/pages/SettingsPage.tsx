import { useState, useEffect, useRef } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@client/components/ui/button";
import {
  Dialog,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@client/components/ui/dialog";
import { Input } from "@client/components/ui/input";
import { Label } from "@client/components/ui/label";
import { useAppUi } from "@client/components/AppUiContext";
import { apiFetch } from "@client/lib/fetcher";
import { ProjectsPage } from "@client/pages/ProjectsPage";
import type { UserSettings, AppConfig, Snapshot } from "@shared/types";

const DAY_LABELS = ["日", "月", "火", "水", "木", "金", "土"];

function minutesToTime(mins: number): string {
  const h = Math.floor(mins / 60);
  const m = mins % 60;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;
}

function timeToMinutes(value: string): number {
  const [h, m] = value.split(":").map(Number);
  return h * 60 + m;
}

export type SettingsCategory =
  | "work-hours"
  | "weekly-report"
  | "projects"
  | "backup"
  | "data-transfer";

export function SettingsPage({ category }: { category: SettingsCategory }) {
  const qc = useQueryClient();
  const { setSettingsDirty } = useAppUi();
  const isTauri = "__TAURI_INTERNALS__" in window;
  const { data: current, isLoading } = useQuery({
    queryKey: ["settings"],
    queryFn: () => apiFetch<UserSettings>("/api/settings"),
  });

  // Work schedule form (minutes since midnight)
  const [workStart, setWorkStart] = useState(600);
  const [workEnd, setWorkEnd] = useState(1110);
  const [workDays, setWorkDays] = useState<number[]>([1, 2, 3, 4, 5]);
  const [weeklyReportTemplate, setWeeklyReportTemplate] = useState("");
  useEffect(() => {
    if (current) {
      setWorkStart(current.workStart);
      setWorkEnd(current.workEnd);
      setWorkDays(current.workDays);
      setWeeklyReportTemplate(current.weeklyReportTemplate);
    }
  }, [current]);

  const normalizedWorkDays = [...workDays].sort((a, b) => a - b);
  const workDirty =
    !!current &&
    (workStart !== current.workStart ||
      workEnd !== current.workEnd ||
      normalizedWorkDays.join(",") !==
        [...current.workDays].sort((a, b) => a - b).join(","));
  const workError =
    workEnd <= workStart
      ? "勤務終了は勤務開始より後にしてください"
      : workDays.length === 0
        ? "勤務日を1日以上選択してください"
        : null;
  const weeklyReportDirty =
    !!current && weeklyReportTemplate !== current.weeklyReportTemplate;
  const weeklyReportError = weeklyReportTemplate.trim()
    ? null
    : "テンプレートを入力してください";

  function resetWorkSettings() {
    if (!current) return;
    setWorkStart(current.workStart);
    setWorkEnd(current.workEnd);
    setWorkDays(current.workDays);
  }

  function resetWeeklyReportSettings() {
    if (!current) return;
    setWeeklyReportTemplate(current.weeklyReportTemplate);
  }

  const updateSettings = useMutation({
    mutationFn: (data: {
      workStart?: number;
      workEnd?: number;
      workDays?: number[];
      weeklyReportTemplate?: string;
    }) =>
      apiFetch("/api/settings", { method: "PATCH", body: JSON.stringify(data) }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["settings"] });
      toast.success("設定を更新しました");
    },
    onError: () => toast.error("更新に失敗しました"),
  });

  // バックアップ設定
  const { data: config } = useQuery({
    queryKey: ["config"],
    queryFn: () => apiFetch<AppConfig>("/api/config"),
  });
  const { data: suggestions = [] } = useQuery({
    queryKey: ["config", "suggestions"],
    queryFn: () => apiFetch<{ label: string; path: string }[]>("/api/config/suggestions"),
  });

  const [exportDir, setExportDir] = useState("");
  const [backupIntervalHours, setBackupIntervalHours] = useState(24);
  const [backupKeep, setBackupKeep] = useState(30);
  useEffect(() => {
    if (config) {
      setExportDir(config.exportDir);
      setBackupIntervalHours(config.backupIntervalHours);
      setBackupKeep(config.backupKeep);
    }
  }, [config]);

  const configDirty =
    !!config &&
    (exportDir !== config.exportDir ||
      backupIntervalHours !== config.backupIntervalHours ||
      backupKeep !== config.backupKeep);
  let configError: string | null = null;
  if (config) {
    if (!exportDir.trim()) {
      configError = "バックアップの保存先を指定してください";
    } else if (
      !Number.isInteger(backupIntervalHours) ||
      backupIntervalHours < 0 ||
      backupIntervalHours > 720
    ) {
      configError = "実行間隔は0〜720時間で指定してください";
    } else if (
      !Number.isInteger(backupKeep) ||
      backupKeep < 1 ||
      backupKeep > 1000
    ) {
      configError = "残す本数は1〜1000本で指定してください";
    }
  }

  function resetBackupSettings() {
    if (!config) return;
    setExportDir(config.exportDir);
    setBackupIntervalHours(config.backupIntervalHours);
    setBackupKeep(config.backupKeep);
  }

  useEffect(() => {
    setSettingsDirty(
      category === "work-hours"
        ? workDirty
        : category === "weekly-report"
          ? weeklyReportDirty
          : category === "backup"
            ? configDirty
            : false,
    );
  }, [
    category,
    configDirty,
    setSettingsDirty,
    weeklyReportDirty,
    workDirty,
  ]);

  useEffect(
    () => () => {
      setSettingsDirty(false);
    },
    [setSettingsDirty],
  );

  const updateConfig = useMutation({
    mutationFn: (patch: Partial<AppConfig>) =>
      apiFetch<AppConfig>("/api/config", { method: "PATCH", body: JSON.stringify(patch) }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["config"] });
      toast.success("バックアップ設定を保存しました");
    },
    onError: (err) => {
      const msg = (err as Error).message;
      if (msg.includes("not_writable")) {
        toast.error("そのフォルダには書き込めません");
      } else if (msg.includes("not_absolute")) {
        toast.error("絶対パスを指定してください");
      } else {
        toast.error("保存に失敗しました");
      }
    },
  });

  async function pickExportDir() {
    try {
      const selected = await open({
        title: "バックアップ保存先を選択",
        directory: true,
        multiple: false,
        canCreateDirectories: true,
        defaultPath: exportDir || config?.defaults.exportDir,
      });
      if (typeof selected === "string") {
        setExportDir(selected);
      }
    } catch {
      toast.error("フォルダ選択を開けませんでした");
    }
  }

  // DB スナップショット
  const { data: backups = [] } = useQuery({
    queryKey: ["backups"],
    queryFn: () => apiFetch<Snapshot[]>("/api/data/backups"),
  });
  const [restoring, setRestoring] = useState<Snapshot | null>(null);

  const runBackup = useMutation({
    mutationFn: () =>
      apiFetch<Snapshot>("/api/data/backup", { method: "POST", body: "{}" }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["backups"] });
      toast.success("バックアップを取りました");
    },
    onError: () => toast.error("バックアップに失敗しました"),
  });

  const runRestore = useMutation({
    mutationFn: (snap: Snapshot) =>
      apiFetch<{ safetyBackup: string | null }>("/api/data/restore", {
        method: "POST",
        body: JSON.stringify({ path: snap.path }),
      }),
    onSuccess: () => {
      setRestoring(null);
      qc.invalidateQueries();
      toast.success("リストアしました");
    },
    onError: (err) => {
      const msg = (err as Error).message;
      toast.error(
        msg.includes("invalid_snapshot")
          ? "このファイルはリストアできません"
          : "リストアに失敗しました",
      );
    },
  });

  // データのエクスポート / インポート
  const fileRef = useRef<HTMLInputElement>(null);
  const [lastExport, setLastExport] = useState<string | null>(null);
  const [validatingImport, setValidatingImport] = useState(false);
  const [pending, setPending] = useState<{
    name: string;
    json: unknown;
    exportedAt: string | null;
    counts: { clients: number; tags: number; projects: number; entries: number };
  } | null>(null);

  const runExport = useMutation({
    mutationFn: () =>
      apiFetch<{ path: string }>("/api/data/export/file", { method: "POST", body: "{}" }),
    onSuccess: (res) => {
      setLastExport(res.path);
      toast.success("エクスポートしました");
    },
    onError: () => toast.error("エクスポートに失敗しました"),
  });

  function showImportError(err: unknown) {
    const msg = (err as Error).message;
    if (msg.includes("inconsistent_data")) {
      toast.error("参照が壊れているためインポートできません");
    } else if (msg.includes("invalid_input")) {
      toast.error("ファイルの形式が Track のエクスポートと合いません");
    } else {
      toast.error("ファイルを確認できませんでした");
    }
  }

  async function onPickImportFile(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    e.target.value = ""; // 同じファイルを続けて選べるようにする
    if (!file) return;

    let json: unknown;
    try {
      json = JSON.parse(await file.text());
    } catch {
      toast.error("JSON として読めませんでした");
      return;
    }

    setValidatingImport(true);
    try {
      const validation = await apiFetch<{
        exportedAt: string | null;
        counts: {
          clients: number;
          tags: number;
          projects: number;
          entries: number;
        };
      }>("/api/data/import/validate", {
        method: "POST",
        body: JSON.stringify(json),
      });
      setPending({
        name: file.name,
        json,
        exportedAt: validation.exportedAt,
        counts: validation.counts,
      });
    } catch (err) {
      showImportError(err);
    } finally {
      setValidatingImport(false);
    }
  }

  const runImport = useMutation({
    mutationFn: (json: unknown) =>
      apiFetch<{ imported: Record<string, number>; safetyBackup: string }>("/api/data/import", {
        method: "POST",
        body: JSON.stringify(json),
      }),
    onSuccess: (res) => {
      setPending(null);
      qc.invalidateQueries();
      const n = res.imported;
      toast.success(
        `安全バックアップを作成してインポートしました (クライアント ${n.clients} / タグ ${n.tags} / プロジェクト ${n.projects} / エントリ ${n.entries})`,
      );
    },
    onError: showImportError,
  });

  function toggleDay(day: number) {
    setWorkDays((prev) =>
      prev.includes(day) ? prev.filter((d) => d !== day) : [...prev, day].sort(),
    );
  }

  if (
    (category === "work-hours" || category === "weekly-report") &&
    isLoading
  ) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-6 w-6 animate-spin rounded-full border-2 border-neutral-300 border-t-neutral-700" />
      </div>
    );
  }

  return (
    <div>
      {category === "work-hours" && (
        <section>
          <div className="mb-6">
            <h2 className="text-2xl font-semibold tracking-tight">勤務時間</h2>
            <p className="mt-1 text-sm text-neutral-500">
              カレンダーの稼働日表示と勤務状況の判定に使います。
            </p>
          </div>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              updateSettings.mutate({ workStart, workEnd, workDays });
            }}
            className="space-y-5 rounded-lg border border-neutral-200 p-5"
          >
            <div className="flex gap-4">
              <div className="space-y-1">
                <Label htmlFor="work-start">勤務開始</Label>
                <Input
                  id="work-start"
                  type="time"
                  step={1800}
                  value={minutesToTime(workStart)}
                  onChange={(e) => setWorkStart(timeToMinutes(e.target.value))}
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="work-end">勤務終了</Label>
                <Input
                  id="work-end"
                  type="time"
                  step={1800}
                  value={minutesToTime(workEnd)}
                  onChange={(e) => setWorkEnd(timeToMinutes(e.target.value))}
                />
              </div>
            </div>
            <div className="space-y-1">
              <div className="text-sm font-medium">勤務日</div>
              <div
                role="group"
                aria-label="勤務日"
                className="flex flex-wrap gap-2"
              >
                {DAY_LABELS.map((label, i) => (
                  <label
                    key={i}
                    className={`flex h-9 w-9 cursor-pointer items-center justify-center rounded-md border text-sm ${
                      workDays.includes(i)
                        ? "border-neutral-900 bg-neutral-900 text-white"
                        : "border-neutral-200 text-neutral-500 hover:bg-neutral-50"
                    }`}
                  >
                    <input
                      type="checkbox"
                      className="sr-only"
                      checked={workDays.includes(i)}
                      onChange={() => toggleDay(i)}
                    />
                    {label}
                  </label>
                ))}
              </div>
            </div>
            {workError && (
              <p role="alert" className="text-sm text-red-600">
                {workError}
              </p>
            )}
            <div className="flex items-center gap-2">
              <Button
                type="submit"
                size="sm"
                disabled={!workDirty || !!workError || updateSettings.isPending}
              >
                {updateSettings.isPending ? "保存中…" : "変更を保存"}
              </Button>
              {workDirty && (
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={resetWorkSettings}
                  disabled={updateSettings.isPending}
                >
                  変更を破棄
                </Button>
              )}
              {!workDirty && !workError && (
                <span className="text-xs text-neutral-500">保存済み</span>
              )}
            </div>
          </form>
        </section>
      )}

      {category === "weekly-report" && (
        <section>
          <div className="mb-6">
            <h2 className="text-2xl font-semibold tracking-tight">
              週報テンプレート
            </h2>
            <p className="mt-1 text-sm text-neutral-500">
              Apple Intelligenceが、この構成に沿って表示中の週の工数をまとめます。
            </p>
          </div>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              updateSettings.mutate({
                weeklyReportTemplate: weeklyReportTemplate.trim(),
              });
            }}
            className="space-y-4 rounded-lg border border-neutral-200 p-5"
          >
            <div className="space-y-1.5">
              <Label htmlFor="weekly-report-template">
                出力テンプレート
              </Label>
              <textarea
                id="weekly-report-template"
                value={weeklyReportTemplate}
                onChange={(e) => setWeeklyReportTemplate(e.target.value)}
                rows={14}
                maxLength={10_000}
                spellCheck
                className="w-full resize-y rounded-md border border-neutral-300 bg-white px-3 py-2 font-mono text-sm leading-6 outline-none transition focus:border-neutral-500 focus:ring-2 focus:ring-neutral-200"
              />
              <p className="text-xs leading-5 text-neutral-500">
                Markdownの見出しや固定文をそのまま書けます。
                <code className="mx-1 rounded bg-neutral-100 px-1 py-0.5">
                  {"{{期間}}"}
                </code>
                と
                <code className="mx-1 rounded bg-neutral-100 px-1 py-0.5">
                  {"{{合計時間}}"}
                </code>
                は生成時に置き換わります。
              </p>
            </div>
            {weeklyReportError && (
              <p role="alert" className="text-sm text-red-600">
                {weeklyReportError}
              </p>
            )}
            <div className="flex items-center gap-2">
              <Button
                type="submit"
                size="sm"
                disabled={
                  !weeklyReportDirty ||
                  !!weeklyReportError ||
                  updateSettings.isPending
                }
              >
                {updateSettings.isPending ? "保存中…" : "テンプレートを保存"}
              </Button>
              {weeklyReportDirty && (
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={resetWeeklyReportSettings}
                  disabled={updateSettings.isPending}
                >
                  変更を破棄
                </Button>
              )}
              {!weeklyReportDirty && !weeklyReportError && (
                <span className="text-xs text-neutral-500">保存済み</span>
              )}
            </div>
          </form>
        </section>
      )}

      {category === "projects" && (
        <section id="projects">
          <div className="mb-6">
            <h2 className="text-2xl font-semibold tracking-tight">
              プロジェクト管理
            </h2>
            <p className="mt-1 text-sm text-neutral-500">
              クライアント、プロジェクト、タグを管理します。
            </p>
          </div>
          <ProjectsPage embedded />
        </section>
      )}

      {category === "backup" && (
        <section>
          <div className="mb-6">
            <h2 className="text-2xl font-semibold tracking-tight">
              バックアップと復元
            </h2>
            <p className="mt-1 text-sm text-neutral-500">
              データベース全体のバックアップ先と履歴を管理します。
            </p>
          </div>

          <div className="space-y-3">
          <div className="rounded-md border border-neutral-200 px-3 py-3">
            <Label htmlFor="backup-directory" className="mb-1 block text-sm">
              保存先
            </Label>
            <p className="mb-2 text-xs text-neutral-500">
              iCloud Drive のフォルダを指定すると、そのままバックアップになります。
              DB 本体（track.db）は同期フォルダに置かないでください — WAL と合わせて
              3ファイルあり、別々に同期されると壊れます。
            </p>
            <div className="flex gap-2">
              <Input
                id="backup-directory"
                value={exportDir}
                onChange={(e) => setExportDir(e.target.value)}
                spellCheck={false}
                className="flex-1 font-mono text-xs"
              />
              {isTauri && (
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={pickExportDir}
                  disabled={updateConfig.isPending}
                >
                  <FolderOpen className="h-4 w-4" />
                  選択
                </Button>
              )}
            </div>
            {suggestions.length > 0 && (
              <div className="mt-2 flex flex-wrap gap-2">
                {suggestions.map((s) => (
                  <Button
                    key={s.path}
                    variant="outline"
                    size="sm"
                    onClick={() => setExportDir(s.path)}
                  >
                    {s.label} を使う
                  </Button>
                ))}
                {config && exportDir !== config.defaults.exportDir && (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setExportDir(config.defaults.exportDir)}
                  >
                    既定に戻す
                  </Button>
                )}
              </div>
            )}
          </div>

          <div className="rounded-md border border-neutral-200 px-3 py-3">
            <div className="mb-2 text-sm font-medium text-neutral-700">自動バックアップ</div>
            <div className="flex items-end gap-3">
              <div className="space-y-1">
                <Label htmlFor="backup-interval" className="text-xs">
                  実行間隔（時間、0で停止）
                </Label>
                <Input
                  id="backup-interval"
                  type="number"
                  min={0}
                  max={720}
                  value={backupIntervalHours}
                  onChange={(e) => setBackupIntervalHours(Number(e.target.value))}
                  className="w-28"
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="backup-keep" className="text-xs">
                  残す本数
                </Label>
                <Input
                  id="backup-keep"
                  type="number"
                  min={1}
                  max={1000}
                  value={backupKeep}
                  onChange={(e) => setBackupKeep(Number(e.target.value))}
                  className="w-28"
                />
              </div>
            </div>
            <p className="mt-2 text-xs text-neutral-500">
              手動で書き出したファイルは本数制限の対象外です。
            </p>
          </div>

          {configError && (
            <p role="alert" className="text-sm text-red-600">
              {configError}
            </p>
          )}
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              onClick={() =>
                updateConfig.mutate({
                  exportDir,
                  backupIntervalHours,
                  backupKeep,
                })
              }
              disabled={!configDirty || !!configError || updateConfig.isPending}
            >
              {updateConfig.isPending ? "保存中…" : "設定を保存"}
            </Button>
            {configDirty && (
              <Button
                variant="ghost"
                size="sm"
                onClick={resetBackupSettings}
                disabled={updateConfig.isPending}
              >
                変更を破棄
              </Button>
            )}
            {!configDirty && !configError && (
              <span className="text-xs text-neutral-500">保存済み</span>
            )}
          </div>

          <div className="rounded-md border border-neutral-200 px-3 py-3">
            <div className="mb-1 flex items-center justify-between">
              <div className="text-sm font-medium text-neutral-700">バックアップ</div>
              <Button
                variant="outline"
                size="sm"
                onClick={() => runBackup.mutate()}
                disabled={runBackup.isPending}
              >
                {runBackup.isPending ? "作成中…" : "今すぐバックアップ"}
              </Button>
            </div>
            <p className="mb-2 text-xs text-neutral-500">
              DB をまるごと1ファイルに写します（VACUUM INTO）。書き込み中のファイルではないので、
              保存先が iCloud Drive でも安全です。
            </p>

            {backups.length === 0 ? (
              <p className="text-sm text-neutral-500">まだありません</p>
            ) : (
              <div className="space-y-1">
                {backups.map((b) => (
                  <div
                    key={b.path}
                    className="flex items-center justify-between rounded border border-neutral-200 px-2 py-1.5"
                  >
                    <div className="min-w-0 text-xs">
                      <div className="truncate font-mono text-neutral-700">{b.name}</div>
                      <div className="text-neutral-500">
                        {new Date(b.createdAt).toLocaleString("ja-JP")} ·{" "}
                        {(b.bytes / 1024).toFixed(0)} KB
                        {b.auto && " · 自動"}
                      </div>
                    </div>
                    <Button variant="ghost" size="sm" onClick={() => setRestoring(b)}>
                      この状態に復元
                    </Button>
                  </div>
                ))}
              </div>
            )}

            <Dialog
              open={!!restoring}
              onOpenChange={(open) => {
                if (!open && !runRestore.isPending) setRestoring(null);
              }}
            >
              {restoring && (
                <div>
                  <DialogHeader>
                    <DialogTitle>バックアップから復元</DialogTitle>
                  </DialogHeader>
                  <p className="text-sm text-neutral-600">
                    <strong>{restoring.name}</strong>
                    の内容で現在のデータを上書きします。実行直前に現在のデータを自動でバックアップします。
                  </p>
                  <DialogFooter>
                    <Button
                      variant="ghost"
                      onClick={() => setRestoring(null)}
                      disabled={runRestore.isPending}
                    >
                      キャンセル
                    </Button>
                    <Button
                      variant="destructive"
                      onClick={() => runRestore.mutate(restoring)}
                      disabled={runRestore.isPending}
                    >
                      {runRestore.isPending ? "復元中…" : "復元する"}
                    </Button>
                  </DialogFooter>
                </div>
              )}
            </Dialog>
          </div>

          </div>
        </section>
      )}

      {category === "data-transfer" && (
        <section>
          <div className="mb-6">
            <h2 className="text-2xl font-semibold tracking-tight">データ移行</h2>
            <p className="mt-1 text-sm text-neutral-500">
              JSON形式で他の環境やツールへデータを移します。通常のバックアップには使用しません。
            </p>
          </div>

          <div className="space-y-3">
            <div className="flex items-center justify-between rounded-md border border-neutral-200 px-3 py-3">
            <div className="text-sm">
              <div className="font-medium text-neutral-700">JSON エクスポート</div>
              <div className="text-xs text-neutral-500">
                他ツールへ渡す / 旧環境から移す用。バックアップ用途ではありません
              </div>
            </div>
            <Button
              variant="outline"
              size="sm"
              onClick={() => runExport.mutate()}
              disabled={runExport.isPending}
            >
              書き出す
            </Button>
          </div>

          {lastExport && (
            <div className="rounded-md border border-neutral-200 bg-neutral-50 px-3 py-2">
              <div className="mb-1 text-sm text-neutral-700">書き出しました</div>
              <code className="block overflow-x-auto text-xs text-neutral-600">
                {lastExport}
              </code>
            </div>
          )}

            <div className="flex items-center justify-between rounded-md border border-neutral-200 px-3 py-3">
            <div className="text-sm">
              <div className="font-medium text-neutral-700">JSON インポート</div>
              <div className="text-xs text-neutral-500">
                既存のデータは全て置き換わります
              </div>
            </div>
            <input
              ref={fileRef}
              type="file"
              accept="application/json,.json"
              className="hidden"
              onChange={onPickImportFile}
            />
            <Button
              variant="outline"
              size="sm"
              onClick={() => fileRef.current?.click()}
              disabled={validatingImport}
            >
              {validatingImport ? "確認中…" : "ファイルを選ぶ"}
            </Button>
          </div>

          <Dialog
            open={!!pending}
            onOpenChange={(open) => {
              if (!open && !runImport.isPending) setPending(null);
            }}
          >
            {pending && (
              <div>
                <DialogHeader>
                  <DialogTitle>データを置き換えますか？</DialogTitle>
                </DialogHeader>
                <div className="space-y-2 text-sm text-neutral-600">
                  <p className="font-medium text-neutral-900">{pending.name}</p>
                  {pending.exportedAt && (
                    <p>
                      書き出し日時:{" "}
                      {new Date(pending.exportedAt).toLocaleString("ja-JP")}
                    </p>
                  )}
                  <p>
                    クライアント {pending.counts.clients} / タグ{" "}
                    {pending.counts.tags} / プロジェクト{" "}
                    {pending.counts.projects} / エントリ{" "}
                    {pending.counts.entries}
                  </p>
                  <p>
                    現在のデータはすべて置き換わります。実行直前に現在のデータを自動でバックアップします。
                  </p>
                </div>
                <DialogFooter>
                  <Button
                    variant="ghost"
                    onClick={() => setPending(null)}
                    disabled={runImport.isPending}
                  >
                    キャンセル
                  </Button>
                  <Button
                    variant="destructive"
                    onClick={() => runImport.mutate(pending.json)}
                    disabled={runImport.isPending}
                  >
                    {runImport.isPending ? "置き換え中…" : "データを置き換える"}
                  </Button>
                </DialogFooter>
              </div>
            )}
          </Dialog>
          </div>
        </section>
      )}
    </div>
  );
}
