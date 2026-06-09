use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use gix::remote;
use ignore::WalkBuilder;
use is_terminal::IsTerminal;
use serde::Serialize;
use std::convert::Infallible;
use std::io::Write;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(
    name = "git-scout",
    about = "Scan directories for Git repositories that need attention.",
    version
)]
struct Cli {
    /// Directory to scan. Defaults to the current directory.
    #[arg(default_value = ".")]
    root: PathBuf,

    /// Maximum directory depth to scan
    #[arg(long, value_name = "N")]
    depth: Option<usize>,

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

    /// Show repositories with no dirty changes and no unpushed commits
    #[arg(long, conflicts_with_all = ["dirty", "unstaged", "staged", "untracked", "unpushed", "all"])]
    clean: bool,

    /// Only show repositories that have an upstream branch configured
    #[arg(long, conflicts_with = "no_upstream")]
    has_upstream: bool,

    /// Only show repositories that do not have an upstream branch configured
    #[arg(long, conflicts_with = "has_upstream")]
    no_upstream: bool,

    /// Only show repositories currently on the given branch
    #[arg(long, value_name = "NAME")]
    branch: Option<String>,

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

    /// Separate output entries with NUL bytes instead of newlines (for use with xargs -0)
    #[arg(short = '0', long, conflicts_with_all = ["json", "pretty"])]
    null: bool,

    /// Bypass all ignore rules (.gitignore, .ignore, global excludes, .git/info/exclude) when scanning
    #[arg(long)]
    no_ignore: bool,

    /// Follow hidden directories (those starting with '.')
    #[arg(long, hide = true)]
    hidden: bool,

    /// Follow symbolic links
    #[arg(long)]
    follow_links: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let is_tty = std::io::stdout().is_terminal();

    // Pretty mode: terminal (or --pretty forced), not overridden by --plain/--json/--null
    let use_pretty = !cli.plain && !cli.json && !cli.null && (cli.pretty || is_tty);
    colored::control::set_override(use_pretty);

    let root = cli.root.canonicalize().unwrap_or_else(|_| cli.root.clone());

    let repo_paths = find_git_repos(root.clone(), cli.depth, cli.hidden, cli.follow_links, cli.no_ignore)?;

    let mut results: Vec<RepoStatus> = Vec::new();
    for path in repo_paths {
        match inspect_repo(&path) {
            Ok(status) if should_show(&status, &cli) => results.push(status),
            Ok(_) => {}
            Err(e) => eprintln!("warning: {}: {e}", path.display()),
        }
    }

    if cli.json {
        output_json(&results, &root, cli.relative)?;
        return Ok(());
    }

    let output = if use_pretty {
        render_pretty(&results, &root, cli.relative)
    } else {
        render_plain(&results, &root, cli.relative, cli.details, cli.null)
    };

    if output.is_empty() {
        return Ok(());
    }

    // Auto-pager: pretty mode on a terminal, unless --no-pager or --null
    let use_pager = !cli.no_pager && !cli.null && (cli.pager || (use_pretty && is_tty));

    if use_pager {
        if let Err(e) = pipe_through_pager(&output) {
            eprintln!("warning: pager failed ({e}), falling back to direct output");
            std::io::stdout().write_all(output.as_bytes())?;
        }
    } else {
        std::io::stdout().write_all(output.as_bytes())?;
    }

    Ok(())
}

fn should_show(status: &RepoStatus, cli: &Cli) -> bool {
    let dirty = status.staged || status.unstaged || status.untracked;
    if cli.unstaged && !status.unstaged { return false; }
    if cli.staged && !status.staged { return false; }
    if cli.untracked && !status.untracked { return false; }
    if cli.dirty && !dirty { return false; }
    if cli.unpushed && !status.unpushed { return false; }
    if cli.all && !(dirty || status.unpushed) { return false; }
    if cli.clean && (dirty || status.unpushed) { return false; }
    if cli.has_upstream && !status.has_upstream { return false; }
    if cli.no_upstream && status.has_upstream { return false; }
    if let Some(ref branch) = cli.branch {
        if status.branch.as_deref() != Some(branch.as_str()) { return false; }
    }
    true
}

fn format_path(path: &Path, root: &Path, relative: bool, tilde: bool) -> String {
    if relative {
        return match path.strip_prefix(root) {
            Ok(rel) if rel.as_os_str().is_empty() => ".".to_owned(),
            Ok(rel) => rel.display().to_string(),
            Err(_) => path.display().to_string(),
        };
    }

    let s = path.display().to_string();
    if tilde {
        if let Ok(home) = std::env::var("HOME") {
            if let Some(tail) = s.strip_prefix(&home) {
                return format!("~{tail}");
            }
        }
    }
    s
}

fn status_labels(status: &RepoStatus) -> Vec<&'static str> {
    let mut v = Vec::new();
    if status.unstaged { v.push("unstaged"); }
    if status.staged { v.push("staged"); }
    if status.untracked { v.push("untracked"); }
    if status.unpushed { v.push("unpushed"); }
    v
}

fn render_pretty(results: &[RepoStatus], root: &Path, relative: bool) -> String {
    if results.is_empty() { return String::new(); }

    let names: Vec<String> = results.iter().map(|r| {
        r.path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| r.path.display().to_string())
    }).collect();

    let paths: Vec<String> = results.iter()
        .map(|r| format_path(&r.path, root, relative, true))
        .collect();

    let max_name = names.iter().map(|s| s.len()).max().unwrap_or(0);
    let max_path = paths.iter().map(|s| s.len()).max().unwrap_or(0);

    let mut out = String::new();
    for ((name, path_str), status) in names.iter().zip(paths.iter()).zip(results.iter()) {
        let labels = render_labels_pretty(status);
        if labels.is_empty() {
            out.push_str(&format!(" {name:<max_name$}   {path_str}\n"));
        } else {
            out.push_str(&format!(" {name:<max_name$}   {path_str:<max_path$}   {labels}\n"));
        }
    }
    out
}

fn render_labels_pretty(status: &RepoStatus) -> String {
    let mut parts: Vec<String> = Vec::new();
    if status.unstaged {
        parts.push(format!("{} {}", "●".red(), "unstaged".red()));
    }
    if status.staged {
        parts.push(format!("{} {}", "●".yellow(), "staged".yellow()));
    }
    if status.untracked {
        parts.push(format!("{} {}", "●".bright_blue(), "untracked".bright_blue()));
    }
    if status.unpushed {
        parts.push(format!("{} {}", "↑".cyan(), "unpushed".cyan()));
    }
    parts.join("  ")
}

fn render_plain(results: &[RepoStatus], root: &Path, relative: bool, details: bool, null: bool) -> String {
    if results.is_empty() { return String::new(); }

    let sep = if null { '\0' } else { '\n' };

    if !details {
        let mut out = String::new();
        for r in results {
            out.push_str(&format_path(&r.path, root, relative, false));
            out.push(sep);
        }
        return out;
    }

    let label_strs: Vec<String> = results.iter()
        .map(|r| status_labels(r).join(","))
        .collect();
    let max_label = label_strs.iter().map(|s| s.len()).max().unwrap_or(0);

    let mut out = String::new();
    for (r, labels) in results.iter().zip(label_strs.iter()) {
        let path = format_path(&r.path, root, relative, false);
        if max_label == 0 || labels.is_empty() && max_label == 0 {
            out.push_str(&path);
        } else {
            out.push_str(&format!("{labels:<max_label$}    {path}"));
        }
        out.push(sep);
    }
    out
}

#[derive(Serialize)]
struct JsonEntry {
    path: String,
    unstaged: bool,
    staged: bool,
    untracked: bool,
    unpushed: bool,
}

fn output_json(results: &[RepoStatus], root: &Path, relative: bool) -> Result<()> {
    let entries: Vec<JsonEntry> = results.iter().map(|r| JsonEntry {
        path: format_path(&r.path, root, relative, false),
        unstaged: r.unstaged,
        staged: r.staged,
        untracked: r.untracked,
        unpushed: r.unpushed,
    }).collect();
    println!("{}", serde_json::to_string_pretty(&entries)?);
    Ok(())
}

fn pipe_through_pager(output: &str) -> Result<()> {
    let pager = std::env::var("PAGER").unwrap_or_else(|_| "less".to_owned());
    if std::env::var("LESS").is_err() {
        // F=exit if one screen, R=pass ANSI codes, X=no termcap init/deinit
        // SAFETY: single-threaded at this point; no concurrent env reads
        unsafe { std::env::set_var("LESS", "-FRX") };
    }
    let mut child = std::process::Command::new(&pager)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("failed to launch pager '{pager}': {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(output.as_bytes());
    }
    child.wait()?;
    Ok(())
}

#[derive(Debug, Default)]
pub struct RepoStatus {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub has_upstream: bool,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub unpushed: bool,
}

pub fn inspect_repo(path: &Path) -> Result<RepoStatus> {
    let repo = gix::open(path)?;
    let mut status = RepoStatus { path: path.to_owned(), ..Default::default() };

    // Branch name and upstream presence
    let branch_fullname = repo.head()?.referent_name().map(|n| n.to_owned());
    if let Some(ref name) = branch_fullname {
        status.branch = Some(name.shorten().to_string());
        if let Some(Ok(_)) = repo.branch_remote_tracking_ref_name(name.as_ref(), remote::Direction::Fetch) {
            status.has_upstream = true;
        }
    }

    // Staged changes: HEAD tree vs index
    {
        let head_tree_id = repo.head_tree_id_or_empty()?;
        let index = repo.index_or_empty()?;
        repo.tree_index_status(
            &head_tree_id,
            &*index,
            None,
            gix::status::tree_index::TrackRenames::Disabled,
            |_, _, _| {
                status.staged = true;
                Ok::<_, Infallible>(ControlFlow::Break(()))
            },
        )?;
    }

    // Unstaged + untracked: index vs working tree
    if repo.workdir().is_some() {
        use gix::status::index_worktree::iter::Summary;
        for item in repo
            .status(gix::progress::Discard)?
            .index_worktree_rewrites(None)
            .into_index_worktree_iter(Vec::new())?
        {
            match item?.summary() {
                Some(Summary::Added) => status.untracked = true,
                Some(
                    Summary::Modified
                    | Summary::Removed
                    | Summary::TypeChange
                    | Summary::Renamed
                    | Summary::Copied
                    | Summary::Conflict,
                ) => status.unstaged = true,
                _ => {}
            }
            if status.unstaged && status.untracked {
                break;
            }
        }
    }

    // Unpushed commits: local commits not present in upstream
    if status.has_upstream {
        if let Some(ref name) = branch_fullname {
            if let Some(Ok(tracking)) = repo.branch_remote_tracking_ref_name(name.as_ref(), remote::Direction::Fetch) {
                if let Ok(mut upstream_ref) = repo.find_reference(tracking.as_ref()) {
                    if let (Ok(local_id), Ok(upstream_id)) =
                        (repo.head_id(), upstream_ref.peel_to_id())
                    {
                        let local = local_id.detach();
                        let upstream = upstream_id.detach();
                        if local != upstream {
                            status.unpushed = repo
                                .rev_walk([local])
                                .with_hidden([upstream])
                                .all()?
                                .next()
                                .is_some();
                        }
                    }
                }
            }
        }
    }

    Ok(status)
}

pub fn find_git_repos(
    root: PathBuf,
    depth: Option<usize>,
    hidden: bool,
    follow_links: bool,
    no_ignore: bool,
) -> Result<Vec<PathBuf>> {
    let mut repos = Vec::new();

    for result in WalkBuilder::new(&root)
        .max_depth(depth)
        .hidden(!hidden)
        .follow_links(follow_links)
        .ignore(!no_ignore)
        .git_ignore(!no_ignore)
        .git_global(!no_ignore)
        .git_exclude(!no_ignore)
        .build()
    {
        match result {
            Err(err) => eprintln!("warning: {err}"),
            Ok(entry) if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) => {
                if entry.path().join(".git").exists() {
                    repos.push(entry.path().to_path_buf());
                }
            }
            _ => {}
        }
    }

    Ok(repos)
}
