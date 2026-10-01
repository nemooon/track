import { existsSync } from "node:fs";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { Codex } from "@openai/codex-sdk";
import type {
  AiGenerationMode,
  AiProgressUpdate,
  AiProviderId,
  AiProviderStatus,
} from "../../shared/types";
import type { Config } from "../config";
import { buildAiPrompt } from "./prompts";
import { resolveExecutable, runProcess } from "./process";

export type GenerateRequest = {
  mode: AiGenerationMode;
  input: string;
};

export type AiRuntime = {
  homeDir: string;
  resourceDir: string;
};

export type GenerateOptions = {
  signal?: AbortSignal;
  onProgress?: (progress: AiProgressUpdate) => void | Promise<void>;
};

const CODEX_GENERATION_TIMEOUT_MS = 10 * 60 * 1000;

const REPORT_AGGREGATION_OUTPUT_SCHEMA = {
  type: "object",
  properties: {
    groups: {
      type: "array",
      items: {
        type: "object",
        properties: {
          label: { type: "string" },
          summary: { type: "string" },
          entryIds: { type: "array", items: { type: "string" } },
          values: {
            type: "array",
            items: {
              type: "object",
              properties: {
                id: { type: "string" },
                value: { type: "string" },
              },
              required: ["id", "value"],
              additionalProperties: false,
            },
          },
        },
        required: ["label", "summary", "entryIds", "values"],
        additionalProperties: false,
      },
    },
  },
  required: ["groups"],
  additionalProperties: false,
} as const;

export function codexReasoningEffortForMode(mode: AiGenerationMode) {
  return mode === "report-aggregation" ? "low" as const : undefined;
}

export function codexOutputSchemaForMode(mode: AiGenerationMode) {
  return mode === "report-aggregation"
    ? REPORT_AGGREGATION_OUTPUT_SCHEMA
    : undefined;
}

export interface AiProvider {
  readonly id: AiProviderId;
  status(): AiProviderStatus;
  generate(request: GenerateRequest, options?: GenerateOptions): Promise<string>;
}

function cleanResponse(value: string): string {
  const response = value.trim();
  if (!response) throw new Error("AIから応答がありませんでした。");
  return response;
}

function appleHelperPath(resourceDir: string): string | null {
  const candidates = [
    path.join(resourceDir, "TrackAIHelper.app"),
    path.join(resourceDir, "src-tauri/binaries/TrackAIHelper.app"),
  ];
  return candidates.find(existsSync) ?? null;
}

class AppleIntelligenceProvider implements AiProvider {
  readonly id = "apple-intelligence" as const;

  constructor(private readonly runtime: AiRuntime) {}

  status(): AiProviderStatus {
    const helper = appleHelperPath(this.runtime.resourceDir);
    const available = process.platform === "darwin" && Boolean(helper);
    return {
      provider: this.id,
      label: "Apple Intelligence",
      available,
      detail: available
        ? "端末内モデルを使用します。"
        : process.platform !== "darwin"
          ? "macOSでのみ利用できます。"
          : "Apple Intelligenceヘルパーが見つかりません。",
    };
  }

  async generate(
    { mode, input }: GenerateRequest,
    options: GenerateOptions = {},
  ): Promise<string> {
    if (process.platform !== "darwin") {
      throw new Error("Apple IntelligenceはmacOSでのみ利用できます。");
    }
    const helper = appleHelperPath(this.runtime.resourceDir);
    if (!helper) {
      throw new Error(
        "Apple Intelligenceヘルパーが見つかりません。アプリを再ビルドしてください。",
      );
    }

    const directory = await mkdtemp(path.join(tmpdir(), "track-ai-"));
    const inputPath = path.join(directory, "prompt.txt");
    const outputPath = path.join(directory, "response.txt");
    const errorPath = path.join(directory, "error.txt");
    try {
      await options.onProgress?.({
        kind: "status",
        message: "Apple Intelligenceで内容を確認しています…",
      });
      await Promise.all([
        writeFile(inputPath, input, { encoding: "utf8", mode: 0o600 }),
        writeFile(outputPath, "", { encoding: "utf8", mode: 0o600 }),
        writeFile(errorPath, "", { encoding: "utf8", mode: 0o600 }),
      ]);
      try {
        await runProcess(
          "/usr/bin/open",
          [
            "-n",
            "-W",
            "--stdout",
            outputPath,
            "--stderr",
            errorPath,
            helper,
            "--args",
            inputPath,
            mode,
          ],
          "",
        );
      } catch (error) {
        const helperError = await readFile(errorPath, "utf8").catch(() => "");
        if (helperError.trim()) throw new Error(helperError.trim());
        throw error;
      }
      const [response, helperError] = await Promise.all([
        readFile(outputPath, "utf8"),
        readFile(errorPath, "utf8"),
      ]);
      if (!response.trim() && helperError.trim()) {
        throw new Error(helperError.trim());
      }
      return cleanResponse(response);
    } finally {
      await rm(directory, { recursive: true, force: true }).catch(() => {});
    }
  }
}

class CodexProvider implements AiProvider {
  readonly id = "codex" as const;
  private readonly executable: string | null;
  private readonly model: string | undefined;

  constructor(
    config: Config,
    private readonly runtime: AiRuntime,
  ) {
    this.executable = resolveExecutable(
      config.aiCodexExecutable || "codex",
      runtime.homeDir,
    );
    this.model = config.aiCodexModel || undefined;
  }

  status(): AiProviderStatus {
    return {
      provider: this.id,
      label: "Codex",
      available: Boolean(this.executable),
      detail: this.executable
        ? `既存のCodexログインを使用します（モデル: ${this.model ?? "Codexの既定値"}、実行ファイル: ${this.executable}）。`
        : "codex実行ファイルが見つかりません。Codexへログイン後、実行ファイルを指定してください。",
    };
  }

  async generate(
    { mode, input }: GenerateRequest,
    options: GenerateOptions = {},
  ): Promise<string> {
    if (!this.executable) {
      throw new Error(
        "codex実行ファイルが見つかりません。Codexへログイン後、設定で実行ファイルを指定してください。",
      );
    }

    const workingDirectory = await mkdtemp(
      path.join(tmpdir(), "track-codex-"),
    );
    const controller = new AbortController();
    let timedOut = false;
    const abort = () => controller.abort();
    if (options.signal?.aborted) abort();
    else options.signal?.addEventListener("abort", abort, { once: true });
    const timer = setTimeout(
      () => {
        timedOut = true;
        controller.abort();
      },
      CODEX_GENERATION_TIMEOUT_MS,
    );
    try {
      const codex = new Codex({ codexPathOverride: this.executable });
      const thread = codex.startThread({
        model: this.model,
        modelReasoningEffort: codexReasoningEffortForMode(mode),
        sandboxMode: "read-only",
        workingDirectory,
        skipGitRepoCheck: true,
        approvalPolicy: "never",
        networkAccessEnabled: false,
        webSearchMode: "disabled",
      });
      await options.onProgress?.({
        kind: "status",
        message: "Codexに処理を依頼しました…",
      });
      const { events } = await thread.runStreamed(buildAiPrompt(mode, input), {
        signal: controller.signal,
        outputSchema: codexOutputSchemaForMode(mode),
      });
      let finalResponse = "";
      for await (const event of events) {
        if (event.type === "turn.started") {
          await options.onProgress?.({
            kind: "status",
            message: "入力内容を確認しています…",
          });
        } else if (
          (event.type === "item.updated" || event.type === "item.completed") &&
          event.item.type === "reasoning" &&
          event.item.text.trim()
        ) {
          await options.onProgress?.({
            kind: "reasoning",
            message: event.item.text.trim(),
          });
        } else if (
          event.type === "item.completed" &&
          event.item.type === "agent_message"
        ) {
          finalResponse = event.item.text;
        } else if (event.type === "turn.failed") {
          throw new Error(event.error.message);
        } else if (event.type === "error") {
          throw new Error(event.message);
        }
      }
      return cleanResponse(finalResponse);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (timedOut) {
        throw new Error(
          `Codexの生成が${Math.round(CODEX_GENERATION_TIMEOUT_MS / 60_000)}分以内に完了しませんでした。対象期間や絞り込みを狭めて再度お試しください。`,
        );
      }
      if (/not logged in|login|unauthorized|401/i.test(message)) {
        throw new Error(
          "Codexへログインしていません。ターミナルで「codex login」を実行してください。",
        );
      }
      throw new Error(`Codexで生成できませんでした: ${message}`);
    } finally {
      clearTimeout(timer);
      options.signal?.removeEventListener("abort", abort);
      await rm(workingDirectory, { recursive: true, force: true }).catch(
        () => {},
      );
    }
  }
}

class CustomCommandProvider implements AiProvider {
  readonly id = "custom-command" as const;
  private readonly executable: string | null;

  constructor(
    private readonly config: Config,
    runtime: AiRuntime,
  ) {
    this.executable = resolveExecutable(
      config.aiCommandExecutable,
      runtime.homeDir,
    );
  }

  status(): AiProviderStatus {
    return {
      provider: this.id,
      label: "カスタムコマンド",
      available: Boolean(this.executable),
      detail: this.executable
        ? `標準入力でプロンプトを渡します（${this.executable}）。`
        : "実行可能なコマンドを指定してください。",
    };
  }

  async generate(
    { mode, input }: GenerateRequest,
    options: GenerateOptions = {},
  ): Promise<string> {
    if (!this.executable) {
      throw new Error("設定したコマンドが見つからないか、実行できません。");
    }
    await options.onProgress?.({
      kind: "status",
      message: "カスタムコマンドを実行しています…",
    });
    const result = await runProcess(
      this.executable,
      this.config.aiCommandArgs,
      buildAiPrompt(mode, input),
    );
    return cleanResponse(result.stdout);
  }
}

export function createAiProvider(
  config: Config,
  runtime: AiRuntime,
): AiProvider {
  switch (config.aiProvider) {
    case "codex":
      return new CodexProvider(config, runtime);
    case "custom-command":
      return new CustomCommandProvider(config, runtime);
    default:
      return new AppleIntelligenceProvider(runtime);
  }
}
