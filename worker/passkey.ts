import { Hono } from "hono";
import { sign } from "hono/jwt";
import { setCookie, getCookie, deleteCookie } from "hono/cookie";
import {
  generateRegistrationOptions,
  verifyRegistrationResponse,
  generateAuthenticationOptions,
  verifyAuthenticationResponse,
} from "@simplewebauthn/server";
import type {
  AuthenticatorTransportFuture,
} from "@simplewebauthn/server";
import { getPrisma } from "./db";
import { signupSchema } from "@/lib/validators";
import type { Env, AuthVars } from "./types";

function uint8ToBase64url(bytes: Uint8Array): string {
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function base64urlToUint8Array(b64: string): Uint8Array {
  const base64 = b64.replace(/-/g, "+").replace(/_/g, "/");
  const padded = base64 + "=".repeat((4 - (base64.length % 4)) % 4);
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

const TOKEN_COOKIE = "session";
const CHALLENGE_COOKIE = "webauthn_challenge";
const TOKEN_MAX_AGE = 60 * 60 * 24 * 30; // 30 days

function getRpInfo(c: { req: { header(name: string): string | undefined } }) {
  const origin = c.req.header("origin") ?? "";
  if (origin) {
    const url = new URL(origin);
    return { rpId: url.hostname, rpName: "track", origin };
  }
  const host = c.req.header("host") ?? "localhost";
  const rpId = host.split(":")[0]; // strip port
  const fallbackOrigin = rpId === "localhost"
    ? `http://${host}`
    : `https://${host}`;
  return { rpId, rpName: "track", origin: fallbackOrigin };
}

function isLocalhost(c: { req: { header(name: string): string | undefined } }) {
  const origin = c.req.header("origin") ?? "";
  if (origin) return new URL(origin).hostname === "localhost";
  const host = c.req.header("host") ?? "";
  return host.startsWith("localhost");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type HonoContext = any;

function setChallenge(c: HonoContext, challenge: string) {
  const local = isLocalhost(c);
  setCookie(c, CHALLENGE_COOKIE, challenge, {
    httpOnly: true,
    secure: !local,
    sameSite: "Lax",
    path: "/",
    maxAge: 300, // 5 min
  });
}

function getChallenge(c: HonoContext): string | undefined {
  return getCookie(c, CHALLENGE_COOKIE);
}

function clearChallenge(c: HonoContext) {
  deleteCookie(c, CHALLENGE_COOKIE, { path: "/" });
}

function issueSession(c: HonoContext, token: string) {
  const local = isLocalhost(c);
  setCookie(c, TOKEN_COOKIE, token, {
    httpOnly: true,
    secure: !local,
    sameSite: "Lax",
    path: "/",
    maxAge: TOKEN_MAX_AGE,
  });
}

// ─── Registration (signup) ───────────────────────────────────────

export const passkeySignup = new Hono<{ Bindings: Env }>();

// Step 1: Generate registration options
passkeySignup.post("/register-options", async (c) => {
  const body = await c.req.json().catch(() => null);
  const parsed = signupSchema.safeParse(body);
  if (!parsed.success) {
    return c.json({ error: "invalid_input", issues: parsed.error.flatten() }, 400);
  }

  const { rpId, rpName } = getRpInfo(c);
  const prisma = getPrisma(c.env.DB);

  const existing = await prisma.user.findUnique({
    where: { email: parsed.data.email },
    select: { id: true },
  });
  if (existing) return c.json({ error: "email_taken" }, 409);

  // First user can register without invitation; subsequent users need one
  const userCount = await prisma.user.count();
  if (userCount > 0) {
    const invited = await prisma.invitation.findUnique({
      where: { email: parsed.data.email },
    });
    if (!invited) return c.json({ error: "not_invited" }, 403);
  }

  const options = await generateRegistrationOptions({
    rpName,
    rpID: rpId,
    userName: parsed.data.email,
    userDisplayName: parsed.data.name || parsed.data.email,
    attestationType: "none",
    authenticatorSelection: {
      residentKey: "preferred",
      userVerification: "preferred",
    },
  });

  // Store challenge + signup info in cookie
  setChallenge(c, JSON.stringify({
    challenge: options.challenge,
    email: parsed.data.email,
    name: parsed.data.name,
  }));

  return c.json(options);
});

// Step 2: Verify registration & create user
passkeySignup.post("/register-verify", async (c) => {
  const body = await c.req.json().catch(() => null);
  if (!body) return c.json({ error: "invalid_body" }, 400);

  const stored = getChallenge(c);
  if (!stored) return c.json({ error: "challenge_expired" }, 400);
  clearChallenge(c);

  const { challenge, email, name } = JSON.parse(stored);
  const { rpId, origin } = getRpInfo(c);

  let verification;
  try {
    verification = await verifyRegistrationResponse({
      response: body,
      expectedChallenge: challenge,
      expectedOrigin: origin,
      expectedRPID: rpId,
    });
  } catch (e) {
    return c.json({ error: "verification_failed" }, 400);
  }

  if (!verification.verified || !verification.registrationInfo) {
    return c.json({ error: "verification_failed" }, 400);
  }

  const { credential } = verification.registrationInfo;
  const prisma = getPrisma(c.env.DB);

  // Double-check email uniqueness
  const existing = await prisma.user.findUnique({
    where: { email },
    select: { id: true },
  });
  if (existing) return c.json({ error: "email_taken" }, 409);

  const user = await prisma.user.create({
    data: {
      email,
      name: name || null,
      credentials: {
        create: {
          credentialId: credential.id,
          publicKey: uint8ToBase64url(new Uint8Array(credential.publicKey)),
          counter: credential.counter,
          transports: body.response?.transports
            ? JSON.stringify(body.response.transports)
            : null,
        },
      },
    },
  });

  const token = await sign(
    { uid: user.id, email: user.email, exp: Math.floor(Date.now() / 1000) + TOKEN_MAX_AGE },
    c.env.JWT_SECRET,
  );
  issueSession(c, token);
  return c.json({ ok: true }, 201);
});

// ─── Authentication (login) ──────────────────────────────────────

export const passkeyLogin = new Hono<{ Bindings: Env }>();

// Step 1: Generate authentication options
passkeyLogin.post("/login-options", async (c) => {
  const { rpId } = getRpInfo(c);

  const options = await generateAuthenticationOptions({
    rpID: rpId,
    userVerification: "preferred",
    // Empty allowCredentials = discoverable credential (passkey)
  });

  setChallenge(c, options.challenge);
  return c.json(options);
});

// Step 2: Verify authentication
passkeyLogin.post("/login-verify", async (c) => {
  const body = await c.req.json().catch(() => null);
  if (!body) return c.json({ error: "invalid_body" }, 400);

  const challenge = getChallenge(c);
  if (!challenge) return c.json({ error: "challenge_expired" }, 400);
  clearChallenge(c);

  const { rpId, origin } = getRpInfo(c);
  const prisma = getPrisma(c.env.DB);

  // Look up credential
  const credIdB64 = body.id; // already base64url from browser
  const cred = await prisma.credential.findUnique({
    where: { credentialId: credIdB64 },
    include: { user: { select: { id: true, email: true } } },
  });
  if (!cred) return c.json({ error: "credential_not_found" }, 401);

  let verification;
  try {
    verification = await verifyAuthenticationResponse({
      response: body,
      expectedChallenge: challenge,
      expectedOrigin: origin,
      expectedRPID: rpId,
      credential: {
        id: cred.credentialId,
        publicKey: base64urlToUint8Array(cred.publicKey) as Uint8Array<ArrayBuffer>,
        counter: cred.counter,
        transports: cred.transports
          ? (JSON.parse(cred.transports) as AuthenticatorTransportFuture[])
          : undefined,
      },
    });
  } catch {
    return c.json({ error: "verification_failed" }, 401);
  }

  if (!verification.verified) {
    return c.json({ error: "verification_failed" }, 401);
  }

  // Update counter
  await prisma.credential.update({
    where: { id: cred.id },
    data: { counter: verification.authenticationInfo.newCounter },
  });

  const token = await sign(
    { uid: cred.user.id, email: cred.user.email, exp: Math.floor(Date.now() / 1000) + TOKEN_MAX_AGE },
    c.env.JWT_SECRET,
  );
  issueSession(c, token);
  return c.json({ ok: true, user: { id: cred.user.id, email: cred.user.email } });
});

// ─── Credential management (authenticated) ──────────────────────

export const passkeyManage = new Hono<{ Bindings: Env; Variables: AuthVars }>();

// List credentials for current user
passkeyManage.get("/", async (c) => {
  const userId = c.get("userId");
  const prisma = getPrisma(c.env.DB);
  const creds = await prisma.credential.findMany({
    where: { userId },
    select: { id: true, createdAt: true },
    orderBy: { createdAt: "desc" },
  });
  return c.json(creds);
});

// Register additional passkey
passkeyManage.post("/register-options", async (c) => {
  const userId = c.get("userId");
  const prisma = getPrisma(c.env.DB);

  const user = await prisma.user.findUnique({
    where: { id: userId },
    select: { id: true, email: true, name: true, credentials: { select: { credentialId: true, transports: true } } },
  });
  if (!user) return c.json({ error: "not_found" }, 404);

  const { rpId, rpName } = getRpInfo(c);

  const options = await generateRegistrationOptions({
    rpName,
    rpID: rpId,
    userName: user.email,
    userDisplayName: user.name || user.email,
    attestationType: "none",
    excludeCredentials: user.credentials.map((cr) => ({
      id: cr.credentialId,
      transports: cr.transports
        ? (JSON.parse(cr.transports) as AuthenticatorTransportFuture[])
        : undefined,
    })),
    authenticatorSelection: {
      residentKey: "preferred",
      userVerification: "preferred",
    },
  });

  setChallenge(c, options.challenge);
  return c.json(options);
});

passkeyManage.post("/register-verify", async (c) => {
  const userId = c.get("userId");
  const body = await c.req.json().catch(() => null);
  if (!body) return c.json({ error: "invalid_body" }, 400);

  const challenge = getChallenge(c);
  if (!challenge) return c.json({ error: "challenge_expired" }, 400);
  clearChallenge(c);

  const { rpId, origin } = getRpInfo(c);

  let verification;
  try {
    verification = await verifyRegistrationResponse({
      response: body,
      expectedChallenge: challenge,
      expectedOrigin: origin,
      expectedRPID: rpId,
    });
  } catch {
    return c.json({ error: "verification_failed" }, 400);
  }

  if (!verification.verified || !verification.registrationInfo) {
    return c.json({ error: "verification_failed" }, 400);
  }

  const { credential } = verification.registrationInfo;
  const prisma = getPrisma(c.env.DB);

  const created = await prisma.credential.create({
    data: {
      userId,
      credentialId: Buffer.from(credential.id).toString("base64url"),
      publicKey: Buffer.from(credential.publicKey).toString("base64url"),
      counter: credential.counter,
      transports: body.response?.transports
        ? JSON.stringify(body.response.transports)
        : null,
    },
    select: { id: true, createdAt: true },
  });

  return c.json(created, 201);
});

// Delete a passkey
passkeyManage.delete("/:id", async (c) => {
  const userId = c.get("userId");
  const credId = c.req.param("id");
  const prisma = getPrisma(c.env.DB);

  // Ensure user has at least 2 credentials before deleting
  const count = await prisma.credential.count({ where: { userId } });
  if (count <= 1) {
    return c.json({ error: "last_credential" }, 400);
  }

  await prisma.credential.deleteMany({
    where: { id: credId, userId },
  });
  return c.json({ ok: true });
});
