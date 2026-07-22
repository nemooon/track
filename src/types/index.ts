export type Client = {
  id: string;
  name: string;
  archived: boolean;
};

export type Project = {
  id: string;
  clientId: string;
  name: string;
  color: string;
  archived: boolean;
  client: Client;
  tags?: TagOnProject[];
};

export type Tag = {
  id: string;
  name: string;
  color: string;
};

export type TagOnEntry = {
  tagId: string;
  tag: Tag;
};

export type TagOnProject = {
  tagId: string;
  tag: Tag;
};

export type TimeEntry = {
  id: string;
  projectId: string | null;
  start: string; // ISO
  end: string; // ISO
  title: string | null;
  note: string | null;
  project: Project | null;
  tags: TagOnEntry[];
  externalEventId: string | null;
  externalEventSource: ExternalEventSource | null;
  breakMinutes: number;
};

export type ReportRow = {
  key: string;
  label: string;
  color?: string;
  totalMinutes: number;
};

export type ReportResponse = {
  rows: ReportRow[];
  totalMinutes: number;
  range: { from: string; to: string };
};

export type ReportEntry = {
  id: string;
  start: string;
  end: string;
  minutes: number;
  title: string | null;
  project: {
    id: string;
    name: string;
    color: string;
    client: { id: string; name: string };
  } | null;
  tags: Tag[];
};

export type ReportEntriesResponse = {
  entries: ReportEntry[];
  totalMinutes: number;
};

export type UserSettings = {
  workStart: number;
  workEnd: number;
  workDays: number[];
};

export type ExternalEventSource = "kot" | "outlook";

export type ExternalEventKind =
  | "timecard-in"
  | "timecard-out"
  | "schedule-allday"
  | "schedule-halfday"
  | "meeting";

export type ExternalEvent = {
  id: string;
  source: ExternalEventSource;
  kind: ExternalEventKind;
  start: string; // ISO
  end: string; // ISO (== start for timecard pins)
  label: string;
  readOnly: true;
};
