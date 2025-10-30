use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader, Write},
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

const DATA_FILE: &str = "tasks.json";

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Task {
    id: u64,
    title: String,
    done: bool,
    created_at: DateTime<Local>,
}

impl Task {
    fn new(id: u64, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            done: false,
            created_at: Local::now(),
        }
    }
}

fn load_tasks(path: &str) -> io::Result<Vec<Task>> {
    if !Path::new(path).exists() {
        return Ok(Vec::new());
    }
    let f = File::open(path)?;
    let reader = BufReader::new(f);
    let tasks: Vec<Task> = serde_json::from_reader(reader).unwrap_or_default();
    Ok(tasks)
}

fn save_tasks(path: &str, tasks: &Vec<Task>) -> io::Result<()> {
    let tmp = format!("{}.tmp", path);
    let mut f = File::create(&tmp)?;
    let json = serde_json::to_string_pretty(tasks).unwrap();
    f.write_all(json.as_bytes())?;
    f.flush()?;
    fs::rename(tmp, path)?;
    Ok(())
}

fn print_menu() {
    println!();
    println!("==== Task Manager ====");
    println!("1. Add task");
    println!("2. List tasks");
    println!("3. Toggle done");
    println!("4. Delete task");
    println!("5. Save tasks");
    println!("0. Exit");
    print!("Enter choice: ");
    io::stdout().flush().unwrap();
}

fn main() {
    let tasks = Arc::new(Mutex::new(load_tasks(DATA_FILE).unwrap()));
    let tasks_clone = Arc::clone(&tasks);

    // Autosave thread demonstrating Arc
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(10));
        let guard = tasks_clone.lock().unwrap();
        if let Err(e) = save_tasks(DATA_FILE, &*guard) {
            eprintln!("Autosave failed: {}", e);
        }
    });

    let mut next_id = {
        let guard = tasks.lock().unwrap();
        guard.iter().map(|t| t.id).max().unwrap_or(0) + 1
    };

    let stdin = io::stdin();
    let mut input = String::new();

    // Ask the user to press Enter to show the menu
    println!("Press Enter to show the menu...");
    while let Ok(bytes_read) = stdin.read_line(&mut input) {
        if bytes_read == 0 {
            println!("Exiting...");
            break;
        }

        // Clear input and show menu
        input.clear();
        print_menu();

        if stdin.read_line(&mut input).is_err() {
            println!("Error reading input. Exiting...");
            break;
        }

        let choice = input.trim();

        match choice {
            "1" => {
                print!("Enter task title: ");
                io::stdout().flush().unwrap();
                let mut title = String::new();
                if stdin.read_line(&mut title).is_ok() {
                    let title = title.trim();
                    if !title.is_empty() {
                        let mut guard = tasks.lock().unwrap();
                        guard.push(Task::new(next_id, title.to_string()));
                        println!("Added task {}", next_id);
                        next_id += 1;
                    }
                }
            }
            "2" => {
                let guard = tasks.lock().unwrap();
                if guard.is_empty() {
                    println!("No tasks.");
                } else {
                    for t in guard.iter() {
                        println!(
                            "{}. [{}] {} (created {})",
                            t.id,
                            if t.done { "x" } else { " " },
                            t.title,
                            t.created_at.format("%Y-%m-%d %H:%M:%S")
                        );
                    }
                }
            }
            "3" => {
                print!("Enter task id to toggle: ");
                io::stdout().flush().unwrap();
                let mut id_str = String::new();
                if stdin.read_line(&mut id_str).is_ok() {
                    if let Ok(id) = id_str.trim().parse::<u64>() {
                        let mut guard = tasks.lock().unwrap();
                        if let Some(t) = guard.iter_mut().find(|t| t.id == id) {
                            t.done = !t.done;
                            println!("Toggled task {} -> {}", id, t.done);
                        } else {
                            println!("No task found.");
                        }
                    }
                }
            }
            "4" => {
                print!("Enter task id to delete: ");
                io::stdout().flush().unwrap();
                let mut id_str = String::new();
                if stdin.read_line(&mut id_str).is_ok() {
                    if let Ok(id) = id_str.trim().parse::<u64>() {
                        let mut guard = tasks.lock().unwrap();
                        let before = guard.len();
                        guard.retain(|t| t.id != id);
                        if guard.len() < before {
                            println!("Deleted task {}", id);
                        } else {
                            println!("No task found.");
                        }
                    }
                }
            }
            "5" => {
                let guard = tasks.lock().unwrap();
                if let Err(e) = save_tasks(DATA_FILE, &*guard) {
                    eprintln!("Failed to save tasks: {}", e);
                } else {
                    println!("Tasks saved.");
                }
            }
            "0" => {
                println!("Saving and exiting...");
                let guard = tasks.lock().unwrap();
                let _ = save_tasks(DATA_FILE, &*guard);
                break;
            }
            _ => println!("Invalid choice."),
        }

        input.clear();
        println!("\nPress Enter to show the menu...");
    }
}
