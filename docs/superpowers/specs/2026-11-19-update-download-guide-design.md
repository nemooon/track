# zip・DMG で入れた Track の更新の案内（小さな変更）

## 目的

「アップデートを確認…」は `brew upgrade` で更新するため、zip や DMG で入れた Track では失敗し、「ターミナルからアップデートしてください」と的外れな案内が出ていた。入れ方を判定して、それぞれに合った案内を出す。

## やること

- 判定：Homebrew があり、`/opt/homebrew/Caskroom`（または `/usr/local/Caskroom`）の `track/<版>/Track.app` が、いま動いている Track.app を指していれば Homebrew で入れたものとする。ファイルを見るだけで、`brew` は実行しない。
- Homebrew で入れたとき：今までどおり「アップデート」で更新し、隔離属性も外す。
- zip・DMG で入れたとき：新しい版とリリースノートは同じように出す。「アップデート」の代わりに次を出す。
  - リリースのページから DMG を落として Track.app を入れ替えるよう案内する一文
  - 開く前に隔離属性を外すコマンド（`xattr -dr com.apple.quarantine /Applications/Track.app`）と「コピー」ボタン
  - 「ダウンロードページを開く」ボタン（GitHub のリリースのページ）

## 作り方

- 案 A（採用）：Caskroom のリンク先を見る。速く、`brew` を走らせずに窓を開いた時点で決まる。
- 案 B：`brew list --cask track` を実行する。Homebrew は入っているが Track だけ DMG、という場合を区別できないので見送り。

## 私の判断

- 開発版で見るため `TRACK_GPUI_UPDATE_DOWNLOAD=1` を足した（`TRACK_GPUI_UPDATE_PREVIEW` と一緒に使う）。
- コマンドの置き場所は `/Applications/Track.app` 固定にした。README のインストールの手順と同じ。
