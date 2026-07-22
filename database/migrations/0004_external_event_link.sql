-- Link a TimeEntry to an external calendar event (e.g. Outlook meeting).
ALTER TABLE "TimeEntry" ADD COLUMN "externalEventId" TEXT;
ALTER TABLE "TimeEntry" ADD COLUMN "externalEventSource" TEXT;
CREATE INDEX "TimeEntry_externalEventId_idx" ON "TimeEntry" ("externalEventId");
