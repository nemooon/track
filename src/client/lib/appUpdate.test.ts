import { afterAll, afterEach, describe, expect, spyOn, test } from "bun:test";
import { getReleaseNotes } from "./appUpdate";

const fetchMock = spyOn(globalThis, "fetch");
afterEach(() => fetchMock.mockReset());
afterAll(() => fetchMock.mockRestore());

describe("release notes", () => {
  test("指定バージョンの本文を取得し、中断シグナルを渡す", async () => {
    fetchMock.mockResolvedValueOnce(Response.json({ tag_name: "v0.6.0", body: "  ## 改善\n\n- 変更内容  " }));
    const signal = new AbortController().signal;
    expect(await getReleaseNotes("0.6.0", signal)).toEqual({
      version: "0.6.0",
      body: "## 改善\n\n- 変更内容",
      url: "https://github.com/nemooon/track/releases/tag/v0.6.0",
    });
    expect(fetchMock).toHaveBeenCalledWith(
      "https://api.github.com/repos/nemooon/track/releases/tags/v0.6.0",
      { headers: { Accept: "application/vnd.github+json" }, signal },
    );
  });

  test("本文がないリリースを空文字として扱う", async () => {
    fetchMock.mockResolvedValueOnce(Response.json({ tag_name: "v0.6.0", body: null }));
    expect((await getReleaseNotes("0.6.0"))?.body).toBe("");
  });

  test("未公開のバージョンはnullを返す", async () => {
    fetchMock.mockResolvedValueOnce(new Response(null, { status: 404 }));
    expect(await getReleaseNotes("0.6.0")).toBeNull();
  });

  test("APIの制限やサーバーエラーは取得失敗として扱う", async () => {
    fetchMock.mockResolvedValueOnce(new Response(null, { status: 403 }));
    await expect(getReleaseNotes("0.6.0")).rejects.toThrow("GitHub Releases API: 403");
  });

  test("異なるバージョンの本文を表示しない", async () => {
    fetchMock.mockResolvedValueOnce(Response.json({ tag_name: "v0.7.0", body: "別バージョン" }));
    await expect(getReleaseNotes("0.6.0")).rejects.toThrow("リリースのバージョンが一致しません");
  });
});
