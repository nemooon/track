import { useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { apiFetch } from "./fetcher";
import type { TimeEntry, UserSettings } from "@/types";

type FaviconColor = "default" | "green" | "yellow" | "red";

function buildSvgDataUrl(color: FaviconColor): string {
  const fill =
    color === "red"
      ? "#dc2626"
      : color === "yellow"
        ? "#eab308"
        : color === "green"
          ? "#16a34a"
          : "#404040";
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32">
  <rect width="32" height="32" rx="6" fill="${fill}"/>
  <text x="16" y="23" text-anchor="middle" font-size="20" font-weight="bold" fill="white" font-family="system-ui,sans-serif">T</text>
</svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

function setFavicon(color: FaviconColor) {
  let link = document.querySelector<HTMLLinkElement>('link[rel="icon"]');
  if (!link) {
    link = document.createElement("link");
    link.rel = "icon";
    link.type = "image/svg+xml";
    document.head.appendChild(link);
  }
  link.href = buildSvgDataUrl(color);
}

function computeColor(
  entries: TimeEntry[],
  settings: UserSettings,
): FaviconColor {
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());

  const todayEntries = entries.filter((e) => {
    const start = new Date(e.start);
    return (
      start.getFullYear() === today.getFullYear() &&
      start.getMonth() === today.getMonth() &&
      start.getDate() === today.getDate()
    );
  });

  // Check red: past work end AND today's total < (work hours - 1h)
  const nowMinutes = now.getHours() * 60 + now.getMinutes();
  const workDurationMin = settings.workEnd - settings.workStart;
  const thresholdMin = workDurationMin - 60;

  const totalMin = todayEntries.reduce((sum, e) => {
    const s = new Date(e.start).getTime();
    const en = new Date(e.end).getTime();
    return sum + (en - s) / 60000;
  }, 0);

  if (nowMinutes >= settings.workEnd) {
    if (totalMin < thresholdMin) {
      return "red";
    }
    return "green";
  }

  // During work hours: green if already hit the threshold
  if (totalMin >= thresholdMin) {
    return "green";
  }

  // Check yellow: last entry ended more than 1 hour ago
  // Only during work hours on a work day
  const isWorkDay = settings.workDays.includes(now.getDay());
  const inWorkHours =
    nowMinutes >= settings.workStart && nowMinutes < settings.workEnd;

  if (isWorkDay && inWorkHours) {
    if (todayEntries.length === 0) {
      // No entries today — if we're past workStart + 1h, yellow
      if (nowMinutes >= settings.workStart + 60) {
        return "yellow";
      }
    } else {
      const latestEnd = Math.max(
        ...todayEntries.map((e) => new Date(e.end).getTime()),
      );
      const minutesSinceLast = (now.getTime() - latestEnd) / 60000;
      if (minutesSinceLast >= 60) {
        return "yellow";
      }
    }
  }

  return "default";
}

export function useFaviconStatus() {
  const todayStr = new Date().toISOString().slice(0, 10);
  const from = `${todayStr}T00:00:00.000Z`;
  const tomorrow = new Date();
  tomorrow.setDate(tomorrow.getDate() + 1);
  const to = `${tomorrow.toISOString().slice(0, 10)}T00:00:00.000Z`;

  const { data: entries } = useQuery<TimeEntry[]>({
    queryKey: ["entries", "favicon", todayStr],
    queryFn: () =>
      apiFetch<TimeEntry[]>(
        `/api/entries?from=${encodeURIComponent(from)}&to=${encodeURIComponent(to)}`,
      ),
    refetchInterval: 60_000,
  });

  const { data: settings } = useQuery<UserSettings>({
    queryKey: ["account"],
    queryFn: () => apiFetch<UserSettings>("/api/account"),
  });

  useEffect(() => {
    if (!entries || !settings) {
      setFavicon("default");
      return;
    }

    const update = () => setFavicon(computeColor(entries, settings));
    update();

    // Re-evaluate every minute even without new data
    const interval = setInterval(update, 60_000);
    return () => clearInterval(interval);
  }, [entries, settings]);
}
