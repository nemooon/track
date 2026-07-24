import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { Archive, ArchiveRestore, Plus, Pencil, Trash2 } from "lucide-react";
import { Button } from "@client/components/ui/button";
import { Input } from "@client/components/ui/input";
import { Label } from "@client/components/ui/label";
import {
  Dialog,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@client/components/ui/dialog";
import { apiFetch } from "@client/lib/fetcher";
import { PROJECT_COLORS, randomColor } from "@client/lib/utils";
import type { Client, Project, Tag } from "@shared/types";

/* ── Client CRUD ─────────────────────────────────────── */

function useClients() {
  return useQuery({
    queryKey: ["clients"],
    queryFn: () => apiFetch<Client[]>("/api/clients"),
  });
}

/* ── Projects CRUD ───────────────────────────────────── */

function useProjects() {
  return useQuery({
    queryKey: ["projects", "all"],
    queryFn: () => apiFetch<Project[]>("/api/projects?includeArchived=1"),
  });
}

/* ── Tags CRUD ──────────────────────────────────────── */

function useTags() {
  return useQuery({
    queryKey: ["tags"],
    queryFn: () => apiFetch<Tag[]>("/api/tags"),
  });
}

/* ── Page ─────────────────────────────────────────────── */

export function ProjectsPage({
  embedded = false,
}: {
  embedded?: boolean;
} = {}) {
  const qc = useQueryClient();
  const { data: clients = [] } = useClients();
  const { data: projects = [] } = useProjects();
  const { data: tags = [] } = useTags();
  const activeClients = clients.filter((client) => !client.archived);
  const orderedClients = [...clients].sort(
    (a, b) => Number(a.archived) - Number(b.archived),
  );
  const orderedProjects = [...projects].sort(
    (a, b) => Number(a.archived) - Number(b.archived),
  );

  const [clientDialogOpen, setClientDialogOpen] = useState(false);
  const [editingClient, setEditingClient] = useState<Client | null>(null);
  const [clientName, setClientName] = useState("");

  const [projectDialogOpen, setProjectDialogOpen] = useState(false);
  const [editingProject, setEditingProject] = useState<Project | null>(null);
  const [projectName, setProjectName] = useState("");
  const [projectClientId, setProjectClientId] = useState("");
  const [projectColor, setProjectColor] = useState(randomColor());
  const [projectTagIds, setProjectTagIds] = useState<string[]>([]);

  const [tagDialogOpen, setTagDialogOpen] = useState(false);
  const [editingTag, setEditingTag] = useState<Tag | null>(null);
  const [tagName, setTagName] = useState("");
  const [tagColor, setTagColor] = useState(randomColor());
  const [deletingTag, setDeletingTag] = useState<Tag | null>(null);

  // Client mutations
  const createClient = useMutation({
    mutationFn: (name: string) =>
      apiFetch("/api/clients", { method: "POST", body: JSON.stringify({ name }) }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["clients"] });
      toast.success("クライアントを作成しました");
    },
  });

  const updateClient = useMutation({
    mutationFn: ({ id, name }: { id: string; name: string }) =>
      apiFetch(`/api/clients/${id}`, { method: "PATCH", body: JSON.stringify({ name }) }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["clients"] });
      qc.invalidateQueries({ queryKey: ["projects"] });
      toast.success("クライアントを更新しました");
    },
  });

  const setClientArchived = useMutation({
    mutationFn: ({ id, archived }: { id: string; archived: boolean }) =>
      apiFetch(`/api/clients/${id}`, {
        method: "PATCH",
        body: JSON.stringify({ archived }),
      }),
    onSuccess: (_data, { archived }) => {
      qc.invalidateQueries({ queryKey: ["clients"] });
      qc.invalidateQueries({ queryKey: ["projects"] });
      toast.success(
        archived
          ? "クライアントと配下のプロジェクトをアーカイブしました"
          : "クライアントを復元しました",
      );
    },
    onError: () => toast.error("クライアントの状態を更新できませんでした"),
  });

  // Project mutations
  const createProject = useMutation({
    mutationFn: (data: { clientId: string; name: string; color: string; tagIds?: string[] }) =>
      apiFetch("/api/projects", { method: "POST", body: JSON.stringify(data) }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["projects"] });
      toast.success("プロジェクトを作成しました");
    },
  });

  const updateProject = useMutation({
    mutationFn: ({ id, ...data }: { id: string; name?: string; color?: string; clientId?: string; tagIds?: string[] }) =>
      apiFetch(`/api/projects/${id}`, { method: "PATCH", body: JSON.stringify(data) }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["projects"] });
      toast.success("プロジェクトを更新しました");
    },
  });

  const setProjectArchived = useMutation({
    mutationFn: ({ id, archived }: { id: string; archived: boolean }) =>
      apiFetch(`/api/projects/${id}`, {
        method: "PATCH",
        body: JSON.stringify({ archived }),
      }),
    onSuccess: (_data, { archived }) => {
      qc.invalidateQueries({ queryKey: ["projects"] });
      toast.success(
        archived
          ? "プロジェクトをアーカイブしました"
          : "プロジェクトを復元しました",
      );
    },
    onError: () => toast.error("プロジェクトの状態を更新できませんでした"),
  });

  // Tag mutations
  const createTag = useMutation({
    mutationFn: (data: { name: string; color: string }) =>
      apiFetch("/api/tags", { method: "POST", body: JSON.stringify(data) }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["tags"] });
      toast.success("タグを作成しました");
    },
    onError: (err) => {
      if ((err as Error).message.includes("tag_already_exists")) {
        toast.error("同じ名前のタグが既にあります");
      } else {
        toast.error("作成に失敗しました");
      }
    },
  });

  const updateTag = useMutation({
    mutationFn: ({ id, ...data }: { id: string; name?: string; color?: string }) =>
      apiFetch(`/api/tags/${id}`, { method: "PATCH", body: JSON.stringify(data) }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["tags"] });
      qc.invalidateQueries({ queryKey: ["entries"] });
      toast.success("タグを更新しました");
    },
  });

  const deleteTag = useMutation({
    mutationFn: (id: string) => apiFetch(`/api/tags/${id}`, { method: "DELETE" }),
    onSuccess: () => {
      setDeletingTag(null);
      qc.invalidateQueries({ queryKey: ["tags"] });
      qc.invalidateQueries({ queryKey: ["entries"] });
      toast.success("タグを削除しました");
    },
    onError: () => toast.error("削除に失敗しました"),
  });

  function openNewTag() {
    setEditingTag(null);
    setTagName("");
    setTagColor(randomColor());
    setTagDialogOpen(true);
  }
  function openEditTag(tag: Tag) {
    setEditingTag(tag);
    setTagName(tag.name);
    setTagColor(tag.color);
    setTagDialogOpen(true);
  }
  function submitTag() {
    if (!tagName.trim()) return;
    if (editingTag) {
      updateTag.mutate({ id: editingTag.id, name: tagName.trim(), color: tagColor });
    } else {
      createTag.mutate({ name: tagName.trim(), color: tagColor });
    }
    setTagDialogOpen(false);
  }

  function openNewClient() {
    setEditingClient(null);
    setClientName("");
    setClientDialogOpen(true);
  }
  function openEditClient(client: Client) {
    setEditingClient(client);
    setClientName(client.name);
    setClientDialogOpen(true);
  }
  function submitClient() {
    if (!clientName.trim()) return;
    if (editingClient) {
      updateClient.mutate({ id: editingClient.id, name: clientName.trim() });
    } else {
      createClient.mutate(clientName.trim());
    }
    setClientDialogOpen(false);
  }

  function openNewProject() {
    setEditingProject(null);
    setProjectName("");
    setProjectClientId(activeClients[0]?.id ?? "");
    setProjectColor(randomColor());
    setProjectTagIds([]);
    setProjectDialogOpen(true);
  }
  function openEditProject(project: Project) {
    setEditingProject(project);
    setProjectName(project.name);
    setProjectClientId(project.clientId);
    setProjectColor(project.color);
    setProjectTagIds(project.tags?.map((t) => t.tagId) ?? []);
    setProjectDialogOpen(true);
  }
  function toggleProjectTag(tagId: string) {
    setProjectTagIds((prev) =>
      prev.includes(tagId) ? prev.filter((id) => id !== tagId) : [...prev, tagId],
    );
  }

  function submitProject() {
    if (!projectName.trim() || !projectClientId) return;
    if (editingProject) {
      updateProject.mutate({
        id: editingProject.id,
        name: projectName.trim(),
        clientId: projectClientId,
        color: projectColor,
        tagIds: projectTagIds,
      });
    } else {
      createProject.mutate({
        clientId: projectClientId,
        name: projectName.trim(),
        color: projectColor,
        tagIds: projectTagIds.length > 0 ? projectTagIds : undefined,
      });
    }
    setProjectDialogOpen(false);
  }

  const SectionHeading = embedded ? "h3" : "h2";
  const selectableClients = clients.filter(
    (client) => !client.archived || client.id === projectClientId,
  );

  return (
    <div className={embedded ? "space-y-8" : "mx-auto max-w-3xl space-y-8 p-4 sm:p-6"}>
      {/* Clients */}
      <section>
        <div className="mb-3 flex items-center justify-between">
          <SectionHeading className="text-lg font-semibold">クライアント</SectionHeading>
          <Button size="sm" onClick={openNewClient}>
            <Plus className="mr-1 h-4 w-4" /> 追加
          </Button>
        </div>
        {clients.length === 0 ? (
          <p className="text-sm text-neutral-400">クライアントがありません</p>
        ) : (
          <ul className="divide-y divide-neutral-100 rounded-md border border-neutral-200">
            {orderedClients.map((client) => (
              <li
                key={client.id}
                className={`flex items-center justify-between px-4 py-2 ${
                  client.archived ? "bg-neutral-50 text-neutral-500" : ""
                }`}
              >
                <div className="flex items-center gap-2">
                  <span className="text-sm">{client.name}</span>
                  {client.archived && (
                    <span className="rounded-full bg-neutral-200 px-2 py-0.5 text-[10px] font-medium text-neutral-600">
                      アーカイブ済み
                    </span>
                  )}
                </div>
                <div className="flex gap-1">
                  <button
                    type="button"
                    onClick={() => openEditClient(client)}
                    aria-label={`${client.name}を編集`}
                    title="編集"
                    className="rounded p-1 text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700"
                  >
                    <Pencil className="h-3.5 w-3.5" />
                  </button>
                  <button
                    type="button"
                    onClick={() =>
                      setClientArchived.mutate({
                        id: client.id,
                        archived: !client.archived,
                      })
                    }
                    aria-label={`${client.name}を${client.archived ? "復元" : "アーカイブ"}`}
                    title={client.archived ? "復元" : "アーカイブ"}
                    className="rounded p-1 text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700"
                  >
                    {client.archived ? (
                      <ArchiveRestore className="h-3.5 w-3.5" />
                    ) : (
                      <Archive className="h-3.5 w-3.5" />
                    )}
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>

      {/* Projects */}
      <section>
        <div className="mb-3 flex items-center justify-between">
          <SectionHeading className="text-lg font-semibold">プロジェクト</SectionHeading>
          <Button size="sm" onClick={openNewProject} disabled={activeClients.length === 0}>
            <Plus className="mr-1 h-4 w-4" /> 追加
          </Button>
        </div>
        {projects.length === 0 ? (
          <p className="text-sm text-neutral-400">プロジェクトがありません</p>
        ) : (
          <ul className="divide-y divide-neutral-100 rounded-md border border-neutral-200">
            {orderedProjects.map((project) => (
              <li
                key={project.id}
                className={`flex items-center justify-between px-4 py-2 ${
                  project.archived ? "bg-neutral-50 text-neutral-500" : ""
                }`}
              >
                <div className="flex items-center gap-2">
                  <span
                    className="inline-block h-3 w-3 rounded-sm"
                    style={{ background: project.color }}
                  />
                  <span className="text-sm">
                    {project.client.name} / {project.name}
                  </span>
                  {project.archived && (
                    <span className="rounded-full bg-neutral-200 px-2 py-0.5 text-[10px] font-medium text-neutral-600">
                      アーカイブ済み
                    </span>
                  )}
                  {project.tags && project.tags.length > 0 && (
                    <div className="flex gap-1">
                      {project.tags.map((t) => (
                        <span
                          key={t.tagId}
                          className="rounded-full px-1.5 py-0.5 text-[10px] text-white"
                          style={{ backgroundColor: t.tag.color }}
                        >
                          {t.tag.name}
                        </span>
                      ))}
                    </div>
                  )}
                </div>
                <div className="flex gap-1">
                  <button
                    type="button"
                    onClick={() => openEditProject(project)}
                    aria-label={`${project.name}を編集`}
                    title="編集"
                    className="rounded p-1 text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700"
                  >
                    <Pencil className="h-3.5 w-3.5" />
                  </button>
                  <button
                    type="button"
                    onClick={() =>
                      setProjectArchived.mutate({
                        id: project.id,
                        archived: !project.archived,
                      })
                    }
                    aria-label={`${project.name}を${project.archived ? "復元" : "アーカイブ"}`}
                    title={
                      project.archived && project.client.archived
                        ? "クライアントを先に復元してください"
                        : project.archived
                          ? "復元"
                          : "アーカイブ"
                    }
                    disabled={project.archived && project.client.archived}
                    className="rounded p-1 text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700 disabled:cursor-not-allowed disabled:opacity-40"
                  >
                    {project.archived ? (
                      <ArchiveRestore className="h-3.5 w-3.5" />
                    ) : (
                      <Archive className="h-3.5 w-3.5" />
                    )}
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>

      {/* Tags */}
      <section>
        <div className="mb-3 flex items-center justify-between">
          <SectionHeading className="text-lg font-semibold">タグ</SectionHeading>
          <Button size="sm" onClick={openNewTag}>
            <Plus className="mr-1 h-4 w-4" /> 追加
          </Button>
        </div>
        {tags.length === 0 ? (
          <p className="text-sm text-neutral-400">タグがありません</p>
        ) : (
          <ul className="divide-y divide-neutral-100 rounded-md border border-neutral-200">
            {tags.map((tag) => (
              <li key={tag.id} className="flex items-center justify-between px-4 py-2">
                <div className="flex items-center gap-2">
                  <span
                    className="inline-block h-3 w-3 rounded-full"
                    style={{ background: tag.color }}
                  />
                  <span className="text-sm">{tag.name}</span>
                </div>
                <div className="flex gap-1">
                  <button
                    type="button"
                    onClick={() => openEditTag(tag)}
                    aria-label={`${tag.name}を編集`}
                    title="編集"
                    className="rounded p-1 text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700"
                  >
                    <Pencil className="h-3.5 w-3.5" />
                  </button>
                  <button
                    type="button"
                    onClick={() => setDeletingTag(tag)}
                    aria-label={`${tag.name}を削除`}
                    title="削除"
                    className="rounded p-1 text-neutral-400 hover:bg-red-50 hover:text-red-600"
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>

      {/* Client Dialog */}
      <Dialog open={clientDialogOpen} onOpenChange={setClientDialogOpen}>
        <div>
          <DialogHeader>
            <DialogTitle>{editingClient ? "クライアント編集" : "クライアント追加"}</DialogTitle>
          </DialogHeader>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              submitClient();
            }}
            className="space-y-4"
          >
            <div className="space-y-1">
              <Label htmlFor="client-name">名前</Label>
              <Input
                id="client-name"
                value={clientName}
                onChange={(e) => setClientName(e.target.value)}
                autoFocus
              />
            </div>
            <Button type="submit" className="w-full">
              {editingClient ? "更新" : "作成"}
            </Button>
          </form>
        </div>
      </Dialog>

      {/* Project Dialog */}
      <Dialog open={projectDialogOpen} onOpenChange={setProjectDialogOpen}>
        <div>
          <DialogHeader>
            <DialogTitle>{editingProject ? "プロジェクト編集" : "プロジェクト追加"}</DialogTitle>
          </DialogHeader>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              submitProject();
            }}
            className="space-y-4"
          >
            <div className="space-y-1">
              <Label htmlFor="project-client">クライアント</Label>
              <select
                id="project-client"
                className="w-full rounded-md border border-neutral-200 px-3 py-2 text-sm"
                value={projectClientId}
                onChange={(e) => setProjectClientId(e.target.value)}
              >
                {selectableClients.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.name}
                  </option>
                ))}
              </select>
            </div>
            <div className="space-y-1">
              <Label htmlFor="project-name">名前</Label>
              <Input
                id="project-name"
                value={projectName}
                onChange={(e) => setProjectName(e.target.value)}
              />
            </div>
            <div className="space-y-1">
              <div className="text-xs font-medium text-neutral-700">カラー</div>
              <div
                role="group"
                aria-label="プロジェクトカラー"
                className="flex flex-wrap gap-1.5"
              >
                {PROJECT_COLORS.map((c) => (
                  <button
                    key={c}
                    type="button"
                    onClick={() => setProjectColor(c)}
                    aria-label={`プロジェクトカラー ${c}`}
                    aria-pressed={projectColor === c}
                    className={`h-7 w-7 rounded-md border-2 ${
                      projectColor === c ? "border-neutral-900" : "border-transparent"
                    }`}
                    style={{ background: c }}
                  />
                ))}
              </div>
            </div>
            {tags.length > 0 && (
              <div className="space-y-1">
                <div className="text-xs font-medium text-neutral-700">タグ</div>
                <div
                  role="group"
                  aria-label="プロジェクトのタグ"
                  className="flex flex-wrap gap-1.5"
                >
                  {tags.map((tag) => (
                    <button
                      key={tag.id}
                      type="button"
                      onClick={() => toggleProjectTag(tag.id)}
                      aria-pressed={projectTagIds.includes(tag.id)}
                      className={`rounded-full border px-2.5 py-1 text-xs transition-colors ${
                        projectTagIds.includes(tag.id)
                          ? "border-transparent text-white"
                          : "border-neutral-200 text-neutral-600 hover:bg-neutral-50"
                      }`}
                      style={
                        projectTagIds.includes(tag.id) ? { backgroundColor: tag.color } : undefined
                      }
                    >
                      {tag.name}
                    </button>
                  ))}
                </div>
              </div>
            )}
            <Button type="submit" className="w-full">
              {editingProject ? "更新" : "作成"}
            </Button>
          </form>
        </div>
      </Dialog>
      {/* Tag Dialog */}
      <Dialog open={tagDialogOpen} onOpenChange={setTagDialogOpen}>
        <div>
          <DialogHeader>
            <DialogTitle>{editingTag ? "タグ編集" : "タグ追加"}</DialogTitle>
          </DialogHeader>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              submitTag();
            }}
            className="space-y-4"
          >
            <div className="space-y-1">
              <Label htmlFor="tag-name">名前</Label>
              <Input
                id="tag-name"
                value={tagName}
                onChange={(e) => setTagName(e.target.value)}
                autoFocus
              />
            </div>
            <div className="space-y-1">
              <div className="text-xs font-medium text-neutral-700">カラー</div>
              <div
                role="group"
                aria-label="タグカラー"
                className="flex flex-wrap gap-1.5"
              >
                {PROJECT_COLORS.map((c) => (
                  <button
                    key={c}
                    type="button"
                    onClick={() => setTagColor(c)}
                    aria-label={`タグカラー ${c}`}
                    aria-pressed={tagColor === c}
                    className={`h-7 w-7 rounded-full border-2 ${
                      tagColor === c ? "border-neutral-900" : "border-transparent"
                    }`}
                    style={{ background: c }}
                  />
                ))}
              </div>
            </div>
            <Button type="submit" className="w-full">
              {editingTag ? "更新" : "作成"}
            </Button>
          </form>
        </div>
      </Dialog>

      <Dialog
        open={!!deletingTag}
        onOpenChange={(open) => {
          if (!open && !deleteTag.isPending) setDeletingTag(null);
        }}
      >
        {deletingTag && (
          <div>
            <DialogHeader>
              <DialogTitle>タグを削除しますか？</DialogTitle>
            </DialogHeader>
            <p className="text-sm text-neutral-600">
              「{deletingTag.name}」を削除します。記録した時間は残りますが、エントリーとプロジェクトからこのタグが外れます。
            </p>
            <DialogFooter>
              <Button
                variant="ghost"
                onClick={() => setDeletingTag(null)}
                disabled={deleteTag.isPending}
              >
                キャンセル
              </Button>
              <Button
                variant="destructive"
                onClick={() => deleteTag.mutate(deletingTag.id)}
                disabled={deleteTag.isPending}
              >
                {deleteTag.isPending ? "削除中…" : "タグを削除"}
              </Button>
            </DialogFooter>
          </div>
        )}
      </Dialog>
    </div>
  );
}
