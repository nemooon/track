export type Env = {
  DB: D1Database;
  JWT_SECRET: string;
  ASSETS: Fetcher;
};

export type AuthVars = {
  userId: string;
};
