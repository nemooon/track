import { create } from "zustand";

interface CalendarState {
  selectedEntryId: string | null;
  setSelected: (id: string | null) => void;
}

export const useCalendarStore = create<CalendarState>((set) => ({
  selectedEntryId: null,
  setSelected: (id) => set({ selectedEntryId: id }),
}));
