-- ローカル単一ユーザー化に伴い、アカウント関連を削除する。
--   - PersonalAccessToken: PAT 認証は廃止 (ローカルは認証なし)
--   - Credential:          パスキーは Web 版専用
--   - Invitation:          多人数前提の招待機能を廃止
--   - User.email / name:   単一ユーザーなので識別する相手がいない
--
-- User テーブル自体は勤務設定 (workStart/workEnd/workDays) の置き場として残す。

DROP TABLE IF EXISTS "PersonalAccessToken";
DROP TABLE IF EXISTS "Credential";
DROP TABLE IF EXISTS "Invitation";

-- email は UNIQUE INDEX が張られており、索引を先に落とさないと DROP COLUMN できない
DROP INDEX IF EXISTS "User_email_key";
ALTER TABLE "User" DROP COLUMN "email";
ALTER TABLE "User" DROP COLUMN "name";
