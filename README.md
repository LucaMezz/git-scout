# git-scout

**Scan directories for Git repositories that need attention.**

`git-scout` is a fast, composable CLI tool for finding local Git repositories with uncommitted changes, untracked files, staged changes, or unpushed commits.

---

## Overview

`git-scout` helps you quickly find Git repositories in a directory tree that still have local work needing attention.

It is designed for developer workspaces like `~/dev`, where you may have many repositories and sometimes forget which ones have changes you have not committed or pushed.

Instead of manually checking each project, run:

```sh
git-scout ~/dev
```

and get a clean list of repositories with local changes.

**Output format is context-aware.** When stdout is a terminal, `git-scout` uses colored, formatted output with aligned columns. When stdout is piped to another tool, it automatically switches to plain text — so it composes naturally with `fzf`, `xargs`, `lazygit`, `nvim`, and shell scripts without any extra flags.

By default, `git-scout` respects `.gitignore` files it encounters during scanning and skips ignored directories. Pass `--no-ignore` to scan everything.

---

## Installation

### With Cargo

```sh
cargo install git-scout
```

### From source

```sh
git clone https://github.com/LucaMezz/git-scout.git
cd git-scout
cargo install --path .
```

### Linux packages

Packages for common Linux distributions will be available on release. Check the [releases page](https://github.com/LucaMezz/git-scout/releases) for the latest distribution-specific packages.

---

## Usage

Scan the current directory:

```sh
git-scout
```

Scan a specific workspace:

```sh
git-scout ~/dev
```

Show repositories with any uncommitted changes:

```sh
git-scout ~/dev --dirty
```

Show repositories with unstaged changes:

```sh
git-scout ~/dev --unstaged
```

Show repositories with staged changes:

```sh
git-scout ~/dev --staged
```

Show repositories with untracked files:

```sh
git-scout ~/dev --untracked
```

Show repositories with local commits not yet pushed:

```sh
git-scout ~/dev --unpushed
```

Show repositories with either uncommitted changes or unpushed commits:

```sh
git-scout ~/dev --all
```

Show why each repository matched:

```sh
git-scout ~/dev --all --details
```

Force plain text output even when on a terminal:

```sh
git-scout ~/dev --all --plain
```

Force colored output even when piped:

```sh
git-scout ~/dev --all --pretty
```

Page through results:

```sh
git-scout ~/dev --all --pager
```

Scan all directories, including those ignored by `.gitignore`:

```sh
git-scout ~/dev --all --no-ignore
```

---

## Output Formats

### Pretty (terminal default)

When stdout is a terminal, `git-scout` renders colored, aligned output:

```
 codelane   ~/dev/codelane    ● unstaged  ● untracked
 mezzarch   ~/dev/mezzarch    ↑ unpushed
 appkit     ~/dev/appkit      ● staged
```

Status labels are color-coded and columns are aligned for readability. Long result sets are automatically paged through `$PAGER` (or `less` if unset).

### Plain (pipe default)

When stdout is piped, output is one path per line (or `label  path` with `--details`):

```
unstaged,untracked    /home/luca/dev/codelane
unpushed              /home/luca/dev/mezzarch
staged                /home/luca/dev/appkit
```

This is the format consumed by `fzf`, `xargs`, and shell scripts. You do not need to pass any flag — the switch is automatic.

Use `--plain` to force this format on a terminal, or `--pretty` to force colored output when piped.

### JSON

Pass `--json` to get structured output for use in scripts or other tools:

```sh
git-scout ~/dev --all --json
```

```json
[
  {
    "path": "/home/luca/dev/codelane",
    "unstaged": true,
    "staged": false,
    "untracked": true,
    "unpushed": false
  },
  {
    "path": "/home/luca/dev/mezzarch",
    "unstaged": false,
    "staged": false,
    "untracked": false,
    "unpushed": true
  }
]
```

---

## Shell Workflows

`git-scout` is built for Unix-style composition. When piped, output is plain text — one path per line — so it connects cleanly with other tools.

### Pick a repo with `fzf`

```sh
git-scout ~/dev --all | fzf
```

### Open a selected repo in `lazygit`

```sh
repo="$(git-scout ~/dev --all | fzf)" && lazygit -p "$repo"
```

### Change into a selected repo

```sh
cd "$(git-scout ~/dev --all | fzf)"
```

### Open a selected repo in Neovim

```sh
repo="$(git-scout ~/dev --all | fzf)" && nvim "$repo"
```

### Preview changes inline with `fzf`

```sh
git-scout ~/dev --all | fzf --preview 'git -C {} status --short'
```

---

## Example Aliases

Add these to your `.zshrc` or `.bashrc`:

```sh
alias gs='git-scout'

# cd into a repo with pending work
cdr() {
  local repo
  repo="$(git-scout "${1:-$HOME/dev}" --all | fzf)" || return
  [ -n "$repo" ] && cd "$repo"
}

# open a repo with pending work in lazygit
lgdr() {
  local repo
  repo="$(git-scout "${1:-$HOME/dev}" --all | fzf)" || return
  [ -n "$repo" ] && lazygit -p "$repo"
}
```

---

## Command Reference

```
Usage: git-scout [ROOT] [OPTIONS]

Arguments:
  [ROOT]              Directory to scan. Defaults to the current directory.

Options:
      --depth <N>     Maximum directory depth to scan
      --unstaged      Show repositories with unstaged tracked changes
      --staged        Show repositories with staged changes
      --untracked     Show repositories with untracked files
  -d, --dirty         Show repositories with unstaged, staged, or untracked changes
      --unpushed      Show repositories with commits not pushed to upstream
  -a, --all           Show repositories with dirty or unpushed work
  -v, --details       Show matching status labels before each path
      --relative      Print paths relative to the scan root
  -j, --json          Print machine-readable JSON output
      --pretty        Force colored, formatted output (default when stdout is a terminal)
      --plain         Force plain text output (default when stdout is piped)
      --pager         Pipe output through a pager (default: $PAGER or less)
      --no-pager      Disable automatic pager
      --no-ignore     Scan all directories, including those ignored by .gitignore
  -h, --help          Print help
  -V, --version       Print version
```

---

## Understanding Repository States

### Dirty

A repository is **dirty** if it has local file changes that have not been committed:

- Unstaged changes to tracked files
- Staged but uncommitted changes
- Untracked files

### Unpushed

A repository has **unpushed commits** if it has local commits that have not been pushed to its upstream branch. This is tracked separately from dirty state because the working tree may be clean — the changes are committed but not yet shared.

### --all

The `--all` flag is the union of dirty and unpushed: it matches any repository that has work which has not yet been reflected in the remote.

---

## .gitignore Behaviour

By default, `git-scout` respects every `.gitignore` file it encounters while walking the directory tree. Any directory that would be ignored by Git is skipped entirely, which keeps scans fast in workspaces that contain large build artifact or dependency directories (e.g. `node_modules`, `target`, `.venv`).

Pass `--no-ignore` to disable this behaviour and scan all directories unconditionally:

```sh
git-scout ~/dev --all --no-ignore
```

This is useful when your workspace layout puts repositories inside directories that are themselves gitignored, or when you want to audit a directory tree that has unusual ignore rules.

---

## Development

Clone the repository:

```sh
git clone https://github.com/LucaMezz/git-scout.git
cd git-scout
```

Run the project:

```sh
cargo run -- ~/dev --all
```

Run checks:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Build a release binary:

```sh
cargo build --release
```

---

## Roadmap

- [ ] Repository scanning
- [ ] Dirty state detection
- [ ] Unpushed commit detection
- [ ] Detailed output (`--details`)
- [ ] JSON output (`--json`)
- [ ] Pretty output (`--pretty` / `--plain`)
- [ ] Built-in pager (`--pager` / `--no-pager`)
- [ ] `.gitignore` bypass (`--no-ignore`)
- [ ] Config file support
- [ ] Shell completions
- [ ] Parallel scanning
- [ ] Release on crates.io
- [ ] Linux packages (`.deb`, `.rpm`, AUR)

---

## Related Tools

`git-scout` focuses on being small, fast, and composable. Unlike multi-repository management tools, it does not require you to register repositories or maintain a config file. It scans a directory and reports what it finds.

It is designed around workflows like:

```sh
git-scout ~/dev --all | fzf
```

where the output feeds directly into other tools, and like:

```sh
git-scout ~/dev --all --pager
```

where results are browsed interactively in the terminal.

---

## License

This project is licensed under the [MIT License](LICENSE).

You are free to use, modify, and distribute it.
