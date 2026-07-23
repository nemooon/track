import { useState, useEffect, useRef } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@client/components/ui/button";
import { Input } from "@client/components/ui/input";
import { Label } from "@client/components/ui/label";
import { apiFetch } from "@client/lib/fetcher";
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

export function SettingsPage() {
  const qc = useQueryClient();
  const isTauri = "__TAURI_INTERNALS__" in window;
  const { data: current, isLoading } = useQuery({
    queryKey: ["settings"],
    queryFn: () => apiFetch<UserSettings>("/api/settings"),
  });

  // Work schedule form (minutes since midnight)
  const [workStart, setWorkStart] = useState(600);
  const [workEnd, setWorkEnd] = useState(1110);
  const [workDays, setWorkDays] = useState<number[]>([1, 2, 3, 4, 5]);
  useEffect(() => {
    if (current) {
      setWorkStart(current.workStart);
      setWorkEnd(current.workEnd);
      setWorkDays(current.workDays);
    }
  }, [current]);

  const updateSettings = useMutation({
    mutationFn: (data: { workStart?: number; workEnd?: number; workDays?: number[] }) =>
      apiFetch("/api/settings", { method: "PATCH", body: JSON.stringify(data) }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["settings"] });
      toast.success("勤務設定を更新しました");
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
  const [pending, setPending] = useState<{
    name: string;
    json: unknown;
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

  async function onPickImportFile(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    e.target.value = ""; // 同じファイルを続けて選べるようにする
    if (!file) return;
    try {
      const json = JSON.parse(await file.text());
      const counts = {
        clients: Array.isArray(json.clients) ? json.clients.length : 0,
        tags: Array.isArray(json.tags) ? json.tags.length : 0,
        projects: Array.isArray(json.projects) ? json.projects.length : 0,
        entries: Array.isArray(json.entries) ? json.entries.length : 0,
      };
      setPending({ name: file.name, json, counts });
    } catch {
      toast.error("JSON として読めませんでした");
    }
  }

  const runImport = useMutation({
    mutationFn: (json: unknown) =>
      apiFetch<{ imported: Record<string, number> }>("/api/data/import", {
        method: "POST",
        body: JSON.stringify(json),
      }),
    onSuccess: (res) => {
      setPending(null);
      qc.invalidateQueries();
      const n = res.imported;
      toast.success(
        `インポートしました (クライアント ${n.clients} / タグ ${n.tags} / プロジェクト ${n.projects} / エントリ ${n.entries})`,
      );
    },
    onError: (err) => {
      const msg = (err as Error).message;
      if (msg.includes("inconsistent_data")) {
        toast.error("参照が壊れているためインポートを中止しました");
      } else if (msg.includes("invalid_input")) {
        toast.error("ファイルの形式が Track のエクスポートと合いません");
      } else {
        toast.error("インポートに失敗しました");
      }
    },
  });

  function toggleDay(day: number) {
    setWorkDays((prev) =>
      prev.includes(day) ? prev.filter((d) => d !== day) : [...prev, day].sort(),
    );
  }

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-6 w-6 animate-spin rounded-full border-2 border-neutral-300 border-t-neutral-700" />
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-xl space-y-8 p-4 sm:p-6">
      {/* Work schedule */}
      <section>
        <h2 className="mb-4 text-lg font-semibold">勤務設定</h2>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            updateSettings.mutate({ workStart, workEnd, workDays });
          }}
          className="space-y-4"
        >
          <div className="flex gap-4">
            <div className="space-y-1">
              <Label>定時開始</Label>
              <Input
                type="time"
                step={1800}
                value={minutesToTime(workStart)}
                onChange={(e) => setWorkStart(timeToMinutes(e.target.value))}
              />
            </div>
            <div className="space-y-1">
              <Label>定時終了</Label>
              <Input
                type="time"
                step={1800}
                value={minutesToTime(workEnd)}
                onChange={(e) => setWorkEnd(timeToMinutes(e.target.value))}
              />
            </div>
          </div>
          <div className="space-y-1">
            <Label>使う曜日</Label>
            <div className="flex gap-2">
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
          <Button type="submit" size="sm">保存</Button>
        </form>
      </section>

      {/* データ */}
      <section>
        <h2 className="mb-2 text-lg font-semibold">データ</h2>
        <p className="mb-3 text-sm text-neutral-500">
          バックアップは DB をまるごと1ファイルに写す方式です。JSON の出し入れは
          他ツール連携や旧環境からの移行用で、バックアップ用途ではありません。
        </p>

        <div className="space-y-3">
          <div className="rounded-md border border-neutral-200 px-3 py-3">
            <div className="mb-1 text-sm font-medium text-neutral-700">保存先</div>
            <p className="mb-2 text-xs text-neutral-500">
              iCloud Drive のフォルダを指定すると、そのままバックアップになります。
              DB 本体（track.db）は同期フォルダに置かないでください — WAL と合わせて
              3ファイルあり、別々に同期されると壊れます。
            </p>
            <div className="flex gap-2">
              <Input
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
              <Button
                size="sm"
                onClick={() => updateConfig.mutate({ exportDir })}
                disabled={updateConfig.isPending || !exportDir.trim()}
              >
                保存
              </Button>
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
                <Label className="text-xs">間隔（時間・0で無効）</Label>
                <Input
                  type="number"
                  min={0}
                  max={720}
                  value={backupIntervalHours}
                  onChange={(e) => setBackupIntervalHours(Number(e.target.value))}
                  className="w-28"
                />
              </div>
              <div className="space-y-1">
                <Label className="text-xs">残す本数</Label>
                <Input
                  type="number"
                  min={1}
                  max={1000}
                  value={backupKeep}
                  onChange={(e) => setBackupKeep(Number(e.target.value))}
                  className="w-28"
                />
              </div>
              <Button
                size="sm"
                onClick={() => updateConfig.mutate({ backupIntervalHours, backupKeep })}
                disabled={updateConfig.isPending}
              >
                保存
              </Button>
            </div>
            <p className="mt-2 text-xs text-neutral-500">
              手動で書き出したファイルは本数制限の対象外です。
            </p>
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
                いま取る
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
                      戻す
                    </Button>
                  </div>
                ))}
              </div>
            )}

            {restoring && (
              <div className="mt-3 rounded-md border border-amber-300 bg-amber-50 p-3">
                <div className="mb-1 text-sm font-medium text-amber-900">
                  {restoring.name} に戻しますか？
                </div>
                <div className="mb-3 text-xs text-amber-800">
                  いまの DB は<strong>このスナップショットの内容で上書きされます</strong>。
                  差し替える直前に現在の DB のバックアップを自動で取るので、戻し間違えても復帰できます。
                </div>
                <div className="flex gap-2">
                  <Button
                    size="sm"
                    onClick={() => runRestore.mutate(restoring)}
                    disabled={runRestore.isPending}
                  >
                    戻す
                  </Button>
                  <Button variant="ghost" size="sm" onClick={() => setRestoring(null)}>
                    やめる
                  </Button>
                </div>
              </div>
            )}
          </div>

          <div className="flex items-center justify-between rounded-md border border-neutral-200 px-3 py-2">
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

          <div className="flex items-center justify-between rounded-md border border-neutral-200 px-3 py-2">
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
            <Button variant="outline" size="sm" onClick={() => fileRef.current?.click()}>
              ファイルを選ぶ
            </Button>
          </div>

          {pending && (
            <div className="rounded-md border border-amber-300 bg-amber-50 p-3">
              <div className="mb-1 text-sm font-medium text-amber-900">
                {pending.name} を読み込みました
              </div>
              <div className="mb-3 text-xs text-amber-800">
                クライアント {pending.counts.clients} / タグ {pending.counts.tags} / プロジェクト{" "}
                {pending.counts.projects} / エントリ {pending.counts.entries}
                <br />
                実行すると<strong>いま入っているデータは全て消えます</strong>。
                心配なら先にエクスポートしてください。
              </div>
              <div className="flex gap-2">
                <Button
                  size="sm"
                  onClick={() => runImport.mutate(pending.json)}
                  disabled={runImport.isPending}
                >
                  置き換える
                </Button>
                <Button variant="ghost" size="sm" onClick={() => setPending(null)}>
                  やめる
                </Button>
              </div>
            </div>
          )}
        </div>
      </section>

    </div>
  );
}
