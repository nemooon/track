import { create } from "zustand";
import { persist } from "zustand/middleware";

export const ZOOM_LEVELS = [
  { hourPx: 48, snapLabel: "1h" },
  { hourPx: 96, snapLabel: "30m" },
  { hourPx: 192, snapLabel: "15m" },
] as const;

const DEFAULT_ZOOM_INDEX = 1;

interface CalendarState {
  selectedEntryId: string | null;
  setSelected: (id: string | null) => void;
  zoomIndex: number;
  zoomIn: () => void;
  zoomOut: () => void;
  showKot: boolean;
  toggleShowKot: () => void;
  showOutlook: boolean;
  toggleShowOutlook: () => void;
  showWorkspace: boolean;
  toggleShowWorkspace: () => void;
  workspaceGapMinutes: 30 | 60 | 120;
  setWorkspaceGapMinutes: (m: 30 | 60 | 120) => void;
}

export const useCalendarStore = create<CalendarState>()(
  persist(
    (set) => ({
      selectedEntryId: null,
      setSelected: (id) => set({ selectedEntryId: id }),
      zoomIndex: DEFAULT_ZOOM_INDEX,
      zoomIn: () =>
        set((s) => ({ zoomIndex: Math.min(s.zoomIndex + 1, ZOOM_LEVELS.length - 1) })),
      zoomOut: () =>
        set((s) => ({ zoomIndex: Math.max(s.zoomIndex - 1, 0) })),
      showKot: false,
      toggleShowKot: () => set((s) => ({ showKot: !s.showKot })),
      showOutlook: false,
      toggleShowOutlook: () => set((s) => ({ showOutlook: !s.showOutlook })),
      showWorkspace: false,
      toggleShowWorkspace: () => set((s) => ({ showWorkspace: !s.showWorkspace })),
      workspaceGapMinutes: 60,
      setWorkspaceGapMinutes: (m) => set({ workspaceGapMinutes: m }),
    }),
    {
      name: "track:calendar",
      version: 4,
      migrate: (persisted, _version) => ({
        ...(persisted as Record<string, unknown>),
        showKot: false,
        showOutlook: false,
        showWorkspace: false,
        workspaceGapMinutes: 60,
      }),
      partialize: (s) => ({
        zoomIndex: s.zoomIndex,
        showKot: s.showKot,
        showOutlook: s.showOutlook,
        showWorkspace: s.showWorkspace,
        workspaceGapMinutes: s.workspaceGapMinutes,
      }),
    },
  ),
);
