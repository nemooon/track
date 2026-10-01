CREATE TABLE "AiRegistration" (
  "requestId" TEXT NOT NULL PRIMARY KEY,
  "payloadHash" TEXT NOT NULL,
  "source" TEXT NOT NULL,
  "sessionId" TEXT NOT NULL,
  "recordedUntil" DATETIME NOT NULL,
  "entryId" TEXT,
  FOREIGN KEY ("entryId") REFERENCES "TimeEntry" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);
CREATE INDEX "AiRegistration_source_sessionId_recordedUntil_idx" ON "AiRegistration" ("source", "sessionId", "recordedUntil");
