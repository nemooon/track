"""Startup / memory / idle-CPU comparison: GPUI prototype vs installed Tauri Track.app.
Read-only for both apps (no input is sent here)."""
import hashlib, json, os, re, statistics, subprocess, sys, time

S = os.path.dirname(os.path.abspath(__file__))
WINPID = os.path.join(S, "winpid")
GPUI = os.path.join(S, "..", "target", "release", "track-gpui")
SHOT = os.path.join("/tmp", "track-gpui-bench-shot.png")

def sh(*a):
    return subprocess.run(a, capture_output=True, text=True).stdout

def window_id(pid):
    r = subprocess.run([WINPID, str(pid)], capture_output=True, text=True)
    return r.stdout.strip() if r.returncode == 0 else None

def shot_hash(wid):
    subprocess.run(["screencapture", "-x", "-o", "-l" + wid, SHOT], capture_output=True)
    try:
        return hashlib.md5(open(SHOT, "rb").read()).hexdigest()
    except FileNotFoundError:
        return None

def wait_ready(pid, t0, timeout=30):
    """Returns (window_seconds, stable_seconds): window on screen, then 3 identical captures."""
    wid = None
    while wid is None:
        if time.time() - t0 > timeout: raise RuntimeError("no window")
        wid = window_id(pid); 
        if wid is None: time.sleep(0.02)
    t_win = time.time() - t0
    hist = []
    while time.time() - t0 < timeout:
        h = shot_hash(wid); now = time.time() - t0
        hist.append((h, now))
        if len(hist) >= 3 and hist[-1][0] == hist[-2][0] == hist[-3][0]:
            return t_win, hist[-3][1], wid
        time.sleep(0.05)
    raise RuntimeError("never stable")

def footprint_mb(pids):
    out = sh("footprint", *sum([["-p", str(p)] for p in pids], []))
    total = 0.0
    for num, unit in re.findall(r"Footprint: ([\d.]+) (KB|MB|GB)", out):
        total += float(num) * {"KB": 1/1024, "MB": 1, "GB": 1024}[unit]
    # footprint prints one line per process; avoid double counting a summary line if present
    return round(total, 1)

def cpu_seconds(pids):
    tot = 0.0
    for p in pids:
        t = sh("ps", "-o", "time=", "-p", str(p)).strip()
        if not t: continue
        parts = t.split(":")
        secs = float(parts[-1]) + 60 * float(parts[-2]) + (3600 * float(parts[-3]) if len(parts) > 2 else 0)
        tot += secs
    return tot

def webkit_pids():
    out = sh("pgrep", "-f", "com.apple.WebKit")
    return {int(x) for x in out.split()}

def tauri_pids():
    main = sh("pgrep", "-f", "Track.app/Contents/MacOS/track$").split()
    srv = sh("pgrep", "-f", "Track.app/Contents/MacOS/track-server").split()
    return [int(x) for x in main], [int(x) for x in srv]

def quit_tauri():
    subprocess.run(["osascript", "-e", 'tell application "Track" to quit'], capture_output=True)
    for _ in range(100):
        m, s = tauri_pids()
        if not m and not s: return
        time.sleep(0.1)
    for p in sum(tauri_pids(), []): os.kill(p, 15)
    time.sleep(1)

def run_tauri(settle=8, idle=10):
    quit_tauri(); time.sleep(2)
    before = webkit_pids()
    t0 = time.time()
    subprocess.run(["open", "-a", "/Applications/Track.app"])
    main = []
    while not main:
        main, _ = tauri_pids(); time.sleep(0.01)
    t_win, t_ready, wid = wait_ready(main[0], t0)
    time.sleep(settle)
    m, s = tauri_pids()
    wk = sorted(webkit_pids() - before)
    pids = m + s + wk
    mem = {"app": footprint_mb(m), "server": footprint_mb(s), "webkit": footprint_mb(wk)}
    c0 = cpu_seconds(pids); time.sleep(idle); c1 = cpu_seconds(pids)
    return {"window_s": round(t_win, 2), "ready_s": round(t_ready, 2), "mem_mb": mem, "mem_total_mb": round(sum(mem.values()), 1),
            "idle_cpu_pct": round((c1 - c0) / idle * 100, 2), "pids": pids, "wid": wid}

def run_gpui(settle=8, idle=10, keep=False):
    env = dict(os.environ, TRACK_GPUI_WINDOW="1440x900")
    t0 = time.time()
    p = subprocess.Popen([GPUI], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    t_win, t_ready, wid = wait_ready(p.pid, t0)
    time.sleep(settle)
    server = [int(x) for x in sh("lsof", "-nP", "-iTCP:8787", "-sTCP:LISTEN", "-t").split()]
    mem = {"app": footprint_mb([p.pid]), "server": footprint_mb(server)}
    c0 = cpu_seconds([p.pid]); time.sleep(idle); c1 = cpu_seconds([p.pid])
    res = {"window_s": round(t_win, 2), "ready_s": round(t_ready, 2), "mem_mb": mem, "mem_total_mb": round(sum(mem.values()), 1),
           "idle_cpu_pct": round((c1 - c0) / idle * 100, 2), "pid": p.pid, "wid": wid}
    if not keep:
        p.terminate(); p.wait()
    return res

if __name__ == "__main__":
    which, n = sys.argv[1], int(sys.argv[2])
    runs = []
    for i in range(n):
        r = run_gpui() if which == "gpui" else run_tauri()
        print(json.dumps(r, ensure_ascii=False), flush=True)
        runs.append(r)
    med = lambda k: statistics.median(r[k] for r in runs)
    print("MEDIAN", which, json.dumps({k: med(k) for k in ["window_s", "ready_s", "mem_total_mb", "idle_cpu_pct"]}))
