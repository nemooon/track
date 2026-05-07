"use client";

import * as React from "react";
import { format } from "date-fns";
import { Dialog, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import type { Project, Tag, TimeEntry } from "@/types";

interface Props {
  entry: TimeEntry | null;
  projects: Project[];
  tags: Tag[];
  onClose: () => void;
  onSave: (id: string, patch: EntryPatch) => void;
  onDelete: (id: string) => void;
}

export interface EntryPatch {
  title: string | null;
  start: string;
  end: string;
  projectId: string | null;
  tagIds: string[];
  note: string | null;
  breakMinutes: number;
}

function toTimeString(iso: string): string {
  return format(new Date(iso), "HH:mm");
}

function applyTime(iso: string, time: string): string {
  const d = new Date(iso);
  const [h, m] = time.split(":").map(Number);
  d.setHours(h, m, 0, 0);
  return d.toISOString();
}

export function EntryEditDialog({ entry, projects, tags, onClose, onSave, onDelete }: Props) {
  const [title, setTitle] = React.useState("");
  const [startTime, setStartTime] = React.useState("");
  const [endTime, setEndTime] = React.useState("");
  const [projectId, setProjectId] = React.useState<string>("");
  const [tagIds, setTagIds] = React.useState<string[]>([]);
  const [note, setNote] = React.useState("");
  const [breakMinutes, setBreakMinutes] = React.useState(0);

  React.useEffect(() => {
    if (!entry) return;
    setTitle(entry.title ?? "");
    setStartTime(toTimeString(entry.start));
    setEndTime(toTimeString(entry.end));
    setProjectId(entry.projectId ?? "");
    setTagIds(entry.tags.map((t) => t.tagId));
    setNote(entry.note ?? "");
    setBreakMinutes(entry.breakMinutes ?? 0);
  }, [entry?.id]);

  React.useEffect(() => {
    if (!entry) return;
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === "Enter" && e.metaKey) {
        e.preventDefault();
        handleSave();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [entry, title, startTime, endTime, projectId, tagIds, note, breakMinutes]);

  function handleSave() {
    if (!entry) return;
    onSave(entry.id, {
      title: title.trim() || null,
      start: applyTime(entry.start, startTime),
      end: applyTime(entry.end, endTime),
      projectId: projectId || null,
      tagIds: [...new Set([...tagIds, ...lockedTagIds])],
      note: note.trim() || null,
      breakMinutes,
    });
    onClose();
  }

  function handleDelete() {
    if (!entry) return;
    onDelete(entry.id);
    onClose();
  }

  const selectedProject = projects.find((p) => p.id === projectId);
  const lockedTagIds = selectedProject?.tags?.map((t) => t.tagId) ?? [];

  function toggleTag(id: string) {
    if (lockedTagIds.includes(id)) return;
    setTagIds((prev) => (prev.includes(id) ? prev.filter((t) => t !== id) : [...prev, id]));
  }

  return (
    <Dialog open={!!entry} onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogHeader>
        <DialogTitle>予定を編集</DialogTitle>
      </DialogHeader>

      {entry?.externalEventSource === "outlook" && (
        <div
          className="mb-3 inline-flex w-fit items-center gap-1.5 rounded border border-outlook bg-outlook/10 px-2 py-1 text-xs text-outlook"
          title={`Outlook イベント (ID: ${entry.externalEventId ?? ""}) から作成された記録です`}
        >
          <span className="size-1.5 rounded-full bg-outlook" />
          Outlook イベントから作成
        </div>
      )}

      <div className="flex flex-col gap-4">
        {/* Title */}
        <div className="flex flex-col gap-1">
          <label className="text-sm font-medium text-neutral-700">タイトル</label>
          <input
            type="text"
            value={title}
            maxLength={100}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="タイトル（任意）"
            className="rounded-md border border-neutral-300 px-3 py-2 text-sm focus:border-neutral-500 focus:outline-none"
          />
        </div>

        {/* Times */}
        <div className="flex gap-3">
          <div className="flex flex-2 flex-col gap-1">
            <label className="text-sm font-medium text-neutral-700">開始</label>
            <input
              type="time"
              step={900}
              value={startTime}
              onChange={(e) => setStartTime(e.target.value)}
              className="rounded-md border border-neutral-300 px-3 py-2 text-sm focus:border-neutral-500 focus:outline-none"
            />
          </div>
          <div className="flex flex-2 flex-col gap-1">
            <label className="text-sm font-medium text-neutral-700">終了</label>
            <input
              type="time"
              step={900}
              value={endTime}
              onChange={(e) => setEndTime(e.target.value)}
              className="rounded-md border border-neutral-300 px-3 py-2 text-sm focus:border-neutral-500 focus:outline-none"
            />
          </div>
          <div className="flex flex-1 flex-col gap-1">
            <label className="text-sm font-medium text-neutral-700">休憩 (分)</label>
            <input
              type="number"
              min={0}
              max={60}
              step={15}
              value={breakMinutes}
              onChange={(e) => {
                const n = Number(e.target.value);
                setBreakMinutes(Number.isFinite(n) ? Math.max(0, Math.min(600, Math.floor(n))) : 0);
              }}
              className="rounded-md border border-neutral-300 px-3 py-2 text-sm focus:border-neutral-500 focus:outline-none"
            />
          </div>
        </div>

        {/* Project */}
        <div className="flex flex-col gap-1">
          <label className="text-sm font-medium text-neutral-700">プロジェクト</label>
          <select
            value={projectId}
            onChange={(e) => setProjectId(e.target.value)}
            className="rounded-md border border-neutral-300 px-3 py-2 text-sm focus:border-neutral-500 focus:outline-none"
          >
            <option value="">なし</option>
            {projects
              .filter((p) => !p.archived)
              .map((p) => (
                <option key={p.id} value={p.id}>
                  {p.client?.name ? `${p.client.name} / ${p.name}` : p.name}
                </option>
              ))}
          </select>
        </div>

        {/* Tags */}
        {tags.length > 0 && (
          <div className="flex flex-col gap-1">
            <label className="text-sm font-medium text-neutral-700">タグ</label>
            <div className="flex flex-wrap gap-2">
              {tags.map((tag) => {
                const locked = lockedTagIds.includes(tag.id);
                const active = locked || tagIds.includes(tag.id);
                return (
                  <button
                    key={tag.id}
                    type="button"
                    onClick={() => toggleTag(tag.id)}
                    disabled={locked}
                    title={locked ? "プロジェクトに紐づいているタグ" : undefined}
                    className="rounded-full border px-3 py-1 text-xs transition-colors disabled:cursor-default disabled:opacity-70"
                    style={{
                      backgroundColor: active ? tag.color : "transparent",
                      borderColor: tag.color,
                      color: active ? "#fff" : tag.color,
                    }}
                  >
                    {tag.name}
                  </button>
                );
              })}
            </div>
          </div>
        )}

        {/* Note */}
        <div className="flex flex-col gap-1">
          <label className="text-sm font-medium text-neutral-700">メモ</label>
          <textarea
            value={note}
            maxLength={500}
            rows={3}
            onChange={(e) => setNote(e.target.value)}
            placeholder="メモ（任意）"
            className="resize-none rounded-md border border-neutral-300 px-3 py-2 text-sm focus:border-neutral-500 focus:outline-none"
          />
        </div>
      </div>

      <DialogFooter className="mt-6 flex justify-between">
        <button
          type="button"
          onClick={handleDelete}
          className="rounded-md border border-red-300 px-3 py-1.5 text-sm text-red-600 hover:bg-red-50"
        >
          削除
        </button>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={onClose}
            className="rounded-md border border-neutral-300 px-3 py-1.5 text-sm hover:bg-neutral-50"
          >
            キャンセル
          </button>
          <button
            type="button"
            onClick={handleSave}
            className="rounded-md bg-neutral-900 px-3 py-1.5 text-sm text-white hover:bg-neutral-700"
          >
            保存
          </button>
        </div>
      </DialogFooter>
    </Dialog>
  );
}
