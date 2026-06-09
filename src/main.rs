use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "git-scout",
    about = "Scan directories for Git repositories that need attention.",
    version
)]
struct Cli {
    /// Directory to scan. Defaults to the current directory.
    root: Option<PathBuf>,

    /// Maximum directory depth to scan
    #[arg(long, value_name = "N")]
    depth: Option<u32>,

    /// Show repositories with unstaged tracked changes
    #[arg(long)]
    unstaged: bool,

    /// Show repositories with staged changes
    #[arg(long)]
    staged: bool,

    /// Show repositories with untracked files
    #[arg(long)]
    untracked: bool,

    /// Show repositories with unstaged, staged, or untracked changes
    #[arg(short = 'd', long)]
    dirty: bool,

    /// Show repositories with commits not pushed to upstream
    #[arg(long)]
    unpushed: bool,

    /// Show repositories with dirty or unpushed work
    #[arg(short = 'a', long)]
    all: bool,

    /// Show matching status labels before each path
    #[arg(short = 'v', long)]
    details: bool,

    /// Print paths relative to the scan root
    #[arg(long)]
    relative: bool,

    /// Print machine-readable JSON output
    #[arg(short = 'j', long)]
    json: bool,

    /// Force colored, formatted output (default when stdout is a terminal)
    #[arg(long, conflicts_with = "plain")]
    pretty: bool,

    /// Force plain text output (default when stdout is piped)
    #[arg(long, conflicts_with = "pretty")]
    plain: bool,

    /// Pipe output through a pager (default: $PAGER or less)
    #[arg(long, conflicts_with = "no_pager")]
    pager: bool,

    /// Disable automatic pager
    #[arg(long, conflicts_with = "pager")]
    no_pager: bool,

    /// Scan all directories, including those ignored by .gitignore
    #[arg(long)]
    no_ignore: bool,
}

fn main() {
    let _cli = Cli::parse();
}
