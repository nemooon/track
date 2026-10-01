import { lazy, Suspense } from "react";
import { useQuery } from "@tanstack/react-query";
import { ExternalLink } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { toast } from "sonner";
import { getReleaseNotes } from "@client/lib/appUpdate";

const ignoreChanges = () => {};
const MarkdownEditor = lazy(() => import("@client/components/MarkdownEditor").then(
  (module) => ({ default: module.MarkdownEditor }),
));

function openExternalLink(event: React.MouseEvent<HTMLElement>, url: string) {
  if (!("__TAURI_INTERNALS__" in window)) return;
  event.preventDefault();
  void openUrl(url).catch(() => toast.error("リンクを開けませんでした"));
}

export function ReleaseNotes({ version, preview = false }: {
  version: string;
  preview?: boolean;
}) {
  const { data, isPending, isError, refetch } = useQuery({
    queryKey: ["release-notes", version, preview],
    queryFn: ({ signal }) => preview ? Promise.resolve({
      version,
      body: "## 新機能\n\n- アプリ内でリリースノートを確認できます。\n\n## 改善\n\n- アップデート前に変更内容を確認できます。",
      url: "https://github.com/nemooon/track/releases",
    }) : getReleaseNotes(version, signal),
    staleTime: 5 * 60 * 1000,
    retry: 1,
  });

  return (
    <section className="min-h-0 text-sm text-neutral-600" aria-label={`バージョン ${version} のリリースノート`}>
      {isPending ? (
        <p role="status" className="py-6 text-center text-neutral-400">リリースノートを読み込んでいます…</p>
      ) : isError ? (
        <div role="alert" className="space-y-2 rounded-md bg-neutral-50 p-4">
          <p>リリースノートを取得できませんでした。</p>
          <button type="button" onClick={() => void refetch()} className="underline underline-offset-4 hover:text-neutral-900">再読み込み</button>
        </div>
      ) : data?.body ? (
        <div onClickCapture={(event) => {
          const link = (event.target as HTMLElement).closest("a");
          if (!link) return;
          const url = link.getAttribute("href") ?? "";
          // 外部の本文に含まれるリンクはHTTPSだけを開く。
          if (!url.startsWith("https://")) {
            event.preventDefault();
            return;
          }
          link.target = "_blank";
          link.rel = "noreferrer";
          openExternalLink(event, url);
        }}>
          <Suspense fallback={<p role="status" className="py-6 text-center text-neutral-400">本文を表示しています…</p>}>
            <MarkdownEditor
              key={`${version}:${data.body}`}
              value={data.body}
              onChange={ignoreChanges}
              readOnly
              showToolbar={false}
              ariaLabel="リリースノート本文"
              className="release-notes-markdown max-h-[40vh] min-h-20 rounded-md border border-neutral-200"
            />
          </Suspense>
        </div>
      ) : (
        <p className="py-6 text-center text-neutral-400">このバージョンのリリースノートはまだ公開されていません。</p>
      )}
      <a
        href={data?.url ?? `https://github.com/nemooon/track/releases/tag/${encodeURIComponent(`v${version}`)}`}
        target="_blank"
        rel="noreferrer"
        onClick={(event) => openExternalLink(event, event.currentTarget.href)}
        className="mt-3 inline-flex items-center gap-1.5 text-xs text-neutral-500 underline-offset-4 hover:text-neutral-900 hover:underline"
      >
        <ExternalLink className="size-3.5" />
        GitHubでリリースを開く
      </a>
    </section>
  );
}
