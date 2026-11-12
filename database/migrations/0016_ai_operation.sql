CREATE TABLE "AiOperation" (
  "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
  "createdAt" DATETIME NOT NULL,
  "source" TEXT NOT NULL,
  "summary" TEXT NOT NULL,
  "count" INTEGER NOT NULL,
  "changes" TEXT NOT NULL,
  "undoneAt" DATETIME
);
CREATE INDEX "AiOperation_createdAt_idx" ON "AiOperation" ("createdAt");
