-- Allow an entry to record an unpaid break embedded within its time range.
-- e.g. 直行直帰 8.5h block with 60 min lunch break -> displayed 8.5h, counted 7.5h.
ALTER TABLE "TimeEntry" ADD COLUMN "breakMinutes" INTEGER NOT NULL DEFAULT 0;
