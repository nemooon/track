import { Hono, type Context, type Next } from "hono";
import { verify } from "hono/jwt";
import { setCookie, getCookie } from "hono/cookie";
import { passkeySignup, passkeyLogin } from "./passkey";
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

// Middleware: extract userId from JWT cookie, 401 if missing
export async function requireAuth(
  c: Context<{ Bindings: Env; Variables: AuthVars }>,
  next: Next,
) {
  const token = getCookie(c, TOKEN_COOKIE);
  if (!token) return c.json({ error: "unauthorized" }, 401);
  try {
    const payload = await verify(token, c.env.JWT_SECRET, "HS256");
    c.set("userId", payload.uid as string);
    await next();
  } catch {
    return c.json({ error: "unauthorized" }, 401);
  }
}
