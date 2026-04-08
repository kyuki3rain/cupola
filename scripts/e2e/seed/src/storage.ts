import fs from "node:fs";
import type { Task } from "./task.js";

export function load(filePath: string): Task[] {
  if (!fs.existsSync(filePath)) return [];
  const raw = fs.readFileSync(filePath, "utf-8");
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed as Task[];
  } catch {
    return [];
  }
}

export function save(filePath: string, tasks: Task[]): void {
  fs.writeFileSync(filePath, JSON.stringify(tasks, null, 2) + "\n", "utf-8");
}

export function nextId(tasks: Task[]): number {
  if (tasks.length === 0) return 1;
  return Math.max(...tasks.map((t) => t.id)) + 1;
}
