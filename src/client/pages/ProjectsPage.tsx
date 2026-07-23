import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { Plus, Pencil, Trash2 } from "lucide-react";
import { Button } from "@client/components/ui/button";
import { Input } from "@client/components/ui/input";
import { Label } from "@client/components/ui/label";
import { Dialog, DialogHeader, DialogTitle } from "@client/components/ui/dialog";
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

  const deleteClient = useMutation({
    mutationFn: (id: string) => apiFetch(`/api/clients/${id}`, { method: "DELETE" }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["clients"] });
      toast.success("クライアントを削除しました");
    },
    onError: () => toast.error("削除に失敗しました"),
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

  const deleteProject = useMutation({
    mutationFn: (id: string) => apiFetch(`/api/projects/${id}`, { method: "DELETE" }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["projects"] });
      toast.success("プロジェクトを削除しました");
    },
    onError: (err) => {
      if ((err as Error).message.includes("project_has_entries")) {
        toast.error("エントリーがあるプロジェクトは削除できません");
      } else {
        toast.error("削除に失敗しました");
      }
    },
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
    setProjectClientId(clients[0]?.id ?? "");
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

  return (
    <div className={embedded ? "space-y-8" : "mx-auto max-w-3xl space-y-8 p-4 sm:p-6"}>
      {/* Clients */}
      <section>
        <div className="mb-3 flex items-center justify-between">
          <h2 className="text-lg font-semibold">クライアント</h2>
          <Button size="sm" onClick={openNewClient}>
            <Plus className="mr-1 h-4 w-4" /> 追加
          </Button>
        </div>
        {clients.length === 0 ? (
          <p className="text-sm text-neutral-400">クライアントがありません</p>
        ) : (
          <ul className="divide-y divide-neutral-100 rounded-md border border-neutral-200">
            {clients.map((client) => (
              <li key={client.id} className="flex items-center justify-between px-4 py-2">
                <span className="text-sm">{client.name}</span>
                <div className="flex gap-1">
                  <button
                    onClick={() => openEditClient(client)}
                    className="rounded p-1 text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700"
                  >
                    <Pencil className="h-3.5 w-3.5" />
                  </button>
                  <button
                    onClick={() => {
                      if (confirm(`「${client.name}」を削除しますか？`)) {
                        deleteClient.mutate(client.id);
                      }
                    }}
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

      {/* Projects */}
      <section>
        <div className="mb-3 flex items-center justify-between">
          <h2 className="text-lg font-semibold">プロジェクト</h2>
          <Button size="sm" onClick={openNewProject} disabled={clients.length === 0}>
            <Plus className="mr-1 h-4 w-4" /> 追加
          </Button>
        </div>
        {projects.length === 0 ? (
          <p className="text-sm text-neutral-400">プロジェクトがありません</p>
        ) : (
          <ul className="divide-y divide-neutral-100 rounded-md border border-neutral-200">
            {projects.map((project) => (
              <li key={project.id} className="flex items-center justify-between px-4 py-2">
                <div className="flex items-center gap-2">
                  <span
                    className="inline-block h-3 w-3 rounded-sm"
                    style={{ background: project.color }}
                  />
                  <span className="text-sm">
                    {project.client.name} / {project.name}
                  </span>
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
                    onClick={() => openEditProject(project)}
                    className="rounded p-1 text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700"
                  >
                    <Pencil className="h-3.5 w-3.5" />
                  </button>
                  <button
                    onClick={() => {
                      if (confirm(`「${project.name}」を削除しますか？`)) {
                        deleteProject.mutate(project.id);
                      }
                    }}
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

      {/* Tags */}
      <section>
        <div className="mb-3 flex items-center justify-between">
          <h2 className="text-lg font-semibold">タグ</h2>
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
                    onClick={() => openEditTag(tag)}
                    className="rounded p-1 text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700"
                  >
                    <Pencil className="h-3.5 w-3.5" />
                  </button>
                  <button
                    onClick={() => {
                      if (confirm(`「${tag.name}」を削除しますか？`)) {
                        deleteTag.mutate(tag.id);
                      }
                    }}
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
              <Label>名前</Label>
              <Input value={clientName} onChange={(e) => setClientName(e.target.value)} autoFocus />
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
              <Label>クライアント</Label>
              <select
                className="w-full rounded-md border border-neutral-200 px-3 py-2 text-sm"
                value={projectClientId}
                onChange={(e) => setProjectClientId(e.target.value)}
              >
                {clients.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.name}
                  </option>
                ))}
              </select>
            </div>
            <div className="space-y-1">
              <Label>名前</Label>
              <Input value={projectName} onChange={(e) => setProjectName(e.target.value)} />
            </div>
            <div className="space-y-1">
              <Label>カラー</Label>
              <div className="flex flex-wrap gap-1.5">
                {PROJECT_COLORS.map((c) => (
                  <button
                    key={c}
                    type="button"
                    onClick={() => setProjectColor(c)}
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
                <Label>タグ</Label>
                <div className="flex flex-wrap gap-1.5">
                  {tags.map((tag) => (
                    <button
                      key={tag.id}
                      type="button"
                      onClick={() => toggleProjectTag(tag.id)}
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
              <Label>名前</Label>
              <Input value={tagName} onChange={(e) => setTagName(e.target.value)} autoFocus />
            </div>
            <div className="space-y-1">
              <Label>カラー</Label>
              <div className="flex flex-wrap gap-1.5">
                {PROJECT_COLORS.map((c) => (
                  <button
                    key={c}
                    type="button"
                    onClick={() => setTagColor(c)}
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
    </div>
  );
}
