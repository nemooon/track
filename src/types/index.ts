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
};

export type TimeEntry = {
  id: string;
  projectId: string | null;
  start: string; // ISO
  end: string; // ISO
  title: string | null;
  note: string | null;
  project: Project | null;
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

export type UserSettings = {
  workStart: number;
  workEnd: number;
  workDays: number[];
};

export type UserProfile = {
  id: string;
  email: string;
  name: string | null;
  workStart: number;
  workEnd: number;
  workDays: number[];
};
