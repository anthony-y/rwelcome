use std::fs::{self, File};
use std::io::{self, Write, Read, BufRead, BufReader};
use std::env;

macro_rules! io_err {
    ($message:expr) => {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("rwelcome: error: {}", $message),
        ))
    };
}

/// Acquire todos from the filesystem at `todos_path`.
pub async fn acquire_todos(todos_path: String) -> io::Result<Vec<String>> {
    let file = fs::File::open(todos_path)?;
    let reader = io::BufReader::new(file);
    let mut todos = Vec::<String>::new();
    for maybe_line in reader.lines() {
        let line = maybe_line?;
        if line.is_empty() {
            break;
        }
        if line.starts_with('#') {
            continue;
        }
        todos.push(line);
    }
    Ok(todos)
}

/// Acquire the current user by looking at the LOGNAME or USER environment variables.
pub fn acquire_current_user() -> Option<String> {
    env::var("LOGNAME")
        .or_else(|_| env::var("USER")).ok()
}

/// Acquire the system's hostname from the filesystem.
/// More specifically, from /proc/sys/kernel/hostname.
pub fn acquire_hostname() -> std::io::Result<String> {
    fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string())
}

/// Acquire the CPU temperature from the filesystem.
/// More specifically, from /sys/class/hwmon/hwmon1/temp2_input (by default).
/// If a value is bound to the environment variable RWELCOME_CPU_TEMP, it will look there instead.
pub fn acquire_cpu_temperature() -> io::Result<f64> {
    let path = env::var("RWELCOME_CPU_TEMP_PATH")
        .unwrap_or("/sys/class/hwmon/hwmon1/temp2_input".to_string());
    let contents = fs::read_to_string(path)?;
    let temp_millidegrees: i32 = contents
                                .trim()
                                .parse()
                                .map_err(|_| {
                                    io::Error::new(
                                        io::ErrorKind::InvalidData,
                                        "invalid temperature data"
                                    )
                                })?;
    Ok(temp_millidegrees as f64 / 1000.0)
}

/// Acquires the kernel version from the filesystem.
/// More specifically, from /proc/version.
pub fn acquire_kernel_version() -> io::Result<String> {
    let contents = fs::read_to_string("/proc/version")?;
    let version_info = contents.split_whitespace()
        .nth(2) // the kernel version typically appears as the third word in /proc/version
        .ok_or(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid kernel version")
        )?
        .to_owned();
    Ok(version_info)
}

/// Attempts to acquire the current system uptime from the filesystem.
pub fn acquire_uptime() -> io::Result<(u64, u64)> {
    let mut file       = fs::File::open("/proc/uptime")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let uptime_str = contents.split_whitespace().next().ok_or(io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid uptime data",
    ))?.trim();
    let uptime_seconds = uptime_str.parse::<f32>()
                        .map_err(|e|
                            io::Error::new(io::ErrorKind::InvalidData, e)
                        )?;

    let hours = uptime_seconds as u64 / 3600;
    let minutes = (uptime_seconds as u64 % 3600) / 60;
    Ok((hours, minutes))
}

/// Attempts to parse a memory value given as a string value into a number.
/// This is a util function used exclusively by the below `acquire_memory_info()`.
fn parse_memory_value(value: &str) -> io::Result<u64> {
    let value = value.split_whitespace().next().ok_or(io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid memory data",
    ))?;
    value.parse()
        .map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                e
            )
        })
}

/// Attempts to acquire, from the filesystem, the used and total memory
/// on the system at the moment. More specifically, from /proc/meminfo.
pub fn acquire_memory_info() -> io::Result<(u64, u64)> {
    let file = fs::File::open("/proc/meminfo")?;
    let reader = BufReader::new(file);
    let mut total_memory     = 0;
    let mut available_memory = 0;
    for line in reader.lines() {
        let line = line?;
        if let Some((key, value)) = line.split_once(':') {
            match key.trim() {
                "MemTotal"     => total_memory = parse_memory_value(value)?,
                "MemAvailable" => available_memory = parse_memory_value(value)?,
                _ => {},
            }
        }
    }
    let used_memory = total_memory - available_memory;
    Ok((used_memory, total_memory))
}

/// Displays an interface allowing the user to edit the todo list.
/// If `wants_editor` is true, it will attempt to open an instance of
/// an appropriate text editor with the todos file loaded.
/// Otherwise, it will attempt to parse action verbs supplied as additional arguments,
/// e.g. rwelcome edit add Get bagels
/// e.g. rwelcome edit done 2
/// If anything goes wrong, it will return an Err containing an error
/// message string that the caller can output to the user.
pub async fn edit_todos(
    current_todos: &mut Vec<String>,
    wants_editor: bool,
    args: &mut Vec<String>,
    todos_path: String
) -> io::Result<Vec<String>> {
    if wants_editor {
        let editor = env::var("EDITOR")
                        .unwrap_or_else(|_| "vi".to_string());
        let status = std::process::Command::new(editor)
            .arg(todos_path.clone())
            .status()
            .expect("rwelcome: error: failed to execute editor");
        if !status.success() {
            return io_err!("editor exited with non-zero status code");
        }
        return acquire_todos(todos_path).await;
    }

    assert!(args.len() > 2);
    let verb = &args[2];

    if verb == "done" || verb == "check" {
        let the_rest = args[3..].join(" ");
        let list_indices: Vec<usize> = the_rest
                                 .split(",")
                                 .map(|s| {
                                    s.parse::<i32>()
                                    .expect("rwelcome: error: you should supply a number to mark as done.")
                                    as usize
                                 })
                                .collect();
        // Remove in reverse order to avoid element shifting,
        // preserving validity of user's given indices.
        for i in (0..list_indices.len()).rev() {
            let list_index = list_indices[i];
            if list_index > current_todos.len() || list_index < 1 {
                return io_err!("please choose a number that's in the list.");
            }
            current_todos.remove(list_index-1);
        }
    }

    else if verb == "fix" {
        if args.len() < 3 {
            return io_err!("please choose a number that's in the list.");
        }
        let idx = args[3]
                .parse::<i32>()
                .expect("rwelcome error: please give a todo number to fix.")
                as usize;
        let content = args[4..].join(" ");
        if idx > current_todos.len() || idx < 1 {
            return io_err!("please choose a number that's in the list.");
        }
        current_todos[idx-1] = content;
    }

    else if verb == "add" {
        let the_rest = args[3..].join(" ");
        current_todos.push(the_rest);
    }

    else {
        return io_err!("unknown verb '{verb}'.");
    }

    let mut data_file: File = File::create(todos_path.clone())
                        .or_else(|_| io_err!("couldn't create your todos file."))?;

    data_file.write(current_todos.join("\n").as_bytes())
            .or_else(|_| io_err!("couldn't update your todos..."))?;

    Ok(current_todos.to_vec())
}
