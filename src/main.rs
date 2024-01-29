mod weather;
mod ext;

use std::env;
use colored::Colorize;
use tokio;
use std::io;
use weather::WeatherResponse;

struct Rwelcome {
    username: String,
    maybe_weather_response: Option<reqwest::Result<WeatherResponse>>,
    todos: io::Result<Vec<String>>,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let environment = load().await?;
    render(environment);
    Ok(())
}

/// Neatly format a list of todos to stdout.
pub fn show_todos(todos: Vec<String>) {
    if todos.is_empty() {
        println!("{}: none!", "Todos".bright_blue());
        return;
    }
    println!("{}:", "Todos".bright_blue());
    for (index, todo) in todos.iter().enumerate() {
        println!("  {}. {}", index + 1, todo);
    }
}

// Send N hyphens to stdout, where N equals `length`.
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

async fn load() -> Result<Rwelcome, String> {
    let username = ext::acquire_current_user().unwrap_or_else(|| "unknown".to_string());

    let default_todos_path = format!("/home/{username}/.local/share/rwelcome/todos");
    let todos_path = env::var("RWELCOME_TODOS_PATH").unwrap_or(default_todos_path);

    /*
     * If we have an API key, acquire weather from Open Weather API.
     *
     * Do this before everything else, so that it's ready by the time
     * we go to render.
     */
    let maybe_weather_response = match env::var("RWELCOME_WEATHER_API_KEY") {
        Ok(key) => Some(weather::acquire(key).await),
        Err(_) => None,
    };

    /*
     * If the RWELCOME_TODOS environment variable is present,
     * parse the todos file into memory for rendering later.
     */
    let mut todos = ext::acquire_todos(todos_path.clone()).await;

    /*
     * Handle arguments
    */
    let mut args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        let given_arg = &args[1];
        if given_arg != "edit" {
            return Err(format!("rwelcome: error: {given_arg} is not a valid verb."));
        }
        let mut the_todos = match todos {
            Ok(todos) => todos,
            Err(err) => return Err(format!("{}", err)),
        };
        let wants_editor = args.len() == 2;
        todos = ext::edit_todos(
            &mut the_todos,
            wants_editor,
            &mut args,
            todos_path.clone()
        ).await; 
    }

    Ok(Rwelcome{ username, maybe_weather_response, todos })
}

fn render(ctx: Rwelcome) {
    println!();
    let hostname = ext::acquire_hostname().unwrap_or_else(|_| "unknown".to_string());
    println!("{}@{}", ctx.username.purple(), hostname);
    let line_length = ctx.username.len() + hostname.len() + 1;
    draw_line(line_length);
    match ext::acquire_uptime() {
        Ok((hours, minutes)) => println!("{}: {}h {}m", "Uptime".bright_blue(), hours, minutes),
        Err(err) => eprintln!("{}: {}", "Uptime".red(), err)
    }
    match ext::acquire_memory_info() {
        Ok((used, total)) => println!("{}: {} MiB / {} MiB", "Memory".bright_blue(), used / 1000, total / 1000),
        Err(err) => eprintln!("{}: {}", "Memory".red(), err),
    }
    match ext::acquire_kernel_version() {
        Ok(version) => println!("{}: Linux {}", "Kernel".bright_blue(), version),
        Err(err) => eprintln!("{}: {}", "Kernel".red(), err)
    }
    match ext::acquire_cpu_temperature() {
        Ok(temp) => println!("{}: {:.1}°C", "CPU temp".bright_blue(), temp),
        Err(err) => eprintln!("{}: {}", "CPU temp".red(), err)
    }
    println!();
    println!("{}@real", "life".purple());
    draw_line(line_length);
    if let Some(weather_response) = ctx.maybe_weather_response {
        match weather_response {
            Ok(weather) => {
                let the_condition = weather.current.condition.text.to_lowercase();
                let emoji = if the_condition == "cloudy" { "☁️" }
                                else if the_condition.contains("fog") { "☁️" }
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
    match ctx.todos {
        Ok(todos) => show_todos(todos),
        Err(err)  => eprintln!("{}: {}", "Todos".red(), err),
    }
    println!();
}
