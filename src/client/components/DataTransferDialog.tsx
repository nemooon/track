import { useRef, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { ArrowDownToLine, ArrowUpFromLine, FileJson2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@client/components/ui/button";
import {
  Dialog,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@client/components/ui/dialog";
import { apiFetch } from "@client/lib/fetcher";

interface DataTransferDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

interface PendingImport {
  name: string;
  json: unknown;
  exportedAt: string | null;
  counts: {
    clients: number;
    tags: number;
    projects: number;
    notes: number;
    entries: number;
  };
}

export function DataTransferDialog({
  open,
  onOpenChange,
}: DataTransferDialogProps) {
  const queryClient = useQueryClient();
  const fileRef = useRef<HTMLInputElement>(null);
  const [lastExport, setLastExport] = useState<string | null>(null);
  const [validatingImport, setValidatingImport] = useState(false);
  const [pending, setPending] = useState<PendingImport | null>(null);

  const runExport = useMutation({
    mutationFn: () =>
      apiFetch<{ path: string }>("/api/data/export/file", {
        method: "POST",
        body: "{}",
      }),
    onSuccess: (result) => {
      setLastExport(result.path);
      toast.success("データを書き出しました");
    },
    onError: () => toast.error("データを書き出せませんでした"),
  });

  function showImportError(error: unknown) {
    const message = (error as Error).message;
    if (message.includes("inconsistent_data")) {
      toast.error("参照が壊れているため読み込めません");
    } else if (message.includes("invalid_input")) {
      toast.error("Trackの書き出しデータではありません");
    } else {
      toast.error("ファイルを確認できませんでした");
    }
  }

  async function onPickImportFile(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;

    let json: unknown;
    try {
      json = JSON.parse(await file.text());
    } catch {
      toast.error("JSONファイルとして読み込めませんでした");
      return;
    }

    setValidatingImport(true);
    try {
      const validation = await apiFetch<{
        exportedAt: string | null;
        counts: PendingImport["counts"];
      }>("/api/data/import/validate", {
        method: "POST",
        body: JSON.stringify(json),
      });
      setPending({
        name: file.name,
        json,
        exportedAt: validation.exportedAt,
        counts: validation.counts,
      });
    } catch (error) {
      showImportError(error);
    } finally {
      setValidatingImport(false);
    }
  }

  const runImport = useMutation({
    mutationFn: (json: unknown) =>
      apiFetch<{
        imported: Record<string, number>;
        safetyBackup: string;
      }>("/api/data/import", {
        method: "POST",
        body: JSON.stringify(json),
      }),
    onSuccess: (result) => {
      setPending(null);
      onOpenChange(false);
      queryClient.invalidateQueries();
      const counts = result.imported;
      toast.success(
        `安全バックアップを作成して読み込みました（プロジェクト ${counts.projects} / メモ ${counts.notes} / エントリ ${counts.entries}）`,
      );
    },
    onError: showImportError,
  });

  return (
    <>
      <Dialog
        open={open}
        onOpenChange={(nextOpen) => {
          if (!nextOpen && !runImport.isPending) onOpenChange(false);
        }}
        contentClassName="w-[min(620px,92vw)]"
      >
        <DialogHeader>
          <DialogTitle>データの移行</DialogTitle>
          <p className="text-sm leading-6 text-neutral-500">
            Trackのデータを別の環境へ移すための機能です。日常的な保管には設定の「バックアップと復元」を使ってください。
          </p>
        </DialogHeader>

        <div className="grid gap-3 sm:grid-cols-2">
          <section className="flex flex-col rounded-lg border border-neutral-200 p-4">
            <div className="mb-4 flex size-9 items-center justify-center rounded-lg bg-neutral-100 text-neutral-600">
              <ArrowUpFromLine className="size-4" />
            </div>
            <h3 className="text-sm font-semibold text-neutral-900">
              この環境から書き出す
            </h3>
            <p className="mt-1 flex-1 text-xs leading-5 text-neutral-500">
              クライアント、プロジェクト、タグ、メモ、工数を1つのJSONファイルにまとめます。
            </p>
            <Button
              className="mt-4 w-full"
              variant="outline"
              size="sm"
              onClick={() => runExport.mutate()}
              disabled={runExport.isPending}
            >
              {runExport.isPending ? "書き出し中…" : "JSONを書き出す"}
            </Button>
          </section>

          <section className="flex flex-col rounded-lg border border-neutral-200 p-4">
            <div className="mb-4 flex size-9 items-center justify-center rounded-lg bg-neutral-100 text-neutral-600">
              <ArrowDownToLine className="size-4" />
            </div>
            <h3 className="text-sm font-semibold text-neutral-900">
              この環境へ読み込む
            </h3>
            <p className="mt-1 flex-1 text-xs leading-5 text-neutral-500">
              Trackから書き出したJSONを確認してから、現在のデータと置き換えます。
            </p>
            <input
              ref={fileRef}
              type="file"
              accept="application/json,.json"
              className="hidden"
              onChange={onPickImportFile}
            />
            <Button
              className="mt-4 w-full"
              variant="outline"
              size="sm"
              onClick={() => fileRef.current?.click()}
              disabled={validatingImport}
            >
              {validatingImport ? "ファイルを確認中…" : "JSONを読み込む"}
            </Button>
          </section>
        </div>

        {lastExport && (
          <div className="mt-3 flex items-start gap-2 rounded-lg bg-emerald-50 px-3 py-2.5 text-emerald-900">
            <FileJson2 className="mt-0.5 size-4 shrink-0" />
            <div className="min-w-0">
              <p className="text-xs font-medium">書き出しました</p>
              <code className="mt-0.5 block truncate text-[11px] text-emerald-800">
                {lastExport}
              </code>
            </div>
          </div>
        )}

        <DialogFooter>
          <Button
            variant="ghost"
            onClick={() => onOpenChange(false)}
            disabled={runImport.isPending}
          >
            閉じる
          </Button>
        </DialogFooter>
      </Dialog>

      <Dialog
        open={!!pending}
        onOpenChange={(nextOpen) => {
          if (!nextOpen && !runImport.isPending) setPending(null);
        }}
      >
        {pending && (
          <>
            <DialogHeader>
              <DialogTitle>現在のデータを置き換えますか？</DialogTitle>
            </DialogHeader>
            <div className="space-y-3 text-sm text-neutral-600">
              <div className="rounded-lg bg-neutral-50 p-3">
                <p className="font-medium text-neutral-900">{pending.name}</p>
                {pending.exportedAt && (
                  <p className="mt-1 text-xs text-neutral-500">
                    書き出し日時{" "}
                    {new Date(pending.exportedAt).toLocaleString("ja-JP")}
                  </p>
                )}
                <p className="mt-2 text-xs leading-5 text-neutral-600">
                  クライアント {pending.counts.clients} ・ タグ{" "}
                  {pending.counts.tags} ・ プロジェクト{" "}
                  {pending.counts.projects} ・ メモ {pending.counts.notes} ・
                  エントリ {pending.counts.entries}
                </p>
              </div>
              <p className="leading-6">
                現在のデータはすべて置き換わります。実行直前の状態は自動でバックアップされます。
              </p>
            </div>
            <DialogFooter>
              <Button
                variant="ghost"
                onClick={() => setPending(null)}
                disabled={runImport.isPending}
              >
                キャンセル
              </Button>
              <Button
                variant="destructive"
                data-dialog-autofocus
                onClick={() => runImport.mutate(pending.json)}
                disabled={runImport.isPending}
              >
                {runImport.isPending ? "置き換え中…" : "データを置き換える"}
              </Button>
            </DialogFooter>
          </>
        )}
      </Dialog>
    </>
  );
}
