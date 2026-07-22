import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { WeekCalendar } from "@client/components/calendar/WeekCalendar";
import { apiFetch } from "@client/lib/fetcher";
import type { Project, Tag, UserSettings } from "@shared/types";

export function CalendarPage() {
  const [anchor, setAnchor] = useState(() => new Date());
  const [dayCount, setDayCount] = useState<1 | 3 | "week" | 7>("week");

  const { data: projects = [], isLoading: projectsLoading } = useQuery({
    queryKey: ["projects"],
    queryFn: () => apiFetch<Project[]>("/api/projects"),
  });

  const { data: tags = [], isLoading: tagsLoading } = useQuery({
    queryKey: ["tags"],
    queryFn: () => apiFetch<Tag[]>("/api/tags"),
  });

  const { data: settings, isLoading: settingsLoading } = useQuery({
    queryKey: ["settings"],
    queryFn: () => apiFetch<UserSettings>("/api/settings"),
  });

  if (projectsLoading || tagsLoading || settingsLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-6 w-6 animate-spin rounded-full border-2 border-neutral-300 border-t-neutral-700" />
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <div className="min-h-0 flex-1">
        <WeekCalendar
          anchor={anchor}
          onNavigate={setAnchor}
          dayCount={dayCount}
          onDayCountChange={setDayCount}
          projects={projects}
          tags={tags}
          workStart={settings?.workStart}
          workEnd={settings?.workEnd}
          workDays={settings?.workDays}
        />
      </div>
    </div>
  );
}
