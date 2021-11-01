//! Convenience commands to help the user move through a stack of commits.

use std::fmt::Write;

use cursive::theme::BaseColor;
use cursive::utils::markup::StyledString;
use eden_dag::DagAlgorithm;
use tracing::instrument;

use crate::commands::smartlog::make_smartlog_graph;
use crate::core::config::get_next_interactive;
use crate::core::effects::Effects;
use crate::core::eventlog::{EventLogDb, EventReplayer};
use crate::core::formatting::{printable_styled_string, Pluralize};
use crate::git::{check_out_commit, sort_commit_set, CommitSet, Dag, GitRunInfo, NonZeroOid, Repo};
use crate::tui::prompt_select_commit;

/// Go back a certain number of commits.
#[instrument]
pub fn prev(
    effects: &Effects,
    git_run_info: &GitRunInfo,
    num_commits: Option<isize>,
) -> eyre::Result<isize> {
    let target = match num_commits {
        None => "HEAD^".into(),
        Some(num_commits) => format!("HEAD~{}", num_commits),
    };
    check_out_commit(effects, git_run_info, None, &target)
}

/// Some commits have multiple children, which makes `next` ambiguous. These
/// values disambiguate which child commit to go to, according to the committed
/// date.
#[derive(Clone, Copy, Debug)]
pub enum Towards {
    /// When encountering multiple children, select the newest one.
    Newest,

    /// When encountering multiple children, select the oldest one.
    Oldest,

    /// When encountering multiple children, interactively prompt for
    /// which one to advance to.
    Interactive,
}

#[instrument]
fn advance(
    effects: &Effects,
    repo: &Repo,
    dag: &Dag,
    current_oid: NonZeroOid,
    num_commits: isize,
    towards: Option<Towards>,
) -> eyre::Result<Option<NonZeroOid>> {
    let towards = match towards {
        Some(towards) => Some(towards),
        None => {
            if get_next_interactive(repo)? {
                Some(Towards::Interactive)
            } else {
                None
            }
        }
    };

    let glyphs = effects.get_glyphs();
    let mut current_oid = current_oid;
    for i in 0..num_commits {
        let children = dag
            .query()
            .children(CommitSet::from(current_oid))?
            .difference(&dag.obsolete_commits);
        let children = sort_commit_set(repo, dag, &children)?;

        let children_pluralize = Pluralize {
            amount: i,
            plural: "children",
            singular: "child",
        };
        let header = format!(
            "Found multiple possible next commits to go to after traversing {}:",
            children_pluralize.to_string(),
        );

        current_oid = match (towards, children.as_slice()) {
            (_, []) => {
                writeln!(
                    effects.get_output_stream(),
                    "{}",
                    printable_styled_string(
                        glyphs,
                        StyledString::styled(
                            format!(
                                "No more child commits to go to after traversing {}.",
                                children_pluralize.to_string(),
                            ),
                            BaseColor::Yellow.light()
                        )
                    )?
                )?;
                break;
            }
            (_, [only_child]) => only_child.get_oid(),
            (Some(Towards::Newest), [.., newest_child]) => newest_child.get_oid(),
            (Some(Towards::Oldest), [oldest_child, ..]) => oldest_child.get_oid(),
            (Some(Towards::Interactive), [_, _, ..]) => {
                match prompt_select_commit(children, Some(&header))? {
                    Some(oid) => oid,
                    None => {
                        return Ok(None);
                    }
                }
            }
            (None, [_, _, ..]) => {
                writeln!(effects.get_output_stream(), "{}", header)?;
                for (j, child) in (0..).zip(children.iter()) {
                    let descriptor = if j == 0 {
                        " (oldest)"
                    } else if j + 1 == children.len() {
                        " (newest)"
                    } else {
                        ""
                    };

                    writeln!(
                        effects.get_output_stream(),
                        "  {} {}{}",
                        glyphs.bullet_point,
                        printable_styled_string(glyphs, child.friendly_describe()?)?,
                        descriptor
                    )?;
                }
                writeln!(effects.get_output_stream(), "(Pass --oldest (-o), --newest (-n), or --interactive (-i) to select between ambiguous next commits)")?;
                return Ok(None);
            }
        };
    }
    Ok(Some(current_oid))
}

/// Go forward a certain number of commits.
#[instrument]
pub fn next(
    effects: &Effects,
    git_run_info: &GitRunInfo,
    num_commits: Option<isize>,
    towards: Option<Towards>,
) -> eyre::Result<isize> {
    let repo = Repo::from_current_dir()?;
    let references_snapshot = repo.get_references_snapshot()?;
    let conn = repo.get_db_conn()?;
    let event_log_db = EventLogDb::new(&conn)?;
    let event_replayer = EventReplayer::from_event_log_db(effects, &repo, &event_log_db)?;
    let event_cursor = event_replayer.make_default_cursor();
    let dag = Dag::open_and_sync(
        effects,
        &repo,
        &event_replayer,
        event_cursor,
        &references_snapshot,
    )?;

    let head_oid = match references_snapshot.head_oid {
        Some(head_oid) => head_oid,
        None => {
            eyre::bail!("No HEAD present; cannot calculate next commit");
        }
    };

    let num_commits = num_commits.unwrap_or(1);
    let current_oid = advance(effects, &repo, &dag, head_oid, num_commits, towards)?;
    let current_oid = match current_oid {
        None => return Ok(1),
        Some(current_oid) => current_oid,
    };

    check_out_commit(effects, git_run_info, None, &current_oid.to_string())
}

/// Interactively checkout a commit from the smartlog.
pub fn checkout(effects: &Effects, git_run_info: &GitRunInfo) -> eyre::Result<isize> {
    let repo = Repo::from_current_dir()?;
    let references_snapshot = repo.get_references_snapshot()?;
    let conn = repo.get_db_conn()?;
    let event_log_db = EventLogDb::new(&conn)?;
    let event_replayer = EventReplayer::from_event_log_db(effects, &repo, &event_log_db)?;
    let event_cursor = event_replayer.make_default_cursor();
    let dag = Dag::open_and_sync(
        effects,
        &repo,
        &event_replayer,
        event_cursor,
        &references_snapshot,
    )?;

    let graph = make_smartlog_graph(effects, &repo, &dag, &event_replayer, event_cursor, true)?;

    match prompt_select_commit(graph.get_commits(), None)? {
        Some(oid) => check_out_commit(effects, git_run_info, None, &oid.to_string()),
        None => Ok(1),
    }
}
