pub const LOGO: &str = r#"
 ▀█▀ ▄▀█ █ █ █▀█ █ █▄▀ █ ▀█▀
  █  █▀█ █▄█ █▀▄ █ █ █ █  █
"#;

use colored::Colorize;

pub fn print_inline_banner() {
    let lines = LOGO.trim_start_matches('\n').lines();
    for line in lines {
        println!("{}", line.truecolor(228, 228, 231).bold());
    }
    println!(
        "{}",
        format!("  v{}", env!("GIT_VERSION")).truecolor(113, 113, 122)
    );
    println!();
}

pub fn print_inline_separator() {
    println!("{}", "─".repeat(52).truecolor(39, 39, 42));
}
