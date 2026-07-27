CREATE TABLE "Note" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "projectId" TEXT,
    "title" TEXT NOT NULL,
    "content" TEXT NOT NULL DEFAULT '',
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "Note_projectId_fkey" FOREIGN KEY ("projectId") REFERENCES "Project" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);

CREATE INDEX "Note_projectId_idx" ON "Note"("projectId");
CREATE INDEX "Note_updatedAt_idx" ON "Note"("updatedAt");
