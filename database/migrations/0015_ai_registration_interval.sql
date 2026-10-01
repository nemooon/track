ALTER TABLE "AiRegistration" ADD COLUMN "recordedFrom" DATETIME;
UPDATE "AiRegistration" SET "recordedFrom" = (
  SELECT "start" FROM "TimeEntry" WHERE "TimeEntry"."id" = "AiRegistration"."entryId"
);
