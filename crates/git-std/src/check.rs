/// Run the `check` subcommand with an inline message. Returns the exit code.
pub fn run(message: &str) -> i32 {
    match standard_commit::parse(message) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("\u{2717} invalid: {e}");
            eprintln!("  Expected: <type>(<scope>): <description>");
            eprintln!("  Got:      {}", first_line(message));
            1
        }
    }
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or(s)
}
