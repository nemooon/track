export type AppChangeEvent = {
  type: "entries.changed";
  action: "created" | "updated" | "deleted";
  entryId: string;
  occurredAt: string;
};

type ChangeListener = (event: AppChangeEvent) => void;

const listeners = new Set<ChangeListener>();

export function publishEntryChange(
  event: Omit<AppChangeEvent, "type" | "occurredAt">,
): void {
  const change: AppChangeEvent = {
    type: "entries.changed",
    ...event,
    occurredAt: new Date().toISOString(),
  };

  for (const listener of listeners) {
    listener(change);
  }
}

export function subscribeToChanges(listener: ChangeListener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
