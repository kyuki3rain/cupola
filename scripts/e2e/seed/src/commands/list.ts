import { load } from "../storage.js";

export function listCommand(filePath: string): void {
  const tasks = load(filePath);
  if (tasks.length === 0) {
    console.log("No tasks.");
    return;
  }
  for (const task of tasks) {
    const badge = task.status === "done" ? "[x]" : "[ ]";
    console.log(`${badge} #${task.id}: ${task.title}`);
  }
}
