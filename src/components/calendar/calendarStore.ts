import { create } from "zustand";

export const ZOOM_LEVELS = [
  { hourPx: 48, snapLabel: "1h" },
  { hourPx: 96, snapLabel: "30m" },
  { hourPx: 192, snapLabel: "15m" },
] as const;

interface CalendarState {
  selectedEntryId: string | null;
  setSelected: (id: string | null) => void;
  zoomIndex: number;
  zoomIn: () => void;
  zoomOut: () => void;
}

export const useCalendarStore = create<CalendarState>((set) => ({
  selectedEntryId: null,
  setSelected: (id) => set({ selectedEntryId: id }),
  zoomIndex: 0,
  zoomIn: () =>
    set((s) => ({ zoomIndex: Math.min(s.zoomIndex + 1, ZOOM_LEVELS.length - 1) })),
  zoomOut: () =>
    set((s) => ({ zoomIndex: Math.max(s.zoomIndex - 1, 0) })),
}));
