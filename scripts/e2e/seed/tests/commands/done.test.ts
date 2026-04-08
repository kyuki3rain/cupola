import { describe, it, expect, afterEach } from "vitest";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { doneCommand } from "../../src/commands/done.js";
import { save, load } from "../../src/storage.js";
import type { Task } from "../../src/task.js";

let tmpDir: string;

function makeTmp(): string {
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "cupola-e2e-done-"));
  return tmpDir;
}

afterEach(() => {
  if (tmpDir && fs.existsSync(tmpDir)) {
    fs.rmSync(tmpDir, { recursive: true });
  }
});

describe("doneCommand", () => {
  it("marks an existing task as done", () => {
    const dir = makeTmp();
    const filePath = path.join(dir, "todo.json");
    const tasks: Task[] = [{ id: 1, title: "buy milk", status: "open", createdAt: "" }];
    save(filePath, tasks);
    doneCommand(filePath, "1");
    const result = load(filePath);
    expect(result[0]?.status).toBe("done");
  });

  it("exits with code 1 for unknown ID", () => {
    const dir = makeTmp();
    const filePath = path.join(dir, "todo.json");
    save(filePath, []);
    const exitSpy = vi.spyOn(process, "exit").mockImplementation((_code?: number | string) => {
      throw new Error("process.exit called");
    });
    expect(() => doneCommand(filePath, "999")).toThrow("process.exit called");
    exitSpy.mockRestore();
  });
});

// vitest global
import { vi } from "vitest";
