import { Hono, type Context, type Next } from "hono";
import { verify } from "hono/jwt";
import { setCookie, getCookie } from "hono/cookie";
import { passkeySignup, passkeyLogin } from "./passkey";
import { getPrisma } from "./db";
import { hashToken } from "./lib/pat";
import type { Env, AuthVars } from "./types";

const TOKEN_COOKIE = "session";

export const authRoutes = new Hono<{ Bindings: Env }>();

// Passkey signup: /api/auth/signup/*
authRoutes.route("/signup", passkeySignup);

// Passkey login: /api/auth/login/*
authRoutes.route("/login", passkeyLogin);

authRoutes.post("/signout", (c) => {
  const local = (c.req.header("origin") ?? c.req.header("host") ?? "").includes("localhost");
  setCookie(c, TOKEN_COOKIE, "", { httpOnly: true, secure: !local, path: "/", maxAge: 0 });
  return c.json({ ok: true });
});

authRoutes.get("/me", async (c) => {
  const token = getCookie(c, TOKEN_COOKIE);
  if (!token) return c.json({ user: null });
  try {
    const payload = await verify(token, c.env.JWT_SECRET, "HS256");
    return c.json({ user: { id: payload.uid, email: payload.email } });
  } catch {
    return c.json({ user: null });
  }
});

// Middleware: extract userId from JWT cookie, with PAT Bearer fallback
// for MCP / CLI clients. 401 if neither valid auth is present.
export async function requireAuth(
  c: Context<{ Bindings: Env; Variables: AuthVars }>,
  next: Next,
) {
  const token = getCookie(c, TOKEN_COOKIE);
  if (token) {
    try {
      const payload = await verify(token, c.env.JWT_SECRET, "HS256");
      c.set("userId", payload.uid as string);
      await next();
      return;
    } catch {
      return c.json({ error: "unauthorized" }, 401);
    }
  }

  const authHeader = c.req.header("authorization");
  if (authHeader) {
    const m = authHeader.match(/^Bearer\s+(.+)$/i);
    if (m) {
      const tokenHash = await hashToken(m[1]);
      const prisma = getPrisma(c.env.DB);
      const pat = await prisma.personalAccessToken.findUnique({
        where: { tokenHash },
        select: { userId: true },
      });
      if (pat) {
        c.set("userId", pat.userId);
        await next();
        return;
      }
    }
  }

  return c.json({ error: "unauthorized" }, 401);
}
