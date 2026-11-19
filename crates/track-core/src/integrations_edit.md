
## 工数・プロジェクトの編集と取り消し

登録済みの工数の修正・プロジェクトの付け替え・削除や、クライアント・プロジェクト・タグの作成と変更を頼まれたときに使う。UIでは面倒なまとめての修正を任される前提で、対象の取り違えに気をつける。

1. 対象を集める。工数は`search`（条件は1つ以上）、参照データは`projects --all`・`clients`・`tags`で読む。各工数の`id`・`projectId`・日時・タイトルを確かめ、依頼の範囲（期間・プロジェクト・語句）に合うものだけを選ぶ。

   ```bash
   TRACK_CLI=$(/usr/bin/plutil -extract cliPath raw ~/.track/runtime.json)
   "$TRACK_CLI" search --project-id '<project id>' --from '<ISO日時>' --to '<ISO日時>'
   "$TRACK_CLI" projects --all
   ```

2. 変更する。書き込みには必ず`<SOURCE>`を付ける。`--id`は件数分くり返す。開始・終了の変更は1件ずつ行う。

   ```bash
   "$TRACK_CLI" update --id '<entry id>' --id '<entry id>' --project-id '<project id>' <SOURCE>
   "$TRACK_CLI" delete --id '<entry id>' --confirmed <SOURCE>
   "$TRACK_CLI" client create --name '<名前>' --confirmed <SOURCE>
   "$TRACK_CLI" project create --client-id '<client id>' --name '<名前>' --confirmed <SOURCE>
   "$TRACK_CLI" project update --id '<project id>' --name '<名前>' --color '#22c55e' <SOURCE>
   "$TRACK_CLI" tag update --id '<tag id>' --name '<名前>' <SOURCE>
   ```

   その他の引数は`"$TRACK_CLI" help`で確認する。プロジェクトはアーカイブ（`--archived true`）だけで削除しない。

3. 確認が必要な操作は、CLIが`confirmation_required`で止める。そのときは対象の全件（日付・時間帯・プロジェクト・タイトル）または作成するクライアント・プロジェクト名を示し、<CONFIRM>で明示的な許可を得てから同じコマンドに`--confirmed`を付けて再実行する。
   - 工数を10件以上まとめて変更する
   - 工数を削除する（件数によらない）
   - クライアント・プロジェクトを作成する
   - タグを削除する
   9件以下の変更、名前・色・アーカイブの変更は確認なしで実行してよい。利用者が対象と内容を具体的に指定して「作って」「消して」と頼んだ場合は、その依頼を許可として扱ってよい。

4. 結果を報告する。出力の`operation.summary`と`operation.id`を伝え、取り消せることも添える。

### 記録時に登録済みプロジェクトがない場合

工数を記録するとき、会話の作業が**明らかに**登録済みのどのプロジェクトにも当たらない（会話に新しい案件名・顧客名・リポジトリが出てくる等）場合は、「プロジェクトなし」に寄せる前に、プロジェクトを作るかを利用者に確認する。クライアントは`clients`の既存から候補を出し、当てはまるものがなければクライアントを作るかも一緒に聞く。作ったプロジェクトの`id`で記録する。根拠が弱いときは従来どおり「プロジェクトなし」にする。

### 取り消し

AIが行った書き換え（記録・編集・作成・削除）は1コマンドごとに操作として残り、取り消せる。利用者に「戻して」と言われたら、どの操作かを`history`で確かめ、`--id`を指定して取り消す。

```bash
"$TRACK_CLI" history
"$TRACK_CLI" undo --id '<operation id>'
```

- `undo_conflict`: その操作のあとで内容が変わっている（`details.changed`）。勝手に上書きせず、変わった行を伝える。
- `already_undone`: 取り消し済み。
- 取り消せるのはAIの操作だけで、Trackの画面で手で直した分は対象外。Trackアプリの画面にも「AIが変更しました［元に戻す］」が出て、そこからも取り消せる。
