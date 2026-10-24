//! Command line interface: link, unlink, list, info, doctor and the TUI.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use serde_json::json;

use crate::config;
use crate::fsops::{self, ExistingKind, LinkInfo, LinkOptions, LinkStatus, TargetKind};
use crate::util::{ago_label, format_timestamp, human_size, tildify, truncate};
use crate::{paint, tui};

const EXAMPLES: &str = "\
Examples:
  linkman link ./dist ~/bin/app        link ~/bin/app -> ./dist
  linkman link ./dist -t ~/bin -r      relative link ~/bin/dist -> ../dist
  linkman list ~/bin                   audit a folder of symlinks
  linkman doctor ~/projects            find broken links and loops
  linkman                              open the interactive TUI
  linkman config --init                write a commented starter config
  linkman completions fish             print fish completions";

#[derive(Parser, Debug)]
#[command(
    name = "linkman",
    version,
    about = "Create, inspect and manage symlinks — from the shell or a beautiful TUI",
    after_help = EXAMPLES,
    propagate_version = true,
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

impl Cli {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Create a symlink
    #[command(visible_alias = "ln")]
    Link(LinkArgs),
    /// Remove symlinks (never regular files)
    #[command(visible_alias = "rm")]
    Unlink(UnlinkArgs),
    /// List symlinks in a directory
    #[command(visible_alias = "ls")]
    List(ListArgs),
    /// Inspect a path or symlink
    Info(InfoArgs),
    /// Find broken symlinks and loops
    Doctor(DoctorArgs),
    /// Open the interactive TUI (default)
    #[command(visible_alias = "ui")]
    Tui(TuiArgs),
    /// Show the config file and effective TUI defaults
    Config(ConfigArgs),
    /// Generate shell completions
    #[command(visible_alias = "completion")]
    Completions(CompletionsArgs),
}

#[derive(Args, Debug)]
pub struct ConfigArgs {
    /// Create a commented starter config if none exists
    #[arg(long)]
    init: bool,
}

#[derive(Args, Debug)]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    shell: Shell,
}

#[derive(Args, Debug)]
pub struct LinkArgs {
    /// Path the link will point to
    #[arg(value_name = "TARGET")]
    target: PathBuf,
    /// Where to create the link (defaults to the target name in the current directory)
    #[arg(value_name = "LINK")]
    link: Option<PathBuf>,
    /// Create the link inside this directory using the target's name
    #[arg(
        short = 't',
        long = "into",
        value_name = "DIR",
        conflicts_with = "link"
    )]
    into: Option<PathBuf>,
    /// Custom link name (with --into)
    #[arg(short = 'n', long, value_name = "NAME", requires = "into")]
    name: Option<String>,
    /// Store the target relative to the link
    #[arg(short = 'r', long)]
    relative: bool,
    /// Replace an existing file or symlink
    #[arg(short = 'f', long)]
    force: bool,
    /// Keep the existing entry as <name>.bak
    #[arg(short = 'b', long)]
    backup: bool,
    /// Create missing parent directories
    #[arg(short = 'p', long = "parents")]
    parents: bool,
    /// Show what would happen without doing it
    #[arg(long)]
    dry_run: bool,
    /// Machine readable output
    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
pub struct UnlinkArgs {
    /// Symlinks to remove
    #[arg(value_name = "LINK", required = true, num_args = 1..)]
    paths: Vec<PathBuf>,
    /// Show what would happen without doing it
    #[arg(long)]
    dry_run: bool,
    /// Machine readable output
    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
pub struct ListArgs {
    /// Directory to scan
    #[arg(value_name = "DIR", default_value = ".")]
    dir: PathBuf,
    /// Scan subdirectories too
    #[arg(short = 'r', long)]
    recursive: bool,
    /// Machine readable output
    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
pub struct InfoArgs {
    /// Path to inspect
    #[arg(value_name = "PATH")]
    path: PathBuf,
    /// Machine readable output
    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
pub struct DoctorArgs {
    /// Directory to check
    #[arg(value_name = "DIR", default_value = ".")]
    dir: PathBuf,
    /// Only scan the top directory
    #[arg(long)]
    shallow: bool,
    /// Machine readable output
    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
pub struct TuiArgs {
    /// Directory to open in the SOURCE pane
    #[arg(value_name = "DIR")]
    dir: Option<PathBuf>,
    /// Directory to open in the DESTINATION pane
    #[arg(long, value_name = "DIR")]
    dest: Option<PathBuf>,
}

pub fn run(cli: Cli) -> Result<i32> {
    match cli.command {
        None => open_tui(TuiArgs {
            dir: None,
            dest: None,
        }),
        Some(Command::Link(args)) => cmd_link(args),
        Some(Command::Unlink(args)) => cmd_unlink(args),
        Some(Command::List(args)) => cmd_list(args),
        Some(Command::Info(args)) => cmd_info(args),
        Some(Command::Doctor(args)) => cmd_doctor(args),
        Some(Command::Tui(args)) => open_tui(args),
        Some(Command::Config(args)) => cmd_config(args),
        Some(Command::Completions(args)) => cmd_completions(args),
    }
}

fn open_tui(args: TuiArgs) -> Result<i32> {
    tui::run(args.dir, args.dest)?;
    Ok(0)
}

fn cmd_completions(args: CompletionsArgs) -> Result<i32> {
    let mut command = Cli::command();
    let mut buffer = Vec::new();
    generate(args.shell, &mut command, "linkman", &mut buffer);
    match std::io::stdout().write_all(&buffer) {
        Ok(()) => Ok(0),
        Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => Ok(0),
        Err(err) => Err(err).context("could not write completions"),
    }
}

fn on_off(value: bool) -> String {
    if value {
        paint::green("on")
    } else {
        paint::dim("off")
    }
}

fn cmd_config(args: ConfigArgs) -> Result<i32> {
    let loaded = config::load();
    let path = loaded.path.clone().or_else(config::default_path);

    if args.init {
        if let Some(existing) = &loaded.path {
            println!(
                "  {} config already exists: {}",
                paint::green(paint::TICK),
                paint::bold(&tildify(existing))
            );
            return Ok(0);
        }
        let Some(path) = path else {
            bail!("cannot find a config location — set HOME or XDG_CONFIG_HOME");
        };
        config::write_sample(&path)
            .with_context(|| format!("could not write {}", path.display()))?;
        println!(
            "  {} wrote {}",
            paint::green(paint::TICK),
            paint::bold(&tildify(&path))
        );
        println!(
            "  {}",
            paint::dim("edit it, then relaunch linkman — JSON and JSONC both work")
        );
        return Ok(0);
    }

    println!();
    match &path {
        Some(path) => {
            let state = if loaded.path.is_some() {
                if loaded.warning.is_some() {
                    paint::yellow("loaded with warnings")
                } else {
                    paint::green("loaded")
                }
            } else {
                paint::dim("not created yet")
            };
            println!(
                "  {}  {}\n          {}",
                paint::dim("config"),
                paint::bold(&tildify(path)),
                state
            );
        }
        None => println!("  {}", paint::dim("no config location available")),
    }
    if let Some(warning) = &loaded.warning {
        println!("  {} {}", paint::red(paint::CROSS), warning);
    }

    println!();
    println!(
        "  {} {}",
        paint::dim(&pad("relative", 9)),
        on_off(loaded.config.relative)
    );
    println!(
        "  {} {}",
        paint::dim(&pad("force", 9)),
        on_off(loaded.config.force)
    );
    println!(
        "  {} {}",
        paint::dim(&pad("hidden", 9)),
        on_off(loaded.config.show_hidden)
    );
    println!(
        "  {} {}",
        paint::dim(&pad("undo", 9)),
        loaded.config.undo_limit
    );
    println!(
        "  {} {}",
        paint::dim(&pad("source", 9)),
        loaded
            .config
            .source
            .as_ref()
            .map(|path| tildify(path))
            .unwrap_or_else(|| paint::dim("(current folder)"))
    );
    println!(
        "  {} {}",
        paint::dim(&pad("dest", 9)),
        loaded
            .config
            .dest
            .as_ref()
            .map(|path| tildify(path))
            .unwrap_or_else(|| paint::dim("(current folder)"))
    );
    if loaded.path.is_none() {
        println!();
        println!(
            "  {}",
            paint::dim("create a commented starter with: linkman config --init")
        );
    }
    println!();
    Ok(0)
}

// ---------------------------------------------------------------------------
// link
// ---------------------------------------------------------------------------

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() || name == "." || name == ".." {
        bail!("invalid link name: {name:?}");
    }
    if name.contains('/') || name.contains('\\') {
        bail!("link name must not contain path separators: {name:?}");
    }
    Ok(())
}

fn inferred_name(target: &Path) -> Result<String> {
    target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            anyhow!(
                "cannot infer a link name from {} — pass LINK or use --name",
                target.display()
            )
        })
}

fn cmd_link(args: LinkArgs) -> Result<i32> {
    let link_path = if let Some(link) = &args.link {
        if args.name.is_some() {
            bail!("--name can only be used with --into");
        }
        link.clone()
    } else {
        let name = match &args.name {
            Some(name) => {
                validate_name(name)?;
                name.clone()
            }
            None => inferred_name(&args.target)?,
        };
        match &args.into {
            Some(dir) => dir.join(name),
            None => PathBuf::from(name),
        }
    };

    let opts = LinkOptions {
        relative: args.relative,
        force: args.force,
        backup: args.backup,
        create_parents: args.parents,
        dry_run: args.dry_run,
    };
    let outcome = fsops::create_link(&args.target, &link_path, &opts)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&outcome)?);
        return Ok(0);
    }

    let verb = if outcome.dry_run {
        "would link"
    } else if outcome.replaced.is_some() {
        "re-linked"
    } else {
        "linked"
    };
    let symbol = if outcome.dry_run {
        paint::dim("·")
    } else {
        paint::green(paint::TICK)
    };
    let mut line = format!(
        "  {symbol} {}  {}  {} {}",
        paint::dim(verb),
        paint::bold(&outcome.link.display().to_string()),
        paint::dim(paint::ARROW),
        paint::cyan(&outcome.stored_target.display().to_string())
    );
    if let Some(kind) = outcome.replaced {
        let what = match kind {
            ExistingKind::Symlink => "existing symlink",
            ExistingKind::File => "existing file",
            ExistingKind::Dir => "existing directory",
            ExistingKind::Other => "existing entry",
        };
        line.push_str(&paint::dim(&format!("  (replaced {what})")));
    }
    if let Some(backup) = &outcome.backup {
        line.push_str(&paint::dim(&format!("  (backup: {})", backup.display())));
    }
    if outcome.status == LinkStatus::Broken {
        line.push_str(&paint::yellow("  (dangling)"));
    }
    println!("{line}");
    Ok(0)
}

// ---------------------------------------------------------------------------
// unlink
// ---------------------------------------------------------------------------

fn cmd_unlink(args: UnlinkArgs) -> Result<i32> {
    let mut ok = Vec::new();
    let mut failed = Vec::new();
    for path in &args.paths {
        match fsops::remove_link(path, args.dry_run) {
            Ok(()) => ok.push(path.clone()),
            Err(err) => failed.push((path.clone(), err.to_string())),
        }
    }

    if args.json {
        let payload = json!({
            "dry_run": args.dry_run,
            "removed": ok,
            "failed": failed.iter().map(|(p, e)| json!({"path": p, "error": e})).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else if args.dry_run {
        for path in &ok {
            println!(
                "  {} would remove {}",
                paint::dim("·"),
                paint::bold(&path.display().to_string())
            );
        }
        for (path, err) in &failed {
            println!(
                "  {} {} — {}",
                paint::red(paint::CROSS),
                path.display(),
                paint::dim(err)
            );
        }
    } else {
        for path in &ok {
            println!(
                "  {} removed {}",
                paint::green(paint::TICK),
                paint::bold(&path.display().to_string())
            );
        }
        for (path, err) in &failed {
            eprintln!(
                "  {} {} — {}",
                paint::red(paint::CROSS),
                path.display(),
                paint::dim(err)
            );
        }
    }

    if !args.json && args.paths.len() > 1 {
        let summary = format!("{} removed · {} failed", ok.len(), failed.len());
        println!("{}", paint::dim(&summary));
    }

    Ok(if failed.is_empty() { 0 } else { 1 })
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

fn pad(text: &str, width: usize) -> String {
    let len = text.chars().count();
    format!("{text}{}", " ".repeat(width.saturating_sub(len)))
}

fn status_paint(status: LinkStatus) -> String {
    match status {
        LinkStatus::Ok => paint::green("ok"),
        LinkStatus::Broken => paint::red("broken"),
        LinkStatus::Loop => paint::yellow("loop"),
    }
}

fn cmd_list(args: ListArgs) -> Result<i32> {
    let dir = fsops::absolutize(&args.dir)?;
    if !dir.is_dir() {
        bail!("not a directory: {}", args.dir.display());
    }
    let links = fsops::scan_links(&dir, args.recursive)?;

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "dir": dir,
                "recursive": args.recursive,
                "count": links.len(),
                "broken": links.iter().filter(|l| l.status == LinkStatus::Broken).count(),
                "links": links,
            }))?
        );
        return Ok(0);
    }

    let broken = links
        .iter()
        .filter(|l| l.status == LinkStatus::Broken)
        .count();
    let loops = links
        .iter()
        .filter(|l| l.status == LinkStatus::Loop)
        .count();
    let mut summary = format!(
        "{}  ·  {} symlink{}",
        paint::bold(&tildify(&dir)),
        links.len(),
        if links.len() == 1 { "" } else { "s" }
    );
    if broken > 0 {
        summary.push_str(&format!("  ·  {}", paint::red(&format!("{broken} broken"))));
    }
    if loops > 0 {
        summary.push_str(&format!(
            "  ·  {}",
            paint::yellow(&format!("{loops} looped"))
        ));
    }
    println!();
    println!("  {summary}");
    println!();

    if links.is_empty() {
        println!("  {}", paint::dim("no symlinks here yet"));
        println!();
        return Ok(if broken > 0 { 1 } else { 0 });
    }

    let label_for = |link: &LinkInfo| -> String {
        if args.recursive {
            link.path
                .strip_prefix(&dir)
                .unwrap_or(&link.path)
                .display()
                .to_string()
        } else {
            link.path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| link.path.display().to_string())
        }
    };
    let name_width = links
        .iter()
        .map(|l| label_for(l).len())
        .max()
        .unwrap_or(4)
        .clamp(4, 40);
    let target_width = links
        .iter()
        .map(|l| l.display_target().len())
        .max()
        .unwrap_or(6)
        .clamp(6, 42);

    println!(
        "  {}  {}  {}  {}",
        paint::dim(&pad("NAME", name_width)),
        paint::dim(&pad("TARGET", target_width)),
        paint::dim(&pad("TYPE", 9)),
        paint::dim("STATUS")
    );
    for link in &links {
        let raw_name = label_for(link);
        let name = pad(&truncate(&raw_name, name_width), name_width);
        let target = pad(
            &truncate(&link.display_target(), target_width),
            target_width,
        );
        let name_colored = if link.status.is_ok() {
            paint::bold(&name)
        } else {
            paint::red(&name)
        };
        let type_label = pad(link.kind.label(), 9);
        println!(
            "  {}  {}  {}  {}",
            name_colored,
            paint::cyan(&target),
            paint::dim(&type_label),
            status_paint(link.status)
        );
    }
    println!();
    Ok(if broken + loops > 0 { 1 } else { 0 })
}

// ---------------------------------------------------------------------------
// info
// ---------------------------------------------------------------------------

#[cfg(unix)]
fn mode_string(md: &fs::Metadata) -> String {
    use std::os::unix::fs::PermissionsExt;
    let mode = md.permissions().mode();
    let mut out = String::new();
    for shift in [6, 3, 0] {
        let bits = (mode >> shift) & 0o7;
        out.push(if bits & 4 != 0 { 'r' } else { '-' });
        out.push(if bits & 2 != 0 { 'w' } else { '-' });
        out.push(if bits & 1 != 0 { 'x' } else { '-' });
    }
    out
}

#[cfg(not(unix))]
fn mode_string(_md: &fs::Metadata) -> String {
    "—".to_string()
}

fn cmd_info(args: InfoArgs) -> Result<i32> {
    let path = fsops::absolutize(&args.path)?;
    let link = fsops::inspect(&path)?;
    let chain = link.as_ref().map(|_| fsops::resolve_chain(&path));

    let followed = fs::metadata(&path).ok();
    let link_md = fs::symlink_metadata(&path)
        .with_context(|| format!("no such path: {}", args.path.display()))?;

    if args.json {
        let kind = if link.is_some() {
            "symlink"
        } else if link_md.is_dir() {
            "directory"
        } else {
            "file"
        };
        let payload = json!({
            "path": path,
            "kind": kind,
            "mode": mode_string(followed.as_ref().unwrap_or(&link_md)),
            "size": followed.as_ref().map(|m| m.len()),
            "link": link,
            "chain": chain,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(0);
    }

    let mut rows: Vec<(&str, String)> = Vec::new();
    rows.push(("path", paint::bold(&tildify(&path))));
    match &link {
        Some(info) => {
            let flavor = if info.is_relative() {
                "relative"
            } else {
                "absolute"
            };
            rows.push((
                "target",
                format!(
                    "{}  {}",
                    info.display_target(),
                    paint::dim(&format!("({flavor})"))
                ),
            ));
            rows.push(("resolves", tildify(&info.resolved)));
            rows.push((
                "status",
                format!("{}  ·  {}", status_paint(info.status), info.kind.label()),
            ));
            if let Some(chain) = &chain {
                if chain.hops.len() > 1 {
                    let hops = chain
                        .hops
                        .iter()
                        .map(|h| h.display().to_string())
                        .collect::<Vec<_>>()
                        .join("  →  ");
                    rows.push(("chain", hops));
                }
                if let Some(final_target) = &chain.final_target
                    && chain.hops.len() > 1
                {
                    rows.push(("ends at", tildify(final_target)));
                }
            }
            if let Some(secs) = info.modified {
                rows.push(("changed", format_timestamp(secs)));
            }
        }
        None => {
            let kind = if link_md.is_dir() {
                "directory"
            } else {
                "file"
            };
            rows.push(("type", kind.to_string()));
            rows.push(("note", paint::dim("not a symlink")));
        }
    }
    if let Some(md) = followed.as_ref().or(Some(&link_md)) {
        rows.push(("size", human_size(md.len())));
        rows.push(("mode", mode_string(md)));
        if let Ok(modified) = md.modified() {
            let secs = modified
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .ok();
            if let Some(secs) = secs {
                rows.push((
                    "modified",
                    format!(
                        "{}  {}",
                        format_timestamp(secs),
                        paint::dim(&format!("UTC · {}", ago_label(Some(secs))))
                    ),
                ));
            }
        }
    }

    let width = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(4);
    println!();
    for (key, value) in rows {
        println!("  {}  {}", paint::dim(&pad(key, width)), value);
    }
    println!();
    Ok(0)
}

// ---------------------------------------------------------------------------
// doctor
// ---------------------------------------------------------------------------

fn target_description(info: &LinkInfo) -> String {
    if info.kind == TargetKind::Missing {
        info.display_target()
    } else {
        format!(
            "{}  {}",
            info.display_target(),
            paint::dim(&format!("({})", info.kind.label()))
        )
    }
}

fn cmd_doctor(args: DoctorArgs) -> Result<i32> {
    let dir = fsops::absolutize(&args.dir)?;
    if !dir.is_dir() {
        bail!("not a directory: {}", args.dir.display());
    }
    let started = Instant::now();
    let links = fsops::scan_links(&dir, !args.shallow)?;
    let issues: Vec<&LinkInfo> = links.iter().filter(|l| !l.status.is_ok()).collect();
    let elapsed = started.elapsed();

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "dir": dir,
                "scanned": links.len(),
                "issues": issues,
                "healthy": issues.is_empty(),
                "elapsed_ms": elapsed.as_millis(),
            }))?
        );
        return Ok(if issues.is_empty() { 0 } else { 1 });
    }

    println!();
    println!(
        "  {}  {}",
        paint::bold("linkman doctor"),
        paint::dim(&tildify(&dir))
    );
    println!(
        "  {}",
        paint::dim(&format!(
            "{} symlink{} scanned in {:.1} ms",
            links.len(),
            if links.len() == 1 { "" } else { "s" },
            elapsed.as_secs_f64() * 1000.0
        ))
    );
    println!();

    if issues.is_empty() {
        println!(
            "  {} {}",
            paint::green(paint::TICK),
            paint::green("all links are healthy")
        );
        println!();
        return Ok(0);
    }

    for info in &issues {
        let (icon, label) = match info.status {
            LinkStatus::Broken => (paint::red(paint::CROSS), paint::red("broken")),
            LinkStatus::Loop => (paint::yellow("↻"), paint::yellow("loop  ")),
            LinkStatus::Ok => unreachable!(),
        };
        println!(
            "  {icon} {label}  {}  {}  {}",
            paint::bold(&info.path.display().to_string()),
            paint::dim(paint::ARROW),
            paint::cyan(&target_description(info))
        );
    }
    println!();
    println!(
        "  {}",
        paint::yellow(&format!(
            "{} issue{} found",
            issues.len(),
            if issues.len() == 1 { "" } else { "s" }
        ))
    );
    println!();
    Ok(1)
}
