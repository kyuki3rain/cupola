import { describe, it, expect, afterEach } from "vitest";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { addCommand } from "../../src/commands/add.js";
import { load } from "../../src/storage.js";

let tmpDir: string;

function makeTmp(): string {
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "cupola-e2e-add-"));
  return tmpDir;
}

afterEach(() => {
  if (tmpDir && fs.existsSync(tmpDir)) {
    fs.rmSync(tmpDir, { recursive: true });
  }
});

describe("addCommand", () => {
  it("adds a task to a new file", () => {
    const dir = makeTmp();
    const filePath = path.join(dir, "todo.json");
    addCommand(filePath, "buy milk");
    const tasks = load(filePath);
    expect(tasks).toHaveLength(1);
    expect(tasks[0]?.title).toBe("buy milk");
    expect(tasks[0]?.status).toBe("open");
    expect(tasks[0]?.id).toBe(1);
  });

  it("increments id for subsequent tasks", () => {
    const dir = makeTmp();
    const filePath = path.join(dir, "todo.json");
    addCommand(filePath, "task one");
    addCommand(filePath, "task two");
    const tasks = load(filePath);
    expect(tasks).toHaveLength(2);
    expect(tasks[1]?.id).toBe(2);
  });
});
