export type TaskStatus = "open" | "done";

export interface Task {
  id: number;
  title: string;
  status: TaskStatus;
  createdAt: string; // ISO string
}

export function createTask(id: number, title: string, now: Date = new Date()): Task {
  if (!title.trim()) throw new Error("title must not be empty");
  return { id, title: title.trim(), status: "open", createdAt: now.toISOString() };
}

export function markDone(task: Task): Task {
  if (task.status === "done") return task;
  return { ...task, status: "done" };
}
