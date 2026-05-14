export type Env = {
  DB: D1Database;
  JWT_SECRET: string;
  ASSETS: Fetcher;
  AI: Ai;
};

export type AuthVars = {
  userId: string;
};
