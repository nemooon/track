import { useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useParams } from "react-router";
import {
  AlignLeft,
  ArrowDown,
  ArrowLeft,
  ArrowUp,
  Building2,
  CalendarDays,
  ChevronDown,
  Clock3,
  Folder,
  Hash,
  ListTree,
  Percent,
  Play,
  Plus,
  Search,
  Square,
  SquareDashed,
  Sparkles,
  StickyNote,
  Tags,
  Timer,
  Trash2,
  Type,
  type LucideIcon,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@client/components/ui/button";
import { Input } from "@client/components/ui/input";
import { Label } from "@client/components/ui/label";
import { Select } from "@client/components/ui/select";
import { useAppUi } from "@client/components/AppUiContext";
import { apiFetch } from "@client/lib/fetcher";
import {
  AI_REPORT_COPY_FIELDS,
  REPORT_COPY_FIELD_LABELS,
  reorderReportCopyColumns,
  reportCopyColumnHeader,
  reportCopyColumnLabel,
  reportCopyFormatUsesAi,
} from "@client/lib/reportCopy";
import type {
  ReportCopyColumn,
  ReportCopyField,
  ReportCopyFormat,
  ReportDurationFormat,
  UserSettings,
} from "@shared/types";

const REPORT_COPY_FIELD_ICONS: Record<ReportCopyField, LucideIcon> = {
  date: CalendarDays,
  start: Play,
  end: Square,
  client: Building2,
  project: Folder,
  title: Type,
  note: StickyNote,
  tags: Tags,
  duration: Clock3,
  durationMinutes: Timer,
  percentage: Percent,
  category: ListTree,
  summary: AlignLeft,
  entryCount: Hash,
};

function createLocalId(prefix: string) {
  return globalThis.crypto?.randomUUID?.() ??
    `${prefix}-${Date.now()}-${Math.random()}`;
}

function copyFormatError(copyFormat: ReportCopyFormat): string | null {
  if (!copyFormat.name.trim()) return "出力フォーマットの名前を入力してください";
  if (copyFormat.columns.length === 0) return "項目を1つ以上追加してください";
  if (
    copyFormat.columns.some(
      (column) =>
        column.kind === "ai" &&
        (!column.label.trim() || !column.prompt.trim()),
    )
  ) {
    return "AI生成項目の見出しと生成指示を入力してください";
  }
  return null;
}

function normalizeCopyFormat(copyFormat: ReportCopyFormat): ReportCopyFormat {
  return {
    ...copyFormat,
    name: copyFormat.name.trim(),
    aiPrompt: copyFormat.aiPrompt.trim(),
    columns: copyFormat.columns.map((column) => {
      if (column.kind === "ai") {
        return {
          ...column,
          label: column.label.trim(),
          prompt: column.prompt.trim(),
        };
      }
      return {
        ...column,
        ...(column.kind === "field" && column.field === "duration"
          ? { durationFormat: column.durationFormat ?? "hours-minutes" }
          : {}),
        ...(column.label === undefined
          ? {}
          : { label: column.label.trim() }),
      };
    }),
  };
}

function EditorCard({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <section className="rounded-xl border border-neutral-200 bg-white shadow-sm shadow-neutral-950/[0.02]">
      <header className="border-b border-neutral-100 px-5 py-4">
        <h2 className="text-sm font-semibold text-neutral-900">{title}</h2>
        {description && (
          <p className="mt-1 text-xs leading-5 text-neutral-500">
            {description}
          </p>
        )}
      </header>
      <div className="p-5">{children}</div>
    </section>
  );
}

function ReportColumnPicker({
  fields,
  disabled,
  triggerStyle = "floating",
  triggerLabel,
  onAddField,
  onAddBlank,
  onAddAi,
}: {
  fields: ReportCopyField[];
  disabled: boolean;
  triggerStyle?: "floating" | "button" | "type";
  triggerLabel?: string;
  onAddField: (field: ReportCopyField) => void;
  onAddBlank: () => void;
  onAddAi: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [anchor, setAnchor] = useState<{
    left: number;
    top: number;
    width: number;
  } | null>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const closeOnOutsideClick = (event: MouseEvent) => {
      const target = event.target as Node;
      if (
        popoverRef.current?.contains(target) ||
        triggerRef.current?.contains(target)
      ) {
        return;
      }
      setOpen(false);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", closeOnOutsideClick);
    document.addEventListener("keydown", closeOnEscape);
    window.addEventListener("resize", updateAnchorPosition);
    window.addEventListener("scroll", updateAnchorPosition, true);
    return () => {
      document.removeEventListener("mousedown", closeOnOutsideClick);
      document.removeEventListener("keydown", closeOnEscape);
      window.removeEventListener("resize", updateAnchorPosition);
      window.removeEventListener("scroll", updateAnchorPosition, true);
    };
  }, [open]);

  useEffect(() => {
    if (disabled) setOpen(false);
  }, [disabled]);

  function updateAnchorPosition() {
    const rect = triggerRef.current?.getBoundingClientRect();
    if (!rect) return;
    const width = Math.min(520, window.innerWidth - 16);
    const estimatedHeight = 350;
    const below = rect.bottom + 6;
    const top =
      below + estimatedHeight <= window.innerHeight - 8
        ? below
        : Math.max(8, rect.top - estimatedHeight - 6);
    const left = Math.max(
      8,
      Math.min(rect.left, window.innerWidth - width - 8),
    );
    setAnchor((current) =>
      current &&
      current.left === left &&
      current.top === top &&
      current.width === width
        ? current
        : { left, top, width },
    );
  }

  function toggle() {
    if (open) {
      setOpen(false);
      return;
    }
    updateAnchorPosition();
    setQuery("");
    setOpen(true);
  }

  function selectItem(select: () => void) {
    select();
    if (triggerStyle === "type") {
      setOpen(false);
    } else {
      requestAnimationFrame(updateAnchorPosition);
    }
  }

  const normalizedQuery = query.trim().toLocaleLowerCase("ja");
  const matches = (label: string, keywords = "") =>
    !normalizedQuery ||
    `${label} ${keywords}`.toLocaleLowerCase("ja").includes(normalizedQuery);
  const filteredFields = fields.filter((field) =>
    matches(REPORT_COPY_FIELD_LABELS[field]),
  );
  const showBlank = matches("空白", "スペース blank");
  const showAi = matches("AI生成", "生成 カスタム");
  const hasResults = filteredFields.length > 0 || showBlank || showAi;

  return (
    <>
      <Button
        ref={triggerRef}
        type="button"
        variant="outline"
        size={triggerStyle === "floating" ? "icon" : "default"}
        onClick={toggle}
        disabled={disabled}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-label={
          triggerStyle === "floating"
            ? "この位置に項目を追加"
            : triggerStyle === "type"
              ? `${triggerLabel ?? "項目"}のタイプを変更`
              : "項目を追加"
        }
        title={
          triggerStyle === "floating"
            ? "この位置に項目を追加"
            : undefined
        }
        className={
          triggerStyle === "floating"
            ? `size-6 rounded-full bg-white shadow-sm transition-opacity ${
                open
                  ? "opacity-100"
                  : "opacity-0 group-hover/column:opacity-100 group-focus-within/column:opacity-100"
              }`
            : triggerStyle === "type"
              ? "h-8 w-auto max-w-32 gap-1 px-2.5 text-sm"
              : undefined
        }
      >
        {triggerStyle === "type" ? (
          <>
            <span className="truncate">{triggerLabel}</span>
            <ChevronDown className="size-3.5 shrink-0 text-neutral-400" />
          </>
        ) : (
          <>
            <Plus
              className={
                triggerStyle === "floating" ? "size-3.5" : "size-4"
              }
            />
            {triggerStyle === "button" && "項目を追加"}
          </>
        )}
      </Button>

      {open &&
        anchor &&
        createPortal(
          <div
            ref={popoverRef}
            role="dialog"
            aria-label={
              triggerStyle === "type" ? "項目タイプを変更" : "出力項目を追加"
            }
            className="fixed z-[80] overflow-hidden rounded-lg border border-neutral-200 bg-white shadow-xl"
            style={anchor}
          >
            <div className="flex items-center gap-2 border-b border-neutral-200 px-3 py-2">
              <Search className="size-4 shrink-0 text-neutral-400" />
              <input
                autoFocus
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="項目を検索"
                className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-neutral-400"
              />
            </div>
            <div className="subtle-scrollbar max-h-[330px] overflow-y-auto p-2">
              {!hasResults && (
                <div className="px-3 py-8 text-center text-xs text-neutral-400">
                  該当する項目がありません
                </div>
              )}

              {hasResults && (
                <div className="grid grid-cols-3 gap-1">
                  {filteredFields.map((field) => {
                    const Icon = REPORT_COPY_FIELD_ICONS[field];
                    return (
                      <button
                        key={field}
                        type="button"
                        onClick={() =>
                          selectItem(() => onAddField(field))
                        }
                        className="group flex min-h-[72px] min-w-0 flex-col items-center justify-center gap-1.5 rounded-md border border-neutral-200 bg-neutral-50/70 px-2.5 py-2 text-center text-sm font-medium text-neutral-700 shadow-sm shadow-neutral-950/[0.02] transition hover:border-neutral-300 hover:bg-white hover:text-neutral-950"
                      >
                        <span className="flex size-7 shrink-0 items-center justify-center rounded-md bg-white text-neutral-500 ring-1 ring-neutral-200 transition group-hover:text-neutral-800">
                          <Icon className="size-4" />
                        </span>
                        <span className="truncate">
                          {REPORT_COPY_FIELD_LABELS[field]}
                        </span>
                      </button>
                    );
                  })}

                  {showBlank && (
                    <button
                      type="button"
                      onClick={() => selectItem(onAddBlank)}
                      className="group flex min-h-[72px] min-w-0 flex-col items-center justify-center gap-1.5 rounded-md border border-neutral-200 bg-neutral-50/70 px-2.5 py-2 text-center text-sm font-medium text-neutral-700 shadow-sm shadow-neutral-950/[0.02] transition hover:border-neutral-300 hover:bg-white hover:text-neutral-950"
                    >
                      <span className="flex size-7 shrink-0 items-center justify-center rounded-md bg-white text-neutral-500 ring-1 ring-neutral-200 transition group-hover:text-neutral-800">
                        <SquareDashed className="size-4" />
                      </span>
                      <span className="truncate">空白</span>
                    </button>
                  )}

                  {showAi && (
                    <button
                      type="button"
                      onClick={() => selectItem(onAddAi)}
                      className="group flex min-h-[72px] min-w-0 flex-col items-center justify-center gap-1.5 rounded-md border border-neutral-200 bg-neutral-50/70 px-2.5 py-2 text-center text-sm font-medium text-neutral-700 shadow-sm shadow-neutral-950/[0.02] transition hover:border-neutral-300 hover:bg-white hover:text-neutral-950"
                    >
                      <span className="flex size-7 shrink-0 items-center justify-center rounded-md bg-white text-neutral-500 ring-1 ring-neutral-200 transition group-hover:text-neutral-800">
                        <Sparkles className="size-4" />
                      </span>
                      <span className="truncate">AI生成</span>
                    </button>
                  )}
                </div>
              )}
            </div>
            <div className="border-t border-neutral-100 px-3 py-2 text-[10px] text-neutral-400">
              {triggerStyle === "type"
                ? "選択した項目タイプへ変更します"
                : "続けて複数の項目を追加できます"}
            </div>
          </div>,
          document.body,
        )}
    </>
  );
}

export function ReportCopyFormatPage() {
  const { formatId = "" } = useParams();
  const isTauri = "__TAURI_INTERNALS__" in window;
  const navigate = useNavigate();
  const qc = useQueryClient();
  const {
    confirmDiscardChanges,
    setSettingsDirty,
  } = useAppUi();
  const { data: settings, isLoading } = useQuery({
    queryKey: ["settings"],
    queryFn: () => apiFetch<UserSettings>("/api/settings"),
  });
  const decodedFormatId = decodeURIComponent(formatId);
  const savedFormat = settings?.reportCopyFormats.find(
    (copyFormat) => copyFormat.id === decodedFormatId,
  );
  const [copyFormat, setCopyFormat] = useState<ReportCopyFormat | null>(null);

  useEffect(() => {
    if (!savedFormat) return;
    setCopyFormat((current) =>
      current?.id === savedFormat.id ? current : savedFormat,
    );
  }, [savedFormat]);

  const dirty = Boolean(
    copyFormat &&
      savedFormat &&
      JSON.stringify(copyFormat) !== JSON.stringify(savedFormat),
  );
  const validationError = copyFormat ? copyFormatError(copyFormat) : null;

  useEffect(() => {
    setSettingsDirty(dirty);
  }, [dirty, setSettingsDirty]);

  useEffect(
    () => () => {
      setSettingsDirty(false);
    },
    [setSettingsDirty],
  );

  const updateSettings = useMutation({
    mutationFn: ({ formats }: { formats: ReportCopyFormat[]; action: "save" | "delete" }) =>
      apiFetch<UserSettings>("/api/settings", {
        method: "PATCH",
        body: JSON.stringify({ reportCopyFormats: formats }),
      }),
    onSuccess: (nextSettings, variables) => {
      qc.setQueryData(["settings"], nextSettings);
      setSettingsDirty(false);
      if (variables.action === "delete") {
        toast.success("出力フォーマットを削除しました");
        navigate("/settings/reports");
        return;
      }
      const saved = nextSettings.reportCopyFormats.find(
        (item) => item.id === decodedFormatId,
      );
      if (saved) setCopyFormat(saved);
      toast.success("出力フォーマットを保存しました");
    },
    onError: () => toast.error("出力フォーマットを更新できませんでした"),
  });

  function updateFormat(
    update: (current: ReportCopyFormat) => ReportCopyFormat,
  ) {
    setCopyFormat((current) => (current ? update(current) : current));
  }

  function goBack() {
    if (!confirmDiscardChanges()) return;
    setSettingsDirty(false);
    navigate("/settings/reports");
  }

  function saveFormat() {
    if (!settings || !copyFormat || validationError) return;
    const normalized = normalizeCopyFormat(copyFormat);
    updateSettings.mutate({
      action: "save",
      formats: settings.reportCopyFormats.map((item) =>
        item.id === normalized.id ? normalized : item,
      ),
    });
  }

  function deleteFormat() {
    if (!settings || !copyFormat || settings.reportCopyFormats.length <= 1) return;
    if (!window.confirm(`「${copyFormat.name || "名前未設定"}」を削除しますか？`)) {
      return;
    }
    updateSettings.mutate({
      action: "delete",
      formats: settings.reportCopyFormats.filter(
        (item) => item.id !== copyFormat.id,
      ),
    });
  }

  function insertColumn(
    column: ReportCopyColumn,
    beforeColumnId: string | null,
  ) {
    updateFormat((current) => {
      const foundIndex = beforeColumnId
        ? current.columns.findIndex((item) => item.id === beforeColumnId)
        : current.columns.length;
      const insertIndex = foundIndex < 0 ? current.columns.length : foundIndex;
      return {
        ...current,
        columns: [
          ...current.columns.slice(0, insertIndex),
          column,
          ...current.columns.slice(insertIndex),
        ],
      };
    });
  }

  function addFieldColumn(
    field: ReportCopyField,
    beforeColumnId: string | null,
  ) {
    const column: ReportCopyColumn = {
      id: createLocalId("column"),
      kind: "field",
      field,
      ...(field === "duration"
        ? { durationFormat: "hours-minutes" as const }
        : {}),
    };
    insertColumn(column, beforeColumnId);
  }

  function addBlankColumn(beforeColumnId: string | null) {
    const column: ReportCopyColumn = {
      id: createLocalId("blank-column"),
      kind: "blank",
    };
    insertColumn(column, beforeColumnId);
  }

  function addAiColumn(beforeColumnId: string | null) {
    const column: ReportCopyColumn = {
      id: createLocalId("ai-column"),
      kind: "ai",
      label: "AI生成項目",
      prompt: "この集計の内容を簡潔にまとめる",
    };
    insertColumn(column, beforeColumnId);
  }

  function changeColumnType(columnId: string, value: string) {
    updateFormat((current) => ({
      ...current,
      columns: current.columns.map((column) => {
        if (column.id !== columnId) return column;
        const customLabel = column.label;
        if (value === "blank") {
          if (column.kind === "blank") return column;
          return {
            id: column.id,
            kind: "blank" as const,
            ...(customLabel === undefined ? {} : { label: customLabel }),
          };
        }
        if (value === "ai") {
          if (column.kind === "ai") return column;
          return {
            id: column.id,
            kind: "ai" as const,
            label: reportCopyColumnHeader(column) || "AI生成項目",
            prompt: "この集計の内容を簡潔にまとめる",
          };
        }
        if (value.startsWith("field:")) {
          const field = value.slice("field:".length) as ReportCopyField;
          if (column.kind === "field" && column.field === field) return column;
          return {
            id: column.id,
            kind: "field" as const,
            field,
            ...(customLabel === undefined ? {} : { label: customLabel }),
            ...(field === "duration"
              ? {
                  durationFormat:
                    column.kind === "field" && column.field === "duration"
                      ? column.durationFormat ?? "hours-minutes"
                      : "hours-minutes",
                }
              : {}),
          };
        }
        return column;
      }),
    }));
  }

  function moveColumn(columnId: string, targetId: string) {
    updateFormat((current) => ({
      ...current,
      columns: reorderReportCopyColumns(
        current.columns,
        columnId,
        targetId,
      ),
    }));
  }

  if (isLoading || (savedFormat && !copyFormat)) {
    return (
      <div className="flex min-h-full items-center justify-center text-sm text-neutral-500">
        読み込み中…
      </div>
    );
  }

  if (!settings || !savedFormat || !copyFormat) {
    return (
      <div className="mx-auto max-w-3xl px-6 py-10">
        <Button type="button" variant="ghost" onClick={goBack}>
          <ArrowLeft className="size-4" />
          レポート設定へ戻る
        </Button>
        <div className="mt-6 rounded-xl border border-neutral-200 bg-white p-8 text-center text-sm text-neutral-500">
          出力フォーマットが見つかりませんでした。
        </div>
      </div>
    );
  }

  const availableFields = AI_REPORT_COPY_FIELDS;
  const renderColumnInsertion = (beforeColumnId: string | null) => (
    <div className="relative h-3">
      <div className="absolute right-0 top-1/2 z-20 translate-x-1/2 -translate-y-1/2">
        <ReportColumnPicker
          fields={availableFields}
          disabled={copyFormat.columns.length >= 30}
          onAddField={(field) => addFieldColumn(field, beforeColumnId)}
          onAddBlank={() => addBlankColumn(beforeColumnId)}
          onAddAi={() => addAiColumn(beforeColumnId)}
        />
      </div>
    </div>
  );

  return (
    <section className="flex h-svh min-h-0 flex-col overflow-hidden bg-neutral-50/60">
      <header
        data-tauri-drag-region={isTauri ? "" : undefined}
        className={`flex h-11 shrink-0 items-center gap-3 border-b border-neutral-200 bg-white pr-4 ${
          isTauri ? "pl-[92px]" : "pl-4"
        }`}
      >
        <button
          type="button"
          onClick={goBack}
          aria-label="レポート設定へ戻る"
          title="レポート設定へ戻る"
          className="flex size-8 shrink-0 items-center justify-center rounded-md text-neutral-500 transition-colors hover:bg-neutral-100 hover:text-neutral-900 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-neutral-400"
        >
          <ArrowLeft className="size-4" />
        </button>
        <h1 className="pointer-events-none min-w-0 flex-1 truncate text-sm font-semibold text-neutral-950">
          出力フォーマット
        </h1>
      </header>
      <form
        className="flex min-h-0 flex-1 flex-col overflow-hidden"
        onSubmit={(event) => {
          event.preventDefault();
          saveFormat();
        }}
      >
        <div className="subtle-scrollbar min-h-0 flex-1 overflow-y-auto">
        <div className="w-full px-5 py-6 sm:px-8 sm:py-8">
          <header className="mb-6 flex flex-wrap items-start justify-between gap-4">
            <div>
              <h1 className="text-2xl font-semibold tracking-tight text-neutral-950">
                {copyFormat.name || "名前未設定"}
              </h1>
              <p className="mt-1 text-sm text-neutral-500">
                出力フォーマットの項目と生成方法を編集します。
              </p>
            </div>
            <span
              className={`rounded-full px-2.5 py-1 text-xs font-medium ${
                reportCopyFormatUsesAi(copyFormat)
                  ? "bg-violet-100 text-violet-700"
                  : "bg-white text-neutral-600 ring-1 ring-neutral-200"
              }`}
            >
              {reportCopyFormatUsesAi(copyFormat) ? "AI使用" : "AIなし"}
            </span>
          </header>

          <div className="grid items-start gap-5 xl:grid-cols-[minmax(300px,380px)_minmax(0,1fr)]">
            <div className="space-y-5">
              <EditorCard title="基本設定">
                <div className="space-y-4">
                  <div className="space-y-1.5">
                    <Label htmlFor="copy-format-name">名前</Label>
                    <Input
                      id="copy-format-name"
                      value={copyFormat.name}
                      maxLength={100}
                      onChange={(event) =>
                        updateFormat((current) => ({
                          ...current,
                          name: event.target.value,
                        }))
                      }
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label htmlFor="copy-format-delimiter">区切り文字</Label>
                    <Select
                      id="copy-format-delimiter"
                      value={copyFormat.delimiter}
                      onChange={(event) =>
                        updateFormat((current) => ({
                          ...current,
                          delimiter: event.target.value as "tab" | "comma",
                        }))
                      }
                    >
                      <option value="tab">タブ（TSV）</option>
                      <option value="comma">カンマ（CSV）</option>
                    </Select>
                  </div>
                  <label className="inline-flex items-center gap-2 text-sm text-neutral-700">
                    <input
                      type="checkbox"
                      checked={copyFormat.includeHeader}
                      onChange={(event) =>
                        updateFormat((current) => ({
                          ...current,
                          includeHeader: event.target.checked,
                        }))
                      }
                      className="size-4 rounded border-neutral-300"
                    />
                    先頭行に項目名を含める
                  </label>
                </div>
              </EditorCard>

              <EditorCard
                title="AIへの指示"
                description="集計全体への指示です。空欄でもAI生成項目や概要項目があればAIを使用します。"
              >
                <textarea
                  value={copyFormat.aiPrompt}
                  onChange={(event) =>
                    updateFormat((current) => ({
                      ...current,
                      aiPrompt: event.target.value,
                    }))
                  }
                  rows={8}
                  maxLength={10_000}
                  placeholder="例: 作業の目的と内容が同じものをまとめる"
                  className="w-full resize-y rounded-md border border-neutral-300 bg-white px-3 py-2 text-sm leading-6 outline-none transition focus:border-neutral-500 focus:ring-2 focus:ring-neutral-200"
                />
              </EditorCard>
            </div>

            <EditorCard
              title="出力項目"
              description="追加後も項目タイプと見出しを変更できます。上下の矢印で出力順を変更します。"
            >
              <div>
                {copyFormat.columns.map((column, columnIndex) => {
                  return (
                  <div key={column.id} className="group/column">
                    {columnIndex === 0 && renderColumnInsertion(column.id)}
                    <div className="rounded-lg border border-neutral-200 bg-neutral-50/70 px-3 py-1.5">
                    <div className="flex min-h-8 items-center gap-2">
                      <span className="flex w-6 items-center justify-center text-center text-xs tabular-nums text-neutral-400">
                        {columnIndex + 1}
                      </span>
                      <ReportColumnPicker
                        fields={availableFields}
                        disabled={false}
                        triggerStyle="type"
                        triggerLabel={reportCopyColumnLabel(column)}
                        onAddField={(field) =>
                          changeColumnType(column.id, `field:${field}`)
                        }
                        onAddBlank={() => changeColumnType(column.id, "blank")}
                        onAddAi={() => changeColumnType(column.id, "ai")}
                      />
                      <Input
                        aria-label={`${reportCopyColumnLabel(column)}項目${columnIndex + 1}の見出し`}
                        value={reportCopyColumnHeader(column)}
                        maxLength={100}
                        placeholder="見出しなし"
                        className={`h-8 min-w-0 ${
                          column.kind === "ai" ? "w-40 shrink-0" : "flex-1"
                        }`}
                        onChange={(event) =>
                          updateFormat((current) => ({
                            ...current,
                            columns: current.columns.map((item) =>
                              item.id === column.id
                                ? { ...item, label: event.target.value }
                                : item,
                            ),
                          }))
                        }
                      />
                      {column.kind === "field" &&
                        column.field === "duration" && (
                          <Select
                            aria-label={`時間項目${columnIndex + 1}の表示形式`}
                            value={column.durationFormat ?? "hours-minutes"}
                            className="h-8 w-40 shrink-0"
                            onChange={(event) => {
                              const durationFormat = event.target
                                .value as ReportDurationFormat;
                              updateFormat((current) => ({
                                ...current,
                                columns: current.columns.map((item) =>
                                  item.id === column.id &&
                                  item.kind === "field" &&
                                  item.field === "duration"
                                    ? {
                                        ...item,
                                        durationFormat,
                                      }
                                    : item,
                                ),
                              }));
                            }}
                          >
                            <option value="hours-minutes">1h 30m</option>
                            <option value="japanese">1時間30分</option>
                            <option value="decimal-with-unit">1.5時間</option>
                            <option value="decimal">1.5（単位なし）</option>
                          </Select>
                        )}
                      {column.kind === "ai" && (
                          <Input
                            aria-label={`AI生成項目${columnIndex + 1}の生成指示`}
                            value={column.prompt}
                            maxLength={1_000}
                            placeholder="生成指示"
                            className="h-8 min-w-0 flex-1"
                            onChange={(event) =>
                              updateFormat((current) => ({
                                ...current,
                                columns: current.columns.map((item) =>
                                  item.id === column.id && item.kind === "ai"
                                    ? { ...item, prompt: event.target.value }
                                    : item,
                                ),
                              }))
                            }
                          />
                      )}
                      <div className="flex shrink-0 items-center">
                        <Button
                          type="button"
                          variant="ghost"
                          size="icon"
                          className="size-7"
                          aria-label={`${reportCopyColumnLabel(column)}を上へ移動`}
                          title="上へ移動"
                          disabled={columnIndex === 0}
                          onClick={() => {
                            const target = copyFormat.columns[columnIndex - 1];
                            if (target) moveColumn(column.id, target.id);
                          }}
                        >
                          <ArrowUp className="size-4" />
                        </Button>
                        <Button
                          type="button"
                          variant="ghost"
                          size="icon"
                          className="size-7"
                          aria-label={`${reportCopyColumnLabel(column)}を下へ移動`}
                          title="下へ移動"
                          disabled={columnIndex === copyFormat.columns.length - 1}
                          onClick={() => {
                            const target = copyFormat.columns[columnIndex + 1];
                            if (target) moveColumn(column.id, target.id);
                          }}
                        >
                          <ArrowDown className="size-4" />
                        </Button>
                      </div>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        className="size-7"
                        aria-label={`${reportCopyColumnLabel(column)}を外す`}
                        title="項目を外す"
                        disabled={copyFormat.columns.length === 1}
                        onClick={() =>
                          updateFormat((current) => ({
                            ...current,
                            columns: current.columns.filter(
                              (item) => item.id !== column.id,
                            ),
                          }))
                        }
                      >
                        <Trash2 className="size-4" />
                      </Button>
                    </div>
                    </div>
                    {columnIndex < copyFormat.columns.length - 1 &&
                      renderColumnInsertion(
                        copyFormat.columns[columnIndex + 1]?.id ?? null,
                      )}
                  </div>
                  );
                })}

                <div className="pt-3">
                  <ReportColumnPicker
                    fields={availableFields}
                    disabled={copyFormat.columns.length >= 30}
                    triggerStyle="button"
                    onAddField={(field) => addFieldColumn(field, null)}
                    onAddBlank={() => addBlankColumn(null)}
                    onAddAi={() => addAiColumn(null)}
                  />
                </div>
              </div>
            </EditorCard>
          </div>
        </div>
        </div>

        <footer className="z-10 shrink-0 border-t border-neutral-200 bg-white/95 px-5 py-3 shadow-[0_-8px_24px_rgba(0,0,0,0.04)] backdrop-blur sm:px-8">
          <div className="flex w-full flex-wrap items-center gap-3">
            <Button
              type="button"
              variant="ghost"
              className="text-red-600 hover:bg-red-50 hover:text-red-700"
              onClick={deleteFormat}
              disabled={
                settings.reportCopyFormats.length <= 1 ||
                updateSettings.isPending
              }
              title={
                settings.reportCopyFormats.length <= 1
                  ? "最後の出力フォーマットは削除できません"
                  : undefined
              }
            >
              <Trash2 className="size-4" />
              このフォーマットを削除
            </Button>
            <div className="min-w-0 flex-1 text-sm text-red-600">
              {validationError}
            </div>
            {dirty && (
              <Button
                type="button"
                variant="ghost"
                onClick={() => setCopyFormat(savedFormat)}
                disabled={updateSettings.isPending}
              >
                変更を破棄
              </Button>
            )}
            <Button
              type="submit"
              disabled={
                !dirty || Boolean(validationError) || updateSettings.isPending
              }
            >
              {updateSettings.isPending ? "保存中…" : "変更を保存"}
            </Button>
          </div>
        </footer>
      </form>
    </section>
  );
}
