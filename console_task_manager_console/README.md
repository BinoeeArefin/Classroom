# Console Task Manager (No TUI)

This version of the project runs purely in the console, using a simple text-based menu.

It demonstrates:
- `while let` loop for input
- `Arc` + `Mutex` for safe concurrent autosave thread

## Build & Run

```bash
cargo run --release
```

## Features
1. Add, list, toggle, and delete tasks
2. Autosaves tasks every 10 seconds
3. Manual save option
