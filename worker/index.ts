import { Hono } from "hono";
import { authRoutes, requireAuth } from "./auth";
import { clients } from "./routes/clients";
import { projects } from "./routes/projects";
import { entries } from "./routes/entries";
import { reports } from "./routes/reports";
import { account } from "./routes/account";
import type { Env, AuthVars } from "./types";

const app = new Hono<{ Bindings: Env; Variables: AuthVars }>();

// Public auth routes
app.route("/api/auth", authRoutes);

// Protected routes — require JWT
app.use("/api/*", requireAuth);
app.route("/api/clients", clients);
app.route("/api/projects", projects);
app.route("/api/entries", entries);
app.route("/api/reports", reports);
app.route("/api/account", account);

export default app;
