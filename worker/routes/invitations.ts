import { Hono } from "hono";
import { getPrisma } from "../db";
import { emailSchema } from "@/lib/validators";
import type { Env, AuthVars } from "../types";

const invitations = new Hono<{ Bindings: Env; Variables: AuthVars }>();

// GET /api/invitations — list invitations created by current user
invitations.get("/", async (c) => {
  const userId = c.get("userId");
  const prisma = getPrisma(c.env.DB);
  const list = await prisma.invitation.findMany({
    where: { invitedById: userId },
    select: { id: true, email: true, createdAt: true },
    orderBy: { createdAt: "desc" },
  });
  return c.json(list);
});

// POST /api/invitations — invite an email address
invitations.post("/", async (c) => {
  const body = await c.req.json<{ email: string }>().catch(() => null);
  if (!body?.email) return c.json({ error: "email_required" }, 400);

  const parsed = emailSchema.safeParse(body.email);
  if (!parsed.success) return c.json({ error: "invalid_email" }, 400);
  const email = parsed.data;

  const prisma = getPrisma(c.env.DB);

  // Check if already registered
  const existingUser = await prisma.user.findUnique({
    where: { email },
    select: { id: true },
  });
  if (existingUser) return c.json({ error: "already_registered" }, 409);

  // Check if already invited
  const existingInvite = await prisma.invitation.findUnique({
    where: { email },
  });
  if (existingInvite) return c.json({ error: "already_invited" }, 409);

  const userId = c.get("userId");
  const invitation = await prisma.invitation.create({
    data: { email, invitedById: userId },
    select: { id: true, email: true, createdAt: true },
  });

  return c.json(invitation, 201);
});

// DELETE /api/invitations/:id
invitations.delete("/:id", async (c) => {
  const userId = c.get("userId");
  const id = c.req.param("id");
  const prisma = getPrisma(c.env.DB);

  await prisma.invitation.deleteMany({
    where: { id, invitedById: userId },
  });
  return c.json({ ok: true });
});

export { invitations };
