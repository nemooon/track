import { Hono } from "hono";
import { authRoutes, requireAuth } from "./auth";
import { passkeyManage } from "./passkey";
import { clients } from "./routes/clients";
import { projects } from "./routes/projects";
import { entries } from "./routes/entries";
import { reports } from "./routes/reports";
import { tags } from "./routes/tags";
import { account } from "./routes/account";
import { invitations } from "./routes/invitations";
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
app.route("/api/tags", tags);
app.route("/api/account", account);
app.route("/api/passkeys", passkeyManage);
app.route("/api/invitations", invitations);

export default app;
