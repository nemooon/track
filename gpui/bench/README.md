# GPUI 版 / Tauri 版の計測

前提: `prepare-db.sh` でコピー DB を作ってあること（GPUI 版は API サーバー不要）、`cargo build --release --workspace` 済み、インストール済みの `/Applications/Track.app`。
操作系の計測にはアクセシビリティ権限、画面取得には画面収録の権限が要る。どちらのアプリのデータも書き換えない
（`⌘]`/`⌘[` の往復、マウス移動、スクロールだけ）。Tauri 版は起動・終了を繰り返す。

```bash
cd gpui/bench
swiftc -O winpid.swift -o winpid && swiftc -O bench.swift -o bench
python3 measure.py gpui 5      # 起動〜表示・描画安定、メモリ（footprint）、待機 CPU
python3 measure.py tauri 5     # 同上（Tauri は起動前後で増えた WebKit プロセスも合算）
./bench key <pid> <windowID> 10            # ⌘] / ⌘[ を10往復、画素が変わるまで / 落ち着くまで
python3 load.py hover <pid> <windowID> 10 <UIのpid...>   # 10秒間マウス移動中の CPU と描画フレーム数
python3 load.py scroll <pid> <windowID> 10 <UIのpid...>  # 10秒間スクロール中の CPU と描画フレーム数
```

ウインドウ ID は `./winpid <pid>` で取れる。GPUI 版は `TRACK_GPUI_WINDOW=1440x900` で Tauri 版と同じ大きさにそろえる。
