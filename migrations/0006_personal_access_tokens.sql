-- Personal Access Tokens for MCP / API access.
-- Token plain text is shown only on creation; only its SHA-256 hash is stored.
CREATE TABLE "PersonalAccessToken" (
  "id"         TEXT NOT NULL PRIMARY KEY,
  "userId"     TEXT NOT NULL,
  "name"       TEXT NOT NULL,
  "tokenHash"  TEXT NOT NULL,
  "createdAt"  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  "lastUsedAt" DATETIME,
  FOREIGN KEY ("userId") REFERENCES "User"("id") ON DELETE CASCADE
);

CREATE UNIQUE INDEX "PersonalAccessToken_tokenHash_key"
  ON "PersonalAccessToken"("tokenHash");

CREATE INDEX "PersonalAccessToken_userId_idx"
  ON "PersonalAccessToken"("userId");
