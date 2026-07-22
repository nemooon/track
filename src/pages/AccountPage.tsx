import { useState, useEffect } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { apiFetch } from "@/lib/fetcher";
import type { UserProfile } from "@/types";
import {
  clearWorkspaceHandle,
  getWorkspaceHandle,
  pickWorkspaceFolder,
  workspaceSupported,
} from "@/lib/workspaceHandle";

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

export function AccountPage() {
  const qc = useQueryClient();
  const { data: user, isLoading } = useQuery({
    queryKey: ["account"],
    queryFn: () => apiFetch<UserProfile>("/api/account"),
  });

  // Profile form
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  useEffect(() => {
    if (user) {
      setName(user.name ?? "");
      setEmail(user.email);
    }
  }, [user]);

  // Work schedule form (minutes since midnight)
  const [workStart, setWorkStart] = useState(600);
  const [workEnd, setWorkEnd] = useState(1110);
  const [workDays, setWorkDays] = useState<number[]>([1, 2, 3, 4, 5]);
  useEffect(() => {
    if (user) {
      setWorkStart(user.workStart);
      setWorkEnd(user.workEnd);
      setWorkDays(user.workDays);
    }
  }, [user]);

  const updateProfile = useMutation({
    mutationFn: (data: { name?: string; email?: string }) =>
      apiFetch("/api/account", { method: "PATCH", body: JSON.stringify(data) }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["account"] });
      toast.success("プロフィールを更新しました");
    },
    onError: (err) => {
      if ((err as Error).message.includes("email_taken")) {
        toast.error("このメールアドレスは既に使われています");
      } else {
        toast.error("更新に失敗しました");
      }
    },
  });

  const updateSettings = useMutation({
    mutationFn: (data: { workStart?: number; workEnd?: number; workDays?: number[] }) =>
      apiFetch("/api/account/settings", { method: "PATCH", body: JSON.stringify(data) }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["account"] });
      toast.success("勤務設定を更新しました");
    },
    onError: () => toast.error("更新に失敗しました"),
  });

  // Workspace folder
  const wsSupported = workspaceSupported();
  const [wsHandle, setWsHandle] = useState<FileSystemDirectoryHandle | null>(null);
  useEffect(() => {
    if (!wsSupported) return;
    getWorkspaceHandle().then(setWsHandle);
  }, [wsSupported]);

  async function pickWorkspace() {
    const handle = await pickWorkspaceFolder();
    if (!handle) return;
    setWsHandle(handle);
    qc.invalidateQueries({ queryKey: ["workspace"] });
    toast.success(`Workspace を「${handle.name}」に設定しました`);
  }

  async function clearWorkspace() {
    await clearWorkspaceHandle();
    setWsHandle(null);
    qc.invalidateQueries({ queryKey: ["workspace"] });
    toast.success("Workspace の設定を解除しました");
  }

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
      {/* Profile */}
      <section>
        <h2 className="mb-4 text-lg font-semibold">プロフィール</h2>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            updateProfile.mutate({ name: name || undefined, email });
          }}
          className="space-y-3"
        >
          <div className="space-y-1">
            <Label>名前</Label>
            <Input value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <div className="space-y-1">
            <Label>メールアドレス</Label>
            <Input type="email" value={email} onChange={(e) => setEmail(e.target.value)} required />
          </div>
          <Button type="submit" size="sm">保存</Button>
        </form>
      </section>

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

      {/* Workspace */}
      <section>
        <h2 className="mb-2 text-lg font-semibold">Workspace ヒント</h2>
        <p className="mb-3 text-sm text-neutral-500">
          ローカルの Workspace フォルダを指定すると、ファイル変更・git コミットの時刻からカレンダーに作業ヒントが表示されます。
        </p>
        {!wsSupported ? (
          <div className="rounded-md border border-neutral-200 bg-neutral-50 px-3 py-2 text-sm text-neutral-500">
            このブラウザは未対応です（Chrome / Edge をお使いください）
          </div>
        ) : (
          <div className="flex items-center justify-between rounded-md border border-neutral-200 px-3 py-2">
            <div className="text-sm">
              <span className="text-neutral-500">フォルダ: </span>
              <span className="font-medium text-neutral-700">
                {wsHandle ? wsHandle.name : "未設定"}
              </span>
            </div>
            <div className="flex gap-2">
              <Button variant="outline" size="sm" onClick={pickWorkspace}>
                {wsHandle ? "変更" : "選択"}
              </Button>
              {wsHandle && (
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-red-600 hover:text-red-700"
                  onClick={clearWorkspace}
                >
                  解除
                </Button>
              )}
            </div>
          </div>
        )}
      </section>

    </div>
  );
}
