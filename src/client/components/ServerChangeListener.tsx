import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";

const ENTRY_QUERY_KEYS = [
  ["entries"],
  ["reports"],
  ["reports-entries"],
] as const;

export function ServerChangeListener() {
  const queryClient = useQueryClient();

  useEffect(() => {
    const source = new EventSource("/api/events");

    const refreshEntryQueries = () => {
      for (const queryKey of ENTRY_QUERY_KEYS) {
        void queryClient.invalidateQueries({ queryKey });
      }
    };

    source.addEventListener("track.connected", refreshEntryQueries);
    source.addEventListener("entries.changed", refreshEntryQueries);

    return () => {
      source.removeEventListener("track.connected", refreshEntryQueries);
      source.removeEventListener("entries.changed", refreshEntryQueries);
      source.close();
    };
  }, [queryClient]);

  return null;
}
