mod load;
mod model;
mod render;
mod validate;

use std::path::PathBuf;

use anyhow::{bail, Result};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);

    match args.next().as_deref() {
        Some("dashboard") => {
            let mut check = false;
            for arg in args {
                match arg.as_str() {
                    "--check" => check = true,
                    other => bail!("unknown argument `{other}`\n\n{}", help_text()),
                }
            }
            let root = find_root()?;
            let dashboard = load::build(&root)?;
            render::write_dashboard(&root, &dashboard, check)
        }
        Some("-h") | Some("--help") | None => {
            println!("{}", help_text());
            Ok(())
        }
        Some(other) => bail!("unknown xtask `{other}`\n\n{}", help_text()),
    }
}

fn find_root() -> Result<PathBuf> {
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join("docs/dashboard/project.json").is_file() {
            return Ok(dir);
        }
        if !dir.pop() {
            bail!("could not find workspace root (docs/dashboard/project.json)");
        }
    }
}

fn help_text() -> String {
    [
        "xtask",
        "",
        "  cargo xtask dashboard          build docs/index.html",
        "  cargo xtask dashboard --check  fail if generated files are stale",
    ]
    .join("\n")
}
