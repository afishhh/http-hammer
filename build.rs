use clap::CommandFactory;
use clap_complete::{
    generate_to,
    shells::{Bash, Fish, Zsh},
};
use std::io::Error;

include!("src/cli.rs");

fn main() -> Result<(), Error> {
    let outdir = match std::env::var_os("OUT_DIR") {
        None => return Ok(()),
        Some(outdir) => PathBuf::from(outdir).join("completions"),
    };

    let mut cmd = Args::command();

    std::fs::create_dir_all(&outdir)?;
    generate_to(Bash, &mut cmd, "http-hammer", &outdir)?;
    generate_to(Fish, &mut cmd, "http-hammer", &outdir)?;
    generate_to(Zsh, &mut cmd, "http-hammer", &outdir)?;

    println!(
        "cargo:warning=Shell completion files generated to {}",
        outdir.display()
    );

    Ok(())
}
