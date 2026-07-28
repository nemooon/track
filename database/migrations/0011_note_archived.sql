ALTER TABLE "Note" ADD COLUMN "archived" BOOLEAN NOT NULL DEFAULT false;

CREATE INDEX "Note_archived_updatedAt_idx" ON "Note"("archived", "updatedAt");
