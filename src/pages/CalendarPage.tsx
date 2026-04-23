import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { WeekCalendar } from "@/components/calendar/WeekCalendar";
import { apiFetch } from "@/lib/fetcher";
import type { Project, Tag, UserSettings } from "@/types";

export function CalendarPage() {
  const [anchor, setAnchor] = useState(() => new Date());

  const { data: projects = [], isLoading: projectsLoading } = useQuery({
    queryKey: ["projects"],
    queryFn: () => apiFetch<Project[]>("/api/projects"),
  });

  const { data: tags = [], isLoading: tagsLoading } = useQuery({
    queryKey: ["tags"],
    queryFn: () => apiFetch<Tag[]>("/api/tags"),
  });

  const { data: settings, isLoading: settingsLoading } = useQuery({
    queryKey: ["account"],
    queryFn: () => apiFetch<UserSettings>("/api/account"),
  });

  if (projectsLoading || tagsLoading || settingsLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-6 w-6 animate-spin rounded-full border-2 border-neutral-300 border-t-neutral-700" />
      </div>
    );
  }

  return (
    <WeekCalendar
      anchor={anchor}
      onNavigate={setAnchor}
      projects={projects}
      tags={tags}
      workStart={settings?.workStart}
      workEnd={settings?.workEnd}
      workDays={settings?.workDays}
    />
  );
}
