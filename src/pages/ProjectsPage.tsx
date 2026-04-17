import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { Plus, Pencil, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Dialog, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { apiFetch } from "@/lib/fetcher";
import { PROJECT_COLORS, randomColor } from "@/lib/utils";
import type { Client, Project } from "@/types";

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

/* ── Page ─────────────────────────────────────────────── */

export function ProjectsPage() {
  const qc = useQueryClient();
  const { data: clients = [] } = useClients();
  const { data: projects = [] } = useProjects();

  const [clientDialogOpen, setClientDialogOpen] = useState(false);
  const [editingClient, setEditingClient] = useState<Client | null>(null);
  const [clientName, setClientName] = useState("");

  const [projectDialogOpen, setProjectDialogOpen] = useState(false);
  const [editingProject, setEditingProject] = useState<Project | null>(null);
  const [projectName, setProjectName] = useState("");
  const [projectClientId, setProjectClientId] = useState("");
  const [projectColor, setProjectColor] = useState(randomColor());

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
    mutationFn: (data: { clientId: string; name: string; color: string }) =>
      apiFetch("/api/projects", { method: "POST", body: JSON.stringify(data) }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["projects"] });
      toast.success("プロジェクトを作成しました");
    },
  });

  const updateProject = useMutation({
    mutationFn: ({ id, ...data }: { id: string; name?: string; color?: string; clientId?: string }) =>
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
    setProjectDialogOpen(true);
  }
  function openEditProject(project: Project) {
    setEditingProject(project);
    setProjectName(project.name);
    setProjectClientId(project.clientId);
    setProjectColor(project.color);
    setProjectDialogOpen(true);
  }
  function submitProject() {
    if (!projectName.trim() || !projectClientId) return;
    if (editingProject) {
      updateProject.mutate({
        id: editingProject.id,
        name: projectName.trim(),
        clientId: projectClientId,
        color: projectColor,
      });
    } else {
      createProject.mutate({
        clientId: projectClientId,
        name: projectName.trim(),
        color: projectColor,
      });
    }
    setProjectDialogOpen(false);
  }

  return (
    <div className="mx-auto max-w-3xl space-y-8 p-6">
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
            <Button type="submit" className="w-full">
              {editingProject ? "更新" : "作成"}
            </Button>
          </form>
        </div>
      </Dialog>
    </div>
  );
}
