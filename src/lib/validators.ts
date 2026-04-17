import { z } from "zod";

export const emailSchema = z.string().email().max(200);
export const passwordSchema = z.string().min(8).max(200);

export const signupSchema = z.object({
  email: emailSchema,
  password: passwordSchema,
  name: z.string().max(100).optional(),
});

export const loginSchema = z.object({
  email: emailSchema,
  password: passwordSchema,
});

export const clientCreateSchema = z.object({
  name: z.string().min(1).max(100),
});

export const clientUpdateSchema = z.object({
  name: z.string().min(1).max(100).optional(),
  archived: z.boolean().optional(),
});

export const projectCreateSchema = z.object({
  clientId: z.string().min(1),
  name: z.string().min(1).max(100),
  color: z.string().regex(/^#[0-9a-fA-F]{6}$/),
});

export const projectUpdateSchema = z.object({
  clientId: z.string().min(1).optional(),
  name: z.string().min(1).max(100).optional(),
  color: z
    .string()
    .regex(/^#[0-9a-fA-F]{6}$/)
    .optional(),
  archived: z.boolean().optional(),
});

export const entryCreateSchema = z
  .object({
    projectId: z.string().min(1).nullable().optional(),
    start: z.string().datetime(),
    end: z.string().datetime(),
    title: z.string().max(100).optional(),
    note: z.string().max(500).optional(),
  })
  .refine((v) => new Date(v.end) > new Date(v.start), "end must be after start");

export const entryUpdateSchema = z
  .object({
    projectId: z.string().min(1).nullable().optional(),
    start: z.string().datetime().optional(),
    end: z.string().datetime().optional(),
    title: z.string().max(100).optional(),
    note: z.string().max(500).optional(),
  })
  .refine(
    (v) => !v.start || !v.end || new Date(v.end) > new Date(v.start),
    "end must be after start",
  );

export const entryRangeSchema = z.object({
  from: z.string().datetime(),
  to: z.string().datetime(),
});

export const reportsQuerySchema = z.object({
  range: z.enum(["week", "month"]),
  anchor: z.string().datetime(),
  groupBy: z.enum(["client", "project"]),
});

export const settingsUpdateSchema = z.object({
  workStart: z.number().int().min(0).max(23).optional(),
  workEnd: z.number().int().min(0).max(23).optional(),
  workDays: z.array(z.number().int().min(0).max(6)).optional(),
});

export const profileUpdateSchema = z.object({
  name: z.string().max(100).optional(),
  email: z.string().email().max(200).optional(),
});

export const passwordChangeSchema = z.object({
  currentPassword: z.string().min(1),
  newPassword: z.string().min(8).max(200),
});
