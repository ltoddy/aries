use std::fs;
use std::path::{Component, Path};

use git2::{Error, ErrorCode, Index, Repository, Status, StatusOptions};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum IndexAction {
    Stage,
    Unstage,
}

pub(super) fn update_index(cwd: &Path, path: &Path, action: IndexAction) -> Result<(), Error> {
    if path.as_os_str().is_empty()
        || path.components().any(|component| !matches!(component, Component::Normal(_)))
        || path.as_os_str().as_encoded_bytes().ends_with(b"/")
    {
        return Err(Error::from_str("expected a repository-relative file path"));
    }

    let repository = Repository::discover(cwd)?;
    let workdir =
        repository.workdir().ok_or_else(|| Error::from_str("repository has no workdir"))?;
    match fs::symlink_metadata(workdir.join(path)) {
        Ok(metadata) if metadata.is_dir() => {
            return Err(Error::from_str("directories cannot be staged or unstaged"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(Error::from_str(&error.to_string())),
    }

    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);
    let statuses = repository.statuses(Some(&mut options))?;
    let mut matches = statuses.iter().filter(|entry| {
        [entry.head_to_index(), entry.index_to_workdir()].into_iter().flatten().any(|delta| {
            delta.old_file().path() == Some(path) || delta.new_file().path() == Some(path)
        })
    });
    let entry = matches.next().ok_or_else(|| Error::from_str("file has no current changes"))?;
    if matches.next().is_some() {
        return Err(Error::from_str("file has ambiguous rename status"));
    }
    let status = entry.status();
    if status.is_conflicted() {
        return Err(Error::from_str("conflicted files cannot be staged or unstaged"));
    }

    let mut paths = vec![path.to_path_buf()];
    let required = match action {
        IndexAction::Stage => {
            if status.is_wt_renamed()
                && let Some(delta) = entry.index_to_workdir()
            {
                paths = [delta.old_file().path(), delta.new_file().path()]
                    .into_iter()
                    .flatten()
                    .map(Path::to_path_buf)
                    .collect();
            }
            Status::WT_NEW
                | Status::WT_MODIFIED
                | Status::WT_DELETED
                | Status::WT_RENAMED
                | Status::WT_TYPECHANGE
        }
        IndexAction::Unstage => {
            if status.is_index_renamed()
                && let Some(delta) = entry.head_to_index()
            {
                paths = [delta.old_file().path(), delta.new_file().path()]
                    .into_iter()
                    .flatten()
                    .map(Path::to_path_buf)
                    .collect();
            }
            Status::INDEX_NEW
                | Status::INDEX_MODIFIED
                | Status::INDEX_DELETED
                | Status::INDEX_RENAMED
                | Status::INDEX_TYPECHANGE
        }
    };
    if !status.intersects(required) {
        return Err(Error::from_str("file has no changes for the requested index action"));
    }

    let mut index = repository.index()?;
    index.read(true)?;
    for target in &paths {
        if (1..=3).any(|stage| index.get_path(target, stage).is_some()) {
            return Err(Error::from_str("conflicted files cannot be staged or unstaged"));
        }
    }
    match action {
        IndexAction::Stage => {
            for target in &paths {
                match fs::symlink_metadata(workdir.join(target)) {
                    Ok(metadata) if metadata.is_dir() => {
                        return Err(Error::from_str("directories cannot be staged"));
                    }
                    Ok(_) => index.add_path(target)?,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        if index.get_path(target, 0).is_some() {
                            index.remove_path(target)?;
                        }
                    }
                    Err(error) => return Err(Error::from_str(&error.to_string())),
                }
            }
        }
        IndexAction::Unstage => {
            let mut head_index = Index::new()?;
            match repository.head() {
                Ok(head) => head_index.read_tree(&head.peel_to_tree()?)?,
                Err(error) if error.code() == ErrorCode::UnbornBranch => {}
                Err(error) => return Err(error),
            }
            for target in &paths {
                if let Some(entry) = head_index.get_path(target, 0) {
                    index.add(&entry)?;
                } else if index.get_path(target, 0).is_some() {
                    index.remove_path(target)?;
                }
            }
        }
    }
    index.write()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use git2::{Oid, Repository, RepositoryInitOptions, Signature, Status};

    use crate::chat::view::git_index::{IndexAction, update_index};

    static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    struct TestRepository {
        repository: Repository,
        path: PathBuf,
    }

    impl TestRepository {
        fn new() -> Self {
            let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("aries-orbit-git-index-{}-{id}", std::process::id()));
            fs::create_dir(&path).expect("test directory should be created");
            let mut options = RepositoryInitOptions::new();
            options.initial_head("main");
            let repository =
                Repository::init_opts(&path, &options).expect("repository should be initialized");
            Self { repository, path }
        }

        fn write(&self, path: &str, content: &str) {
            let target = self.path.join(path);
            fs::create_dir_all(target.parent().expect("file should have a parent"))
                .expect("parent should exist");
            fs::write(target, content).expect("file should be written");
        }

        fn update(&self, path: &str, action: IndexAction) {
            update_index(&self.path, Path::new(path), action).expect("index action should succeed");
        }

        fn commit(&self) {
            let mut index = self.repository.index().expect("index should exist");
            index.read(true).expect("index should be refreshed");
            let tree_id = index.write_tree().expect("tree should be written");
            let tree = self.repository.find_tree(tree_id).expect("tree should exist");
            let signature =
                Signature::now("Test", "test@example.com").expect("signature should be valid");
            self.repository
                .commit(Some("HEAD"), &signature, &signature, "initial", &tree, &[])
                .expect("commit should be created");
        }

        fn tracked(&self, path: &str, content: &str) {
            self.write(path, content);
            self.update(path, IndexAction::Stage);
        }

        fn indexed(&self, path: &str) -> Option<Oid> {
            let mut index = self.repository.index().expect("index should exist");
            index.read(true).expect("index should be refreshed");
            index.get_path(Path::new(path), 0).map(|entry| entry.id)
        }

        fn indexed_content(&self, path: &str) -> Vec<u8> {
            let id = self.indexed(path).expect("file should be indexed");
            self.repository.find_blob(id).expect("blob should exist").content().to_vec()
        }

        fn status(&self, path: &str) -> Status {
            self.repository
                .index()
                .expect("index should exist")
                .read(true)
                .expect("index should be refreshed");
            self.repository.status_file(Path::new(path)).expect("status should be available")
        }

        fn snapshot(&self) -> Vec<u8> {
            fs::read(self.repository.path().join("index")).expect("index should be readable")
        }
    }

    impl Drop for TestRepository {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn stage_unstage_cycle_preserves_worktree() {
        let repo = TestRepository::new();
        repo.tracked("file", "original");
        repo.commit();
        repo.write("file", "modified");
        for _ in 0..2 {
            repo.update("file", IndexAction::Stage);
            assert_eq!(repo.status("file"), Status::INDEX_MODIFIED);
            assert_eq!(repo.indexed_content("file"), b"modified");
            repo.update("file", IndexAction::Unstage);
            assert_eq!(repo.status("file"), Status::WT_MODIFIED);
            assert_eq!(repo.indexed_content("file"), b"original");
            assert_eq!(fs::read(repo.path.join("file")).expect("file should exist"), b"modified");
        }
    }

    #[test]
    fn partial_staging_uses_latest_worktree_and_head() {
        let repo = TestRepository::new();
        repo.tracked("file", "head");
        repo.commit();
        repo.write("file", "staged");
        repo.update("file", IndexAction::Stage);
        repo.write("file", "latest");
        assert_eq!(repo.status("file"), Status::INDEX_MODIFIED | Status::WT_MODIFIED);
        repo.update("file", IndexAction::Unstage);
        assert_eq!(repo.indexed_content("file"), b"head");
        assert_eq!(fs::read(repo.path.join("file")).expect("file should exist"), b"latest");
        repo.update("file", IndexAction::Stage);
        assert_eq!(repo.indexed_content("file"), b"latest");
        repo.write("file", "newest");
        repo.update("file", IndexAction::Stage);
        assert_eq!(repo.indexed_content("file"), b"newest");
    }

    #[test]
    fn unborn_unstage_removes_only_selected_entry() {
        let repo = TestRepository::new();
        repo.tracked("file", "new");
        repo.tracked("other", "other");
        let other = repo.indexed("other");
        repo.write("file", "latest");
        repo.update("file", IndexAction::Unstage);
        assert_eq!(repo.indexed("file"), None);
        assert_eq!(repo.indexed("other"), other);
        assert_eq!(fs::read(repo.path.join("file")).expect("file should exist"), b"latest");
        repo.update("file", IndexAction::Stage);
        fs::remove_file(repo.path.join("file")).expect("file should be removed");
        repo.update("file", IndexAction::Unstage);
        assert_eq!(repo.indexed("file"), None);
        assert!(!repo.path.join("file").exists());
    }

    #[test]
    fn additions_and_deletions_do_not_modify_worktree() {
        let repo = TestRepository::new();
        repo.tracked("deleted", "original");
        repo.commit();
        repo.write("new", "new content");
        repo.update("new", IndexAction::Stage);
        repo.update("new", IndexAction::Unstage);
        assert_eq!(repo.indexed("new"), None);
        assert_eq!(repo.status("new"), Status::WT_NEW);
        assert_eq!(fs::read(repo.path.join("new")).expect("file should exist"), b"new content");
        fs::remove_file(repo.path.join("deleted")).expect("file should be removed");
        repo.update("deleted", IndexAction::Stage);
        assert_eq!(repo.indexed("deleted"), None);
        assert_eq!(repo.status("deleted"), Status::INDEX_DELETED);
        repo.update("deleted", IndexAction::Unstage);
        assert_eq!(repo.indexed_content("deleted"), b"original");
        assert_eq!(repo.status("deleted"), Status::WT_DELETED);
        assert!(!repo.path.join("deleted").exists());
    }

    #[test]
    fn worktree_rename_can_be_staged() {
        let repo = TestRepository::new();
        repo.tracked("old", "rename content\n");
        repo.commit();
        fs::rename(repo.path.join("old"), repo.path.join("new")).expect("file should be renamed");
        repo.update("new", IndexAction::Stage);
        if repo.indexed("old").is_some() {
            repo.update("old", IndexAction::Stage);
        }
        assert_eq!(repo.indexed("old"), None);
        assert_eq!(repo.indexed_content("new"), b"rename content\n");
        repo.update("new", IndexAction::Unstage);
        assert_eq!(repo.indexed_content("old"), b"rename content\n");
        assert_eq!(repo.indexed("new"), None);
    }

    #[test]
    fn staged_rename_unstages_both_paths() {
        for selected in ["old", "new"] {
            let repo = TestRepository::new();
            repo.tracked("old", "rename content\n");
            repo.tracked("other", "original other");
            repo.commit();
            repo.write("other", "staged other");
            repo.update("other", IndexAction::Stage);
            let other = repo.indexed("other");
            fs::rename(repo.path.join("old"), repo.path.join("new"))
                .expect("file should be renamed");
            let mut index = repo.repository.index().expect("index should exist");
            index.remove_path(Path::new("old")).expect("old path should be removed");
            index.add_path(Path::new("new")).expect("new path should be staged");
            index.write().expect("index should be written");
            let mut options = git2::StatusOptions::new();
            options.renames_head_to_index(true);
            assert!(
                repo.repository
                    .statuses(Some(&mut options))
                    .expect("status should exist")
                    .iter()
                    .any(|entry| entry.status().is_index_renamed())
            );
            repo.write("new", "latest rename content\n");
            repo.update(selected, IndexAction::Unstage);
            assert_eq!(repo.indexed_content("old"), b"rename content\n");
            assert_eq!(repo.indexed("new"), None);
            assert_eq!(repo.indexed("other"), other);
            assert!(!repo.path.join("old").exists());
            assert_eq!(
                fs::read(repo.path.join("new")).expect("new path should exist"),
                b"latest rename content\n"
            );
        }
    }

    #[test]
    fn preserves_unrelated_partial_staging() {
        let repo = TestRepository::new();
        repo.tracked("file", "head");
        repo.tracked("other", "other head");
        repo.commit();
        repo.write("other", "other staged");
        repo.update("other", IndexAction::Stage);
        repo.write("other", "other worktree");
        let other = repo.indexed("other");
        repo.write("file", "modified");
        repo.update("file", IndexAction::Stage);
        repo.update("file", IndexAction::Unstage);
        assert_eq!(repo.indexed("other"), other);
        assert_eq!(repo.indexed_content("other"), b"other staged");
        assert_eq!(repo.status("other"), Status::INDEX_MODIFIED | Status::WT_MODIFIED);
    }

    #[test]
    fn wildcard_names_are_literal() {
        let repo = TestRepository::new();
        for path in ["*.txt", "[ab]?.txt", "a.txt", "b1.txt"] {
            repo.tracked(path, path);
        }
        repo.commit();
        for path in ["*.txt", "[ab]?.txt", "a.txt", "b1.txt"] {
            repo.write(path, "modified");
        }
        for path in ["*.txt", "[ab]?.txt"] {
            repo.update(path, IndexAction::Stage);
            assert_eq!(repo.status(path), Status::INDEX_MODIFIED);
            assert_eq!(repo.status("a.txt"), Status::WT_MODIFIED);
            assert_eq!(repo.status("b1.txt"), Status::WT_MODIFIED);
            repo.update(path, IndexAction::Unstage);
            assert_eq!(repo.indexed_content(path), path.as_bytes());
        }
    }

    #[test]
    fn discovers_from_subdirectory_but_paths_are_root_relative() {
        let repo = TestRepository::new();
        repo.write("nested/file", "nested");
        repo.write("file", "root");
        let cwd = repo.path.join("nested");
        update_index(&cwd, Path::new("nested/file"), IndexAction::Stage)
            .expect("nested file should be staged");
        update_index(&cwd, Path::new("file"), IndexAction::Stage)
            .expect("root file should be staged");
        assert_eq!(repo.indexed_content("file"), b"root");
        assert_eq!(repo.indexed_content("nested/file"), b"nested");
        update_index(&cwd, Path::new("nested/file"), IndexAction::Unstage)
            .expect("nested file should be unstaged");
        assert_eq!(repo.indexed("nested/file"), None);
        assert_eq!(repo.indexed_content("file"), b"root");
    }

    #[test]
    fn invalid_and_stale_paths_leave_index_unchanged() {
        let repo = TestRepository::new();
        repo.tracked("clean", "clean");
        repo.commit();
        repo.write("nested/file", "new");
        repo.write(".gitignore", "ignored\n");
        repo.write("ignored", "ignored");
        let snapshot = repo.snapshot();
        let absolute = repo.path.join("clean");
        for path in [
            Path::new(""),
            Path::new("."),
            Path::new(".."),
            Path::new("../clean"),
            Path::new("nested/../clean"),
            Path::new("nested"),
            Path::new("nested/"),
            Path::new("missing"),
            Path::new("clean"),
            Path::new("ignored"),
            &absolute,
        ] {
            for action in [IndexAction::Stage, IndexAction::Unstage] {
                assert!(update_index(&repo.path, path, action).is_err(), "{path:?} {action:?}");
                assert_eq!(repo.snapshot(), snapshot, "{path:?} {action:?}");
            }
        }
        assert!(update_index(&repo.path, Path::new("nested/file"), IndexAction::Unstage).is_err());
        repo.update("nested/file", IndexAction::Stage);
        let snapshot = repo.snapshot();
        assert!(update_index(&repo.path, Path::new("nested/file"), IndexAction::Stage).is_err());
        assert_eq!(repo.snapshot(), snapshot);
        assert!(
            update_index(
                &repo.repository.path().join("missing"),
                Path::new("file"),
                IndexAction::Stage
            )
            .is_err()
        );
    }

    #[test]
    fn conflicts_are_rejected_without_touching_index_or_worktree() {
        let repo = TestRepository::new();
        repo.tracked("conflict", "base");
        repo.tracked("other", "other");
        repo.commit();
        repo.write("conflict", "conflicted worktree");
        let mut index = repo.repository.index().expect("index should exist");
        let mut entry = index.get_path(Path::new("conflict"), 0).expect("entry should exist");
        index.remove_path(Path::new("conflict")).expect("entry should be removed");
        for stage in 1..=3 {
            entry.flags = (entry.flags & !0x3000) | (stage << 12);
            index.add(&entry).expect("conflict entry should be added");
        }
        index.write().expect("index should be written");
        assert!(repo.status("conflict").is_conflicted());
        let snapshot = repo.snapshot();
        for action in [IndexAction::Stage, IndexAction::Unstage] {
            assert!(update_index(&repo.path, Path::new("conflict"), action).is_err());
            assert_eq!(repo.snapshot(), snapshot);
            assert_eq!(
                fs::read(repo.path.join("conflict")).expect("file should exist"),
                b"conflicted worktree"
            );
        }
        repo.write("other", "modified other");
        repo.update("other", IndexAction::Stage);
        assert!(repo.status("conflict").is_conflicted());
    }
}
