-- Tag
CREATE TABLE "Tag" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "userId" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "color" TEXT NOT NULL,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "Tag_userId_fkey" FOREIGN KEY ("userId") REFERENCES "User" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE UNIQUE INDEX "Tag_userId_name_key" ON "Tag"("userId", "name");
CREATE INDEX "Tag_userId_idx" ON "Tag"("userId");

-- TagOnEntry (many-to-many)
CREATE TABLE "TagOnEntry" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "tagId" TEXT NOT NULL,
    "entryId" TEXT NOT NULL,
    CONSTRAINT "TagOnEntry_tagId_fkey" FOREIGN KEY ("tagId") REFERENCES "Tag" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "TagOnEntry_entryId_fkey" FOREIGN KEY ("entryId") REFERENCES "TimeEntry" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE UNIQUE INDEX "TagOnEntry_tagId_entryId_key" ON "TagOnEntry"("tagId", "entryId");
CREATE INDEX "TagOnEntry_entryId_idx" ON "TagOnEntry"("entryId");
CREATE INDEX "TagOnEntry_tagId_idx" ON "TagOnEntry"("tagId");
