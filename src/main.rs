mod weather;
use std::fs;
use std::env;
use std::io::Write;
use std::io::{self, Read, BufRead, BufReader};
use colored::Colorize;
use tokio;

/// Neatly format a list of todos to stdout.
fn show_todos(todos: Vec<String>) {
    if todos.is_empty() {
        println!("{}: none!", "Todos".bright_blue());
        return;
    }
    println!("{}:", "Todos".bright_blue());
    for (index, todo) in todos.iter().enumerate() {
        println!("  {}. {}", index + 1, todo);
    }
}

/// Acquire todos from the filesystem at `TODOS_PATH`.
fn acquire_todos(todos_path: String) -> io::Result<Vec<String>> {
    let file = fs::File::open(todos_path)?;
    let reader = io::BufReader::new(file);
    let mut todos = Vec::<String>::new();

    for maybe_line in reader.lines() {
        let line = maybe_line?;
        if line.is_empty() || !line.starts_with('#') {
            break;
        }
        if line.len() == 1 {
            continue;
        }
        // We are basically just skipping the # (and space after it, if there is one.)
        // If you add more than one space, that will not be excluded from the todo text.
        let offset = if line.chars().nth(1).unwrap() == ' ' { 2 } else { 1 };
        todos.push(line[offset..].to_string());
    }

    Ok(todos)
}

/// Acquire the current user by looking at the LOGNAME or USER environment variables.
fn acquire_current_user() -> Option<String> {
    env::var("LOGNAME")
        .or_else(|_| env::var("USER")).ok()
}

/// Acquire the system's hostname from the filesystem.
/// More specifically, from /proc/sys/kernel/hostname.
fn acquire_hostname() -> std::io::Result<String> {
    fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string())
}

/// Acquire the CPU temperature from the filesystem.
/// More specifically, from /sys/class/hwmon/hwmon1/temp2_input (by default).
/// If a value is bound to the environment variable RWELCOME_CPU_TEMP, it will look there instead.
fn acquire_cpu_temperature() -> io::Result<f64> {
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
fn acquire_kernel_version() -> io::Result<String> {
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
fn acquire_uptime() -> io::Result<(u64, u64)> {
    let mut file       = fs::File::open("/proc/uptime")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let uptime_str = contents.split_whitespace().next().ok_or(io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid uptime data",
    ))?.trim();

    let uptime_seconds = uptime_str.parse::<f32>().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

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
    value
        .parse()
        .map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                e
            )
        })
}

/// Attempts to acquire, from the filesystem, the used and total memory
/// on the system at the moment. More specifically, from /proc/meminfo.
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

/// Displays an interface allowing the user to edit the todo list.
/// If `wants_editor` is true, it will attempt to open an instance of
/// an appropriate text editor with the todos file loaded.
/// Otherwise, it will render an interactive TUI allowing the addition
/// and removal of todo list items.
/// If anything goes wrong, it will return an Err containing an error
/// message string that the caller can display to the user.
fn edit_todos(wants_editor: bool, todos_path: String) -> Result<(), &'static str> {

    if wants_editor {
        let editor = env::var("EDITOR")
                            .unwrap_or_else(|_| "vi".to_string());

        let status = std::process::Command::new(editor)
            .arg(todos_path)
            .status()
            .expect("rwelcome: error: failed to execute editor");
        
        if !status.success() {
            return Err("rwelcome: error: editor exited with non-zero status code");
        }

        return Ok(());
    }

    let mut current_todos = match acquire_todos(todos_path) {
        Ok(todos) => todos,
        Err(err) => panic!("rwelcome: error: couldn't acquire todo list: {}", err),
    };

    println!(
        "{}:\n  {}: # Todo text\n  {}: -1 (where 1 is the index of the todo to remove)\n  {}: {} or {} or {}",
        "Usage".green(),
        "Add a todo".bright_purple(),
        "Remove a todo".bright_purple(),
        "Exit".bright_purple(),
        "!".bright_yellow(),
        "quit".bright_yellow(),
        "exit".bright_yellow()
    );

    draw_line(12);

    let mut wants_menu = true;
    while wants_menu {
        show_todos(current_todos.clone());
        print!("> ");
        _ = io::stdout().flush();
        let mut selection = String::new();
        match io::stdin().read_line(&mut selection) {
            Ok(read_length) => if read_length == 0 { wants_menu = false; },
            Err(_) => return Err("rwelcome: error: there was a problem reading your selection")
        }
        selection = selection.to_lowercase();
        if selection == "!"
        || selection.contains("quit")
        || selection.contains("exit") {
            wants_menu = false;
        } else if selection.starts_with("#") {
            let start_offset = if selection.chars().nth(1).unwrap_or(0 as char) == ' ' { 2 } else { 1 };
            current_todos.push(selection[start_offset..].to_string());
        } else if selection.starts_with("-") {
            let as_number = match selection.parse::<i32>() {
                Ok(number) => number,
                Err(err) => {
                    println!("{}", err);
                    return Err("rwelcome: info: to remove a todo item, type a '-' followed by it's list index, e.g.: '-2' removes the second todo list item.");
                }
            };
            current_todos.remove(as_number as usize + 1);
        } else {
            return Err("rwelcome: info: usage: -[index] to remove, #[todo text] to add a new todo, '!'/'quit'/'exit' to leave the menu.");
        }
    }

    Ok(())
}

// Send _n_ hyphens to stdout, where _n_ equals `length`.
fn draw_line(length: usize) {
    let mut i = 0;
    loop {
        if length == i {
            println!();
            break;
        }
        print!("-");
        i += 1;
    }
}

#[tokio::main]
async fn main() {

    // Make the request to the weather service at the beginning, so that
    // it has time to get a response before we go to render everything.
    // This should mean there's less of a pause when printing to the screen.
    //
    // (In theory, this could also be done when acquiring info from the filesystem,
    // such as uptime and kernel version.)
    //

    let wants_weather = match env::var("RWELCOME_WEATHER") {
        Ok(val) => val.parse::<bool>().unwrap_or(true),
        Err(err) => if err == env::VarError::NotPresent { true } else { false },
    };

    let maybe_weather_response = 
        if wants_weather { Some(weather::acquire().await) }
        else { None };

    let todos_path = env::var("RWELCOME_TODOS_PATH")
        .unwrap_or("/home/ant/.local/share/rwelcome/todos".to_string());

    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        let given_arg = &args[1];
        if given_arg == "edit" {
            let wants_editor = args.len() > 2 && args[2] == "--editor";
            match edit_todos(wants_editor, todos_path.clone()) {
                Ok(_) => (),
                Err(msg) => {
                    eprintln!("{}", msg)
                }
            }
        }
    }
    println!();
    let username = acquire_current_user().unwrap_or_else(|| "unknown".to_string());
    let hostname = acquire_hostname().unwrap_or_else(|_| "unknown".to_string());
    println!("{}@{}", username.bright_red(), hostname);
    draw_line(username.len() + hostname.len() + 1);
    match acquire_uptime() {
        Ok((hours, minutes)) => println!("{}: {}h {}m", "Uptime".bright_blue(), hours, minutes),
        Err(err) => eprintln!("{}: {}", "Uptime".red(), err)
    }
    match acquire_memory_info() {
        Ok((used, total)) => println!("{}: {} MiB / {} MiB", "Memory".bright_blue(), used / 1000, total / 1000),
        Err(err) => eprintln!("{}: {}", "Memory".red(), err),
    }
    match acquire_kernel_version() {
        Ok(version) => println!("{}: Linux {}", "Kernel".bright_blue(), version),
        Err(err) => eprintln!("{}: {}", "Kernel".red(), err)
    }
    match acquire_cpu_temperature() {
        Ok(temp) => println!("{}: {:.1}°C", "CPU temp".bright_blue(), temp),
        Err(err) => eprintln!("{}: {}", "CPU temp".red(), err)
    }
    if let Some(weather_response) = maybe_weather_response {
        match weather_response {
            Ok(weather) => {
                let the_condition = weather.current.condition.text.to_lowercase();
                let emoji = if the_condition == "cloudy" { "☁️" }
                                else if the_condition.contains("sunny") { "🌤️" }
                                else if the_condition.contains("rain") { "🌧️" }
                                else { "🌥️" };
                println!(
                    "{}: {}°C and {} in {} {}",
                    "Weather".bright_blue(),
                    weather.current.temp_c,
                    the_condition,
                    weather.location.name,
                    emoji,
                );
            },
            Err(err) => eprintln!("{}: {}", "Weather".red(), err),
        }
    }
    match acquire_todos(todos_path) {
        Ok(todos) => show_todos(todos),
        Err(err) => eprintln!("{}: {}", "Todos".red(), err),
    };
    println!();
}
