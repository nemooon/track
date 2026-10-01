import type {
  AiGenerateResponse,
  AiGenerateStreamEvent,
  AiGenerationMode,
  AiProgressUpdate,
  AiProviderId,
} from "@shared/types";

export function aiProviderLabel(provider?: AiProviderId): string {
  switch (provider) {
    case "codex":
      return "Codex";
    case "custom-command":
      return "カスタムコマンド";
    default:
      return "Apple Intelligence";
  }
}

export async function generateAiText(
  mode: AiGenerationMode,
  input: string,
): Promise<AiGenerateResponse> {
  return generateAiTextStream(mode, input, () => {});
}

export async function generateAiTextStream(
  mode: AiGenerationMode,
  input: string,
  onProgress: (progress: AiProgressUpdate) => void,
): Promise<AiGenerateResponse> {
  const response = await fetch("/api/ai/generate-stream", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ mode, input }),
  });
  if (!response.ok) {
    const text = await response.text().catch(() => "");
    throw new Error(text || `HTTP ${response.status}`);
  }
  if (!response.body) throw new Error("AIの進捗ストリームを開始できませんでした。");

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let result: AiGenerateResponse | undefined;

  const consumeLine = (line: string) => {
    if (!line.trim()) return;
    const event = JSON.parse(line) as AiGenerateStreamEvent;
    if (event.type === "progress") onProgress(event);
    else if (event.type === "result") {
      result = { text: event.text, provider: event.provider };
    } else if (event.type === "error") {
      throw new Error(event.message);
    }
  };

  while (true) {
    const { done, value } = await reader.read();
    buffer += decoder.decode(value, { stream: !done });
    const lines = buffer.split("\n");
    buffer = lines.pop() ?? "";
    for (const line of lines) consumeLine(line);
    if (done) break;
  }
  consumeLine(buffer);
  if (!result) throw new Error("AIから生成結果を受信できませんでした。");
  return result;
}
