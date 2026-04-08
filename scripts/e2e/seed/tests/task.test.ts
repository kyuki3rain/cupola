import { describe, it, expect } from "vitest";
import { createTask, markDone } from "../src/task.js";

describe("createTask", () => {
  it("creates a task with correct fields", () => {
    const now = new Date("2024-01-01T00:00:00.000Z");
    const task = createTask(1, "buy milk", now);
    expect(task.id).toBe(1);
    expect(task.title).toBe("buy milk");
    expect(task.status).toBe("open");
    expect(task.createdAt).toBe("2024-01-01T00:00:00.000Z");
  });

  it("throws when title is empty", () => {
    expect(() => createTask(1, "")).toThrow("title must not be empty");
  });

  it("throws when title is whitespace only", () => {
    expect(() => createTask(1, "   ")).toThrow("title must not be empty");
  });

  it("trims whitespace from title", () => {
    const task = createTask(2, "  hello  ");
    expect(task.title).toBe("hello");
  });
});

describe("markDone", () => {
  it("marks an open task as done", () => {
    const task = createTask(1, "walk dog");
    const done = markDone(task);
    expect(done.status).toBe("done");
  });

  it("is idempotent when task is already done", () => {
    const task = createTask(1, "walk dog");
    const done = markDone(task);
    const done2 = markDone(done);
    expect(done2.status).toBe("done");
    expect(done2).toBe(done); // same reference
  });
});
