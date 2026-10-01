import { useEffect, useRef, useState } from "react";
import { Crepe } from "@milkdown/crepe";
import "@milkdown/crepe/theme/common/style.css";
import "@milkdown/crepe/theme/frame.css";
import { cn } from "@client/lib/utils";

export function MarkdownEditor({
  value,
  onChange,
  showToolbar = true,
  readOnly = false,
  ariaLabel = "Markdown本文",
  className,
}: {
  value: string;
  onChange: (value: string) => void;
  showToolbar?: boolean;
  readOnly?: boolean;
  ariaLabel?: string;
  className?: string;
}) {
  const rootRef = useRef<HTMLDivElement>(null);
  const onChangeRef = useRef(onChange);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);
  onChangeRef.current = onChange;

  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;

    let disposed = false;
    let destroyed = false;
    const crepe = new Crepe({
      root,
      defaultValue: value,
      features: {
        [Crepe.Feature.TopBar]: showToolbar && !readOnly,
        [Crepe.Feature.Toolbar]: showToolbar && !readOnly,
        ...(readOnly ? {
          [Crepe.Feature.BlockEdit]: false,
          [Crepe.Feature.Cursor]: false,
          [Crepe.Feature.Placeholder]: false,
          [Crepe.Feature.LinkTooltip]: false,
        } : {}),
        [Crepe.Feature.ImageBlock]: false,
        [Crepe.Feature.Latex]: false,
        [Crepe.Feature.AI]: false,
      },
      featureConfigs: {
        [Crepe.Feature.Placeholder]: {
          text: "自由にメモを入力できます。「/」でブロックを追加できます。",
          mode: "doc",
        },
        [Crepe.Feature.TopBar]: {
          headingOptions: [
            { label: "本文", level: null },
            { label: "見出し 1", level: 1 },
            { label: "見出し 2", level: 2 },
            { label: "見出し 3", level: 3 },
          ],
        },
      },
    });
    crepe.setReadonly(readOnly);

    crepe.on((listener) => {
      listener.markdownUpdated((_ctx, markdown) => {
        if (!disposed && !readOnly) onChangeRef.current(markdown);
      });
    });

    const creation = crepe
      .create()
      .then(() => {
        if (disposed) return;
        root
          .querySelector<HTMLElement>(".ProseMirror")
          ?.setAttribute("aria-label", ariaLabel);
        if (!showToolbar || readOnly) {
          setLoading(false);
          return;
        }
        const topBarLabels = [
          "太字",
          "斜体",
          "取り消し線",
          "インラインコード",
          "箇条書き",
          "番号付きリスト",
          "チェックリスト",
          "リンク",
          "表",
          "コードブロック",
          "引用",
          "区切り線",
        ];
        root
          .querySelector<HTMLElement>(".top-bar-heading-button")
          ?.setAttribute("aria-label", "段落スタイル");
        const headingButton = root.querySelector<HTMLElement>(
          ".top-bar-heading-button",
        );
        if (headingButton) headingButton.dataset.tooltip = "段落スタイル";
        root
          .querySelectorAll<HTMLElement>(".top-bar-item")
          .forEach((button, index) => {
            const label = topBarLabels[index];
            if (!label) return;
            button.setAttribute("aria-label", label);
            button.dataset.tooltip = label;
            button.removeAttribute("title");
          });
        setLoading(false);
      })
      .catch(() => {
        if (!disposed) {
          setLoading(false);
          setError(true);
        }
      });

    return () => {
      disposed = true;
      void creation.finally(() => {
        if (destroyed) return;
        destroyed = true;
        void crepe.destroy();
      });
    };
  }, []);

  return (
    <div
      className={cn(
        "milkdown-markdown-editor subtle-scrollbar relative min-h-0 overflow-auto",
        className,
      )}
    >
      <div ref={rootRef} className="min-h-full" />
      {loading && (
        <div className="absolute inset-0 flex items-center justify-center bg-white">
          <div className="size-5 animate-spin rounded-full border-2 border-neutral-300 border-t-neutral-700" />
        </div>
      )}
      {error && (
        <div className="absolute inset-0 flex items-center justify-center bg-white px-6 text-center text-sm text-red-600">
          {readOnly ? "Markdown本文を表示できませんでした" : "Markdownエディタを読み込めませんでした"}
        </div>
      )}
    </div>
  );
}
