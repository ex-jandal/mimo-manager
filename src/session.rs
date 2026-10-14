//! Editing session: applies association edits and tracks undo/redo history.
//!
//! History is snapshot-based: before every write the current contents of the
//! single writable file are captured, so undo is exact regardless of which
//! edit produced the state. Snapshots are held in memory for the session only,
//! matching the "session write-history" behaviour of the interface.

use std::fs;
use std::path::PathBuf;

use crate::assoc::{self, AssocEdit};
use crate::error::{Error, Result};
use crate::xdg::Paths;

/// In-memory state of the session's writes.
pub struct Session {
    paths: Paths,
    target: PathBuf,
    undo: Vec<Option<String>>,
    redo: Vec<Option<String>>,
}

impl Session {
    pub fn new(paths: Paths) -> Self {
        let target = assoc::write_target(&paths);
        Self {
            paths,
            target,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// The only file this session will ever write.
    pub fn target(&self) -> &PathBuf {
        &self.target
    }

    /// Apply an edit, recording the previous file contents for undo.
    pub fn apply(&mut self, edit: &AssocEdit) -> Result<()> {
        let previous = self.snapshot()?;
        assoc::apply(&self.paths, edit)?;
        self.undo.push(previous);
        self.redo.clear();
        Ok(())
    }

    /// Apply several edits as a single undoable step.
    pub fn apply_batch(&mut self, edits: &[AssocEdit]) -> Result<()> {
        if edits.is_empty() {
            return Ok(());
        }
        let previous = self.snapshot()?;
        assoc::apply_all(&self.paths, edits)?;
        self.undo.push(previous);
        self.redo.clear();
        Ok(())
    }

    /// Undo the most recent write. Returns whether anything changed.
    pub fn undo(&mut self) -> Result<bool> {
        let Some(previous) = self.undo.pop() else {
            return Ok(false);
        };
        self.redo.push(self.snapshot()?);
        self.restore(previous)?;
        Ok(true)
    }

    /// Redo the most recently undone write. Returns whether anything changed.
    pub fn redo(&mut self) -> Result<bool> {
        let Some(next) = self.redo.pop() else {
            return Ok(false);
        };
        self.undo.push(self.snapshot()?);
        self.restore(next)?;
        Ok(true)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Number of writes that can be undone.
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    fn snapshot(&self) -> Result<Option<String>> {
        match fs::read_to_string(&self.target) {
            Ok(text) => Ok(Some(text)),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(Error::Io {
                path: self.target.clone(),
                source,
            }),
        }
    }

    fn restore(&self, snapshot: Option<String>) -> Result<()> {
        match snapshot {
            Some(text) => assoc::write_file_atomic(&self.target, &text),
            None => match fs::remove_file(&self.target) {
                Ok(()) => Ok(()),
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(source) => Err(Error::Io {
                    path: self.target.clone(),
                    source,
                }),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> (tempfile::TempDir, Session) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::rooted(dir.path().join("home"));
        (dir, Session::new(paths))
    }

    #[test]
    fn apply_creates_then_undo_removes_then_redo_restores() {
        let (_tmp, mut session) = session();
        let edit = AssocEdit::SetDefault {
            mime: "text/plain".into(),
            app: "editor.desktop".into(),
        };

        session.apply(&edit).unwrap();
        assert!(
            fs::read_to_string(session.target())
                .unwrap()
                .contains("text/plain=editor.desktop;")
        );
        assert!(session.can_undo());
        assert!(!session.can_redo());

        assert!(session.undo().unwrap());
        assert!(
            !session.target().exists(),
            "undo of the first write removes the file"
        );
        assert!(session.can_redo());

        assert!(session.redo().unwrap());
        assert!(
            fs::read_to_string(session.target())
                .unwrap()
                .contains("text/plain=editor.desktop;")
        );
    }

    #[test]
    fn undo_without_history_is_a_noop() {
        let (_tmp, mut session) = session();
        assert!(!session.undo().unwrap());
        assert!(!session.redo().unwrap());
    }

    #[test]
    fn batch_applies_once_and_undoes_once() {
        let (_tmp, mut session) = session();
        let edits = [
            AssocEdit::SetDefault {
                mime: "audio/mpeg".into(),
                app: "a.desktop".into(),
            },
            AssocEdit::SetDefault {
                mime: "audio/x-wav".into(),
                app: "a.desktop".into(),
            },
        ];
        session.apply_batch(&edits).unwrap();
        assert_eq!(session.undo_depth(), 1, "a batch is one undo step");

        let text = fs::read_to_string(session.target()).unwrap();
        assert!(text.contains("audio/mpeg=a.desktop;"));
        assert!(text.contains("audio/x-wav=a.desktop;"));

        session.undo().unwrap();
        assert!(
            !session.target().exists(),
            "one undo reverts the whole batch"
        );
    }

    #[test]
    fn new_write_clears_the_redo_stack() {
        let (_tmp, mut session) = session();
        session
            .apply(&AssocEdit::SetDefault {
                mime: "text/plain".into(),
                app: "a.desktop".into(),
            })
            .unwrap();
        session.undo().unwrap();
        assert!(session.can_redo());

        session
            .apply(&AssocEdit::SetDefault {
                mime: "text/plain".into(),
                app: "b.desktop".into(),
            })
            .unwrap();
        assert!(!session.can_redo());
        assert_eq!(session.undo_depth(), 1);
    }
}
