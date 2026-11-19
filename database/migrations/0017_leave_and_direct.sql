-- 直行直帰: a TimeEntry marked 'direct' (one day of scheduled work for a project).
ALTER TABLE "TimeEntry" ADD COLUMN "kind" TEXT NOT NULL DEFAULT 'normal';

-- 休暇: one row per local date. kind is 'full', 'am', 'pm', or 'none' (cancels a holiday's
-- automatic leave).
CREATE TABLE "Leave" (
  "id" TEXT NOT NULL PRIMARY KEY,
  "date" TEXT NOT NULL,
  "kind" TEXT NOT NULL,
  "createdAt" DATETIME NOT NULL,
  "updatedAt" DATETIME NOT NULL
);
CREATE UNIQUE INDEX "Leave_date_key" ON "Leave" ("date");
