use super::events::{Event, EventProvider};
use chrono::prelude::*;
use git2::{Commit, Repository};

// git2 revwalk
// https://github.com/rust-lang/git2-rs/blob/master/examples/log.rs

pub struct Git {
    pub repo_folder: String, // Path
    pub commit_author: String,
}

impl Git {
    fn git2_time_to_datetime(time: git2::Time) -> DateTime<Local> {
        Utc.timestamp(time.seconds(), 0).with_timezone(&Local)
    }

    fn get_commit_diff<'a>(repo: &'a Repository, c: &Commit) -> Option<git2::Diff<'a>> {
        if c.parent_count() > 1 {
            return None;
        }
        let commit_tree = c.tree().ok()?;
        let parent = c.parent(0).ok()?;
        let parent_tree = parent.tree().ok()?;
        repo.diff_tree_to_tree(Some(&parent_tree), Some(&commit_tree), None)
            .ok()
    }

    fn get_commit_full_diffstr(diff: &git2::Diff) -> Option<String> {
        let stats = diff.stats().ok()?;
        stats
            .to_buf(git2::DiffStatsFormat::FULL, 1024)
            .ok()
            .and_then(|b| b.as_str().map(|s| s.to_string()))
    }

    fn get_commit_extra_info<'a>(diff: &git2::Diff<'a>) -> Option<String> {
        // not done here. i want to get the list of files and copy the
        // getcommitExtraInfo algo from the cigale haskell version.
        let mut file_cb = |diff_delta: git2::DiffDelta<'_>, count| {
            println!("delta {:?}, count {}", diff_delta.new_file().path(), count);
            true
        };
        diff.foreach(&mut file_cb, None, None, None).ok()?;
        None
    }
}

impl EventProvider for Git {
    fn get_desc(&self) -> &'static str {
        "Git"
    }

    fn get_icon(&self) -> &'static str {
        "code-branch"
    }

    fn get_events(&self, day: &Date<Local>) -> Result<Vec<Event>, Box<dyn std::error::Error>> {
        let day_start = day.and_hms(0, 0, 0);
        let next_day_start = day_start + chrono::Duration::days(1);
        let repo = Repository::open(&self.repo_folder)?;
        let mut revwalk = repo.revwalk()?;
        revwalk.set_sorting(/*git2::Sort::REVERSE |*/ git2::Sort::TIME);
        revwalk.push_head()?;
        let mut commits: Vec<Commit> = revwalk
            .map(|r| {
                let oid = r?;
                repo.find_commit(oid)
            })
            .filter_map(|c| match c {
                Ok(commit) => Some(commit),
                Err(e) => {
                    println!("Error walking the revisions {}, skipping", e);
                    None
                }
            })
            .take_while(|c| {
                let commit_date = Git::git2_time_to_datetime(c.time());
                commit_date >= day_start
            })
            .filter(|c| {
                let commit_date = Git::git2_time_to_datetime(c.time());
                // TODO move to option.contains when it stabilizes https://github.com/rust-lang/rust/issues/62358
                commit_date < next_day_start && c.author().name() == Some(&self.commit_author)
            })
            .collect();
        commits.reverse();
        Ok(commits
            .iter()
            .map(|c| {
                let commit_date = Git::git2_time_to_datetime(c.time());
                let diff = Git::get_commit_diff(&repo, &c);
                let contents_base =
                    "<big><b>".to_owned() + &c.message().unwrap_or("").to_string() + "</b></big>";
                let (contents, extra_details) = match diff {
                    None => (contents_base, None),
                    Some(d) => (
                        contents_base
                            + "\n<span font-family=\"monospace\">"
                            + &Git::get_commit_full_diffstr(&d).unwrap_or("".to_string())
                            + "</span>",
                        Git::get_commit_extra_info(&d),
                    ),
                };
                Event::new(
                    self.get_desc(),
                    self.get_icon(),
                    commit_date.time(),
                    c.summary().unwrap_or("").to_string(),
                    contents,
                    extra_details,
                )
            })
            .collect())
    }
}
