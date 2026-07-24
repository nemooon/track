import Darwin
import Foundation
import FoundationModels

private let defaultWorklog = """
月曜日:
- Trackのレポート画面を改善: 3時間
- A社との定例会議: 1時間

火曜日:
- カレンダーの表示不具合を修正: 4時間
- コードレビュー: 1時間

水曜日:
- バックアップ機能を実装: 5時間
- B社との要件確認: 1時間
"""

private func writeError(_ message: String) {
    FileHandle.standardError.write(Data("\(message)\n".utf8))
}

private func writeStatus(_ message: String) {
    FileHandle.standardError.write(Data("\(message)\n".utf8))
}

private func unavailableMessage(
    for reason: SystemLanguageModel.Availability.UnavailableReason
) -> String {
    switch reason {
    case .deviceNotEligible:
        return "このMacはApple Intelligenceの対象外です。"
    case .appleIntelligenceNotEnabled:
        return "システム設定でApple Intelligenceを有効にしてください。"
    case .modelNotReady:
        return "端末内モデルを準備中です。ダウンロード完了後に再実行してください。"
    @unknown default:
        return "端末内モデルを利用できません。"
    }
}

private func generationErrorMessage(
    for error: LanguageModelSession.GenerationError
) -> String {
    switch error {
    case .exceededContextWindowSize(let context):
        return "入力がモデルの処理上限を超えました: \(context.debugDescription)"
    case .assetsUnavailable(let context):
        return "モデルの必要ファイルを利用できません: \(context.debugDescription)"
    case .guardrailViolation(let context):
        return "安全性チェックにより生成が停止しました: \(context.debugDescription)"
    case .unsupportedGuide(let context):
        return "未対応の生成指定です: \(context.debugDescription)"
    case .unsupportedLanguageOrLocale(let context):
        return "現在の言語または地域には対応していません: \(context.debugDescription)"
    case .decodingFailure(let context):
        return "モデルの出力を読み取れませんでした: \(context.debugDescription)"
    case .rateLimited(let context):
        return "端末内モデルが一時的に混雑しています: \(context.debugDescription)"
    case .concurrentRequests(let context):
        return "別の生成処理が実行中です: \(context.debugDescription)"
    case .refusal(_, let context):
        return "モデルが生成を拒否しました: \(context.debugDescription)"
    @unknown default:
        return "不明な生成エラーです。"
    }
}

@main
enum TrackAISample {
    static func main() async {
        let model = SystemLanguageModel.default

        switch model.availability {
        case .available:
            if CommandLine.arguments.dropFirst() == ["--check"] {
                print("Apple Intelligenceの端末内モデルを利用できます。")
                return
            }
        case .unavailable(let reason):
            writeError(unavailableMessage(for: reason))
            exit(EXIT_FAILURE)
        }

        let arguments = Array(CommandLine.arguments.dropFirst())
        let worklog = arguments.isEmpty ? defaultWorklog : arguments.joined(separator: " ")
        let session = LanguageModelSession(
            model: model,
            instructions: """
            あなたは工数管理アプリTrackの週報作成アシスタントです。
            入力にある事実だけを使い、日本語で簡潔にまとめてください。
            推測で成果や進捗を追加しないでください。
            出力は「概要」「主な作業」「次に確認すること」の3項目にしてください。
            「次に確認すること」が入力から判断できなければ「特になし」としてください。
            """
        )

        do {
            writeStatus("端末内モデルで週報を生成しています...\n")
            let response = try await session.respond(
                to: """
                次の工数記録から週報の下書きを作成してください。

                \(worklog)
                """
            )
            print(response.content)
        } catch let error as LanguageModelSession.GenerationError {
            writeError("週報を生成できませんでした。")
            writeError(generationErrorMessage(for: error))
            if let reason = error.failureReason {
                writeError("原因: \(reason)")
            }
            if let suggestion = error.recoverySuggestion {
                writeError("対処: \(suggestion)")
            }
            exit(EXIT_FAILURE)
        } catch {
            let nsError = error as NSError
            writeError("週報を生成できませんでした: \(error.localizedDescription)")
            writeError("エラー情報: \(nsError.domain) (\(nsError.code))")
            if !nsError.userInfo.isEmpty {
                writeError("詳細: \(nsError.userInfo)")
            }
            exit(EXIT_FAILURE)
        }
    }
}
