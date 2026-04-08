import { markDone } from "../task.js";
import { load, save } from "../storage.js";

export function doneCommand(filePath: string, idStr: string): void {
  const id = parseInt(idStr, 10);
  if (isNaN(id)) {
    console.error(`Invalid task ID: ${idStr}`);
    process.exit(1);
  }
  const tasks = load(filePath);
  const index = tasks.findIndex((t) => t.id === id);
  if (index === -1) {
    console.error(`Task #${id} not found.`);
    process.exit(1);
  }
  tasks[index] = markDone(tasks[index]!);
  save(filePath, tasks);
  console.log(`Task #${id} marked as done.`);
}
