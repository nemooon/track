import { useState, useEffect } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  startRegistration,
  type PublicKeyCredentialCreationOptionsJSON,
} from "@simplewebauthn/browser";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { apiFetch } from "@/lib/fetcher";
import type { UserProfile } from "@/types";

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

type PasskeyInfo = { id: string; createdAt: string };
type InvitationInfo = { id: string; email: string; createdAt: string };

export function AccountPage() {
  const qc = useQueryClient();
  const { data: user, isLoading } = useQuery({
    queryKey: ["account"],
    queryFn: () => apiFetch<UserProfile>("/api/account"),
  });

  const { data: passkeys = [] } = useQuery({
    queryKey: ["passkeys"],
    queryFn: () => apiFetch<PasskeyInfo[]>("/api/passkeys"),
  });

  const { data: myInvitations = [] } = useQuery({
    queryKey: ["invitations"],
    queryFn: () => apiFetch<InvitationInfo[]>("/api/invitations"),
  });

  const [inviteEmail, setInviteEmail] = useState("");

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

  const addPasskey = useMutation({
    mutationFn: async () => {
      const options = await apiFetch("/api/passkeys/register-options", {
        method: "POST",
        body: "{}",
      });
      const credential = await startRegistration({
        optionsJSON: options as PublicKeyCredentialCreationOptionsJSON,
      });
      return apiFetch("/api/passkeys/register-verify", {
        method: "POST",
        body: JSON.stringify(credential),
      });
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["passkeys"] });
      toast.success("パスキーを追加しました");
    },
    onError: (err) => {
      const msg = (err as Error).message;
      if (msg.includes("NotAllowedError") || msg.includes("cancelled")) {
        return; // user cancelled, no toast needed
      }
      toast.error("パスキーの追加に失敗しました");
    },
  });

  const deletePasskey = useMutation({
    mutationFn: (id: string) =>
      apiFetch(`/api/passkeys/${id}`, { method: "DELETE" }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["passkeys"] });
      toast.success("パスキーを削除しました");
    },
    onError: (err) => {
      if ((err as Error).message.includes("last_credential")) {
        toast.error("最後のパスキーは削除できません");
      } else {
        toast.error("削除に失敗しました");
      }
    },
  });

  const sendInvite = useMutation({
    mutationFn: (email: string) =>
      apiFetch("/api/invitations", { method: "POST", body: JSON.stringify({ email }) }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["invitations"] });
      setInviteEmail("");
      toast.success("招待しました");
    },
    onError: (err) => {
      const msg = (err as Error).message;
      if (msg.includes("already_registered")) {
        toast.error("このメールアドレスは既に登録済みです");
      } else if (msg.includes("already_invited")) {
        toast.error("このメールアドレスは既に招待済みです");
      } else {
        toast.error("招待に失敗しました");
      }
    },
  });

  const deleteInvite = useMutation({
    mutationFn: (id: string) =>
      apiFetch(`/api/invitations/${id}`, { method: "DELETE" }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["invitations"] });
      toast.success("招待を取り消しました");
    },
    onError: () => toast.error("取り消しに失敗しました"),
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

      {/* Passkeys */}
      <section>
        <h2 className="mb-4 text-lg font-semibold">パスキー</h2>
        <div className="space-y-3">
          {passkeys.map((pk) => (
            <div
              key={pk.id}
              className="flex items-center justify-between rounded-md border border-neutral-200 px-3 py-2"
            >
              <span className="text-sm text-neutral-700">
                登録日: {new Date(pk.createdAt).toLocaleDateString("ja-JP")}
              </span>
              <Button
                variant="ghost"
                size="sm"
                className="text-red-600 hover:text-red-700"
                onClick={() => deletePasskey.mutate(pk.id)}
                disabled={passkeys.length <= 1}
              >
                削除
              </Button>
            </div>
          ))}
          {passkeys.length === 0 && (
            <p className="text-sm text-neutral-500">パスキーが登録されていません</p>
          )}
          <Button
            variant="outline"
            size="sm"
            onClick={() => addPasskey.mutate()}
            disabled={addPasskey.isPending}
          >
            パスキーを追加
          </Button>
        </div>
      </section>

      {/* Invitations */}
      <section>
        <h2 className="mb-4 text-lg font-semibold">メンバー招待</h2>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (inviteEmail) sendInvite.mutate(inviteEmail);
          }}
          className="flex gap-2"
        >
          <Input
            type="email"
            placeholder="メールアドレス"
            value={inviteEmail}
            onChange={(e) => setInviteEmail(e.target.value)}
            required
            className="flex-1"
          />
          <Button type="submit" size="sm" disabled={sendInvite.isPending}>
            招待
          </Button>
        </form>
        {myInvitations.length > 0 && (
          <div className="mt-3 space-y-2">
            {myInvitations.map((inv) => (
              <div
                key={inv.id}
                className="flex items-center justify-between rounded-md border border-neutral-200 px-3 py-2"
              >
                <span className="text-sm text-neutral-700">{inv.email}</span>
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-red-600 hover:text-red-700"
                  onClick={() => deleteInvite.mutate(inv.id)}
                >
                  取消
                </Button>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
