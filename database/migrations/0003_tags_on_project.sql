-- TagOnProject (many-to-many)
CREATE TABLE "TagOnProject" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "tagId" TEXT NOT NULL,
    "projectId" TEXT NOT NULL,
    CONSTRAINT "TagOnProject_tagId_fkey" FOREIGN KEY ("tagId") REFERENCES "Tag" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "TagOnProject_projectId_fkey" FOREIGN KEY ("projectId") REFERENCES "Project" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);
CREATE UNIQUE INDEX "TagOnProject_tagId_projectId_key" ON "TagOnProject"("tagId", "projectId");
CREATE INDEX "TagOnProject_projectId_idx" ON "TagOnProject"("projectId");
CREATE INDEX "TagOnProject_tagId_idx" ON "TagOnProject"("tagId");
