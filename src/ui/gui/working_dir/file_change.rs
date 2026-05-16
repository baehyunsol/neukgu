use super::{IcedContext, Popup};
use crate::Error;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct FileChange {
    pub path: String,
    pub diff: String,
    pub expanded: bool,
}

impl IcedContext {
    pub fn update_file_changes(&mut self) -> Result<(), Error> {
        match &self.curr_popup {
            Some(Popup::FileChanges(changes)) => {
                let expanded: HashMap<String, bool> = changes.iter().map(|c| (c.path.to_string(), c.expanded)).collect();
                let changed_files = self.fe_context.get_changed_files()?;
                let mut changes = Vec::with_capacity(changed_files.len());

                for (file, original_content) in changed_files.iter() {
                    let diff = self.fe_context.get_file_change(file, original_content)?;

                    if let Some(diff) = diff {
                        changes.push(FileChange {
                            path: file.to_string(),
                            diff,
                            expanded: expanded.get(file).cloned().unwrap_or(false),
                        });
                    }
                }

                self.curr_popup = Some(Popup::FileChanges(changes));
                Ok(())
            },
            _ => Ok(()),
        }
    }
}
