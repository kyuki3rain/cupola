import path from "node:path";
import { addCommand } from "./commands/add.js";
import { listCommand } from "./commands/list.js";
import { doneCommand } from "./commands/done.js";

const TODO_FILE = path.join(process.cwd(), "todo.json");

function usage(): void {
  console.error("Usage: todo <command> [args]");
  console.error("Commands:");
  console.error("  add <title>   Add a new task");
  console.error("  list          List all tasks");
  console.error("  done <id>     Mark a task as done");
}

const [, , cmd, ...args] = process.argv;

switch (cmd) {
  case "add": {
    const title = args.join(" ");
    if (!title.trim()) {
      console.error("Error: title is required.");
      usage();
      process.exit(1);
    }
    addCommand(TODO_FILE, title);
    break;
  }
  case "list":
    listCommand(TODO_FILE);
    break;
  case "done": {
    const idStr = args[0];
    if (!idStr) {
      console.error("Error: task ID is required.");
      usage();
      process.exit(1);
    }
    doneCommand(TODO_FILE, idStr);
    break;
  }
  default:
    console.error(`Unknown command: ${cmd ?? "(none)"}`);
    usage();
    process.exit(1);
}
