-- 単一ユーザーのローカルアプリでは userId が常に同じ値になり、意味を持たない。
-- 全テーブルから userId を落とし、User は勤務設定の置き場 Settings に改名する。
--
-- userId は FK なので ALTER TABLE DROP COLUMN が使えず、テーブルごと作り直す。
-- そのため実行側 (worker/migrate.ts) は foreign_keys=OFF と
-- legacy_alter_table=ON を立てた接続でこのファイルを流す。
-- 適用後に foreign_key_check で整合性を確認している。

-- User -> Settings (単一行)
CREATE TABLE "Settings" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "workStart" INTEGER NOT NULL DEFAULT 600,
    "workEnd" INTEGER NOT NULL DEFAULT 1110,
    "workDays" TEXT NOT NULL DEFAULT '1,2,3,4,5',
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "Settings" ("id", "workStart", "workEnd", "workDays", "createdAt")
    SELECT "id", "workStart", "workEnd", "workDays", "createdAt"
    FROM "User" ORDER BY "createdAt" ASC LIMIT 1;

-- Client
CREATE TABLE "Client_new" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "name" TEXT NOT NULL,
    "archived" BOOLEAN NOT NULL DEFAULT false,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "Client_new" SELECT "id", "name", "archived", "createdAt" FROM "Client";
DROP TABLE "Client";
ALTER TABLE "Client_new" RENAME TO "Client";

-- Project
CREATE TABLE "Project_new" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "clientId" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "color" TEXT NOT NULL,
    "archived" BOOLEAN NOT NULL DEFAULT false,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "Project_clientId_fkey" FOREIGN KEY ("clientId") REFERENCES "Client" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
INSERT INTO "Project_new" SELECT "id", "clientId", "name", "color", "archived", "createdAt" FROM "Project";
DROP TABLE "Project";
ALTER TABLE "Project_new" RENAME TO "Project";
CREATE INDEX "Project_clientId_idx" ON "Project"("clientId");

-- TimeEntry
CREATE TABLE "TimeEntry_new" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "projectId" TEXT,
    "start" DATETIME NOT NULL,
    "end" DATETIME NOT NULL,
    "title" TEXT,
    "note" TEXT,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" DATETIME NOT NULL,
    "externalEventId" TEXT,
    "externalEventSource" TEXT,
    "breakMinutes" INTEGER NOT NULL DEFAULT 0,
    CONSTRAINT "TimeEntry_projectId_fkey" FOREIGN KEY ("projectId") REFERENCES "Project" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);
INSERT INTO "TimeEntry_new"
    SELECT "id", "projectId", "start", "end", "title", "note", "createdAt", "updatedAt",
           "externalEventId", "externalEventSource", "breakMinutes"
    FROM "TimeEntry";
DROP TABLE "TimeEntry";
ALTER TABLE "TimeEntry_new" RENAME TO "TimeEntry";
-- userId が消えたので複合索引 (userId, start) は start 単独でよい
CREATE INDEX "TimeEntry_start_idx" ON "TimeEntry"("start");
CREATE INDEX "TimeEntry_projectId_idx" ON "TimeEntry"("projectId");
CREATE INDEX "TimeEntry_externalEventId_idx" ON "TimeEntry"("externalEventId");

-- Tag
CREATE TABLE "Tag_new" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "name" TEXT NOT NULL,
    "color" TEXT NOT NULL,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO "Tag_new" SELECT "id", "name", "color", "createdAt" FROM "Tag";
DROP TABLE "Tag";
ALTER TABLE "Tag_new" RENAME TO "Tag";
-- (userId, name) の複合ユニークは name 単独のユニークになる
CREATE UNIQUE INDEX "Tag_name_key" ON "Tag"("name");

DROP TABLE "User";
