import { describe, it, expect, afterEach, vi } from "vitest";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { listCommand } from "../../src/commands/list.js";
import { save } from "../../src/storage.js";
import type { Task } from "../../src/task.js";

let tmpDir: string;

function makeTmp(): string {
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "cupola-e2e-list-"));
  return tmpDir;
}

afterEach(() => {
  if (tmpDir && fs.existsSync(tmpDir)) {
    fs.rmSync(tmpDir, { recursive: true });
  }
  vi.restoreAllMocks();
});

describe("listCommand", () => {
  it("prints 'No tasks.' when file is empty", () => {
    const dir = makeTmp();
    const filePath = path.join(dir, "todo.json");
    const spy = vi.spyOn(console, "log").mockImplementation(() => undefined);
    listCommand(filePath);
    expect(spy).toHaveBeenCalledWith("No tasks.");
  });

  it("prints tasks with badges", () => {
    const dir = makeTmp();
    const filePath = path.join(dir, "todo.json");
    const tasks: Task[] = [
      { id: 1, title: "open task", status: "open", createdAt: "" },
      { id: 2, title: "done task", status: "done", createdAt: "" },
    ];
    save(filePath, tasks);
    const spy = vi.spyOn(console, "log").mockImplementation(() => undefined);
    listCommand(filePath);
    expect(spy).toHaveBeenCalledWith("[ ] #1: open task");
    expect(spy).toHaveBeenCalledWith("[x] #2: done task");
  });
});
