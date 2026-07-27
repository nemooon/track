import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ChevronLeft,
  FileText,
  Plus,
  Search,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";
import {
  Dialog,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@client/components/ui/dialog";
import { MarkdownEditor } from "@client/components/MarkdownEditor";
import { apiFetch } from "@client/lib/fetcher";
import { cn } from "@client/lib/utils";
import type { Note, Project } from "@shared/types";

type NoteDraft = Pick<Note, "title" | "content" | "projectId">;
type SaveStatus = "saved" | "unsaved" | "saving" | "error";

const AUTOSAVE_DELAY_MS = 700;

function serializeDraft(draft: NoteDraft) {
  return JSON.stringify(draft);
}

function normalizedDraft(draft: NoteDraft): NoteDraft {
  return {
    ...draft,
    title: draft.title.trim() || "無題のメモ",
  };
}

function formatUpdatedAt(value: string) {
  const date = new Date(value);
  const today = new Date();
  if (date.toDateString() === today.toDateString()) {
    return new Intl.DateTimeFormat("ja-JP", {
      hour: "2-digit",
      minute: "2-digit",
    }).format(date);
  }
  return new Intl.DateTimeFormat("ja-JP", {
    month: "numeric",
    day: "numeric",
  }).format(date);
}

function noteExcerpt(content: string) {
  const line = content
    .split("\n")
    .map((value) => value.trim())
    .find(Boolean);
  if (!line) return "本文はまだありません";
  return line
    .replace(/^#{1,6}\s+|^[-*]\s+(?:\[[ xX]\]\s*)?/, "")
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/[*_~`]/g, "");
}

function NoteEditor({
  note,
  projects,
  onBack,
  onDelete,
  onSaved,
}: {
  note: Note;
  projects: Project[];
  onBack: () => void;
  onDelete: () => void;
  onSaved: (note: Note) => void;
}) {
  const [title, setTitle] = useState(note.title);
  const [content, setContent] = useState(note.content);
  const [projectId, setProjectId] = useState(note.projectId ?? "");
  const [saveStatus, setSaveStatus] = useState<SaveStatus>("saved");

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

  return (
    <section className="flex h-full min-h-0 min-w-0 flex-1 flex-col bg-white">
      <div className="flex min-h-14 shrink-0 items-center gap-2 border-b border-neutral-200 px-3 sm:px-4">
        <button
          type="button"
          onClick={onBack}
          aria-label="メモ一覧へ戻る"
          className="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-neutral-500 hover:bg-neutral-100 hover:text-neutral-800 md:hidden"
        >
          <ChevronLeft className="size-5" />
        </button>
        <input
          value={title}
          onChange={(event) => setTitle(event.target.value)}
          onBlur={() => {
            if (!title.trim()) setTitle("無題のメモ");
          }}
          maxLength={200}
          aria-label="メモのタイトル"
          className="min-w-0 flex-1 bg-transparent text-base font-semibold text-neutral-900 outline-none placeholder:text-neutral-400"
          placeholder="無題のメモ"
        />
        <span
          className={cn(
            "shrink-0 text-[11px]",
            saveStatus === "error" ? "text-red-600" : "text-neutral-400",
          )}
        >
          {statusText}
        </span>
        <button
          type="button"
          onClick={onDelete}
          aria-label="メモを削除"
          title="メモを削除"
          className="inline-flex size-8 shrink-0 items-center justify-center rounded-md text-neutral-400 hover:bg-red-50 hover:text-red-600"
        >
          <Trash2 className="size-4" />
        </button>
      </div>

      <div className="flex min-h-11 shrink-0 items-center gap-3 border-b border-neutral-200 bg-neutral-50/60 px-3 sm:px-4">
        <label
          htmlFor={`note-project-${note.id}`}
          className="shrink-0 text-xs font-medium text-neutral-500"
        >
          プロジェクト
        </label>
        <select
          id={`note-project-${note.id}`}
          value={projectId}
          onChange={(event) => setProjectId(event.target.value)}
          className="min-w-0 max-w-sm flex-1 rounded-md border border-neutral-300 bg-white px-2 py-1.5 text-xs text-neutral-700 outline-none focus:ring-2 focus:ring-neutral-300"
        >
          <option value="">プロジェクトなし</option>
          {projects.map((project) => (
            <option key={project.id} value={project.id}>
              {project.client.name} · {project.name}
              {project.archived ? "（アーカイブ済み）" : ""}
            </option>
          ))}
        </select>
      </div>

      <MarkdownEditor
        value={content}
        onChange={setContent}
        ariaLabel="メモ本文"
        className="note-markdown-editor flex-1"
      />
    </section>
  );
}

export function NotesPage() {
  const queryClient = useQueryClient();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [mobileListOpen, setMobileListOpen] = useState(true);
  const [search, setSearch] = useState("");
  const [projectFilter, setProjectFilter] = useState("");
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);

  const { data: notes = [], isLoading } = useQuery({
    queryKey: ["notes"],
    queryFn: () => apiFetch<Note[]>("/api/notes"),
  });
  const { data: projects = [] } = useQuery({
    queryKey: ["projects", "all"],
    queryFn: () =>
      apiFetch<Project[]>("/api/projects?includeArchived=1"),
  });

  const visibleNotes = useMemo(() => {
    const normalizedSearch = search.trim().toLocaleLowerCase("ja");
    return notes.filter((note) => {
      if (projectFilter && note.projectId !== projectFilter) return false;
      if (!normalizedSearch) return true;
      return `${note.title}\n${note.content}`
        .toLocaleLowerCase("ja")
        .includes(normalizedSearch);
    });
  }, [notes, projectFilter, search]);

  useEffect(() => {
    if (visibleNotes.length === 0) {
      setSelectedId(null);
    } else if (
      !selectedId ||
      !visibleNotes.some((note) => note.id === selectedId)
    ) {
      setSelectedId(visibleNotes[0].id);
    }
  }, [selectedId, visibleNotes]);

  const selectedNote =
    notes.find((note) => note.id === selectedId) ?? null;

  const createNote = useMutation({
    mutationFn: () =>
      apiFetch<Note>("/api/notes", {
        method: "POST",
        body: JSON.stringify({
          title: "無題のメモ",
          content: "",
          projectId: projectFilter || null,
        }),
      }),
    onSuccess: (created) => {
      queryClient.setQueryData<Note[]>(["notes"], (current = []) => [
        created,
        ...current,
      ]);
      setSearch("");
      setSelectedId(created.id);
      setMobileListOpen(false);
    },
    onError: () => toast.error("メモを作成できませんでした"),
  });

  const deleteNote = useMutation({
    mutationFn: (id: string) =>
      apiFetch<{ ok: true }>(`/api/notes/${id}`, { method: "DELETE" }),
    onSuccess: (_result, id) => {
      queryClient.setQueryData<Note[]>(["notes"], (current = []) =>
        current.filter((note) => note.id !== id),
      );
      setDeleteDialogOpen(false);
      toast.success("メモを削除しました");
    },
    onError: () => toast.error("メモを削除できませんでした"),
  });

  const handleSaved = useCallback(
    (updated: Note) => {
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

  return (
    <div className="flex h-full min-h-0 overflow-hidden bg-white">
      <aside
        className={cn(
          "h-full min-h-0 w-full shrink-0 flex-col border-r border-neutral-200 bg-neutral-50/50 md:flex md:w-[320px]",
          mobileListOpen ? "flex" : "hidden",
        )}
      >
        <div className="flex min-h-14 shrink-0 items-center justify-between border-b border-neutral-200 px-3">
          <div>
            <h1 className="text-sm font-semibold text-neutral-900">メモ</h1>
            <p className="text-[11px] text-neutral-400">{notes.length}件</p>
          </div>
          <button
            type="button"
            onClick={() => createNote.mutate()}
            disabled={createNote.isPending}
            className="inline-flex h-8 items-center gap-1 rounded-md bg-[#2e3a35] px-2.5 text-xs font-medium text-white hover:bg-[#24302b] disabled:opacity-50"
          >
            <Plus className="size-4" />
            新規
          </button>
        </div>

        <div className="shrink-0 space-y-2 border-b border-neutral-200 p-3">
          <label className="relative block">
            <Search className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-neutral-400" />
            <input
              type="search"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              placeholder="メモを検索"
              aria-label="メモを検索"
              className="h-8 w-full rounded-md border border-neutral-300 bg-white pl-8 pr-2 text-xs outline-none placeholder:text-neutral-400 focus:ring-2 focus:ring-neutral-300"
            />
          </label>
          <select
            value={projectFilter}
            onChange={(event) => setProjectFilter(event.target.value)}
            aria-label="プロジェクトで絞り込み"
            className="h-8 w-full rounded-md border border-neutral-300 bg-white px-2 text-xs text-neutral-700 outline-none focus:ring-2 focus:ring-neutral-300"
          >
            <option value="">すべてのプロジェクト</option>
            {projects.map((project) => (
              <option key={project.id} value={project.id}>
                {project.client.name} · {project.name}
              </option>
            ))}
          </select>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto">
          {isLoading ? (
            <div className="flex h-32 items-center justify-center">
              <div className="size-5 animate-spin rounded-full border-2 border-neutral-300 border-t-neutral-700" />
            </div>
          ) : visibleNotes.length === 0 ? (
            <div className="flex h-40 flex-col items-center justify-center px-6 text-center text-neutral-400">
              <FileText className="mb-2 size-6" />
              <p className="text-xs">
                {notes.length === 0
                  ? "メモはまだありません"
                  : "条件に一致するメモがありません"}
              </p>
            </div>
          ) : (
            <ul className="divide-y divide-neutral-200">
              {visibleNotes.map((note) => {
                const active = note.id === selectedId;
                return (
                  <li key={note.id}>
                    <button
                      type="button"
                      onClick={() => {
                        setSelectedId(note.id);
                        setMobileListOpen(false);
                      }}
                      className={cn(
                        "w-full border-l-2 px-3 py-3 text-left transition-colors",
                        active
                          ? "border-[#2e3a35] bg-white"
                          : "border-transparent hover:bg-white/80",
                      )}
                    >
                      <div className="flex items-start justify-between gap-2">
                        <span className="truncate text-sm font-medium text-neutral-800">
                          {note.title}
                        </span>
                        <time className="shrink-0 text-[10px] text-neutral-400">
                          {formatUpdatedAt(note.updatedAt)}
                        </time>
                      </div>
                      <p className="mt-1 truncate text-xs text-neutral-500">
                        {noteExcerpt(note.content)}
                      </p>
                      <p className="mt-1.5 truncate text-[10px] text-neutral-400">
                        {note.project
                          ? `${note.project.client.name} · ${note.project.name}`
                          : "プロジェクトなし"}
                      </p>
                    </button>
                  </li>
                );
              })}
            </ul>
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
            onBack={() => setMobileListOpen(true)}
            onDelete={() => setDeleteDialogOpen(true)}
            onSaved={handleSaved}
          />
        </div>
      ) : (
        <section className="hidden min-h-0 min-w-0 flex-1 flex-col items-center justify-center bg-white text-neutral-400 md:flex">
          <FileText className="mb-3 size-8" />
          <p className="text-sm">
            {notes.length === 0
              ? "新しいメモを作成してください"
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
