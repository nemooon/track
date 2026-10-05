import {
  useState,
  useEffect,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { open } from "@tauri-apps/plugin-dialog";
import {
  CheckCircle2,
  ChevronRight,
  Clock3,
  FolderOpen,
  HardDrive,
  Plus,
  RotateCcw,
} from "lucide-react";
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
import { Select } from "@client/components/ui/select";
import { useAppUi } from "@client/components/AppUiContext";
import { apiFetch } from "@client/lib/fetcher";
import { ProjectsPage } from "@client/pages/ProjectsPage";
import type {
  AiProviderId,
  AiProviderStatus,
  CodexModelCatalog,
  UserSettings,
  AppConfig,
  Snapshot,
  ReportCopyField,
  ReportCopyFormat,
} from "@shared/types";
import {
  reportCopyFormatUsesAi,
} from "@client/lib/reportCopy";

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

function SettingsRow({
  title,
  description,
  htmlFor,
  children,
}: {
  title: string;
  description?: ReactNode;
  htmlFor?: string;
  children: ReactNode;
}) {
  const heading = htmlFor ? (
    <Label htmlFor={htmlFor} className="text-sm font-semibold text-neutral-900">
      {title}
    </Label>
  ) : (
    <div className="text-sm font-semibold text-neutral-900">{title}</div>
  );

  return (
    <div className="grid gap-3 border-t border-neutral-100 py-5 first:border-t-0 first:pt-0 md:grid-cols-[minmax(0,190px)_minmax(0,1fr)] md:gap-8">
      <div>
        {heading}
        {description && (
          <p className="mt-1 text-xs leading-5 text-neutral-500">
            {description}
          </p>
        )}
      </div>
      <div className="min-w-0">{children}</div>
    </div>
  );
}

function SettingsActions({
  status,
  children,
}: {
  status?: ReactNode;
  children: ReactNode;
}) {
  const [target, setTarget] = useState<HTMLElement | null>(null);

  useEffect(() => {
    setTarget(document.getElementById("settings-actions-root"));
  }, []);

  if (!target) return null;

  return createPortal(
    <div className="mx-auto w-full max-w-7xl px-5 py-3 sm:px-8 lg:px-10">
      <div className="flex min-h-8 items-center gap-4">
        <div className="min-w-0 flex-1 text-xs text-neutral-500">
          {status}
        </div>
        <div className="flex shrink-0 flex-wrap items-center justify-end gap-2">
          {children}
        </div>
      </div>
    </div>,
    target,
  );
}

function SettingsCard({
  title,
  description,
  children,
}: {
  title: string;
  description?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="overflow-hidden rounded-xl border border-neutral-200 bg-white shadow-sm shadow-neutral-950/[0.02]">
      <header className="border-b border-neutral-100 bg-neutral-50/60 px-5 py-4">
        <h3 className="text-sm font-semibold text-neutral-900">{title}</h3>
        {description && (
          <p className="mt-1 text-xs leading-5 text-neutral-500">
            {description}
          </p>
        )}
      </header>
      <div className="px-5 py-5">{children}</div>
    </section>
  );
}

export type SettingsCategory =
  | "calendar"
  | "notes"
  | "reports"
  | "projects"
  | "ai"
  | "backup";

export function SettingsPage({ category }: { category: SettingsCategory }) {
  const qc = useQueryClient();
  const navigate = useNavigate();
  const {
    confirmDiscardChanges,
    setSettingsDirty,
  } = useAppUi();
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
  const reportsDirty = weeklyReportDirty;

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

  function createLocalId(prefix: string) {
    return globalThis.crypto?.randomUUID?.() ?? `${prefix}-${Date.now()}-${Math.random()}`;
  }

  function createFieldColumn(field: ReportCopyField) {
    return {
      id: createLocalId("column"),
      kind: "field" as const,
      field,
      ...(field === "duration"
        ? { durationFormat: "hours-minutes" as const }
        : {}),
    };
  }

  function addCopyFormat() {
    if (!current || !confirmDiscardChanges()) return;
    const id = createLocalId("format");
    const fields = [
      "category", "duration", "percentage",
    ] as ReportCopyField[];
    const copyFormat: ReportCopyFormat = {
      id,
      name: "新しい出力形式",
      target: "ai-aggregation",
      delimiter: "tab",
      includeHeader: true,
      aiPrompt: "",
      columns: fields.map(createFieldColumn),
    };
    updateSettings.mutate(
      { reportCopyFormats: [...current.reportCopyFormats, copyFormat] },
      {
        onSuccess: () => {
          setSettingsDirty(false);
          navigate(`/report-formats/${encodeURIComponent(id)}`);
        },
      },
    );
  }

  const updateSettings = useMutation({
    mutationFn: (data: {
      workStart?: number;
      workEnd?: number;
      workDays?: number[];
      weeklyReportTemplate?: string;
      reportCopyFormats?: ReportCopyFormat[];
    }) =>
      apiFetch<UserSettings>("/api/settings", {
        method: "PATCH",
        body: JSON.stringify(data),
      }),
    onSuccess: (settings) => {
      qc.setQueryData(["settings"], settings);
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
  const [aiProvider, setAiProvider] =
    useState<AiProviderId>("apple-intelligence");
  const [aiCodexExecutable, setAiCodexExecutable] = useState("");
  const [aiCodexModel, setAiCodexModel] = useState("");
  const [aiCommandExecutable, setAiCommandExecutable] = useState("");
  const [aiCommandArgsText, setAiCommandArgsText] = useState("");
  useEffect(() => {
    if (config) {
      setExportDir(config.exportDir);
      setBackupIntervalHours(config.backupIntervalHours);
      setBackupKeep(config.backupKeep);
      setAiProvider(config.aiProvider);
      setAiCodexExecutable(config.aiCodexExecutable);
      setAiCodexModel(config.aiCodexModel);
      setAiCommandExecutable(config.aiCommandExecutable);
      setAiCommandArgsText(config.aiCommandArgs.join("\n"));
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
  const aiCommandArgs = aiCommandArgsText
    .split("\n")
    .map((arg) => arg.trim())
    .filter(Boolean);
  const aiConfigDirty =
    !!config &&
    (aiProvider !== config.aiProvider ||
      aiCodexExecutable !== config.aiCodexExecutable ||
      aiCodexModel !== config.aiCodexModel ||
      aiCommandExecutable !== config.aiCommandExecutable ||
      aiCommandArgs.join("\n") !== config.aiCommandArgs.join("\n"));
  const aiConfigError =
    aiProvider === "custom-command" && !aiCommandExecutable.trim()
      ? "実行するコマンドを指定してください"
      : aiCommandArgs.length > 32
        ? "引数は32個以内で指定してください"
        : null;

  const { data: aiStatus } = useQuery({
    queryKey: ["ai-status", config?.aiProvider],
    queryFn: () => apiFetch<AiProviderStatus>("/api/ai/status"),
    enabled: Boolean(config) && !aiConfigDirty,
  });

  const {
    data: codexModels,
    isFetching: codexModelsFetching,
    isError: codexModelsError,
    refetch: refreshCodexModels,
  } = useQuery({
    queryKey: ["codex-models", config?.aiCodexExecutable],
    queryFn: () => apiFetch<CodexModelCatalog>("/api/ai/codex-models"),
    enabled: Boolean(config) && category === "ai" && aiProvider === "codex",
    staleTime: 0,
    refetchInterval: 5 * 60_000,
    retry: false,
  });

  function resetBackupSettings() {
    if (!config) return;
    setExportDir(config.exportDir);
    setBackupIntervalHours(config.backupIntervalHours);
    setBackupKeep(config.backupKeep);
  }

  function resetAiSettings() {
    if (!config) return;
    setAiProvider(config.aiProvider);
    setAiCodexExecutable(config.aiCodexExecutable);
    setAiCodexModel(config.aiCodexModel);
    setAiCommandExecutable(config.aiCommandExecutable);
    setAiCommandArgsText(config.aiCommandArgs.join("\n"));
  }

  useEffect(() => {
    setSettingsDirty(
      category === "calendar"
        ? workDirty
        : category === "ai"
          ? aiConfigDirty
        : category === "reports"
          ? reportsDirty
          : category === "backup"
            ? configDirty
            : false,
    );
  }, [
    category,
    aiConfigDirty,
    configDirty,
    setSettingsDirty,
    weeklyReportDirty,
    reportsDirty,
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
      qc.invalidateQueries({ queryKey: ["ai-status"] });
      toast.success("設定を保存しました");
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
  const latestBackup = backups[0];
  const automaticBackupEnabled = (config?.backupIntervalHours ?? 0) > 0;
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

  function toggleDay(day: number) {
    setWorkDays((prev) =>
      prev.includes(day) ? prev.filter((d) => d !== day) : [...prev, day].sort(),
    );
  }

  if (
    ((category === "calendar" || category === "reports") &&
      isLoading) ||
    ((category === "ai" || category === "backup") && !config)
  ) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-6 w-6 animate-spin rounded-full border-2 border-neutral-300 border-t-neutral-700" />
      </div>
    );
  }

  return (
    <div>
      {category === "calendar" && (
        <section>
          <p className="mb-6 text-sm text-neutral-500">
            カレンダーの表示と勤務状況の判定を設定します。
          </p>
          <form
            id="calendar-settings-form"
            onSubmit={(e) => {
              e.preventDefault();
              updateSettings.mutate({ workStart, workEnd, workDays });
            }}
            className="space-y-4"
          >
            <SettingsCard
              title="勤務スケジュール"
              description="カレンダー表示と勤務状況の判定に使う標準スケジュールです。"
            >
              <SettingsRow
                title="勤務時間"
                description="1日の標準的な開始・終了時刻です。"
              >
              <div className="flex flex-wrap gap-4">
                <div className="space-y-1">
                  <Label htmlFor="work-start">勤務開始</Label>
                  <Input
                    id="work-start"
                    type="time"
                    step={1800}
                    value={minutesToTime(workStart)}
                    onChange={(e) =>
                      setWorkStart(timeToMinutes(e.target.value))
                    }
                  />
                </div>
                <div className="space-y-1">
                  <Label htmlFor="work-end">勤務終了</Label>
                  <Input
                    id="work-end"
                    type="time"
                    step={1800}
                    value={minutesToTime(workEnd)}
                    onChange={(e) =>
                      setWorkEnd(timeToMinutes(e.target.value))
                    }
                  />
                </div>
              </div>
              </SettingsRow>

              <SettingsRow
                title="勤務日"
                description="勤務状況の判定対象にする曜日です。"
              >
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
              {workError && (
                <p role="alert" className="mt-2 text-sm text-red-600">
                  {workError}
                </p>
              )}
              </SettingsRow>

              <SettingsActions
                status={!workDirty && !workError ? "保存済み" : undefined}
              >
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
                <Button
                  type="submit"
                  form="calendar-settings-form"
                  size="sm"
                  disabled={
                    !workDirty || !!workError || updateSettings.isPending
                  }
                >
                  {updateSettings.isPending ? "保存中…" : "変更を保存"}
                </Button>
              </SettingsActions>
            </SettingsCard>
          </form>
        </section>
      )}

      {category === "notes" && (
        <section>
          <p className="mb-6 text-sm text-neutral-500">
            メモビューの動作を設定します。
          </p>
          <div className="rounded-xl bg-neutral-50/60 px-5 py-10 text-center">
            <p className="text-sm font-medium text-neutral-700">
              現在、メモ固有の設定はありません
            </p>
            <p className="mt-1 text-xs text-neutral-500">
              今後の設定項目はこのページに追加されます。
            </p>
          </div>
        </section>
      )}

      {category === "ai" && (
        <section>
          <p className="mb-6 text-sm text-neutral-500">
            Track全体で使うAI機能を設定します。
          </p>
          <form
            id="ai-settings-form"
            onSubmit={(event) => {
              event.preventDefault();
              updateConfig.mutate({
                aiProvider,
                aiCodexExecutable: aiCodexExecutable.trim(),
                aiCodexModel: aiCodexModel.trim(),
                aiCommandExecutable: aiCommandExecutable.trim(),
                aiCommandArgs,
              });
            }}
            className="space-y-4"
          >
            <SettingsCard
              title="生成プロバイダー"
              description="メモタイトルと週報の生成に共通で使う実行環境です。"
            >
              <SettingsRow
                title="プロバイダー"
                description="利用するAI環境を選択します。"
                htmlFor="ai-provider"
              >
                <Select
                  id="ai-provider"
                  value={aiProvider}
                  onChange={(event) =>
                    setAiProvider(event.target.value as AiProviderId)
                  }
                >
                  <option value="apple-intelligence">
                    Apple Intelligence
                  </option>
                  <option value="codex">Codex</option>
                  <option value="custom-command">カスタムコマンド</option>
                </Select>
              </SettingsRow>
            </SettingsCard>

            {aiProvider === "apple-intelligence" && (
              <SettingsCard
                title="Apple Intelligence"
                description="Appleの端末内モデルを使うための設定です。"
              >
                <SettingsRow
                  title="利用方法"
                  description="Appleの端末内モデルを利用します。"
                >
                  <p className="rounded-md bg-neutral-50 px-3 py-2 text-sm leading-6 text-neutral-600">
                    macOS 26以降で利用できます。APIキーや外部通信は不要です。
                  </p>
                </SettingsRow>
              </SettingsCard>
            )}

            {aiProvider === "codex" && (
              <SettingsCard
                title="Codex"
                description="利用するモデルと実行環境を設定します。"
              >
                <SettingsRow
                  title="モデル"
                  description="空欄ではCodex側の既定モデルを使います。任意のモデルIDも入力できます。"
                  htmlFor="ai-codex-model"
                >
                  <Input
                    id="ai-codex-model"
                    list="ai-codex-model-suggestions"
                    value={aiCodexModel}
                    onChange={(event) => setAiCodexModel(event.target.value)}
                    placeholder="Codexの既定モデル"
                    maxLength={200}
                    spellCheck={false}
                  />
                  <datalist id="ai-codex-model-suggestions">
                    {codexModels?.models.map((model) => (
                      <option
                        key={model.id}
                        value={model.id}
                        label={model.label}
                      />
                    ))}
                  </datalist>
                  <div className="mt-2 flex items-center gap-3">
                    <p className="flex-1 text-xs leading-5 text-neutral-500" role="status">
                      {codexModelsError
                        ? "モデル一覧を更新できませんでした。任意のモデルIDを入力できます。"
                        : codexModels?.detail ?? "Codexのモデル一覧を取得しています…"}
                    </p>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      disabled={codexModelsFetching}
                      onClick={() => void refreshCodexModels()}
                    >
                      {codexModelsFetching ? "更新中…" : "一覧を更新"}
                    </Button>
                  </div>
                  {aiCodexExecutable !== config?.aiCodexExecutable && (
                    <p className="mt-2 text-xs text-neutral-500">
                      実行ファイルの変更を保存すると、そのCodexから候補を取得します。
                    </p>
                  )}
                </SettingsRow>
                <SettingsRow
                  title="codex実行ファイル"
                  description="空欄ではPATHとChatGPTアプリから新しいバージョンを自動検出します。"
                  htmlFor="ai-codex-executable"
                >
                  <Input
                    id="ai-codex-executable"
                    value={aiCodexExecutable}
                    onChange={(event) =>
                      setAiCodexExecutable(event.target.value)
                    }
                    placeholder="自動検出"
                    spellCheck={false}
                  />
                  <p className="text-xs leading-5 text-neutral-500">
                    Codex SDKが既存のログインを再利用し、TrackはAPIキーを保存しません。
                  </p>
                </SettingsRow>
              </SettingsCard>
            )}

            {aiProvider === "custom-command" && (
              <SettingsCard
                title="カスタムコマンド"
                description="任意のローカルコマンドをAI処理に利用します。"
              >
                <SettingsRow
                  title="実行ファイル"
                  description="AI処理に使用するコマンドの絶対パスです。"
                  htmlFor="ai-command-executable"
                >
                  <Input
                    id="ai-command-executable"
                    value={aiCommandExecutable}
                    onChange={(event) =>
                      setAiCommandExecutable(event.target.value)
                    }
                    placeholder="/path/to/command"
                    spellCheck={false}
                  />
                </SettingsRow>
                <SettingsRow
                  title="引数"
                  description="コマンドへ渡す引数を1行に1つ入力します。"
                  htmlFor="ai-command-args"
                >
                  <textarea
                    id="ai-command-args"
                    value={aiCommandArgsText}
                    onChange={(event) =>
                      setAiCommandArgsText(event.target.value)
                    }
                    rows={4}
                    spellCheck={false}
                    className="w-full resize-y rounded-md border border-neutral-300 bg-white px-3 py-2 font-mono text-sm leading-6 outline-none transition focus:border-neutral-500 focus:ring-2 focus:ring-neutral-200"
                  />
                  <p className="text-xs leading-5 text-neutral-500">
                    シェルは使わず、プロンプトを標準入力へ渡します。認証はコマンド側の既存設定を使います。
                    利用するサービスの規約とライセンスは、ご自身で確認してください。
                  </p>
                </SettingsRow>
              </SettingsCard>
            )}

            {(aiConfigError || (!aiConfigDirty && aiStatus)) && (
              <SettingsCard
                title="状態"
                description="現在の設定でAI機能を利用できるか確認します。"
              >
                {aiConfigError && (
                  <SettingsRow title="入力内容">
                    <p role="alert" className="text-sm text-red-600">
                      {aiConfigError}
                    </p>
                  </SettingsRow>
                )}
                {!aiConfigDirty && aiStatus && (
                  <SettingsRow title="接続状態">
                    <p
                      className={`rounded-md px-3 py-2 text-sm ${
                        aiStatus.available
                          ? "bg-emerald-50 text-emerald-800"
                          : "bg-amber-50 text-amber-800"
                      }`}
                    >
                      {aiStatus.detail}
                    </p>
                  </SettingsRow>
                )}
              </SettingsCard>
            )}

            <SettingsActions
              status={
                !aiConfigDirty && !aiConfigError ? "保存済み" : undefined
              }
            >
              {aiConfigDirty && (
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={resetAiSettings}
                  disabled={updateConfig.isPending}
                >
                  変更を破棄
                </Button>
              )}
              <Button
                type="submit"
                form="ai-settings-form"
                size="sm"
                disabled={
                  !aiConfigDirty || !!aiConfigError || updateConfig.isPending
                }
              >
                {updateConfig.isPending ? "保存中…" : "変更を保存"}
              </Button>
            </SettingsActions>
          </form>
        </section>
      )}

      {category === "reports" && (
        <section>
          <p className="mb-6 text-sm text-neutral-500">
            カスタム出力のフォーマットと、AI週報の出力形式を設定します。
          </p>
          <form
            id="reports-settings-form"
            onSubmit={(e) => {
              e.preventDefault();
              updateSettings.mutate({
                weeklyReportTemplate: weeklyReportTemplate.trim(),
              });
            }}
            className="space-y-4"
          >
            <SettingsCard
              title="出力フォーマット"
              description="フォーマットを選択すると、専用の編集画面を開きます。"
            >
              <div className="space-y-3">
                {(current?.reportCopyFormats ?? []).map((copyFormat) => (
                  <button
                    key={copyFormat.id}
                    type="button"
                    onClick={() => {
                      if (!confirmDiscardChanges()) return;
                      setSettingsDirty(false);
                      navigate(
                        `/report-formats/${encodeURIComponent(copyFormat.id)}`,
                      );
                    }}
                    className="flex w-full items-center gap-3 rounded-lg border border-neutral-200 bg-white px-4 py-3 text-left transition-colors hover:border-neutral-300 hover:bg-neutral-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-neutral-400"
                  >
                    <span className="min-w-0 flex-1 truncate text-sm font-medium text-neutral-900">
                      {copyFormat.name || "名前未設定"}
                    </span>
                    <span
                      className={`shrink-0 rounded-full px-2 py-0.5 text-[11px] font-medium ${
                        reportCopyFormatUsesAi(copyFormat)
                          ? "bg-violet-100 text-violet-700"
                          : "bg-neutral-100 text-neutral-600"
                      }`}
                    >
                      {reportCopyFormatUsesAi(copyFormat) ? "AI使用" : "AIなし"}
                    </span>
                    <span className="shrink-0 text-xs text-neutral-400">
                      {copyFormat.columns.length}列 ·{" "}
                      {copyFormat.delimiter === "comma" ? "CSV" : "TSV"}
                    </span>
                    <ChevronRight className="size-4 shrink-0 text-neutral-400" />
                  </button>
                ))}

                <div className="pt-2">
                  <Button
                    type="button"
                    variant="outline"
                    onClick={addCopyFormat}
                    disabled={
                      updateSettings.isPending ||
                      (current?.reportCopyFormats.length ?? 0) >= 20
                    }
                  >
                    <Plus className="size-4" />
                    出力フォーマットを追加
                  </Button>
                </div>
              </div>
            </SettingsCard>

            <SettingsCard
              title="週報テンプレート"
              description="AIが週の工数をまとめる際の出力構成を設定します。"
            >
              <SettingsRow
                title="出力テンプレート"
                description="Markdownの見出しや固定文を利用できます。"
                htmlFor="weekly-report-template"
              >
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
              {weeklyReportError && (
                <p role="alert" className="mt-2 text-sm text-red-600">
                  {weeklyReportError}
                </p>
              )}
              </SettingsRow>
            </SettingsCard>
            <SettingsActions
              status={
                !reportsDirty && !weeklyReportError ? "保存済み" : undefined
              }
            >
              {reportsDirty && (
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
              <Button
                type="submit"
                form="reports-settings-form"
                size="sm"
                disabled={
                  !reportsDirty ||
                  !!weeklyReportError ||
                  updateSettings.isPending
                }
              >
                {updateSettings.isPending ? "保存中…" : "変更を保存"}
              </Button>
            </SettingsActions>
          </form>
        </section>
      )}

      {category === "projects" && (
        <section id="projects">
          <p className="mb-6 text-sm text-neutral-500">
            クライアント、プロジェクト、タグを管理します。
          </p>
          <ProjectsPage embedded />
        </section>
      )}

      {category === "backup" && (
        <section>
          <p className="mb-6 text-sm text-neutral-500">
            自動保存の状態を確認し、必要な時点へ安全に戻せます。
          </p>

          <div className="space-y-4">
            <div
              className={`flex flex-col gap-4 rounded-xl border p-4 shadow-sm shadow-neutral-950/[0.02] sm:flex-row sm:items-center ${
                automaticBackupEnabled
                  ? "border-emerald-100 bg-emerald-50/70"
                  : "border-neutral-200 bg-neutral-50"
              }`}
            >
              <div className="flex min-w-0 flex-1 items-center gap-3">
                <div
                  className={`flex size-10 shrink-0 items-center justify-center rounded-full bg-white shadow-sm ring-1 ${
                    automaticBackupEnabled
                      ? "text-emerald-700 ring-emerald-200"
                      : "text-neutral-500 ring-neutral-200"
                  }`}
                >
                  {automaticBackupEnabled ? (
                    <CheckCircle2 className="size-5" />
                  ) : (
                    <Clock3 className="size-5" />
                  )}
                </div>
                <div className="min-w-0">
                  <p
                    className={`text-sm font-semibold ${
                      automaticBackupEnabled
                        ? "text-emerald-950"
                        : "text-neutral-800"
                    }`}
                  >
                    {automaticBackupEnabled
                      ? "自動バックアップは有効です"
                      : "手動バックアップのみです"}
                  </p>
                  <p
                    className={`mt-0.5 text-xs ${
                      automaticBackupEnabled
                        ? "text-emerald-800"
                        : "text-neutral-500"
                    }`}
                  >
                    {latestBackup
                      ? `最終バックアップ ${new Date(latestBackup.createdAt).toLocaleString("ja-JP")}`
                      : "最初のバックアップを作成してください"}
                  </p>
                </div>
              </div>
              <Button
                variant="outline"
                size="sm"
                className={
                  automaticBackupEnabled
                    ? "border-emerald-300 bg-white text-emerald-950 hover:bg-emerald-100"
                    : "bg-white"
                }
                onClick={() => runBackup.mutate()}
                disabled={runBackup.isPending || configDirty}
                title={
                  configDirty
                    ? "先に設定の変更を保存してください"
                    : undefined
                }
              >
                <HardDrive className="size-4" />
                {runBackup.isPending ? "作成中…" : "今すぐバックアップ"}
              </Button>
            </div>

            <SettingsCard
              title="保存の設定"
              description="iCloud Driveなど、普段同期されるフォルダも保存先にできます。"
            >
                <SettingsRow
                  title="保存先フォルダ"
                  description="Track本体ではなく、作成されるバックアップだけを同期してください。"
                  htmlFor="backup-directory"
                >
                  <div className="flex gap-2">
                    <Input
                      id="backup-directory"
                      value={exportDir}
                      onChange={(event) => setExportDir(event.target.value)}
                      spellCheck={false}
                      className="min-w-0 flex-1 font-mono text-xs"
                    />
                    {isTauri && (
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        onClick={pickExportDir}
                        disabled={updateConfig.isPending}
                      >
                        <FolderOpen className="size-4" />
                        選択
                      </Button>
                    )}
                  </div>
                  {suggestions.length > 0 && (
                    <div className="flex flex-wrap gap-2 pt-1">
                      {suggestions.map((suggestion) => (
                        <Button
                          key={suggestion.path}
                          variant="outline"
                          size="sm"
                          onClick={() => setExportDir(suggestion.path)}
                        >
                          {suggestion.label}を使う
                        </Button>
                      ))}
                      {config &&
                        exportDir !== config.defaults.exportDir && (
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() =>
                              setExportDir(config.defaults.exportDir)
                            }
                          >
                            既定に戻す
                          </Button>
                        )}
                    </div>
                  )}
                </SettingsRow>

                <SettingsRow
                  title="実行間隔"
                  description="0時間にすると自動バックアップを停止します。"
                  htmlFor="backup-interval"
                >
                  <div className="max-w-64">
                    <div className="relative">
                      <Clock3 className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-neutral-400" />
                      <Input
                        id="backup-interval"
                        type="number"
                        min={0}
                        max={720}
                        value={backupIntervalHours}
                        onChange={(event) =>
                          setBackupIntervalHours(Number(event.target.value))
                        }
                        className="pl-9 pr-14"
                      />
                      <span className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-xs text-neutral-500">
                        時間
                      </span>
                    </div>
                  </div>
                </SettingsRow>

                <SettingsRow
                  title="保存数"
                  description="手動で作成したバックアップは削除されません。"
                  htmlFor="backup-keep"
                >
                  <div className="max-w-64">
                    <div className="relative">
                      <Input
                        id="backup-keep"
                        type="number"
                        min={1}
                        max={1000}
                        value={backupKeep}
                        onChange={(event) =>
                          setBackupKeep(Number(event.target.value))
                        }
                        className="pr-12"
                      />
                      <span className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-xs text-neutral-500">
                        件
                      </span>
                    </div>
                  </div>
                </SettingsRow>

                {configError && (
                  <SettingsRow title="入力内容">
                    <p role="alert" className="text-sm text-red-600">
                      {configError}
                    </p>
                  </SettingsRow>
                )}

                <SettingsActions
                  status={!configDirty && !configError ? "保存済み" : undefined}
                >
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
                  <Button
                    size="sm"
                    onClick={() =>
                      updateConfig.mutate({
                        exportDir,
                        backupIntervalHours,
                        backupKeep,
                      })
                    }
                    disabled={
                      !configDirty || !!configError || updateConfig.isPending
                    }
                  >
                    {updateConfig.isPending ? "保存中…" : "変更を保存"}
                  </Button>
                </SettingsActions>
            </SettingsCard>

            <section className="overflow-hidden rounded-xl border border-neutral-200 bg-white shadow-sm shadow-neutral-950/[0.02]">
              <header className="border-b border-neutral-100 bg-neutral-50/60 px-5 py-4">
                <h3 className="text-sm font-semibold text-neutral-900">
                  復元ポイント
                </h3>
                <p className="mt-1 text-xs text-neutral-500">
                  過去の状態を選ぶと、現在の状態を退避してから復元します。
                </p>
              </header>

              {backups.length === 0 ? (
                <div className="flex flex-col items-center px-5 py-10 text-center">
                  <HardDrive className="mb-3 size-7 text-neutral-300" />
                  <p className="text-sm font-medium text-neutral-700">
                    バックアップはまだありません
                  </p>
                  <p className="mt-1 text-xs text-neutral-500">
                    「今すぐバックアップ」から最初の復元ポイントを作成できます。
                  </p>
                </div>
              ) : (
                <div className="divide-y divide-neutral-100">
                  {backups.map((backup, index) => (
                    <div
                      key={backup.path}
                      className="flex items-center gap-3 px-5 py-3"
                    >
                      <div className="flex size-8 shrink-0 items-center justify-center rounded-full bg-neutral-100 text-neutral-500">
                        <HardDrive className="size-4" />
                      </div>
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <p className="truncate text-sm font-medium text-neutral-800">
                            {new Date(backup.createdAt).toLocaleString("ja-JP")}
                          </p>
                          {index === 0 && (
                            <span className="rounded-full bg-emerald-50 px-2 py-0.5 text-[10px] font-medium text-emerald-700">
                              最新
                            </span>
                          )}
                        </div>
                        <p className="mt-0.5 truncate text-xs text-neutral-500">
                          {backup.auto ? "自動バックアップ" : "手動バックアップ"}
                          {" ・ "}
                          {(backup.bytes / 1024).toFixed(0)} KB
                          {" ・ "}
                          {backup.name}
                        </p>
                      </div>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => setRestoring(backup)}
                      >
                        <RotateCcw className="size-3.5" />
                        復元
                      </Button>
                    </div>
                  ))}
                </div>
              )}
            </section>

            <Dialog
              open={!!restoring}
              onOpenChange={(open) => {
                if (!open && !runRestore.isPending) setRestoring(null);
              }}
            >
              {restoring && (
                <>
                  <DialogHeader>
                    <DialogTitle>この時点へ復元しますか？</DialogTitle>
                  </DialogHeader>
                  <div className="space-y-3 text-sm text-neutral-600">
                    <div className="rounded-lg bg-neutral-50 p-3">
                      <p className="font-medium text-neutral-900">
                        {new Date(restoring.createdAt).toLocaleString("ja-JP")}
                      </p>
                      <p className="mt-1 truncate font-mono text-xs text-neutral-500">
                        {restoring.name}
                      </p>
                    </div>
                    <p className="leading-6">
                      現在のデータをこの時点の内容で置き換えます。実行直前の状態も自動でバックアップされます。
                    </p>
                  </div>
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
                      data-dialog-autofocus
                      onClick={() => runRestore.mutate(restoring)}
                      disabled={runRestore.isPending}
                    >
                      {runRestore.isPending ? "復元中…" : "この時点へ復元"}
                    </Button>
                  </DialogFooter>
                </>
              )}
            </Dialog>
          </div>
        </section>
      )}

    </div>
  );
}
