import { describe, it, expect, afterEach } from "vitest";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { load, save, nextId } from "../src/storage.js";
import type { Task } from "../src/task.js";

let tmpDir: string;

function makeTmp(): string {
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "cupola-e2e-test-"));
  return tmpDir;
}

afterEach(() => {
  if (tmpDir && fs.existsSync(tmpDir)) {
    fs.rmSync(tmpDir, { recursive: true });
  }
});

const sampleTask: Task = {
  id: 1,
  title: "hello",
  status: "open",
  createdAt: "2024-01-01T00:00:00.000Z",
};

describe("load", () => {
  it("returns empty array when file does not exist", () => {
    const dir = makeTmp();
    const result = load(path.join(dir, "todo.json"));
    expect(result).toEqual([]);
  });

  it("loads tasks from a valid JSON file", () => {
    const dir = makeTmp();
    const filePath = path.join(dir, "todo.json");
    fs.writeFileSync(filePath, JSON.stringify([sampleTask]), "utf-8");
    const result = load(filePath);
    expect(result).toHaveLength(1);
    expect(result[0]?.title).toBe("hello");
  });

  it("returns empty array for invalid JSON", () => {
    const dir = makeTmp();
    const filePath = path.join(dir, "todo.json");
    fs.writeFileSync(filePath, "not-json", "utf-8");
    const result = load(filePath);
    expect(result).toEqual([]);
  });
});

describe("save", () => {
  it("writes tasks as JSON", () => {
    const dir = makeTmp();
    const filePath = path.join(dir, "todo.json");
    save(filePath, [sampleTask]);
    const raw = fs.readFileSync(filePath, "utf-8");
    const parsed = JSON.parse(raw) as Task[];
    expect(parsed).toHaveLength(1);
    expect(parsed[0]?.id).toBe(1);
  });

  it("overwrites existing file", () => {
    const dir = makeTmp();
    const filePath = path.join(dir, "todo.json");
    save(filePath, [sampleTask]);
    save(filePath, []);
    const result = load(filePath);
    expect(result).toHaveLength(0);
  });
});

describe("nextId", () => {
  it("returns 1 for empty list", () => {
    expect(nextId([])).toBe(1);
  });

  it("returns max id + 1", () => {
    const tasks: Task[] = [
      { id: 3, title: "c", status: "open", createdAt: "" },
      { id: 1, title: "a", status: "open", createdAt: "" },
    ];
    expect(nextId(tasks)).toBe(4);
  });
});
