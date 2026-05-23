use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

const SOURCE_SYNC_LEDGER: &str = "policy/source-sync.toml";
const SOURCE_MAIN_REF: &str = "refs/unsafe-review-sync/source-main";
const SWARM_MAIN_REF: &str = "refs/unsafe-review-sync/swarm-main";

struct SourceSyncCheckpoint {
    source_repo: String,
    swarm_repo: String,
    acknowledged_source_main: String,
    acknowledged_by: String,
}

pub(crate) fn report_source_divergence() -> Result<(), String> {
    let checkpoint = source_sync_checkpoint()?;
    fetch_main_ref(&checkpoint.source_repo, SOURCE_MAIN_REF)?;
    fetch_main_ref(&checkpoint.swarm_repo, SWARM_MAIN_REF)?;

    let counts = git_stdout([
        "rev-list",
        "--left-right",
        "--count",
        &format!("{SOURCE_MAIN_REF}...{SWARM_MAIN_REF}"),
    ])?;
    let (source_only, swarm_only) = parse_rev_list_counts(&counts)?;
    let new_source_commits = git_stdout([
        "rev-list",
        "--count",
        &format!("{}..{SOURCE_MAIN_REF}", checkpoint.acknowledged_source_main),
    ])?
    .parse::<usize>()
    .map_err(|err| format!("invalid source checkpoint count: {err}"))?;
    let source_head = git_stdout(["rev-parse", "--short", SOURCE_MAIN_REF])?;
    let swarm_head = git_stdout(["rev-parse", "--short", SWARM_MAIN_REF])?;
    let source_commits = git_stdout([
        "log",
        "--oneline",
        "--max-count=10",
        SOURCE_MAIN_REF,
        "--not",
        &checkpoint.acknowledged_source_main,
    ])?;
    let swarm_commits = git_stdout([
        "log",
        "--oneline",
        "--max-count=10",
        SWARM_MAIN_REF,
        "--not",
        SOURCE_MAIN_REF,
    ])?;

    println!("source-divergence: advisory");
    println!("source_repo={}", checkpoint.source_repo);
    println!("swarm_repo={}", checkpoint.swarm_repo);
    println!("source_main={source_head}");
    println!("swarm_main={swarm_head}");
    println!(
        "acknowledged_source_main={}",
        checkpoint.acknowledged_source_main
    );
    println!("acknowledged_by={}", checkpoint.acknowledged_by);
    println!("new_source_commits={new_source_commits}");
    println!("raw_source_only={source_only}");
    println!("raw_swarm_only={swarm_only}");

    if new_source_commits == 0 {
        println!("status: no source commits after the acknowledged swarm sync point");
    } else {
        println!(
            "status: source is ahead of swarm; open a swarm sync PR before routine development"
        );
    }
    if swarm_only > 0 {
        println!(
            "note: swarm has work not present in source; this is expected for unpromoted workbench changes"
        );
    }

    print_commit_section("new_source_commits", &source_commits);
    print_commit_section("swarm_only_commits", &swarm_commits);
    Ok(())
}

fn source_sync_checkpoint() -> Result<SourceSyncCheckpoint, String> {
    let value = super::parse_toml_file(Path::new(SOURCE_SYNC_LEDGER))?;
    super::require_toml_string(&value, "schema_version", SOURCE_SYNC_LEDGER)?;
    super::require_toml_string(&value, "policy", SOURCE_SYNC_LEDGER)?;
    let source_repo =
        super::required_toml_string(&value, "source_repo", SOURCE_SYNC_LEDGER)?.to_string();
    let swarm_repo =
        super::required_toml_string(&value, "swarm_repo", SOURCE_SYNC_LEDGER)?.to_string();
    let acknowledged_source_main =
        super::required_toml_string(&value, "acknowledged_source_main", SOURCE_SYNC_LEDGER)?
            .to_string();
    let acknowledged_by =
        super::required_toml_string(&value, "acknowledged_by", SOURCE_SYNC_LEDGER)?.to_string();
    super::require_file(&acknowledged_by)?;
    Ok(SourceSyncCheckpoint {
        source_repo,
        swarm_repo,
        acknowledged_source_main,
        acknowledged_by,
    })
}

fn fetch_main_ref(repo_url: &str, target_ref: &str) -> Result<(), String> {
    let refspec = format!("+refs/heads/main:{target_ref}");
    let output = Command::new("git")
        .args(["fetch", "--no-tags", "--quiet", repo_url, &refspec])
        .output()
        .map_err(|err| format!("failed to run git fetch for {repo_url}: {err}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "git fetch {repo_url} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn git_stdout<I, S>(args: I) -> Result<String, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|err| format!("failed to run git: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(crate) fn parse_rev_list_counts(text: &str) -> Result<(usize, usize), String> {
    let mut fields = text.split_whitespace();
    let Some(left) = fields.next() else {
        return Err("git rev-list count output is empty".to_string());
    };
    let Some(right) = fields.next() else {
        return Err(format!(
            "git rev-list count output must contain two counts: {text}"
        ));
    };
    if fields.next().is_some() {
        return Err(format!(
            "git rev-list count output must contain only two counts: {text}"
        ));
    }
    Ok((
        parse_rev_list_count(left, "source-only")?,
        parse_rev_list_count(right, "swarm-only")?,
    ))
}

fn parse_rev_list_count(text: &str, label: &str) -> Result<usize, String> {
    text.parse::<usize>()
        .map_err(|err| format!("invalid {label} count `{text}`: {err}"))
}

fn print_commit_section(label: &str, commits: &str) {
    println!("{label}:");
    if commits.trim().is_empty() {
        println!("  none");
        return;
    }
    for line in commits.lines() {
        println!("  {line}");
    }
}
