"""Fills an empty demo home with made-up clients, projects, tags and a week of entries
(2026-11-16 to 19), for the README's screenshots. No real data.

    cargo build --workspace
    python3 scripts/seed-demo.py /tmp/demohome
    HOME=/tmp/demohome TRACK_DATA_DIR=/tmp/demohome/.track-gpui TRACK_GPUI_WINDOW=1440x900 \
      TRACK_GPUI_APPEARANCE=light ./target/debug/track-gpui        # then scripts/winshot.sh
"""
import json, os, subprocess, sys
D = sys.argv[1]
CLI = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "target", "debug", "track-cli")
env = dict(os.environ, HOME=D, TRACK_DATA_DIR=D + "/.track-gpui")
def cli(*a):
    r = subprocess.run([CLI, *a], env=env, capture_output=True, text=True)
    out = json.loads(r.stdout)
    if not out.get("ok", True):
        raise SystemExit(f"{a}: {out}")
    return out
def find_id(o):
    if isinstance(o, dict):
        for k in ("id",):
            if k in o and isinstance(o[k], str): return o[k]
        for v in o.values():
            r = find_id(v)
            if r: return r
    if isinstance(o, list):
        for v in o:
            r = find_id(v)
            if r: return r
tags = {n: find_id(cli("tag", "create", "--name", n, "--color", c)) for n, c in [("打合せ", "#8b5cf6"), ("開発", "#0ea5e9"), ("デザイン", "#f97316"), ("事務", "#64748b")]}
clients = {n: find_id(cli("client", "create", "--name", n, "--confirmed")) for n in ["アオバ製作所", "ミナト商事", "社内"]}
projects = {}
for key, client, name, color in [
    ("web", "アオバ製作所", "コーポレートサイト改修", "#2563eb"),
    ("sys", "ミナト商事", "受発注システム保守", "#16a34a"),
    ("rec", "ミナト商事", "採用サイト", "#ea580c"),
    ("in", "社内", "定例・事務", "#7c3aed"),
]:
    projects[key] = find_id(cli("project", "create", "--client-id", clients[client], "--name", name, "--color", color, "--confirmed"))
E = [
    # (day, start, end, project, title, tag)
    (16, "10:00", "10:30", "in", "朝会", "打合せ"),
    (16, "10:30", "12:30", "web", "トップページのワイヤー作成", "デザイン"),
    (16, "13:30", "15:00", "web", "お知らせ一覧のコーディング", "開発"),
    (16, "15:00", "16:00", "sys", "問い合わせ対応", None),
    (16, "16:00", "18:00", "sys", "発注 CSV の取込み修正", "開発"),
    (17, "10:00", "10:30", "in", "朝会", "打合せ"),
    (17, "10:30", "12:00", "rec", "採用サイトの構成案", "デザイン"),
    (17, "13:00", "14:00", "rec", "採用担当者と打合せ", "打合せ"),
    (17, "14:00", "17:30", "web", "お知らせ一覧のコーディング", "開発"),
    (17, "17:30", "18:30", "in", "経費精算", "事務"),
    (18, "10:00", "10:30", "in", "朝会", "打合せ"),
    (18, "10:30", "13:00", "sys", "在庫画面の検索を速くする", "開発"),
    (18, "14:00", "15:00", "web", "デザインレビュー", "打合せ"),
    (18, "15:00", "18:30", "rec", "募集要項ページのデザイン", "デザイン"),
    (19, "10:00", "10:30", "in", "朝会", "打合せ"),
    (19, "10:30", "12:30", "sys", "在庫画面の検索を速くする", "開発"),
    (19, "13:30", "15:30", "web", "フォームの入力チェック", "開発"),
    (19, "15:30", "16:30", "rec", "写真の選定", "デザイン"),
]
for day, s, e, p, title, tag in E:
    args = ["create", "--start", f"2026-11-{day}T{s}:00+09:00", "--end", f"2026-11-{day}T{e}:00+09:00", "--title", title, "--project-id", projects[p], "--confirmed"]
    if tag: args += ["--tag-id", tags[tag]]
    cli(*args)
print(cli("status"))
