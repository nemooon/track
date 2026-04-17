-- Convert User.workStart / workEnd from hours (0-23) to minutes since midnight (0-1440)
UPDATE User SET workStart = workStart * 60, workEnd = workEnd * 60;
