use super::{
    LineDiff,
    QuestionKind,
    QuestionToUser,
    ToolCall,
    ToolCallError,
    ToolCallSuccess,
    UserAnswer,
    ask_question_to_user,
};
use crate::{Config, Context, Error, InterruptId};
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum WriteContent {
    String(String),
    Diff(Vec<LineDiff>),
    Output,  // of a command/program
}

pub async fn ask_permission_to_user(tool: &ToolCall, context: &mut Context, config: &Config) -> Result<Result<(), ToolCallError>, Error> {
    let mut to_write = vec![];
    let mut to_run = vec![];

    match tool {
        ToolCall::Read { .. } => {},
        ToolCall::Write { path, content, .. } => {
            to_write.push((path.to_vec(), WriteContent::String(content.to_string())));
        },
        ToolCall::Patch { path, diff } => {
            to_write.push((path.to_vec(), WriteContent::Diff(diff.to_vec())));
        },
        ToolCall::Run { command, stdout, stderr, .. } => {
            if let Some(stdout) = stdout {
                to_write.push((stdout.to_vec(), WriteContent::Output));
            }

            if let Some(stderr) = stderr {
                to_write.push((stderr.to_vec(), WriteContent::Output));
            }

            to_run.push(command.to_vec());
        },
        ToolCall::Ask { .. } => {},
        ToolCall::Chrome { output, .. } => {
            to_write.push((output.to_vec(), WriteContent::Output));
        },
        ToolCall::ImageEdit { output, .. } => {
            to_write.push((output.to_vec(), WriteContent::Output));
        },
    }

    for (path, content) in to_write.into_iter() {
        match config.write_permission {
            PermissionConfig::Allow => break,
            PermissionConfig::Deny => {
                return Ok(Err(ToolCallError::WritePermissionDeniedByUser { path: path.join("/"), not_responding: false }));
            },
            PermissionConfig::Ask => {},
        }

        let interrupt_id = InterruptId::new();
        let question = QuestionToUser {
            question: String::from("Will you allow me to write a file?"),
            kind: QuestionKind::WritePermission { path: path.join("/"), content },
        };

        let (permission, not_responding) = match ask_question_to_user(interrupt_id, &question, context, config).await? {
            Ok(ToolCallSuccess::Ask { answer: UserAnswer::Permission(p), .. }) => (p, false),
            Err(ToolCallError::UserNotResponding) => (Permission::Deny, true),
            Err(ToolCallError::UserRejectedToRespond) => (Permission::Deny, false),
            _ => unreachable!(),
        };

        if let Permission::AlwaysAllow = permission {
            let mut new_config = config.clone();
            new_config.write_permission = PermissionConfig::Allow;
            context.new_config = Some(new_config);
        }

        if let Permission::AlwaysDeny = permission {
            let mut new_config = config.clone();
            new_config.write_permission = PermissionConfig::Deny;
            context.new_config = Some(new_config);
        }

        if let Permission::Deny | Permission::AlwaysDeny = permission {
            return Ok(Err(ToolCallError::WritePermissionDeniedByUser { path: path.join("/"), not_responding }));
        }
    }

    for command in to_run.into_iter() {
        let binary = &command[0];
        let permission = config.run_permission.get(binary).unwrap_or(&PermissionConfig::Ask);

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
            new_config.run_permission.insert(binary.to_string(), PermissionConfig::Allow);
            context.new_config = Some(new_config);
        }

        if let Permission::AlwaysDeny = permission {
            let mut new_config = config.clone();
            new_config.run_permission.insert(binary.to_string(), PermissionConfig::Deny);
            context.new_config = Some(new_config);
        }

        if let Permission::Deny | Permission::AlwaysDeny = permission {
            return Ok(Err(ToolCallError::RunPermissionDeniedByUser { command, not_responding }));
        }
    }

    Ok(Ok(()))
}
