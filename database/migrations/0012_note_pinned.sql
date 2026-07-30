ALTER TABLE "Note" ADD COLUMN "pinned" BOOLEAN NOT NULL DEFAULT false;

CREATE INDEX "Note_pinned_updatedAt_idx" ON "Note"("pinned", "updatedAt");
