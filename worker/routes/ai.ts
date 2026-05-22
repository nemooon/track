import { Hono } from "hono";
import { getPrisma } from "../db";
import type { Env, AuthVars } from "../types";

const ai = new Hono<{ Bindings: Env; Variables: AuthVars }>();

ai.get("/cheer", async (c) => {
  const userId = c.get("userId");
  const prisma = getPrisma(c.env.DB);
  const now = new Date();
  const weekStart = new Date(now);
  weekStart.setHours(0, 0, 0, 0);
  const dayOfWeek = (weekStart.getDay() + 6) % 7;
  weekStart.setDate(weekStart.getDate() - dayOfWeek);

  const entries = await prisma.timeEntry.findMany({
    where: { userId, start: { gte: weekStart, lt: now } },
    include: { project: true, tags: { include: { tag: true } } },
  });

  if (entries.length === 0) {
    return c.json({
      message: "今週はまだ記録ゼロ。さあ、最初の1件をどうぞ。",
    });
  }

  const projectHours = new Map<string, number>();
  const tagHours = new Map<string, number>();
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
  }

  const topProjects = [...projectHours.entries()]
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5);
  const topTags = [...tagHours.entries()]
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5);

  const todayLabel = ["月", "火", "水", "木", "金", "土", "日"][dayOfWeek];
  const remainingWeekdays = Math.max(0, 4 - dayOfWeek);
  const lines = [
    `期間: 今週（月曜開始）`,
    `今日: ${todayLabel}曜日（週の${dayOfWeek + 1}日目）`,
    `今週の残り平日: ${remainingWeekdays}日（今日を除く、金曜まで）`,
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
            "あなたはユーザーの今週（月曜始まり）の工数記録を眺めて、ユーモアと少しの皮肉を交えつつ応援する友達です。まずはプロジェクトの傾向や総時間といった全体像を主役にしてください。タグはあくまで補足情報で、特に偏りが目立つときだけ軽く触れる程度。タグ名を羅列したり、タグの話に終始したりしないこと。入力には「今日」の曜日と「今週の残り平日」が含まれます。週の立ち位置は必ずこの2つで判断し、「今週まだまだあるよ」のように残り日数の感覚を間違えないこと。残り0日（金曜・週末）なら週の締めとして振り返るトーンにすること。ユーザーは基本的にフルタイムで働いている前提なので、稼働時間が経過日数の割に少ない場合は「サボってる」ではなく「入力が漏れているのでは？記録つけ忘れに注意」というニュアンスで指摘してください。月曜・火曜で総時間がまだ少ない時は単に週の入り口なので、入力漏れの指摘はせず、週の始まりを前向きに後押しする一言にしてください。表現は毎回変えて、「ぼちぼち」のような特定の決まり文句を繰り返さないこと。軽口を交えた2文以内の日本語メッセージを返してください。前置きや解説は不要、メッセージ本文だけを返してください。絵文字は使わないこと。",
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
