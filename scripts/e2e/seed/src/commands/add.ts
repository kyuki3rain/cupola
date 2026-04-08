import { createTask } from "../task.js";
import { load, save, nextId } from "../storage.js";

export function addCommand(filePath: string, title: string): void {
  const tasks = load(filePath);
  const id = nextId(tasks);
  const task = createTask(id, title);
  tasks.push(task);
  save(filePath, tasks);
  console.log(`Added task #${task.id}: ${task.title}`);
}
