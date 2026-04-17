-- Add work schedule settings to User
ALTER TABLE User ADD COLUMN workStart INTEGER NOT NULL DEFAULT 9;
ALTER TABLE User ADD COLUMN workEnd INTEGER NOT NULL DEFAULT 18;
ALTER TABLE User ADD COLUMN workDays TEXT NOT NULL DEFAULT '1,2,3,4,5';

-- Make projectId nullable on TimeEntry
-- SQLite doesn't support ALTER COLUMN, so we recreate the table
CREATE TABLE TimeEntry_new (
  id        TEXT PRIMARY KEY NOT NULL,
  userId    TEXT NOT NULL,
  projectId TEXT,
  start     DATETIME NOT NULL,
  "end"     DATETIME NOT NULL,
  note      TEXT,
  createdAt DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updatedAt DATETIME NOT NULL,
  FOREIGN KEY (userId) REFERENCES User(id) ON DELETE CASCADE,
  FOREIGN KEY (projectId) REFERENCES Project(id) ON DELETE RESTRICT
);

INSERT INTO TimeEntry_new SELECT * FROM TimeEntry;
DROP TABLE TimeEntry;
ALTER TABLE TimeEntry_new RENAME TO TimeEntry;

CREATE INDEX idx_timeentry_user_start ON TimeEntry(userId, start);
CREATE INDEX idx_timeentry_project ON TimeEntry(projectId);
