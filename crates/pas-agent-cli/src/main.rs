use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use pas_agent_core::{
    capture_git_state, generate_context, get_changed_files, write_context_file, Agent,
    ContextFormat, GitRef, Session, SessionError, SessionStore, TaskState, WriteOutcome, STORE_DIR,
};
use std::collections::BTreeSet;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Parser)]
#[command(
    name = "pas-agent",
    about = "Portable work sessions across AI coding agents",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

// Parsed once per run, so the size gap between variants is irrelevant.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
enum Commands {
    /// Initialize a PAS-Agent session at the root of the current project
    Init {
        /// Project name (defaults to the project directory name)
        #[arg(short, long)]
        name: Option<String>,

        /// Task description (prompted for if omitted)
        #[arg(short, long)]
        task: Option<String>,
    },

    /// Save a checkpoint of the current session state
    Checkpoint {
        /// Checkpoint message describing current progress (prompted for if omitted)
        message: Vec<String>,
    },

    /// Show current session status
    Status,

    /// List the session's checkpoints
    List,

    /// Write session context into the file a target agent reads at startup
    Export {
        /// Target agent
        #[arg(short, long, value_enum)]
        to: Target,

        /// Output file (defaults to CLAUDE.md, AGENTS.md or GEMINI.md in the project root)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Replace the whole file instead of only the PAS-Agent block
        #[arg(long)]
        force: bool,
    },

    /// Update session task state (list flags can be repeated)
    Update {
        /// Set task description
        #[arg(long)]
        task: Option<String>,

        /// Set objective
        #[arg(long)]
        objective: Option<String>,

        /// Set next action
        #[arg(long, conflicts_with = "clear_next")]
        next: Option<String>,

        /// Clear the next action
        #[arg(long)]
        clear_next: bool,

        /// Add a completed item
        #[arg(long, value_name = "ITEM")]
        completed: Vec<String>,

        /// Add a remaining item
        #[arg(long, value_name = "ITEM")]
        remaining: Vec<String>,

        /// Add a decision
        #[arg(long, value_name = "ITEM")]
        decision: Vec<String>,

        /// Add a blocker
        #[arg(long, value_name = "ITEM")]
        blocker: Vec<String>,

        /// Add a constraint
        #[arg(long, value_name = "ITEM")]
        constraint: Vec<String>,

        /// Add a discovery
        #[arg(long, value_name = "ITEM")]
        discovery: Vec<String>,

        /// Move remaining item #N (as numbered by `status`) to completed
        #[arg(long, value_name = "N")]
        done: Vec<usize>,

        /// Remove resolved blocker #N (as numbered by `status`)
        #[arg(long, value_name = "N")]
        resolve: Vec<usize>,
    },

    /// Remove items from a task list by number (as shown by `status`)
    Remove {
        /// List to remove from
        #[arg(value_enum)]
        list: ListKind,

        /// Item numbers to remove
        #[arg(required = true, value_name = "N")]
        items: Vec<usize>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Target {
    #[value(name = "claude-code", alias = "claude")]
    ClaudeCode,
    Codex,
    #[value(name = "opencode")]
    OpenCode,
    #[value(name = "kiro-cli", alias = "kiro")]
    KiroCli,
    #[value(alias = "agy")]
    Antigravity,
    #[value(name = "gemini-cli", alias = "gemini")]
    GeminiCli,
    Cursor,
}

impl From<Target> for Agent {
    fn from(target: Target) -> Self {
        match target {
            Target::ClaudeCode => Agent::ClaudeCode,
            Target::Codex => Agent::Codex,
            Target::OpenCode => Agent::OpenCode,
            Target::KiroCli => Agent::KiroCli,
            Target::Antigravity => Agent::Antigravity,
            Target::GeminiCli => Agent::GeminiCli,
            Target::Cursor => Agent::Cursor,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum ListKind {
    Completed,
    Remaining,
    #[value(alias = "decision")]
    Decisions,
    #[value(alias = "blocker")]
    Blockers,
    #[value(alias = "constraint")]
    Constraints,
    #[value(alias = "discovery")]
    Discoveries,
}

impl ListKind {
    fn get(self, task: &mut TaskState) -> &mut Vec<String> {
        match self {
            ListKind::Completed => &mut task.completed,
            ListKind::Remaining => &mut task.remaining,
            ListKind::Decisions => &mut task.decisions,
            ListKind::Blockers => &mut task.blockers,
            ListKind::Constraints => &mut task.constraints,
            ListKind::Discoveries => &mut task.discoveries,
        }
    }

    fn name(self) -> &'static str {
        match self {
            ListKind::Completed => "completed",
            ListKind::Remaining => "remaining",
            ListKind::Decisions => "decisions",
            ListKind::Blockers => "blockers",
            ListKind::Constraints => "constraints",
            ListKind::Discoveries => "discoveries",
        }
    }
}

/// Removes the given 1-based positions from `list`, returning the removed items in
/// their original order. Fails without modifying `list` if any position is invalid.
fn take_items(list: &mut Vec<String>, positions: &[usize], list_name: &str) -> Result<Vec<String>> {
    let unique: BTreeSet<usize> = positions.iter().copied().collect();
    if let Some(bad) = unique.iter().find(|&&n| n == 0 || n > list.len()) {
        return Err(format!(
            "no item #{bad} in {list_name} (it has {} item(s); run `pas-agent status` to see numbers)",
            list.len()
        )
        .into());
    }
    let mut removed: Vec<String> = unique.iter().rev().map(|&n| list.remove(n - 1)).collect();
    removed.reverse();
    Ok(removed)
}

fn push_items(list: &mut Vec<String>, items: Vec<String>) {
    list.extend(
        items
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
    );
}

fn prompt(message: &str) -> Result<String> {
    eprint!("{message} ");
    io::stderr().flush()?;
    let mut input = String::new();
    io::stdin().lock().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn short_commit(commit: Option<&str>) -> &str {
    commit.map_or("no commits", |c| &c[..c.len().min(8)])
}

fn describe_git(git: &GitRef) -> String {
    format!(
        "{} @ {}{}",
        git.branch.as_deref().unwrap_or("detached HEAD"),
        short_commit(git.commit.as_deref()),
        if git.dirty {
            " (uncommitted changes)"
        } else {
            ""
        }
    )
}

fn print_numbered(heading: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    println!();
    println!("{heading} ({}):", items.len());
    for (i, item) in items.iter().enumerate() {
        println!("  {:>2}. {item}", i + 1);
    }
}

/// Finds the session for the current directory or one of its parents.
fn open_session(cwd: &Path) -> Result<(SessionStore, Session)> {
    let Some(store) = SessionStore::discover(cwd) else {
        return Err(format!(
            "no PAS-Agent session found in {} or any parent directory; run `pas-agent init` first",
            cwd.display()
        )
        .into());
    };
    let session = store.load()?;
    Ok((store, session))
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;

    match cli.command {
        Commands::Init { name, task } => {
            let root = SessionStore::init_root(&cwd);
            let store = SessionStore::new(&root);
            if store.exists() {
                return Err(format!(
                    "a PAS-Agent session already exists at {}",
                    store.session_path().display()
                )
                .into());
            }

            let project_name = name.unwrap_or_else(|| {
                root.file_name()
                    .map_or_else(|| "unknown".into(), |n| n.to_string_lossy().into_owned())
            });

            let description = match task {
                Some(t) => t.trim().to_string(),
                None => prompt("Task description:")?,
            };
            if description.is_empty() {
                return Err("task description cannot be empty".into());
            }

            let mut session = Session::new(project_name.clone(), description);
            session.git = capture_git_state(&root);
            store.save(&session)?;

            println!("PAS-Agent session initialized.");
            println!("  Project: {project_name}");
            println!("  Session: {}", session.id);
            println!("  Store:   {}", store.session_path().display());
            if let Some(git) = &session.git {
                println!("  Git:     {}", describe_git(git));
            }
            println!();
            println!(
                "Tip: commit {STORE_DIR}/ to carry the session across machines, or add it to .gitignore to keep it local."
            );
        }

        Commands::Checkpoint { message } => {
            let (store, mut session) = open_session(&cwd)?;
            let msg = if message.is_empty() {
                prompt("Checkpoint message:")?
            } else {
                message.join(" ").trim().to_string()
            };
            if msg.is_empty() {
                return Err("checkpoint message cannot be empty".into());
            }

            session.git = capture_git_state(store.project_dir());
            let files = get_changed_files(store.project_dir());
            let file_count = files.len();
            session.add_checkpoint(msg.clone(), files);
            store.save(&session)?;

            println!("Checkpoint #{} saved: {msg}", session.checkpoints.len());
            println!("  Files with uncommitted changes: {file_count}");
        }

        Commands::Status => {
            let (store, session) = open_session(&cwd)?;
            let task = &session.task;

            println!("Project:   {}", session.project_name);
            println!("Session:   {}", session.id);
            println!("Root:      {}", store.project_dir().display());
            println!(
                "Created:   {}",
                session.created_at.format("%Y-%m-%d %H:%M UTC")
            );
            println!(
                "Updated:   {}",
                session.updated_at.format("%Y-%m-%d %H:%M UTC")
            );
            println!();
            println!("Task:      {}", task.description);
            if !task.objective.is_empty() {
                println!("Objective: {}", task.objective);
            }
            if let Some(next) = &task.next_action {
                println!("Next:      {next}");
            }

            print_numbered("Completed", &task.completed);
            print_numbered("Remaining", &task.remaining);
            print_numbered("Blockers", &task.blockers);
            print_numbered("Decisions", &task.decisions);
            print_numbered("Constraints", &task.constraints);
            print_numbered("Discoveries", &task.discoveries);

            if let Some(git) = capture_git_state(store.project_dir()) {
                println!();
                println!("Git:       {}", describe_git(&git));
            }

            println!();
            println!("Checkpoints: {}", session.checkpoints.len());
            if let Some(cp) = session.latest_checkpoint() {
                println!(
                    "  Latest: {} — {}",
                    cp.timestamp.format("%Y-%m-%d %H:%M UTC"),
                    cp.message
                );
            }
        }

        Commands::List => {
            let (_, session) = open_session(&cwd)?;
            if session.checkpoints.is_empty() {
                println!("No checkpoints yet. Save one with `pas-agent checkpoint <message>`.");
            } else {
                println!(
                    "Checkpoints for {} ({}):",
                    session.project_name,
                    session.checkpoints.len()
                );
                for (i, cp) in session.checkpoints.iter().enumerate() {
                    let git = cp.git.as_ref().map_or_else(String::new, |g| {
                        format!("  [{}]", short_commit(g.commit.as_deref()))
                    });
                    let n = cp.files_changed.len();
                    println!(
                        "  {:>3}. {}  {}  ({n} file{}){git}",
                        i + 1,
                        cp.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
                        cp.message,
                        if n == 1 { "" } else { "s" }
                    );
                }
            }
        }

        Commands::Export { to, output, force } => {
            let (store, mut session) = open_session(&cwd)?;
            let agent = Agent::from(to);
            let format = ContextFormat::for_agent(agent);

            session.git = capture_git_state(store.project_dir());
            store.save(&session)?;

            let content = generate_context(&session);
            let out_path = output.unwrap_or_else(|| store.project_dir().join(format.filename()));
            let outcome = write_context_file(&out_path, &content, force)?;

            let path = out_path.display();
            match outcome {
                WriteOutcome::Created => println!("Created {path} for {agent}."),
                WriteOutcome::Updated => {
                    println!("Updated the PAS-Agent block in {path} for {agent}.");
                }
                WriteOutcome::Appended => println!(
                    "Added a PAS-Agent block to {path} for {agent}; your existing content was kept."
                ),
                WriteOutcome::Overwritten => println!("Overwrote {path} for {agent}."),
            }

            let default_name = format.filename();
            if out_path.file_name().and_then(|n| n.to_str()) == Some(default_name) {
                println!("{agent} reads {default_name} from the project root at session start.");
            } else {
                println!(
                    "Note: {agent} loads {default_name} automatically; point it at this file yourself."
                );
            }
        }

        Commands::Update {
            task,
            objective,
            next,
            clear_next,
            completed,
            remaining,
            decision,
            blocker,
            constraint,
            discovery,
            done,
            resolve,
        } => {
            let (store, mut session) = open_session(&cwd)?;
            let before = session.task.clone();
            let t = &mut session.task;

            // Numbered operations first, so numbers match what `status` showed.
            let finished = take_items(&mut t.remaining, &done, "remaining")?;
            t.completed.extend(finished);
            take_items(&mut t.blockers, &resolve, "blockers")?;

            if let Some(v) = task.map(|s| s.trim().to_string()) {
                if v.is_empty() {
                    return Err("task description cannot be empty".into());
                }
                t.description = v;
            }
            if let Some(v) = objective {
                t.objective = v.trim().to_string();
            }
            if let Some(v) = next {
                t.next_action = Some(v.trim().to_string()).filter(|s| !s.is_empty());
            }
            if clear_next {
                t.next_action = None;
            }
            push_items(&mut t.completed, completed);
            push_items(&mut t.remaining, remaining);
            push_items(&mut t.decisions, decision);
            push_items(&mut t.blockers, blocker);
            push_items(&mut t.constraints, constraint);
            push_items(&mut t.discoveries, discovery);

            if session.task == before {
                return Err("nothing to update; see `pas-agent update --help`".into());
            }

            session.git = capture_git_state(store.project_dir());
            session.updated_at = Utc::now();
            store.save(&session)?;
            println!("Session updated.");
        }

        Commands::Remove { list, items } => {
            let (store, mut session) = open_session(&cwd)?;
            let removed = take_items(list.get(&mut session.task), &items, list.name())?;
            session.updated_at = Utc::now();
            store.save(&session)?;
            for item in removed {
                println!("Removed from {}: {item}", list.name());
            }
        }
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        if let Some(SessionError::Parse { .. }) = e.downcast_ref::<SessionError>() {
            eprintln!("The session file is damaged. Fix the JSON by hand or restore it from git.");
        }
        process::exit(1);
    }
}
