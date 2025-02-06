mod format;
mod task;

use std::{
    cmp::Ordering,
    env,
    fs::{remove_file, File, OpenOptions},
    io::{self, stdin, stdout, BufReader, Seek, Write},
    path::Path,
};

use chrono::Local;
use colored::*;
use format::format_duration;
use regex::Regex;
use task::Task;
use thiserror::Error;

#[derive(Debug, Error)]
enum CliError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Invalid input: {0}")]
    Input(String),

    #[error("Task not found")]
    TaskNotFound,

    #[error("Invalid command")]
    InvalidCommand,

    #[error("Invalid file format")]
    InvalidFileFormat,

    #[error("Invalid arguments")]
    InvalidArguments,
}

type Result<T> = std::result::Result<T, CliError>;

fn print_help() -> Result<()> {
    println!("{}", "Usage: todo-cli <command> [arguments]".bold());
    println!("\n{}", "Commands:".bold());
    println!(
        "  {} [file]                      {}",
        "add".green(),
        "Add a new task".white()
    );
    println!(
        "  {} [file]                     {}",
        "list".green(),
        "List all tasks".white()
    );
    println!(
        "  {} <id> [file]              {}",
        "remove".green(),
        "Remove a task by ID".white()
    );
    println!(
        "  {} <id> <amount> [file]   {}",
        "progress".green(),
        "Update task progress".white()
    );
    println!(
        "  {} <id> [file]                {}",
        "edit".green(),
        "Edit an existing task".white()
    );
    println!("\n{}", "Arguments:".bold());
    println!(
        "  {}                            {}",
        "file".yellow(),
        "Optional path to task file (default: ./task_list)".white()
    );
    println!(
        "  {}                              {}",
        "id".yellow(),
        "Task ID".white()
    );
    println!(
        "  {}                          {}",
        "amount".yellow(),
        "Progress amount (e.g. 2h 30m, 50%)".white()
    );
    println!("\n{}", "Examples:".bold());
    println!("  {}", "todo-cli add".cyan());
    println!("  {}", "todo-cli list".cyan());
    println!("  {}", "todo-cli remove 1".cyan());
    println!("  {}", "todo-cli progress 2 30m".cyan());
    println!("  {}", "todo-cli edit 3".cyan());
    Ok(())
}

fn save_tasks(tasks: &[Task], file_path: &Path, overwrite: bool) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .append(!overwrite)
        .create(true)
        .truncate(overwrite)
        .open(file_path)
        .map_err(|e| CliError::Io(e))?;

    for task in tasks {
        file.write_all(&task.serialize())
            .map_err(|e| CliError::Io(e))?;
    }
    Ok(())
}

fn read_tasks(file_path: &Path) -> Result<Vec<Task>> {
    let mut tasks = Vec::new();
    let f = File::open(file_path).map_err(|e| CliError::Io(e))?;

    let total_size = f.metadata()?.len();
    let mut br = BufReader::new(f);

    while br.stream_position()? < total_size {
        tasks.push(Task::from(&mut br).map_err(|_| CliError::InvalidFileFormat)?);
    }

    Ok(tasks)
}

fn query<V, F>(msg: &str, regex: &str, f: F) -> Result<V>
where
    F: Fn(Vec<Option<String>>) -> Result<V>,
{
    let regex = Regex::new(regex).map_err(|e| CliError::Input(e.to_string()))?;
    loop {
        print!("{}", msg.bold());
        stdout().flush().map_err(CliError::Io)?;

        let mut input = String::new();
        stdin().read_line(&mut input).map_err(CliError::Io)?;

        match regex.captures(input.trim()) {
            Some(captures) => {
                let groups = captures
                    .iter()
                    .skip(1)
                    .map(|x| x.map(|x| x.as_str().to_owned()))
                    .collect();

                match f(groups) {
                    Ok(v) => return Ok(v),
                    Err(e) => eprintln!("{}", format!("Error: {e}").red()),
                }
            }
            None => eprintln!("{}", "Invalid input format, please try again".red()),
        }
    }
}

fn main() {
    if let Err(e) = try_main() {
        eprintln!("{}", format!("Error: {e}").red());
        std::process::exit(1);
    }
}

fn try_main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() == 1 {
        print_help()?;
        return Ok(());
    }

    match args[1].as_str() {
        "add" => handle_add(&args),
        "list" => handle_list(&args),
        "remove" => handle_remove(&args),
        "progress" => handle_progress(&args),
        "edit" => handle_edit(&args),
        _ => {
            print_help()?;
            Err(CliError::InvalidCommand)
        }
    }
}

fn handle_add(args: &[String]) -> Result<()> {
    if args.len() < 2 {
        return Err(CliError::InvalidArguments);
    }

    let file_path = Path::new(if args.len() == 2 {
        "./task_list"
    } else {
        &args[2]
    });

    let max_id = if file_path.exists() {
        read_tasks(file_path)?
            .iter()
            .map(|t| t.id())
            .max()
            .unwrap_or(-1)
    } else {
        -1
    };

    let mut task = Task::with_id(max_id + 1);

    task.deadline = query(
        &format!(
            "Due (format: {} or {}): ",
            "YYYY-MM-DD HH:MM:SS".yellow(),
            "HH:MM:SS".yellow()
        ),
        r"^(?:(\d{4}-\d{2}-\d{2})(?: (\d{2}:\d{2}:\d{2}))?|(\d{2}:\d{2}:\d{2}))$",
        |v| {
            let date = match &v[0] {
                Some(date) => chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d"),
                None => Ok(Local::now().date_naive()),
            }
            .map_err(|_| CliError::Input("Invalid date format".into()))?;

            let time = match &v[1] {
                Some(time) => chrono::NaiveTime::parse_from_str(time, "%H:%M:%S"),
                None => match &v[2] {
                    Some(time) => chrono::NaiveTime::parse_from_str(time, "%H:%M:%S"),
                    None => Ok(Local::now().time()),
                },
            }
            .map_err(|_| CliError::Input("Invalid time format".into()))?;

            Ok(chrono::NaiveDateTime::new(date, time)
                .and_local_timezone(Local)
                .unwrap()
                .timestamp())
        },
    )?;

    task.estimated_time = query(
        "Estimated time to complete: ",
        r"^(?:(\d+)h\s*)?(?:(\d+)m\s*)?(?:(\d+)s)?$",
        |v| {
            Ok(v[0]
                .clone()
                .unwrap_or("0".into())
                .parse::<i64>()
                .map_err(|_| CliError::Parse("Invalid hours".into()))?
                * 3600
                + v[1]
                    .clone()
                    .unwrap_or("0".into())
                    .parse::<i64>()
                    .map_err(|_| CliError::Parse("Invalid minutes".into()))?
                    * 60
                + v[2]
                    .clone()
                    .unwrap_or("0".into())
                    .parse::<i64>()
                    .map_err(|_| CliError::Parse("Invalid seconds".into()))?)
        },
    )?;

    task.name = query("Name: ", r"(.*)", |v| {
        Ok(v[0]
            .clone()
            .ok_or(CliError::Input("Name cannot be empty".into()))?)
    })?;

    task.description = query("Description: ", r"(.*)", |v| {
        Ok(v[0]
            .clone()
            .ok_or(CliError::Input("Description cannot be empty".into()))?)
    })?;

    save_tasks(&[task], file_path, false)?;
    println!("{}", "Task added successfully".green());
    Ok(())
}

fn handle_list(args: &[String]) -> Result<()> {
    let file_path = Path::new(if args.len() == 2 {
        "./task_list"
    } else {
        &args[2]
    });

    if !file_path.exists() {
        return Err(CliError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Task file {} not found", file_path.display()),
        )));
    }

    let mut tasks = read_tasks(file_path)?;
    tasks.sort_by(|a, b| match a.get_time_left().cmp(&b.get_time_left()) {
        Ordering::Less => Ordering::Less,
        Ordering::Greater => Ordering::Greater,
        Ordering::Equal => a.id().cmp(&b.id()),
    });

    for task in read_tasks(file_path)? {
        println!("{}\n", task);
    }
    Ok(())
}

fn handle_remove(args: &[String]) -> Result<()> {
    if args.len() < 3 {
        return Err(CliError::InvalidArguments);
    }

    let file_path = Path::new(if args.len() == 3 {
        "./task_list"
    } else {
        &args[3]
    });

    if !file_path.exists() {
        return Err(CliError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Task file {} not found", file_path.display()),
        )));
    }

    let mut tasks = read_tasks(file_path)?;
    let target_id = args[2]
        .parse()
        .map_err(|_| CliError::Parse("Invalid task ID".into()))?;

    let index = tasks
        .iter()
        .position(|t| t.id() == target_id)
        .ok_or(CliError::TaskNotFound)?;

    tasks.swap_remove(index);

    if tasks.is_empty() {
        remove_file(file_path)?;
    } else {
        save_tasks(&tasks, file_path, true)?;
    }

    println!(
        "{}",
        format!(
            "{}{}",
            "Successfully removed task with id ".green(),
            target_id.to_string().cyan()
        )
    );
    Ok(())
}

fn handle_progress(args: &[String]) -> Result<()> {
    if args.len() < 4 {
        return Err(CliError::InvalidArguments);
    }

    let file_path = Path::new(if args.len() == 4 {
        "./task_list"
    } else {
        &args[4]
    });

    if !file_path.exists() {
        return Err(CliError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Task file {} not found", file_path.display()),
        )));
    }

    let mut tasks = read_tasks(file_path)?;
    let target_id = args[2]
        .parse()
        .map_err(|_| CliError::Parse("Invalid task ID".into()))?;

    let index = tasks
        .iter()
        .position(|t| t.id() == target_id)
        .ok_or(CliError::TaskNotFound)?;

    let progress_input = args[3..].join(" ");
    let progress_made = parse_progress(&progress_input, &tasks[index])?;

    tasks[index].progress += progress_made;
    let completed = tasks[index].progress >= tasks[index].estimated_time;

    if completed {
        tasks.remove(index);
    }

    if tasks.is_empty() {
        remove_file(file_path)?;
    } else {
        save_tasks(&tasks, file_path, true)?;
    }

    println!(
        "{}",
        format!(
            "Task {}",
            if completed {
                "completed".green().into()
            } else {
                format!(
                    "progress updated to {:.1}%",
                    (tasks[index].progress as f32 / tasks[index].estimated_time as f32) * 100.0
                )
                .cyan()
            }
        )
        .bold()
    );
    Ok(())
}

fn parse_progress(input: &str, task: &Task) -> Result<i64> {
    let time_re = Regex::new(r"^(?:(\d+)h\s*)?(?:(\d+)m\s*)?(?:(\d+)s)?$")
        .map_err(|e| CliError::Input(e.to_string()))?;

    let percent_re = Regex::new(r"^(\d+)%$").map_err(|e| CliError::Input(e.to_string()))?;

    if let Some(caps) = time_re.captures(input) {
        Ok(caps
            .get(1)
            .map_or(0, |m| m.as_str().parse::<i64>().unwrap_or(0))
            * 3600
            + caps
                .get(2)
                .map_or(0, |m| m.as_str().parse::<i64>().unwrap_or(0))
                * 60
            + caps
                .get(3)
                .map_or(0, |m| m.as_str().parse::<i64>().unwrap_or(0)))
    } else if let Some(caps) = percent_re.captures(input) {
        let percent = caps[1]
            .parse::<f32>()
            .map_err(|_| CliError::Parse("Invalid percentage".into()))?;
        Ok((task.estimated_time as f32 * (percent / 100.0)).round() as i64)
    } else {
        Err(CliError::Input("Invalid progress format".into()))
    }
}

fn handle_edit(args: &[String]) -> Result<()> {
    if args.len() < 3 {
        return Err(CliError::InvalidArguments);
    }

    let file_path = Path::new(if args.len() == 3 {
        "./task_list"
    } else {
        &args[3]
    });

    if !file_path.exists() {
        return Err(CliError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Task file {} not found", file_path.display()),
        )));
    }

    let mut tasks = read_tasks(file_path)?;
    let target_id = args[2]
        .parse()
        .map_err(|_| CliError::Parse("Invalid task ID".into()))?;

    let index = tasks
        .iter()
        .position(|t| t.id() == target_id)
        .ok_or(CliError::TaskNotFound)?;

    let original_task = tasks[index].clone();

    tasks[index].deadline = query(
        &format!("Due (press Enter to keep {}): ", original_task.format_due()),
        r"^(?:(\d{4}-\d{2}-\d{2})(?: (\d{2}:\d{2}:\d{2}))?|(\d{2}:\d{2}:\d{2}))?$",
        |v| {
            if v[0].as_ref().map(|s| s.trim().is_empty()).unwrap_or(true) {
                return Ok(original_task.deadline);
            }

            let date = match &v[0] {
                Some(date) => chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d"),
                None => Ok(Local::now().date_naive()),
            }
            .map_err(|_| CliError::Input("Invalid date format".into()))?;

            let time = match &v[1] {
                Some(time) => chrono::NaiveTime::parse_from_str(time, "%H:%M:%S"),
                None => match &v[2] {
                    Some(time) => chrono::NaiveTime::parse_from_str(time, "%H:%M:%S"),
                    None => Ok(Local::now().time()),
                },
            }
            .map_err(|_| CliError::Input("Invalid time format".into()))?;

            Ok(chrono::NaiveDateTime::new(date, time)
                .and_local_timezone(Local)
                .unwrap()
                .timestamp())
        },
    )?;

    tasks[index].estimated_time = query(
        &format!(
            "Estimated time (press Enter to keep {}): ",
            format_duration(original_task.estimated_time)
        ),
        r"^(?:(\d+)h\s*)?(?:(\d+)m\s*)?(?:(\d+)s)?$",
        |v| {
            if v[0].as_ref().map(|s| s.trim().is_empty()).unwrap_or(true) {
                return Ok(original_task.estimated_time);
            }

            Ok(v[0]
                .clone()
                .unwrap_or("0".into())
                .parse::<i64>()
                .map_err(|_| CliError::Parse("Invalid hours".into()))?
                * 3600
                + v[1]
                    .clone()
                    .unwrap_or("0".into())
                    .parse::<i64>()
                    .map_err(|_| CliError::Parse("Invalid minutes".into()))?
                    * 60
                + v[2]
                    .clone()
                    .unwrap_or("0".into())
                    .parse::<i64>()
                    .map_err(|_| CliError::Parse("Invalid seconds".into()))?)
        },
    )?;

    tasks[index].name = query(
        &format!("Name (press Enter to keep \"{}\"): ", original_task.name),
        r"(.*)",
        |v| {
            let input = v[0].clone().unwrap_or_default();
            if input.trim().is_empty() {
                Ok(original_task.name.clone())
            } else {
                Ok(input.trim().to_owned())
            }
        },
    )?;

    tasks[index].description = query(
        &format!(
            "Description (press Enter to keep \"{}\"): ",
            original_task.description
        ),
        r"(.*)",
        |v| {
            let input = v[0].clone().unwrap_or_default();
            if input.trim().is_empty() {
                Ok(original_task.description.clone())
            } else {
                Ok(input.trim().to_owned())
            }
        },
    )?;

    save_tasks(&tasks, file_path, true)?;
    println!("{}", "Task updated successfully".green());
    println!("{}", tasks[index]);
    Ok(())
}
