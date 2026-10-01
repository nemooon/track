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

export type Note = {
  id: string;
  projectId: string | null;
  project: Project | null;
  title: string;
  content: string;
  archived: boolean;
  pinned: boolean;
  createdAt: string;
  updatedAt: string;
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
  note: string | null;
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

export type ReportCopyDelimiter = "tab" | "comma";

export type ReportDurationFormat =
  | "hours-minutes"
  | "japanese"
  | "decimal-with-unit"
  | "decimal";

export type ReportCopyTarget = "entries" | "ai-aggregation";

export type ReportCopyField =
  | "date"
  | "start"
  | "end"
  | "client"
  | "project"
  | "title"
  | "note"
  | "tags"
  | "duration"
  | "durationMinutes"
  | "percentage"
  | "category"
  | "summary"
  | "entryCount";

export type ReportCopyColumn =
  | {
      id: string;
      kind: "field";
      field: ReportCopyField;
      label?: string;
      durationFormat?: ReportDurationFormat;
    }
  | {
      id: string;
      kind: "blank";
      label?: string;
    }
  | {
      id: string;
      kind: "ai";
      label: string;
      prompt: string;
    };

export type ReportCopyFormat = {
  id: string;
  name: string;
  target: ReportCopyTarget;
  delimiter: ReportCopyDelimiter;
  includeHeader: boolean;
  aiPrompt: string;
  columns: ReportCopyColumn[];
};

export type UserSettings = {
  workStart: number;
  workEnd: number;
  workDays: number[];
  weeklyReportTemplate: string;
  reportCopyFormats: ReportCopyFormat[];
};

export type AiProviderId =
  | "apple-intelligence"
  | "codex"
  | "custom-command";

export type AiGenerationMode =
  | "weekly-report"
  | "report-aggregation"
  | "note-title";

export type AiGenerateResponse = {
  text: string;
  provider: AiProviderId;
};

export type AiProgressUpdate = {
  kind: "status" | "reasoning";
  message: string;
};

export type AiGenerateStreamEvent =
  | ({ type: "progress" } & AiProgressUpdate)
  | { type: "heartbeat" }
  | { type: "result"; text: string; provider: AiProviderId }
  | { type: "error"; message: string };

export type AiProviderStatus = {
  provider: AiProviderId;
  label: string;
  available: boolean;
  detail: string;
};

export type Snapshot = {
  name: string;
  path: string;
  bytes: number;
  createdAt: string;
  auto: boolean;
};

export type AppConfig = {
  exportDir: string;
  backupIntervalHours: number;
  backupKeep: number;
  aiProvider: AiProviderId;
  aiCodexExecutable: string;
  aiCodexModel: string;
  aiCommandExecutable: string;
  aiCommandArgs: string[];
  defaults: {
    exportDir: string;
    backupIntervalHours: number;
    backupKeep: number;
    aiProvider: AiProviderId;
    aiCodexExecutable: string;
    aiCodexModel: string;
    aiCommandExecutable: string;
    aiCommandArgs: string[];
  };
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
