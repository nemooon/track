import subprocess, sys, time
sys.path.insert(0, ".")
import measure
mode, pid, wid, secs, *ui = sys.argv[1:]
pids = [int(x) for x in ui]
c0 = measure.cpu_seconds(pids); t0 = time.time()
out = subprocess.run(["./bench", mode, pid, wid, secs], capture_output=True, text=True).stdout.strip().splitlines()[-1]
c1 = measure.cpu_seconds(pids); dt = time.time() - t0
print(f"{mode}: {out} | UI cpu {round((c1 - c0) / dt * 100, 1)}% (wall {dt:.1f}s)")
