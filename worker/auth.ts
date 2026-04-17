import { Hono, type Context, type Next } from "hono";
import { sign, verify } from "hono/jwt";
import { setCookie, getCookie } from "hono/cookie";
import bcrypt from "bcryptjs";
import { getPrisma } from "./db";
import { signupSchema, loginSchema } from "@/lib/validators";
import type { Env, AuthVars } from "./types";

const TOKEN_COOKIE = "session";
const TOKEN_MAX_AGE = 60 * 60 * 24 * 30; // 30 days

export const authRoutes = new Hono<{ Bindings: Env }>();

authRoutes.post("/signup", async (c) => {
  const body = await c.req.json().catch(() => null);
  const parsed = signupSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }
  const prisma = getPrisma(c.env.DB);
  const existing = await prisma.user.findUnique({
    where: { email: parsed.data.email },
    select: { id: true },
  });
  if (existing) {
    return c.json({ error: "email_taken" }, 409);
  }
  const passwordHash = await bcrypt.hash(parsed.data.password, 10);
  const user = await prisma.user.create({
    data: { email: parsed.data.email, name: parsed.data.name, passwordHash },
  });

  // Auto sign-in after signup
  const token = await sign(
    { uid: user.id, email: user.email, exp: Math.floor(Date.now() / 1000) + TOKEN_MAX_AGE },
    c.env.JWT_SECRET,
  );
  setCookie(c, TOKEN_COOKIE, token, {
    httpOnly: true,
    secure: true,
    sameSite: "Lax",
    path: "/",
    maxAge: TOKEN_MAX_AGE,
  });
  return c.json({ ok: true }, 201);
});

authRoutes.post("/signin", async (c) => {
  const body = await c.req.json().catch(() => null);
  const parsed = loginSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input" }, 400);
  }
  const prisma = getPrisma(c.env.DB);
  const user = await prisma.user.findUnique({ where: { email: parsed.data.email } });
  if (!user) {
    return c.json({ error: "invalid_credentials" }, 401);
  }
  const ok = await bcrypt.compare(parsed.data.password, user.passwordHash);
  if (!ok) {
    return c.json({ error: "invalid_credentials" }, 401);
  }
  const token = await sign(
    { uid: user.id, email: user.email, exp: Math.floor(Date.now() / 1000) + TOKEN_MAX_AGE },
    c.env.JWT_SECRET,
  );
  setCookie(c, TOKEN_COOKIE, token, {
    httpOnly: true,
    secure: true,
    sameSite: "Lax",
    path: "/",
    maxAge: TOKEN_MAX_AGE,
  });
  return c.json({ ok: true, user: { id: user.id, email: user.email, name: user.name } });
});

authRoutes.post("/signout", (c) => {
  setCookie(c, TOKEN_COOKIE, "", { httpOnly: true, secure: true, path: "/", maxAge: 0 });
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
