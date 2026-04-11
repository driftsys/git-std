use std::fs;
use std::path::PathBuf;

use clap::CommandFactory;

#[path = "src/app.rs"]
mod app;

fn main() -> std::io::Result<()> {
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR not set"));
    let man_dir = out_dir.join("man");
    fs::create_dir_all(&man_dir)?;

    let cmd = app::Cli::command();

    // Generate main man page: git-std(1).
    let man = clap_mangen::Man::new(cmd.clone());
    let mut buf = Vec::new();
    man.render(&mut buf)?;
    fs::write(man_dir.join("git-std.1"), buf)?;

    // Generate one man page per subcommand: git-std-<sub>(1).
    for sub in cmd.get_subcommands() {
        if sub.get_name() == "help" {
            continue;
        }
        let name = format!("git-std-{}", sub.get_name());
        let man = clap_mangen::Man::new(sub.clone()).title(&name);
        let mut buf = Vec::new();
        man.render(&mut buf)?;
        fs::write(man_dir.join(format!("{name}.1")), buf)?;
    }

    // Tell Cargo to re-run if the CLI definition or skills change.
    println!("cargo:rerun-if-changed=src/app.rs");
    println!("cargo:rerun-if-changed=../../skills/");

    Ok(())
}
