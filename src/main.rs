use std::fs;
use std::env;
use std::io::{self, Read, BufRead, BufReader, ErrorKind, Error};
use colored::Colorize;

const TODOS_PATH: &str = "/home/ant/.local/share/ftc/todos";

fn show_todos(todos: Vec<String>) {

    if todos.is_empty() {
        println!("{}: all done!", "Todos".bright_blue());
        return;
     }

    println!("{}:", "Todos".bright_blue());
    for (index, todo) in todos.iter().enumerate() {
        println!("  {}. {}", index + 1, todo);
    }
}

fn acquire_todos() -> io::Result<Vec<String>> {
    let file = fs::File::open(TODOS_PATH)?;
    let reader = BufReader::new(file);

    let mut todos = Vec::<String>::new();

    for line in reader.lines() {
        let line = line?;

        if line.is_empty() || !line.starts_with('#') {
            break;
        }

        // we are basically just skipping the space after the '#' if there is one.
        let start = if line.chars().nth(1).unwrap() == ' ' { 2 } else { 1 };

        todos.push(line[start..].to_string());
    }

    Ok(todos)
}

fn acquire_current_user() -> Option<String> {
    env::var("LOGNAME")
        .or_else(|_| env::var("USER")).ok()
}

fn acquire_hostname() -> std::io::Result<String> {
    fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string())
}

fn acquire_cpu_temperature() -> io::Result<f64> {
    
    let path = "/sys/class/hwmon/hwmon1/temp2_input";
    let contents = fs::read_to_string(path)?;

    let temp_millidegrees: i32 = contents
                                .trim()
                                .parse()
                                .map_err(|_| {
                                    Error::new(
                                        ErrorKind::InvalidData,
                                        "Invalid temperature data"
                                    )
                                })?;

    // Convert millidegrees Celsius to degrees Celsius
    Ok(temp_millidegrees as f64 / 1000.0)
}

fn acquire_kernel_version() -> io::Result<String> {
    let contents = fs::read_to_string("/proc/version")?;
    let version_info = contents.split_whitespace()
        .nth(2) // the kernel version typically appears as the third word in /proc/version
        .ok_or(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid kernel version data")
        )?
        .to_owned();
    Ok(version_info)
}

fn acquire_uptime() -> io::Result<(u64, u64)> {
    let mut file       = fs::File::open("/proc/uptime")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let uptime_str = contents.split_whitespace().next().ok_or(io::Error::new(
        io::ErrorKind::InvalidData,
        "Invalid uptime data",
    ))?.trim();

    let uptime_seconds = uptime_str.parse::<f32>().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let hours = uptime_seconds as u64 / 3600;
    let minutes = (uptime_seconds as u64 % 3600) / 60;

    Ok((hours, minutes))
}

fn parse_memory_value(value: &str) -> io::Result<u64> {
    let value = value.split_whitespace().next().ok_or(io::Error::new(
        io::ErrorKind::InvalidData,
        "Invalid memory data",
    ))?;

    value
        .parse()
        .map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                e
            )
        })
}

fn acquire_memory_info() -> io::Result<(u64, u64)> {
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

fn edit_todos() {
    let editor = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let status = std::process::Command::new(editor)
        .arg(TODOS_PATH)
        .status()
        .expect("Failed to execute editor");
    if !status.success() {
        panic!("Editor exited with non-zero status code");
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        let given_arg = &args[1];
        if given_arg == "edit" {
            edit_todos();
        }
    }
    let username = acquire_current_user().unwrap_or_else(|| "unknown".to_string());
    let hostname = acquire_hostname().unwrap_or_else(|_| "unknown".to_string());
    println!("{}@{}", username.bright_red(), hostname);
    let mut i = 0;
    loop {
        if username.len() + hostname.len() == i {
            println!();
            break;
        }
        print!("-");
        i += 1;
    }
    match acquire_uptime() {
        Ok((hours, minutes)) => println!("{}: {}h {}m", "Uptime".bright_blue(), hours, minutes),
        Err(err) => panic!("{}: {}", "Uptime".red(), err)
    }
    match acquire_memory_info() {
        Ok((used, total)) => println!("{}: {} MiB / {} MiB", "Memory".bright_blue(), used / 1000, total / 1000),
        Err(err) => panic!("{}: {}", "Memory".red(), err),
    }
    match acquire_kernel_version() {
        Ok(version) => println!("{}: {}", "Kernel".bright_blue(), version),
        Err(err) => panic!("{}: {}", "Kernel".red(), err)
    }
    match acquire_cpu_temperature() {
        Ok(temp) => println!("{}: {:.1}°C", "CPU Temp".bright_blue(), temp),
        Err(err) => println!("{}: {}", "CPU Temp".red(), err)
    }
    match acquire_todos() {
        Ok(todos) => show_todos(todos),
        Err(err) => println!("{}: {}", "Todos".red(), err),
    };
    println!();
}
