const DB_NAME = "track-workspace";
const STORE = "handles";
const KEY = "workspace-root";

function isSupported(): boolean {
  return typeof window !== "undefined" && "showDirectoryPicker" in window;
}

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains(STORE)) db.createObjectStore(STORE);
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function withStore<T>(
  mode: IDBTransactionMode,
  fn: (store: IDBObjectStore) => IDBRequest<T>,
): Promise<T> {
  const db = await openDb();
  return new Promise<T>((resolve, reject) => {
    const tx = db.transaction(STORE, mode);
    const req = fn(tx.objectStore(STORE));
    req.onsuccess = () => resolve(req.result as T);
    req.onerror = () => reject(req.error);
    tx.oncomplete = () => db.close();
  });
}

export async function getWorkspaceHandle(): Promise<FileSystemDirectoryHandle | null> {
  if (!isSupported()) return null;
  try {
    const v = await withStore<unknown>("readonly", (s) => s.get(KEY));
    return (v as FileSystemDirectoryHandle | undefined) ?? null;
  } catch {
    return null;
  }
}

export async function setWorkspaceHandle(handle: FileSystemDirectoryHandle): Promise<void> {
  await withStore<IDBValidKey>("readwrite", (s) => s.put(handle, KEY));
}

export async function clearWorkspaceHandle(): Promise<void> {
  await withStore<undefined>("readwrite", (s) => s.delete(KEY));
}

export async function ensureWorkspacePermission(
  handle: FileSystemDirectoryHandle,
): Promise<"granted" | "denied"> {
  type Perm = { queryPermission?: (o: { mode: "read" }) => Promise<PermissionState>; requestPermission?: (o: { mode: "read" }) => Promise<PermissionState> };
  const h = handle as unknown as Perm;
  const queried = (await h.queryPermission?.({ mode: "read" })) ?? "prompt";
  if (queried === "granted") return "granted";
  const requested = (await h.requestPermission?.({ mode: "read" })) ?? "denied";
  return requested === "granted" ? "granted" : "denied";
}

export async function pickWorkspaceFolder(): Promise<FileSystemDirectoryHandle | null> {
  if (!isSupported()) return null;
  try {
    const handle = await (window as unknown as {
      showDirectoryPicker: (opts?: { mode?: "read" }) => Promise<FileSystemDirectoryHandle>;
    }).showDirectoryPicker({ mode: "read" });
    await setWorkspaceHandle(handle);
    return handle;
  } catch {
    return null;
  }
}

export const workspaceSupported = isSupported;
