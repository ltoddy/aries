use std::collections::{HashSet, VecDeque};

use gpui_kit::*;

use crate::workspace::workspace_view::{FileConfirmation, PendingFileOperation};

pub struct FileQueue {
    pub pending: VecDeque<PendingFileOperation>,
    pub confirmation: Option<FileConfirmation>,
    pub focus: Option<FocusHandle>,
    pub discarded: HashSet<usize>,
    pub wait: Option<Task<()>>,
}

impl FileQueue {
    pub fn new() -> Self {
        Self {
            pending: VecDeque::new(),
            confirmation: None,
            focus: None,
            discarded: HashSet::new(),
            wait: None,
        }
    }
}
