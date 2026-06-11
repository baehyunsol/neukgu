use super::{
    LineDiff,
    QuestionKind,
    QuestionToUser,
    ToolCall,
    ToolCallError,
    ToolCallSuccess,
    UserAnswer,
    ask_question_to_user,
    normalize_path,
};
use crate::{Config, Context, Error, InterruptId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Permission {
    Allow,
    AlwaysAllow,
    Deny,
    AlwaysDeny,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum PermissionConfig {
    Allow,
    Deny,
    Ask,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ToolPermissionKind {
    Read,
    ReadExt,
    Write,
    WriteExt,
    Remove,
    RemoveExt,
}

impl ToolPermissionKind {
    pub fn all() -> Vec<ToolPermissionKind> {
        vec![
            ToolPermissionKind::Read,
            ToolPermissionKind::ReadExt,
            ToolPermissionKind::Write,
            ToolPermissionKind::WriteExt,
            ToolPermissionKind::Remove,
            ToolPermissionKind::RemoveExt,
        ]
    }

    pub fn question(&self) -> &'static str {
        match self {
            ToolPermissionKind::Read => "Will you allow me to read a file?",
            ToolPermissionKind::ReadExt => "Will you allow me to read an external file?",
            ToolPermissionKind::Write => "Will you allow me to write to a file?",
            ToolPermissionKind::WriteExt => "Will you allow me to write to an external file?",
            ToolPermissionKind::Remove => "Will you allow me to remove a file?",
            ToolPermissionKind::RemoveExt => "Will you allow me to remove an external file?",
        }
    }

    // format!("a permission to {} `{path}`", self.describe()) must make sense
    pub fn describe(&self) -> &'static str {
        match self {
            ToolPermissionKind::Read => "read a file",
            ToolPermissionKind::ReadExt => "read an external file",
            ToolPermissionKind::Write => "write to a file",
            ToolPermissionKind::WriteExt => "write to an external file",
            ToolPermissionKind::Remove => "remove a file",
            ToolPermissionKind::RemoveExt => "remove an external file",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            ToolPermissionKind::Read => "read-file",
            ToolPermissionKind::ReadExt => "read-ext-file",
            ToolPermissionKind::Write => "write-file",
            ToolPermissionKind::WriteExt => "write-ext-file",
            ToolPermissionKind::Remove => "remove-file",
            ToolPermissionKind::RemoveExt => "remove-ext-file",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum PermissionPreview {
    String(String),
    Diff(Vec<LineDiff>),
    None,
}

pub fn default_tool_permissions() -> HashMap<ToolPermissionKind, PermissionConfig> {
    [
        (ToolPermissionKind::Read, PermissionConfig::Allow),
        (ToolPermissionKind::ReadExt, PermissionConfig::Ask),
        (ToolPermissionKind::Write, PermissionConfig::Ask),
        (ToolPermissionKind::WriteExt, PermissionConfig::Ask),
        (ToolPermissionKind::Remove, PermissionConfig::Ask),
        (ToolPermissionKind::RemoveExt, PermissionConfig::Ask),
    ].into_iter().collect()
}

pub async fn ask_permission_to_user(tool: &ToolCall, context: &mut Context, config: &Config) -> Result<Result<(), ToolCallError>, Error> {
    let mut tools = vec![];
    let mut runs = vec![];

    match tool {
        ToolCall::Read { path, .. } => {
            tools.push((ToolPermissionKind::Read, path.to_string(), PermissionPreview::None));
        },
        ToolCall::Write { path, content, .. } => {
            tools.push((ToolPermissionKind::Write, path.to_string(), PermissionPreview::String(content.to_string())));
        },
        ToolCall::Patch { path, diff } => {
            tools.push((ToolPermissionKind::Write, path.to_string(), PermissionPreview::Diff(diff.to_vec())));
        },
        ToolCall::Remove { path } => {
            tools.push((ToolPermissionKind::Remove, path.to_string(), PermissionPreview::None));
        },
        ToolCall::Run { command, path, stdout, stderr, .. } => {
            if let Some(path) = path {
                tools.push((ToolPermissionKind::Read, path.to_string(), PermissionPreview::None));
            }

            if let Some(stdout) = stdout {
                tools.push((ToolPermissionKind::Write, stdout.to_string(), PermissionPreview::None));
            }

            if let Some(stderr) = stderr {
                tools.push((ToolPermissionKind::Write, stderr.to_string(), PermissionPreview::None));
            }

            runs.push(command.to_vec());
        },
        ToolCall::Ask { .. } => {},
        ToolCall::Chrome { input, output, .. } => {
            tools.push((ToolPermissionKind::Read, input.to_string(), PermissionPreview::None));
            tools.push((ToolPermissionKind::Write, output.to_string(), PermissionPreview::None));
        },
        ToolCall::ImageEdit { input, output, .. } => {
            tools.push((ToolPermissionKind::Read, input.to_string(), PermissionPreview::None));
            tools.push((ToolPermissionKind::Write, output.to_string(), PermissionPreview::None));
        },
    }

    for (permission_kind, path, preview) in tools.into_iter() {
        let path = match normalize_path(&path, &context.working_dir) {
            Some(path) => path,

            // If it's an invalid path, we don't have to check permission because harness will raise an error.
            None => continue,
        };

        let permission_kind = match (permission_kind, path.relative.is_none()) {
            (ToolPermissionKind::Read, true) => ToolPermissionKind::ReadExt,
            (ToolPermissionKind::Write, true) => ToolPermissionKind::WriteExt,
            (ToolPermissionKind::Remove, true) => ToolPermissionKind::RemoveExt,
            (p, _) => p,
        };
        let permission = config.tool_permissions.get(&permission_kind).unwrap_or(&PermissionConfig::Ask);

        match permission {
            PermissionConfig::Allow => break,
            PermissionConfig::Deny => {
                return Ok(Err(ToolCallError::ToolPermissionDeniedByUser { kind: permission_kind, path: path.clone(), not_responding: false }));
            },
            PermissionConfig::Ask => {},
        }

        let interrupt_id = InterruptId::new();
        let question = QuestionToUser {
            question: permission_kind.question().to_string(),
            kind: QuestionKind::ToolPermission { kind: permission_kind, path: path.to_string(), preview },
        };

        let (permission, not_responding) = match ask_question_to_user(interrupt_id, &question, context, config).await? {
            Ok(ToolCallSuccess::Ask { answer: UserAnswer::Permission(p), .. }) => (p, false),
            Err(ToolCallError::UserNotResponding) => (Permission::Deny, true),
            Err(ToolCallError::UserRejectedToRespond) => (Permission::Deny, false),
            _ => unreachable!(),
        };

        if let Permission::AlwaysAllow = permission {
            let mut new_config = config.clone();
            new_config.tool_permissions.insert(permission_kind, PermissionConfig::Allow);
            context.new_config = Some(new_config);
        }

        if let Permission::AlwaysDeny = permission {
            let mut new_config = config.clone();
            new_config.tool_permissions.insert(permission_kind, PermissionConfig::Deny);
            context.new_config = Some(new_config);
        }

        if let Permission::Deny | Permission::AlwaysDeny = permission {
            return Ok(Err(ToolCallError::ToolPermissionDeniedByUser { kind: permission_kind, path: path.clone(), not_responding }));
        }
    }

    for command in runs.into_iter() {
        let binary = &command[0];
        let permission = config.run_permissions.get(binary).unwrap_or(&PermissionConfig::Ask);

        match permission {
            PermissionConfig::Allow => continue,
            PermissionConfig::Deny => {
                return Ok(Err(ToolCallError::RunPermissionDeniedByUser { command, not_responding: false }));
            },
            PermissionConfig::Ask => {},
        }

        let interrupt_id = InterruptId::new();
        let question = QuestionToUser {
            question: String::from("Will you allow me to run a command?"),
            kind: QuestionKind::RunPermission { command: command.to_vec() },
        };

        let (permission, not_responding) = match ask_question_to_user(interrupt_id, &question, context, config).await? {
            Ok(ToolCallSuccess::Ask { answer: UserAnswer::Permission(p), .. }) => (p, false),
            Err(ToolCallError::UserNotResponding) => (Permission::Deny, true),
            Err(ToolCallError::UserRejectedToRespond) => (Permission::Deny, false),
            _ => unreachable!(),
        };

        if let Permission::AlwaysAllow = permission {
            let mut new_config = config.clone();
            new_config.run_permissions.insert(binary.to_string(), PermissionConfig::Allow);
            context.new_config = Some(new_config);
        }

        if let Permission::AlwaysDeny = permission {
            let mut new_config = config.clone();
            new_config.run_permissions.insert(binary.to_string(), PermissionConfig::Deny);
            context.new_config = Some(new_config);
        }

        if let Permission::Deny | Permission::AlwaysDeny = permission {
            return Ok(Err(ToolCallError::RunPermissionDeniedByUser { command, not_responding }));
        }
    }

    Ok(Ok(()))
}
