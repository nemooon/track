import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Archive,
  ArchiveRestore,
  Check,
  ChevronDown,
  ChevronLeft,
  Copy,
  Ellipsis,
  FileCode2,
  FileText,
  LoaderCircle,
  Pin,
  PinOff,
  Search,
  Sparkles,
  SquarePen,
  Trash2,
  X,
} from "lucide-react";
import { toast } from "sonner";
import {
  Dialog,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@client/components/ui/dialog";
import { Tooltip } from "@client/components/ui/tooltip";
import { MarkdownEditor } from "@client/components/MarkdownEditor";
import { noteToBacklog } from "@client/lib/backlog";
import { apiFetch } from "@client/lib/fetcher";
import { generateAiText } from "@client/lib/ai";
import { cn } from "@client/lib/utils";
import type { Note, Project } from "@shared/types";

type NoteDraft = Pick<Note, "title" | "content" | "projectId">;
type NoteGrouping = "recent" | "date" | "project";
type NoteSort = "updated" | "created" | "title";
type SaveStatus = "saved" | "unsaved" | "saving" | "error";
type NoteGroup = {
  key: string;
  label: string | null;
  color?: string | null;
  notes: Note[];
};

const AUTOSAVE_DELAY_MS = 700;
const JAPANESE_WEEKDAYS = ["日", "月", "火", "水", "木", "金", "土"];
const NOTE_DATE_TIME_FORMATTER = new Intl.DateTimeFormat("ja-JP", {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  hourCycle: "h23",
});

function serializeDraft(draft: NoteDraft) {
  return JSON.stringify(draft);
}

function normalizedDraft(draft: NoteDraft): NoteDraft {
  return {
    ...draft,
    title: draft.title.trim() || "無題のメモ",
  };
}

function isUntouchedNewNoteDraft(draft: NoteDraft) {
  const meaningfulContent = draft.content
    .replace(/<br\s*\/?>/gi, "")
    .replace(/&nbsp;/gi, "")
    .trim();
  return (
    draft.title.trim() === "無題のメモ" &&
    !meaningfulContent &&
    !draft.projectId
  );
}

function localDateGroup(dateValue: string) {
  const date = new Date(dateValue);
  return {
    key: [
      date.getFullYear(),
      String(date.getMonth() + 1).padStart(2, "0"),
      String(date.getDate()).padStart(2, "0"),
    ].join("-"),
    label: `${date.getMonth() + 1}月${date.getDate()}日（${JAPANESE_WEEKDAYS[date.getDay()]}）`,
    timestamp: new Date(
      date.getFullYear(),
      date.getMonth(),
      date.getDate(),
    ).getTime(),
  };
}

function formatNoteDateTime(dateValue: string) {
  return NOTE_DATE_TIME_FORMATTER.format(new Date(dateValue));
}

function noteToMarkdown(title: string, content: string) {
  const normalizedTitle = title.trim() || "無題のメモ";
  const normalizedContent = content.trimEnd();
  return normalizedContent
    ? `# ${normalizedTitle}\n\n${normalizedContent}`
    : `# ${normalizedTitle}`;
}

async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const textarea = document.createElement("textarea");
    textarea.value = text;
    textarea.style.position = "fixed";
    textarea.style.opacity = "0";
    document.body.appendChild(textarea);
    try {
      textarea.select();
      if (!document.execCommand("copy")) throw new Error("copy_failed");
    } finally {
      document.body.removeChild(textarea);
    }
  }
}

function normalizeGeneratedTitle(value: string) {
  const line =
    value
      .split("\n")
      .map((part) => part.trim())
      .find(Boolean) ?? "";
  const normalized = line
    .replace(/^#{1,6}\s+/, "")
    .replace(/^[-*+]\s+(?:\[[ xX]\]\s*)?/, "")
    .replace(/^\d+[.)、]\s*/, "")
    .replace(/^(?:タイトル|件名)\s*[:：]\s*/, "")
    .replace(/^[「『"'“”]+|[」』"'“”]+$/g, "")
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/[*_~`]/g, "")
    .replace(/[。．]$/, "")
    .trim();
  return Array.from(normalized).slice(0, 200).join("");
}

function NoteEditor({
  note,
  projects,
  onBack,
  onPin,
  onArchive,
  onDelete,
  onSaved,
  onDraftChange,
}: {
  note: Note;
  projects: Project[];
  onBack: () => void;
  onPin: () => void;
  onArchive: () => void;
  onDelete: () => void;
  onSaved: (note: Note) => void;
  onDraftChange: (id: string, draft: NoteDraft) => void;
}) {
  const [title, setTitle] = useState(note.title);
  const [content, setContent] = useState(note.content);
  const [projectId, setProjectId] = useState(note.projectId ?? "");
  const [saveStatus, setSaveStatus] = useState<SaveStatus>("saved");
  const [projectMenuOpen, setProjectMenuOpen] = useState(false);
  const [titleGenerating, setTitleGenerating] = useState(false);
  const [copyingFormat, setCopyingFormat] = useState<
    "markdown" | "backlog" | null
  >(null);
  const projectMenuRef = useRef<HTMLDivElement>(null);
  const selectedProject =
    projects.find((project) => project.id === projectId) ?? null;

  const draft: NoteDraft = {
    title,
    content,
    projectId: projectId || null,
  };
  const draftRef = useRef(draft);
  draftRef.current = draft;
  const savedRef = useRef(serializeDraft(draft));
  const queueRef = useRef<Promise<void>>(Promise.resolve());
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const mountedRef = useRef(true);

  const enqueueSave = useCallback(
    (rawDraft: NoteDraft, showStatus = true) => {
      const snapshot = normalizedDraft(rawDraft);
      const serialized = serializeDraft(snapshot);
      if (serialized === savedRef.current) return;

      if (showStatus && mountedRef.current) setSaveStatus("saving");
      queueRef.current = queueRef.current.then(async () => {
        try {
          const updated = await apiFetch<Note>(`/api/notes/${note.id}`, {
            method: "PATCH",
            body: JSON.stringify(snapshot),
          });
          savedRef.current = serialized;
          onSaved(updated);
          if (mountedRef.current) {
            setSaveStatus(
              serializeDraft(normalizedDraft(draftRef.current)) === serialized
                ? "saved"
                : "unsaved",
            );
          }
        } catch {
          if (mountedRef.current) setSaveStatus("error");
        }
      });
    },
    [note.id, onSaved],
  );
  const enqueueSaveRef = useRef(enqueueSave);
  enqueueSaveRef.current = enqueueSave;

  useEffect(() => {
    onDraftChange(note.id, draft);
  }, [content, note.id, onDraftChange, projectId, title]);

  useEffect(() => {
    if (timerRef.current) clearTimeout(timerRef.current);
    const serialized = serializeDraft(normalizedDraft(draft));
    if (serialized === savedRef.current) {
      setSaveStatus("saved");
      return;
    }

    setSaveStatus("unsaved");
    timerRef.current = setTimeout(() => {
      enqueueSaveRef.current(draftRef.current);
    }, AUTOSAVE_DELAY_MS);
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [content, projectId, title]);

  useEffect(() => {
    function onBeforeUnload(event: BeforeUnloadEvent) {
      if (
        serializeDraft(normalizedDraft(draftRef.current)) === savedRef.current
      ) {
        return;
      }
      event.preventDefault();
      event.returnValue = "";
    }
    window.addEventListener("beforeunload", onBeforeUnload);
    return () => window.removeEventListener("beforeunload", onBeforeUnload);
  }, []);

  useEffect(() => {
    if (!projectMenuOpen) return;

    function onPointerDown(event: MouseEvent) {
      if (!projectMenuRef.current?.contains(event.target as Node)) {
        setProjectMenuOpen(false);
      }
    }

    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") setProjectMenuOpen(false);
    }

    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [projectMenuOpen]);

  useEffect(
    () => () => {
      mountedRef.current = false;
      if (timerRef.current) clearTimeout(timerRef.current);
      enqueueSaveRef.current(draftRef.current, false);
    },
    [],
  );

  const statusText =
    saveStatus === "saving"
      ? "保存中…"
      : saveStatus === "unsaved"
        ? "未保存"
        : saveStatus === "error"
          ? "保存できませんでした"
          : "保存済み";

  async function generateTitle() {
    if (!content.trim() || titleGenerating) return;

    setTitleGenerating(true);
    try {
      const generated = await generateAiText("note-title", content);
      const normalized = normalizeGeneratedTitle(generated.text);
      if (!normalized) throw new Error("タイトルを生成できませんでした。");
      onDraftChange(note.id, {
        ...draftRef.current,
        title: normalized,
      });
      setTitle(normalized);
    } catch (error) {
      toast.error(
        typeof error === "string"
          ? error
          : error instanceof Error
            ? error.message
            : "タイトルを生成できませんでした。",
      );
    } finally {
      setTitleGenerating(false);
    }
  }

  async function copyNote(format: "markdown" | "backlog") {
    if (copyingFormat) return;
    setCopyingFormat(format);
    const text =
      format === "markdown"
        ? noteToMarkdown(title, content)
        : noteToBacklog(title, content);

    try {
      await copyText(text);
      toast.success(
        format === "markdown"
          ? "Markdown形式でコピーしました"
          : "Backlog形式でコピーしました",
      );
    } catch {
      toast.error(
        format === "markdown"
          ? "Markdown形式でコピーできませんでした"
          : "Backlog形式でエクスポートできませんでした",
      );
    } finally {
      setCopyingFormat(null);
    }
  }

  return (
    <section className="flex h-full min-h-0 min-w-0 flex-1 flex-col bg-white">
      <div className="flex min-h-14 shrink-0 items-center gap-2 border-b border-neutral-200 px-3 sm:px-4">
        <Tooltip content="メモ一覧へ戻る">
          <button
            type="button"
            onClick={onBack}
            aria-label="メモ一覧へ戻る"
            className="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-neutral-500 hover:bg-neutral-100 hover:text-neutral-800 md:hidden"
          >
            <ChevronLeft className="size-5" />
          </button>
        </Tooltip>
        <div
          ref={projectMenuRef}
          className="relative min-w-0 max-w-[40%] shrink-0"
        >
          <Tooltip content="プロジェクトを変更">
            <button
              type="button"
              onClick={() => setProjectMenuOpen((open) => !open)}
              aria-label={`プロジェクト: ${selectedProject?.name ?? "プロジェクトなし"}`}
              aria-haspopup="listbox"
              aria-expanded={projectMenuOpen}
              className="block max-w-full cursor-pointer truncate rounded py-1 text-left text-sm text-neutral-500 decoration-neutral-300 underline-offset-4 outline-none transition-colors hover:text-neutral-800 hover:underline focus-visible:ring-2 focus-visible:ring-neutral-300"
            >
              {selectedProject?.name ?? "プロジェクトなし"}
            </button>
          </Tooltip>
          {projectMenuOpen && (
            <div
              role="listbox"
              aria-label="プロジェクトを選択"
              className="absolute left-0 top-full z-30 mt-1 w-72 max-w-[calc(100vw-2rem)] overflow-hidden rounded-lg border border-neutral-200 bg-white shadow-lg"
            >
              <div className="subtle-scrollbar max-h-72 overflow-y-auto p-1">
                <button
                  type="button"
                  role="option"
                  aria-selected={!projectId}
                  onClick={() => {
                    onDraftChange(note.id, {
                      ...draftRef.current,
                      projectId: null,
                    });
                    setProjectId("");
                    setProjectMenuOpen(false);
                  }}
                  className={cn(
                    "flex w-full items-center gap-3 rounded-md px-3 py-2 text-left hover:bg-neutral-100",
                    !projectId && "bg-neutral-100",
                  )}
                >
                  <span className="size-3 shrink-0 rounded-full border border-neutral-300 bg-white" />
                  <span className="min-w-0">
                    <span className="block truncate text-sm font-medium text-neutral-800">
                      プロジェクトなし
                    </span>
                    <span className="block truncate text-[11px] text-neutral-400">
                      メモをプロジェクトに紐付けない
                    </span>
                  </span>
                </button>
                {projects.map((project) => (
                  <button
                    key={project.id}
                    type="button"
                    role="option"
                    aria-selected={project.id === projectId}
                    onClick={() => {
                      onDraftChange(note.id, {
                        ...draftRef.current,
                        projectId: project.id,
                      });
                      setProjectId(project.id);
                      setProjectMenuOpen(false);
                    }}
                    className={cn(
                      "flex w-full items-center gap-3 rounded-md px-3 py-2 text-left hover:bg-neutral-100",
                      project.id === projectId && "bg-neutral-100",
                    )}
                  >
                    <span
                      className="size-3 shrink-0 rounded-full"
                      style={{ backgroundColor: project.color }}
                    />
                    <span className="min-w-0">
                      <span className="block truncate text-sm font-medium text-neutral-800">
                        {project.name}
                      </span>
                      <span className="block truncate text-[11px] text-neutral-400">
                        {project.client.name}
                        {project.archived ? " · アーカイブ済み" : ""}
                      </span>
                    </span>
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>
        <span
          aria-hidden="true"
          className="shrink-0 text-sm text-neutral-300"
        >
          /
        </span>
        <input
          value={title}
          onChange={(event) => {
            onDraftChange(note.id, {
              ...draftRef.current,
              title: event.target.value,
            });
            setTitle(event.target.value);
          }}
          onBlur={() => {
            if (!title.trim()) setTitle("無題のメモ");
          }}
          maxLength={200}
          aria-label="メモのタイトル"
          className="min-w-0 flex-1 bg-transparent text-sm font-medium text-neutral-900 outline-none placeholder:text-neutral-400"
          placeholder="無題のメモ"
        />
        <Tooltip
          content={
            content.trim()
              ? "本文からタイトルを生成"
              : "本文を入力するとタイトルを生成できます"
          }
        >
          <button
            type="button"
            onClick={generateTitle}
            aria-disabled={!content.trim() || titleGenerating}
            aria-label="本文からタイトルを生成"
            className="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700 aria-disabled:cursor-not-allowed aria-disabled:opacity-35"
          >
            {titleGenerating ? (
              <LoaderCircle className="size-4 animate-spin" />
            ) : (
              <Sparkles className="size-4" />
            )}
          </button>
        </Tooltip>
        <Tooltip content="Markdown形式でコピー">
          <button
            type="button"
            onClick={() => copyNote("markdown")}
            disabled={copyingFormat !== null}
            aria-label="Markdown形式でコピー"
            className="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700 disabled:cursor-not-allowed disabled:opacity-40"
          >
            {copyingFormat === "markdown" ? (
              <LoaderCircle className="size-4 animate-spin" />
            ) : (
              <FileCode2 className="size-4" />
            )}
          </button>
        </Tooltip>
        <Tooltip content="Backlog形式でコピー">
          <button
            type="button"
            onClick={() => copyNote("backlog")}
            disabled={copyingFormat !== null}
            aria-label="Backlog形式でコピー"
            className="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700 disabled:cursor-not-allowed disabled:opacity-40"
          >
            {copyingFormat === "backlog" ? (
              <LoaderCircle className="size-4 animate-spin" />
            ) : (
              <Copy className="size-4" />
            )}
          </button>
        </Tooltip>
        {!note.archived && (
          <Tooltip content={note.pinned ? "ピン留めを外す" : "ピン留め"}>
            <button
              type="button"
              onClick={onPin}
              aria-label={
                note.pinned ? "メモのピン留めを外す" : "メモをピン留め"
              }
              aria-pressed={note.pinned}
              className="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700"
            >
              {note.pinned ? (
                <PinOff className="size-4" />
              ) : (
                <Pin className="size-4" />
              )}
            </button>
          </Tooltip>
        )}
        <Tooltip content={note.archived ? "復元" : "アーカイブ"}>
          <button
            type="button"
            onClick={onArchive}
            aria-label={note.archived ? "メモを復元" : "メモをアーカイブ"}
            className="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700"
          >
            {note.archived ? (
              <ArchiveRestore className="size-4" />
            ) : (
              <Archive className="size-4" />
            )}
          </button>
        </Tooltip>
        <Tooltip content="メモを削除">
          <button
            type="button"
            onClick={onDelete}
            aria-label="メモを削除"
            className="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-neutral-400 hover:bg-red-50 hover:text-red-600"
          >
            <Trash2 className="size-4" />
          </button>
        </Tooltip>
      </div>

      <MarkdownEditor
        value={content}
        onChange={(value) => {
          onDraftChange(note.id, {
            ...draftRef.current,
            content: value,
          });
          setContent(value);
        }}
        ariaLabel="メモ本文"
        className="note-markdown-editor flex-1"
      />
      <div className="subtle-scrollbar flex h-8 shrink-0 items-center justify-between gap-4 overflow-x-auto whitespace-nowrap border-t border-neutral-200 bg-neutral-50/70 px-3 text-[11px] text-neutral-400 sm:px-4">
        <div className="flex items-center gap-2">
          <span>作成 {formatNoteDateTime(note.createdAt)}</span>
          <span aria-hidden="true">·</span>
          <span>更新 {formatNoteDateTime(note.updatedAt)}</span>
        </div>
        <div className="flex items-center gap-2">
          <span>本文 {Array.from(content).length.toLocaleString("ja-JP")}文字</span>
          {note.archived && (
            <>
              <span aria-hidden="true">·</span>
              <span>アーカイブ済み</span>
            </>
          )}
          <span aria-hidden="true">·</span>
          <span className={saveStatus === "error" ? "text-red-600" : undefined}>
            {statusText}
          </span>
        </div>
      </div>
    </section>
  );
}

export function NotesPage() {
  const queryClient = useQueryClient();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [mobileListOpen, setMobileListOpen] = useState(true);
  const [noteGrouping, setNoteGrouping] = useState<NoteGrouping>("recent");
  const [noteSort, setNoteSort] = useState<NoteSort>("updated");
  const [search, setSearch] = useState("");
  const [searchOpen, setSearchOpen] = useState(false);
  const [showArchived, setShowArchived] = useState(false);
  const [listMenuOpen, setListMenuOpen] = useState(false);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const listMenuRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const discardableNoteIdsRef = useRef(new Set<string>());
  const selectedNoteRef = useRef<Note | null>(null);
  const pageMountedRef = useRef(true);

  const { data: notes = [], isLoading } = useQuery({
    queryKey: ["notes"],
    queryFn: () => apiFetch<Note[]>("/api/notes?includeArchived=1"),
  });
  const { data: projects = [] } = useQuery({
    queryKey: ["projects", "all"],
    queryFn: () =>
      apiFetch<Project[]>("/api/projects?includeArchived=1"),
  });

  const discardUntouchedNewNote = useCallback(
    (note: Note | null) => {
      if (!note || !discardableNoteIdsRef.current.has(note.id)) return false;
      if (
        !isUntouchedNewNoteDraft({
          title: note.title,
          content: note.content,
          projectId: note.projectId,
        })
      ) {
        discardableNoteIdsRef.current.delete(note.id);
        return false;
      }

      discardableNoteIdsRef.current.delete(note.id);
      queryClient.setQueryData<Note[]>(["notes"], (current = []) =>
        current.filter((currentNote) => currentNote.id !== note.id),
      );
      void apiFetch<{ ok: true }>(`/api/notes/${note.id}`, {
        method: "DELETE",
        keepalive: true,
      }).catch(() => {
        void queryClient.invalidateQueries({ queryKey: ["notes"] });
        toast.error("空の新規メモを削除できませんでした");
      });
      return true;
    },
    [queryClient],
  );

  const handleDraftChange = useCallback(
    (id: string, draft: NoteDraft) => {
      if (!isUntouchedNewNoteDraft(draft)) {
        discardableNoteIdsRef.current.delete(id);
      }
    },
    [],
  );

  const visibleNotes = useMemo(() => {
    const normalizedSearch = search.trim().toLocaleLowerCase("ja");
    return notes
      .filter((note) => {
        if (note.archived !== showArchived) return false;
        if (!normalizedSearch) return true;
        return `${note.title}\n${note.content}`
          .toLocaleLowerCase("ja")
          .includes(normalizedSearch);
      })
      .sort((a, b) => {
        if (noteSort === "title") {
          return a.title.localeCompare(b.title, "ja");
        }
        const field = noteSort === "created" ? "createdAt" : "updatedAt";
        return (
          new Date(b[field]).getTime() - new Date(a[field]).getTime()
        );
      });
  }, [noteSort, notes, search, showArchived]);

  const pinnedNotes = useMemo(
    () => visibleNotes.filter((note) => note.pinned && !note.archived),
    [visibleNotes],
  );
  const regularNotes = useMemo(
    () => visibleNotes.filter((note) => !note.pinned || note.archived),
    [visibleNotes],
  );

  const noteGroups = useMemo(() => {
    if (noteGrouping === "recent") {
      return [
        {
          key: "recent",
          label:
            pinnedNotes.length > 0 && regularNotes.length > 0
              ? "その他のメモ"
              : null,
          notes: regularNotes,
        },
      ];
    }

    if (noteGrouping === "date") {
      const groups = new Map<
        string,
        NoteGroup & { timestamp: number }
      >();

      for (const note of regularNotes) {
        const dateGroup = localDateGroup(note.createdAt);
        const current = groups.get(dateGroup.key);
        if (current) {
          current.notes.push(note);
          continue;
        }
        groups.set(dateGroup.key, {
          key: dateGroup.key,
          label: dateGroup.label,
          timestamp: dateGroup.timestamp,
          notes: [note],
        });
      }

      return Array.from(groups.values()).sort(
        (a, b) => b.timestamp - a.timestamp,
      );
    }

    const groups = new Map<string, NoteGroup>();

    for (const note of regularNotes) {
      const key = note.projectId ?? "without-project";
      const current = groups.get(key);
      if (current) {
        current.notes.push(note);
        continue;
      }
      groups.set(key, {
        key,
        label: note.project?.name ?? "プロジェクトなし",
        color: note.project?.color ?? null,
        notes: [note],
      });
    }

    return Array.from(groups.values());
  }, [noteGrouping, pinnedNotes.length, regularNotes]);

  useEffect(() => {
    if (!listMenuOpen) return;

    function onPointerDown(event: MouseEvent) {
      if (!listMenuRef.current?.contains(event.target as Node)) {
        setListMenuOpen(false);
      }
    }

    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") setListMenuOpen(false);
    }

    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [listMenuOpen]);

  useEffect(() => {
    if (searchOpen) searchInputRef.current?.focus();
  }, [searchOpen]);

  useEffect(() => {
    if (visibleNotes.length === 0) {
      if (selectedId) {
        discardUntouchedNewNote(
          notes.find((note) => note.id === selectedId) ?? null,
        );
      }
      setSelectedId(null);
    } else if (
      !selectedId ||
      !visibleNotes.some((note) => note.id === selectedId)
    ) {
      if (selectedId) {
        discardUntouchedNewNote(
          notes.find((note) => note.id === selectedId) ?? null,
        );
      }
      setSelectedId(visibleNotes[0].id);
    }
  }, [
    discardUntouchedNewNote,
    notes,
    selectedId,
    visibleNotes,
  ]);

  const selectedNote =
    visibleNotes.find((note) => note.id === selectedId) ?? null;
  selectedNoteRef.current = selectedNote;

  useEffect(() => {
    pageMountedRef.current = true;
    return () => {
      pageMountedRef.current = false;
      discardUntouchedNewNote(selectedNoteRef.current);
    };
  }, [discardUntouchedNewNote]);

  const createNote = useMutation({
    mutationFn: () =>
      apiFetch<Note>("/api/notes", {
        method: "POST",
        body: JSON.stringify({
          title: "無題のメモ",
          content: "",
          projectId: null,
        }),
      }),
    onSuccess: (created) => {
      if (!pageMountedRef.current) {
        void apiFetch<{ ok: true }>(`/api/notes/${created.id}`, {
          method: "DELETE",
          keepalive: true,
        }).catch(() => {
          void queryClient.invalidateQueries({ queryKey: ["notes"] });
        });
        return;
      }
      discardableNoteIdsRef.current.add(created.id);
      queryClient.setQueryData<Note[]>(["notes"], (current = []) => [
        created,
        ...current,
      ]);
      setSearch("");
      setSearchOpen(false);
      setShowArchived(false);
      setSelectedId(created.id);
      setMobileListOpen(false);
    },
    onError: () => toast.error("メモを作成できませんでした"),
  });

  const deleteNote = useMutation({
    mutationFn: (id: string) =>
      apiFetch<{ ok: true }>(`/api/notes/${id}`, { method: "DELETE" }),
    onSuccess: (_result, id) => {
      discardableNoteIdsRef.current.delete(id);
      queryClient.setQueryData<Note[]>(["notes"], (current = []) =>
        current.filter((note) => note.id !== id),
      );
      setDeleteDialogOpen(false);
      toast.success("メモを削除しました");
    },
    onError: () => toast.error("メモを削除できませんでした"),
  });

  const setNoteArchived = useMutation({
    mutationFn: ({
      id,
      archived,
    }: {
      id: string;
      archived: boolean;
    }) =>
      apiFetch<Note>(`/api/notes/${id}`, {
        method: "PATCH",
        body: JSON.stringify({ archived }),
      }),
    onSuccess: (updated) => {
      discardableNoteIdsRef.current.delete(updated.id);
      queryClient.setQueryData<Note[]>(["notes"], (current = []) =>
        current
          .map((note) => (note.id === updated.id ? updated : note))
          .sort(
            (a, b) =>
              new Date(b.updatedAt).getTime() -
              new Date(a.updatedAt).getTime(),
          ),
      );
      toast.success(
        updated.archived ? "メモをアーカイブしました" : "メモを復元しました",
      );
    },
    onError: () => toast.error("メモの状態を変更できませんでした"),
  });

  const setNotePinned = useMutation({
    mutationFn: ({ id, pinned }: { id: string; pinned: boolean }) =>
      apiFetch<Note>(`/api/notes/${id}`, {
        method: "PATCH",
        body: JSON.stringify({ pinned }),
      }),
    onSuccess: (updated) => {
      discardableNoteIdsRef.current.delete(updated.id);
      queryClient.setQueryData<Note[]>(["notes"], (current = []) =>
        current
          .map((note) => (note.id === updated.id ? updated : note))
          .sort(
            (a, b) =>
              new Date(b.updatedAt).getTime() -
              new Date(a.updatedAt).getTime(),
          ),
      );
      toast.success(
        updated.pinned
          ? "メモをピン留めしました"
          : "メモのピン留めを外しました",
      );
    },
    onError: () => toast.error("メモのピン留めを変更できませんでした"),
  });

  const handleSaved = useCallback(
    (updated: Note) => {
      discardableNoteIdsRef.current.delete(updated.id);
      queryClient.setQueryData<Note[]>(["notes"], (current = []) =>
        current
          .map((note) => (note.id === updated.id ? updated : note))
          .sort(
            (a, b) =>
              new Date(b.updatedAt).getTime() -
              new Date(a.updatedAt).getTime(),
          ),
      );
    },
    [queryClient],
  );

  function renderNoteItem(note: Note) {
    const active = note.id === selectedId;
    return (
      <li
        key={note.id}
        className={cn(
          "group relative rounded-lg transition-colors",
          active
            ? "bg-neutral-200/80"
            : "hover:bg-neutral-200/50 focus-within:bg-neutral-200/50",
        )}
      >
        <button
          type="button"
          onClick={() => {
            if (note.id !== selectedId) {
              discardUntouchedNewNote(selectedNoteRef.current);
            }
            setSelectedId(note.id);
            setMobileListOpen(false);
          }}
          className={cn(
            "flex h-9 w-full cursor-pointer items-center rounded-lg py-0 pl-3 text-left text-sm",
            note.archived ? "pr-10" : "pr-[4.5rem]",
            active
              ? "text-neutral-900"
              : "text-neutral-700 group-hover:text-neutral-900",
          )}
        >
          <span className="truncate">{note.title}</span>
        </button>
        {!note.archived && (
          <Tooltip content={note.pinned ? "ピン留めを外す" : "ピン留め"}>
            <button
              type="button"
              onClick={() => {
                discardableNoteIdsRef.current.delete(note.id);
                setNotePinned.mutate({
                  id: note.id,
                  pinned: !note.pinned,
                });
              }}
              disabled={setNotePinned.isPending}
              aria-label={`${note.title}を${note.pinned ? "ピン留めから外す" : "ピン留めする"}`}
              aria-pressed={note.pinned}
              className="pointer-events-none absolute right-8 top-1/2 inline-flex size-7 -translate-y-1/2 cursor-pointer items-center justify-center rounded-md text-neutral-400 opacity-0 transition-opacity hover:bg-neutral-300/60 hover:text-neutral-700 focus:pointer-events-auto focus:opacity-100 focus-visible:ring-2 focus-visible:ring-neutral-400 group-hover:pointer-events-auto group-hover:opacity-100 disabled:opacity-40"
            >
              {note.pinned ? (
                <PinOff className="size-4" />
              ) : (
                <Pin className="size-4" />
              )}
            </button>
          </Tooltip>
        )}
        <Tooltip content={note.archived ? "復元" : "アーカイブ"}>
          <button
            type="button"
            onClick={() => {
              discardableNoteIdsRef.current.delete(note.id);
              setNoteArchived.mutate({
                id: note.id,
                archived: !note.archived,
              });
            }}
            disabled={setNoteArchived.isPending}
            aria-label={`${note.title}を${note.archived ? "復元" : "アーカイブ"}`}
            className="pointer-events-none absolute right-1 top-1/2 inline-flex size-7 -translate-y-1/2 cursor-pointer items-center justify-center rounded-md text-neutral-400 opacity-0 transition-opacity hover:bg-neutral-300/60 hover:text-neutral-700 focus:pointer-events-auto focus:opacity-100 focus-visible:ring-2 focus-visible:ring-neutral-400 group-hover:pointer-events-auto group-hover:opacity-100 disabled:opacity-40"
          >
            {note.archived ? (
              <ArchiveRestore className="size-4" />
            ) : (
              <Archive className="size-4" />
            )}
          </button>
        </Tooltip>
      </li>
    );
  }

  return (
    <div className="flex h-full min-h-0 overflow-hidden bg-white">
      <aside
        className={cn(
          "h-full min-h-0 w-full shrink-0 flex-col border-r border-neutral-200 bg-neutral-50/50 md:flex md:w-[320px]",
          mobileListOpen ? "flex" : "hidden",
        )}
      >
        <div className="flex min-h-14 shrink-0 items-center justify-between gap-2 px-3">
          {searchOpen ? (
            <div className="flex min-w-0 flex-1 items-center">
              <input
                ref={searchInputRef}
                type="text"
                role="searchbox"
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Escape") {
                    setSearch("");
                    setSearchOpen(false);
                  }
                }}
                placeholder="メモを検索"
                aria-label="メモを検索"
                className="h-8 min-w-0 flex-1 bg-transparent px-1.5 text-sm text-neutral-800 outline-none placeholder:text-neutral-400"
              />
            </div>
          ) : (
            <span className="min-w-0 truncate px-1.5 text-sm font-medium text-neutral-500">
              {showArchived ? "アーカイブ済み" : "すべてのメモ"}
            </span>
          )}

          <div className="flex items-center gap-0.5">
            <Tooltip content={searchOpen ? "検索を閉じる" : "メモを検索"}>
              <button
                type="button"
                onClick={() => {
                  if (searchOpen) {
                    setSearch("");
                    setSearchOpen(false);
                  } else {
                    setSearchOpen(true);
                  }
                }}
                aria-label={searchOpen ? "検索を閉じる" : "メモを検索"}
                className="inline-flex size-8 items-center justify-center rounded-md text-neutral-500 hover:bg-neutral-200/70 hover:text-neutral-800"
              >
                {searchOpen ? (
                  <X className="size-4" />
                ) : (
                  <Search className="size-[18px]" />
                )}
              </button>
            </Tooltip>
            <div ref={listMenuRef} className="relative">
              <Tooltip content="メモ一覧メニュー">
                <button
                  type="button"
                  onClick={() => setListMenuOpen((open) => !open)}
                  aria-label="メモ一覧メニュー"
                  aria-haspopup="menu"
                  aria-expanded={listMenuOpen}
                  className="inline-flex size-8 items-center justify-center rounded-md text-neutral-500 hover:bg-neutral-200/70 hover:text-neutral-800"
                >
                  <Ellipsis className="size-5" />
                </button>
              </Tooltip>
              {listMenuOpen && (
                <div
                  role="menu"
                  className="absolute right-0 top-full z-20 mt-1 w-60 rounded-xl border border-neutral-200 bg-white p-1.5 shadow-lg"
                >
                  <p className="px-2.5 pb-1 pt-1 text-[11px] font-medium text-neutral-400">
                    整理
                  </p>
                  {(
                    [
                      ["date", "日付別"],
                      ["project", "プロジェクト別"],
                      ["recent", "1つのリストで表示"],
                    ] as const
                  ).map(([value, label]) => (
                    <button
                      key={value}
                      type="button"
                      role="menuitemradio"
                      aria-checked={noteGrouping === value}
                      onClick={() => {
                        setNoteGrouping(value);
                        setListMenuOpen(false);
                      }}
                      className="flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-sm text-neutral-700 hover:bg-neutral-100"
                    >
                      <Check
                        className={cn(
                          "size-4 shrink-0",
                          noteGrouping === value
                            ? "text-neutral-700"
                            : "text-transparent",
                        )}
                      />
                      {label}
                    </button>
                  ))}

                  <p className="px-2.5 pb-1 pt-3 text-[11px] font-medium text-neutral-400">
                    並べ替え
                  </p>
                  {(
                    [
                      ["updated", "最終更新日時"],
                      ["created", "作成日時"],
                      ["title", "タイトル順"],
                    ] as const
                  ).map(([value, label]) => (
                    <button
                      key={value}
                      type="button"
                      role="menuitemradio"
                      aria-checked={noteSort === value}
                      onClick={() => {
                        setNoteSort(value);
                        setListMenuOpen(false);
                      }}
                      className="flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-sm text-neutral-700 hover:bg-neutral-100"
                    >
                      <Check
                        className={cn(
                          "size-4 shrink-0",
                          noteSort === value
                            ? "text-neutral-700"
                            : "text-transparent",
                        )}
                      />
                      {label}
                    </button>
                  ))}

                  <p className="px-2.5 pb-1 pt-3 text-[11px] font-medium text-neutral-400">
                    ステータス
                  </p>
                  {(
                    [
                      ["active", "通常のメモ"],
                      ["archived", "アーカイブ済み"],
                    ] as const
                  ).map(([value, label]) => {
                    const selected =
                      showArchived === (value === "archived");
                    return (
                      <button
                        key={value}
                        type="button"
                        role="menuitemradio"
                        aria-checked={selected}
                        onClick={() => {
                          setShowArchived(value === "archived");
                          setListMenuOpen(false);
                        }}
                        className="flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-sm text-neutral-700 hover:bg-neutral-100"
                      >
                        <Check
                          className={cn(
                            "size-4 shrink-0",
                            selected
                              ? "text-neutral-700"
                              : "text-transparent",
                          )}
                        />
                        {label}
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
            <Tooltip content="新規メモ">
              <button
                type="button"
                onClick={() => {
                  discardUntouchedNewNote(selectedNoteRef.current);
                  setSearch("");
                  setSearchOpen(false);
                  setShowArchived(false);
                  createNote.mutate();
                }}
                disabled={createNote.isPending}
                aria-label="新規メモ"
                className="inline-flex size-8 items-center justify-center rounded-md text-neutral-500 hover:bg-neutral-200/70 hover:text-neutral-800 disabled:opacity-50"
              >
                <SquarePen className="size-[18px]" />
              </button>
            </Tooltip>
          </div>
        </div>

        <div className="subtle-scrollbar min-h-0 flex-1 overflow-y-auto px-2 pb-3">
          {isLoading ? (
            <div className="flex h-32 items-center justify-center">
              <div className="size-5 animate-spin rounded-full border-2 border-neutral-300 border-t-neutral-700" />
            </div>
          ) : visibleNotes.length === 0 ? (
            <div className="flex h-40 flex-col items-center justify-center px-6 text-center text-neutral-400">
              <FileText className="mb-2 size-6" />
              <p className="text-xs">
                {search.trim()
                  ? "一致するメモがありません"
                  : showArchived
                    ? "アーカイブ済みのメモはありません"
                    : "メモはまだありません"}
              </p>
            </div>
          ) : (
            <div className="space-y-4">
              {pinnedNotes.length > 0 && (
                <section>
                  <div className="flex h-7 items-center px-2 text-[11px] font-medium text-neutral-400">
                    <span>ピン留め</span>
                  </div>
                  <ul className="space-y-0.5">
                    {pinnedNotes.map(renderNoteItem)}
                  </ul>
                </section>
              )}
              {noteGroups.map((group) => (
                <section key={group.key}>
                  {group.label && (
                    <div className="flex h-7 items-center gap-2 px-2 text-[11px] font-medium text-neutral-400">
                      {group.color !== undefined && (
                        <span
                          className={cn(
                            "size-2 shrink-0 rounded-full",
                            !group.color && "border border-neutral-300",
                          )}
                          style={
                            group.color
                              ? { backgroundColor: group.color }
                              : undefined
                          }
                        />
                      )}
                      <span className="truncate">{group.label}</span>
                    </div>
                  )}
                  <ul className="space-y-0.5">
                    {group.notes.map(renderNoteItem)}
                  </ul>
                </section>
              ))}
            </div>
          )}
        </div>
      </aside>

      {selectedNote ? (
        <div
          className={cn(
            "h-full min-h-0 min-w-0 flex-1",
            mobileListOpen ? "hidden md:block" : "block",
          )}
        >
          <NoteEditor
            key={selectedNote.id}
            note={selectedNote}
            projects={projects}
            onBack={() => {
              discardUntouchedNewNote(selectedNoteRef.current);
              setMobileListOpen(true);
            }}
            onPin={() => {
              discardableNoteIdsRef.current.delete(selectedNote.id);
              setNotePinned.mutate({
                id: selectedNote.id,
                pinned: !selectedNote.pinned,
              });
            }}
            onArchive={() => {
              discardableNoteIdsRef.current.delete(selectedNote.id);
              setNoteArchived.mutate({
                id: selectedNote.id,
                archived: !selectedNote.archived,
              });
            }}
            onDelete={() => setDeleteDialogOpen(true)}
            onSaved={handleSaved}
            onDraftChange={handleDraftChange}
          />
        </div>
      ) : (
        <section className="hidden min-h-0 min-w-0 flex-1 flex-col items-center justify-center bg-white text-neutral-400 md:flex">
          <FileText className="mb-3 size-8" />
          <p className="text-sm">
            {search.trim()
              ? "一致するメモがありません"
              : visibleNotes.length === 0
                ? showArchived
                  ? "アーカイブ済みのメモはありません"
                  : "新しいメモを作成してください"
              : "メモを選択してください"}
          </p>
        </section>
      )}

      <Dialog
        open={deleteDialogOpen}
        onOpenChange={setDeleteDialogOpen}
      >
        <DialogHeader>
          <DialogTitle>メモを削除しますか？</DialogTitle>
        </DialogHeader>
        <p className="text-sm leading-6 text-neutral-600">
          「{selectedNote?.title ?? "このメモ"}」を削除します。この操作は取り消せません。
        </p>
        <DialogFooter>
          <button
            type="button"
            onClick={() => setDeleteDialogOpen(false)}
            className="rounded-md border border-neutral-300 px-3 py-1.5 text-sm text-neutral-700 hover:bg-neutral-50"
          >
            キャンセル
          </button>
          <button
            type="button"
            data-dialog-autofocus
            onClick={() => {
              if (selectedNote) deleteNote.mutate(selectedNote.id);
            }}
            disabled={deleteNote.isPending}
            className="rounded-md bg-red-600 px-3 py-1.5 text-sm text-white hover:bg-red-700 disabled:opacity-50"
          >
            削除
          </button>
        </DialogFooter>
      </Dialog>
    </div>
  );
}
