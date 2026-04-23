import { useState } from "react";
import { Link, useNavigate } from "react-router";
import {
  startRegistration,
  type PublicKeyCredentialCreationOptionsJSON,
} from "@simplewebauthn/browser";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { apiFetch } from "@/lib/fetcher";
import { useAuth } from "@/lib/auth";

export function SignupPage() {
  const navigate = useNavigate();
  const { refresh } = useAuth();
  const [email, setEmail] = useState("");
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    setLoading(true);
    setError(null);
    try {
      const options = await apiFetch("/api/auth/signup/register-options", {
        method: "POST",
        body: JSON.stringify({ email, name: name || undefined }),
      });

      const credential = await startRegistration({
        optionsJSON: options as PublicKeyCredentialCreationOptionsJSON,
      });

      await apiFetch("/api/auth/signup/register-verify", {
        method: "POST",
        body: JSON.stringify(credential),
      });

      await refresh();
      navigate("/calendar", { replace: true });
    } catch (err) {
      const msg = (err as Error).message;
      // Try to parse the server error JSON
      let errorCode = "";
      let zodIssues: { formErrors?: string[]; fieldErrors?: Record<string, string[]> } | null = null;
      try {
        const parsed = JSON.parse(msg);
        errorCode = parsed.error ?? "";
        zodIssues = parsed.issues ?? null;
      } catch {
        // Not JSON — use raw message
        errorCode = msg;
      }

      if (errorCode === "email_taken") {
        setError("このメールアドレスは既に登録されています");
      } else if (errorCode === "not_invited") {
        setError("このメールアドレスは招待されていません");
      } else if (errorCode === "invalid_input" && zodIssues) {
        const fieldMsgs = Object.values(zodIssues.fieldErrors ?? {}).flat();
        const allMsgs = [...(zodIssues.formErrors ?? []), ...fieldMsgs];
        setError(allMsgs[0] ?? "入力内容に誤りがあります");
      } else if (msg.includes("NotAllowedError") || msg.includes("cancelled")) {
        setError("パスキーの登録がキャンセルされました");
      } else {
        setError("登録に失敗しました。パスキーに対応したデバイスをお使いください");
      }
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="flex min-h-svh items-center justify-center bg-neutral-50 px-4">
      <form
        onSubmit={onSubmit}
        className="w-full max-w-sm space-y-4 rounded-lg bg-white p-6 shadow"
      >
        <h1 className="text-xl font-semibold">新規登録</h1>
        <p className="text-sm text-neutral-600">
          招待されたメールアドレスで登録できます
        </p>
        <div className="space-y-1">
          <Label htmlFor="name">名前（任意）</Label>
          <Input
            id="name"
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </div>
        <div className="space-y-1">
          <Label htmlFor="email">メールアドレス</Label>
          <Input
            id="email"
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
          />
        </div>
        {error && <p className="text-sm text-red-600">{error}</p>}
        <Button type="submit" className="w-full" disabled={loading}>
          {loading ? "..." : "パスキーで登録"}
        </Button>
        <p className="text-center text-xs text-neutral-500">
          アカウントがある?{" "}
          <Link to="/login" className="underline">
            ログイン
          </Link>
        </p>
      </form>
    </div>
  );
}
