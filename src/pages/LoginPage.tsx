import { useState } from "react";
import { Link, useNavigate, useLocation } from "react-router";
import {
  startAuthentication,
  type PublicKeyCredentialRequestOptionsJSON,
} from "@simplewebauthn/browser";
import { Button } from "@/components/ui/button";
import { apiFetch } from "@/lib/fetcher";
import { useAuth } from "@/lib/auth";

export function LoginPage() {
  const navigate = useNavigate();
  const location = useLocation();
  const { refresh } = useAuth();
  const from = (location.state as { from?: string })?.from || "/calendar";

  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handleLogin() {
    setLoading(true);
    setError(null);
    try {
      // Step 1: Get authentication options
      const options = await apiFetch("/api/auth/login/login-options", {
        method: "POST",
        body: "{}",
      });

      // Step 2: Authenticate via browser WebAuthn API
      const credential = await startAuthentication({
        optionsJSON: options as PublicKeyCredentialRequestOptionsJSON,
      });

      // Step 3: Verify with server
      await apiFetch("/api/auth/login/login-verify", {
        method: "POST",
        body: JSON.stringify(credential),
      });

      await refresh();
      navigate(from, { replace: true });
    } catch (err) {
      const msg = (err as Error).message;
      if (msg.includes("NotAllowedError") || msg.includes("cancelled")) {
        setError("認証がキャンセルされました");
      } else if (msg.includes("credential_not_found")) {
        setError("このパスキーに対応するアカウントが見つかりません");
      } else {
        setError("ログインに失敗しました");
      }
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="flex min-h-svh items-center justify-center bg-neutral-50 px-4">
      <div className="w-full max-w-sm space-y-4 rounded-lg bg-white p-6 shadow">
        <h1 className="text-xl font-semibold">ログイン</h1>
        <p className="text-sm text-neutral-600">
          パスキーを使ってログインします
        </p>
        {error && <p className="text-sm text-red-600">{error}</p>}
        <Button
          onClick={handleLogin}
          className="w-full"
          disabled={loading}
        >
          {loading ? "..." : "パスキーでログイン"}
        </Button>
        <p className="text-center text-xs text-neutral-500">
          アカウントがない?{" "}
          <Link to="/signup" className="underline">
            新規登録
          </Link>
        </p>
      </div>
    </div>
  );
}
