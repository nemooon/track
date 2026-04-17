import { useState, useEffect } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { apiFetch } from "@/lib/fetcher";
import type { UserProfile } from "@/types";

const DAY_LABELS = ["日", "月", "火", "水", "木", "金", "土"];

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

  // Work schedule form
  const [workStart, setWorkStart] = useState(9);
  const [workEnd, setWorkEnd] = useState(18);
  const [workDays, setWorkDays] = useState<number[]>([1, 2, 3, 4, 5]);
  useEffect(() => {
    if (user) {
      setWorkStart(user.workStart);
      setWorkEnd(user.workEnd);
      setWorkDays(user.workDays);
    }
  }, [user]);

  // Password form
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");

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

  const changePassword = useMutation({
    mutationFn: (data: { currentPassword: string; newPassword: string }) =>
      apiFetch("/api/account/password", { method: "POST", body: JSON.stringify(data) }),
    onSuccess: () => {
      setCurrentPassword("");
      setNewPassword("");
      toast.success("パスワードを変更しました");
    },
    onError: (err) => {
      if ((err as Error).message.includes("wrong_password")) {
        toast.error("現在のパスワードが違います");
      } else {
        toast.error("変更に失敗しました");
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
    <div className="mx-auto max-w-xl space-y-8 p-6">
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
              <select
                className="rounded-md border border-neutral-200 px-3 py-2 text-sm"
                value={workStart}
                onChange={(e) => setWorkStart(Number(e.target.value))}
              >
                {Array.from({ length: 24 }, (_, i) => (
                  <option key={i} value={i}>{`${i}:00`}</option>
                ))}
              </select>
            </div>
            <div className="space-y-1">
              <Label>定時終了</Label>
              <select
                className="rounded-md border border-neutral-200 px-3 py-2 text-sm"
                value={workEnd}
                onChange={(e) => setWorkEnd(Number(e.target.value))}
              >
                {Array.from({ length: 24 }, (_, i) => (
                  <option key={i} value={i}>{`${i}:00`}</option>
                ))}
              </select>
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

      {/* Password */}
      <section>
        <h2 className="mb-4 text-lg font-semibold">パスワード変更</h2>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            changePassword.mutate({ currentPassword, newPassword });
          }}
          className="space-y-3"
        >
          <div className="space-y-1">
            <Label>現在のパスワード</Label>
            <Input
              type="password"
              value={currentPassword}
              onChange={(e) => setCurrentPassword(e.target.value)}
              required
            />
          </div>
          <div className="space-y-1">
            <Label>新しいパスワード (8文字以上)</Label>
            <Input
              type="password"
              value={newPassword}
              onChange={(e) => setNewPassword(e.target.value)}
              required
              minLength={8}
            />
          </div>
          <Button type="submit" size="sm">変更</Button>
        </form>
      </section>
    </div>
  );
}
