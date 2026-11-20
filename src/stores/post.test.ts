import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { usePostStore } from "./post";
import * as api from "../api/tauri";
import type { Post } from "../api/tauri";

vi.mock("../api/tauri", () => ({
  loadPosts: vi.fn(),
  loadPost: vi.fn(),
  deletePost: vi.fn(),
  savePost: vi.fn(),
  newDraft: vi.fn((id: string) => ({
    id,
    title: "",
    slug: "",
    content: "",
    status: "draft",
    tags: [],
    category: "",
    cover: "",
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
  })),
}));

const mockedSave = vi.mocked(api.savePost);

/** 让 savePost 挂起，返回的数组用于手动结算每次保存 */
function deferSaves(): Array<() => void> {
  const pending: Array<() => void> = [];
  mockedSave.mockImplementation(
    (post) =>
      new Promise<Post>((resolve) => {
        pending.push(() => resolve({ ...post }));
      }),
  );
  return pending;
}

function startEditing(content = "a") {
  const store = usePostStore();
  store.initNewPost("post-1");
  store.current!.content = content;
  store.markDirty();
  return store;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.useFakeTimers();
  mockedSave.mockReset();
});

afterEach(() => {
  vi.clearAllTimers();
  vi.useRealTimers();
});

describe("post store 自动保存状态机", () => {
  it("保存进行中的编辑不会被旧快照覆盖，并会自动补存", async () => {
    const pending = deferSaves();
    const store = startEditing("a");

    await vi.advanceTimersByTimeAsync(1000);
    expect(store.saveState).toBe("saving");
    expect(mockedSave).toHaveBeenCalledTimes(1);

    // 保存未完成时继续编辑
    store.current!.content = "ab";
    store.markDirty();
    expect(store.saveState).toBe("saving");

    pending[0]();
    await vi.advanceTimersByTimeAsync(0);
    // 旧快照绝不覆盖新输入，等待补存
    expect(store.current!.content).toBe("ab");
    expect(store.saveState).toBe("pending");

    await vi.advanceTimersByTimeAsync(300);
    expect(mockedSave).toHaveBeenCalledTimes(2);
    expect(mockedSave.mock.calls[1][0].content).toBe("ab");

    pending[1]();
    await vi.advanceTimersByTimeAsync(0);
    expect(store.saveState).toBe("saved");
  });

  it("保存进行中切换文章，不会覆盖新文章内容", async () => {
    const pending = deferSaves();
    const store = startEditing("old");

    await vi.advanceTimersByTimeAsync(1000);
    expect(store.saveState).toBe("saving");

    // 保存期间切到另一篇文章
    store.initNewPost("post-2");
    store.current!.content = "new";

    pending[0]();
    await vi.advanceTimersByTimeAsync(0);
    expect(store.current!.id).toBe("post-2");
    expect(store.current!.content).toBe("new");
    expect(store.saveState).toBe("pending");

    // 补存针对的是当前文章
    await vi.advanceTimersByTimeAsync(300);
    expect(mockedSave).toHaveBeenCalledTimes(2);
    expect(mockedSave.mock.calls[1][0].id).toBe("post-2");
  });

  it("保存失败时进入 error 状态并保留编辑内容", async () => {
    mockedSave.mockRejectedValueOnce(new Error("disk full"));
    const store = startEditing("keep me");

    await vi.advanceTimersByTimeAsync(1000);

    expect(store.saveState).toBe("error");
    expect(store.lastError).toContain("disk full");
    expect(store.current!.content).toBe("keep me");
  });

  it("saveNow 在持续编辑时会多轮冲刷直到内容稳定", async () => {
    const store = usePostStore();
    let rounds = 0;
    mockedSave.mockImplementation(async (post) => {
      rounds++;
      if (rounds < 3) {
        // 模拟一轮保存期间又产生新编辑
        store.current!.content += "!";
        store.markDirty();
      }
      return { ...post };
    });

    store.initNewPost("post-1");
    store.current!.content = "a";
    store.markDirty();
    await store.saveNow();

    expect(rounds).toBe(3);
    expect(store.current!.content).toBe("a!!");
    expect(store.saveState).toBe("saved");
  });
});
