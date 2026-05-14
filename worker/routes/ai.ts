import { Hono } from "hono";
import { getPrisma } from "../db";
import type { Env, AuthVars } from "../types";

const ai = new Hono<{ Bindings: Env; Variables: AuthVars }>();

ai.get("/cheer", async (c) => {
  const userId = c.get("userId");
  const prisma = getPrisma(c.env.DB);
  const now = new Date();
  const from = new Date(now.getTime() - 14 * 24 * 60 * 60 * 1000);

  const entries = await prisma.timeEntry.findMany({
    where: { userId, start: { gte: from, lt: now } },
    include: { project: true, tags: { include: { tag: true } } },
  });

  if (entries.length === 0) {
    return c.json({
      message: "ここ2週間、記録ゼロ。まずは1件、何か残してみない？",
    });
  }

  const projectHours = new Map<string, number>();
  const tagHours = new Map<string, number>();
  const activeDays = new Set<string>();
  let totalHours = 0;

  for (const e of entries) {
    const start = new Date(e.start).getTime();
    const end = new Date(e.end).getTime();
    const breakMs = (e.breakMinutes ?? 0) * 60 * 1000;
    const hours = Math.max(0, (end - start - breakMs) / 3_600_000);
    if (hours === 0) continue;
    totalHours += hours;
    const projectName = e.project?.name ?? "未分類";
    projectHours.set(projectName, (projectHours.get(projectName) ?? 0) + hours);
    for (const t of e.tags) {
      tagHours.set(t.tag.name, (tagHours.get(t.tag.name) ?? 0) + hours);
    }
    activeDays.add(new Date(e.start).toISOString().slice(0, 10));
  }

  const topProjects = [...projectHours.entries()]
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5);
  const topTags = [...tagHours.entries()]
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5);

  const lines = [
    `期間: 過去14日`,
    `稼働日数: ${activeDays.size}日`,
    `合計: ${totalHours.toFixed(1)}時間`,
    `プロジェクト別:`,
    ...topProjects.map(([name, h]) => `  - ${name}: ${h.toFixed(1)}時間`),
  ];
  if (topTags.length > 0) {
    lines.push("タグ別:");
    for (const [name, h] of topTags) {
      lines.push(`  - ${name}: ${h.toFixed(1)}時間`);
    }
  }
  const summary = lines.join("\n");

  const result = (await c.env.AI.run(
    "@cf/meta/llama-3.3-70b-instruct-fp8-fast" as never,
    {
      messages: [
        {
          role: "system",
          content:
            "あなたはユーザーの工数記録を眺めて、ユーモアと少しの皮肉を交えつつ応援する友達です。記録から何を頑張っているか、または何をサボっているかを汲み取り、軽口を交えた2文以内の日本語メッセージを返してください。前置きや解説は不要、メッセージ本文だけを返してください。絵文字は使わないこと。",
        },
        { role: "user", content: summary },
      ],
      max_tokens: 200,
    } as never,
  )) as { response?: string };

  const message = result.response?.trim() || "今日もおつかれさま。";
  return c.json({ message });
});

export { ai };
