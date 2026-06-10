// https://agentskills.io/specification

use crate::{Error, get_global_config, save_global_config};
use ragit_fs::{
    WriteMode,
    basename,
    create_dir,
    exists,
    is_dir,
    join,
    join3,
    read_bytes,
    read_dir,
    write_string,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use yaml_rust::{Yaml, YamlLoader};

#[derive(Clone, Debug)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub body: String,
}

impl Skill {
    pub fn try_init(
        name: String,
        description: String,
        license: Option<String>,
        compatibility: Option<String>,
        body: String,
    ) -> Result<Skill, SkillSchemaError> {
        let skill = Skill {
            name,
            description,
            license,
            compatibility,
            body,
        };
        skill.validate()?;
        Ok(skill)
    }

    pub fn save(&self, global_index_dir: &str) -> Result<(), Error> {
        let skill_dir = join3(global_index_dir, "skills", &self.name)?;

        if !exists(&skill_dir) {
            create_dir(&skill_dir)?;
        }

        write_string(
            &join(&skill_dir, "SKILL.md")?,
            &self.to_markdown(),
            WriteMode::CreateOrTruncate,
        )?;
        Ok(())
    }

    // This function cannot return file-io errors (instead it panics).
    // So, before calling this function, you must make sure that `dir`
    // exists, and it's a directory.
    pub fn load(dir: &str) -> Result<Skill, SkillSchemaError> {
        let dir_name = basename(dir).unwrap();
        let skill_md = join(dir, "SKILL.md").unwrap();

        if !exists(&skill_md) || is_dir(&skill_md) {
            return Err(SkillSchemaError::SkillDotMdNotFound);
        }

        // The specification doesn't tell me that it should be valid utf-8.
        let skill_md = String::from_utf8_lossy(&read_bytes(&skill_md).unwrap()).to_string();
        let skill = Skill::parse_skill_md(&skill_md)?;

        if skill.name != dir_name {
            return Err(SkillSchemaError::DirNameDifferent {
                dir: dir_name,
                frontmatter: skill.name,
            });
        }

        Ok(skill)
    }

    pub fn parse_skill_md(md: &str) -> Result<Skill, SkillSchemaError> {
        #[derive(Clone, Copy, Debug)]
        enum ParseState {
            Init,
            Frontmatter,
            Markdown,
        }

        let mut parse_state = ParseState::Init;
        let mut frontmatter_lines = vec![];
        let mut markdown_lines = vec![];

        for line in md.lines() {
            match parse_state {
                ParseState::Init => {
                    if line == "---" {
                        parse_state = ParseState::Frontmatter;
                    }

                    else if !line.is_empty() {
                        return Err(SkillSchemaError::CannotParseFrontmatter);
                    }
                },
                ParseState::Frontmatter => {
                    if line == "---" {
                        parse_state = ParseState::Markdown;
                    }

                    else {
                        frontmatter_lines.push(line.to_string());
                    }
                },
                ParseState::Markdown => {
                    markdown_lines.push(line.to_string());
                },
            }
        }

        let frontmatter = frontmatter_lines.join("\n");
        let frontmatter = match YamlLoader::load_from_str(&frontmatter) {
            Ok(docs) => match docs.get(0) {
                Some(Yaml::Hash(frontmatter)) => frontmatter.clone(),
                _ => return Err(SkillSchemaError::CannotParseFrontmatter),
            },
            _ => return Err(SkillSchemaError::CannotParseFrontmatter),
        };

        let name = match frontmatter.get(&Yaml::String(String::from("name"))) {
            Some(Yaml::String(name)) => name.to_string(),
            _ => return Err(SkillSchemaError::FrontmatterNotFound { field: String::from("name") }),
        };
        let description = match frontmatter.get(&Yaml::String(String::from("description"))) {
            Some(Yaml::String(description)) => description.to_string(),
            _ => return Err(SkillSchemaError::FrontmatterNotFound { field: String::from("description") }),
        };
        let license = match frontmatter.get(&Yaml::String(String::from("license"))) {
            Some(Yaml::String(license)) => Some(license.to_string()),
            _ => None,
        };
        let compatibility = match frontmatter.get(&Yaml::String(String::from("compatibility"))) {
            Some(Yaml::String(compatibility)) => Some(compatibility.to_string()),
            _ => None,
        };

        let body = markdown_lines.join("\n");
        let skill = Skill {
            name,
            description,
            license,
            compatibility,
            body,
        };
        skill.validate()?;
        Ok(skill)
    }

    pub fn validate(&self) -> Result<(), SkillSchemaError> {
        if self.name.is_empty() {
            return Err(SkillSchemaError::NameTooShort);
        }

        if self.name.chars().count() > 64 {
            return Err(SkillSchemaError::NameTooLong(self.name.chars().count()));
        }

        for ch in self.name.chars() {
            match ch {
                'a'..='z' | '0'..='9' | '-' => {},
                _ => {
                    return Err(SkillSchemaError::InvalidCharacter(ch));
                },
            }
        }

        if self.name.starts_with("-") {
            return Err(SkillSchemaError::CannotStartWithHyphen);
        }

        if self.name.ends_with("-") {
            return Err(SkillSchemaError::CannotEndWithHyphen);
        }

        if self.name.contains("--") {
            return Err(SkillSchemaError::CannotContainConsecutiveHyphens);
        }

        if self.description.is_empty() {
            return Err(SkillSchemaError::DescriptionTooShort);
        }

        if self.description.chars().count() > 1024 {
            return Err(SkillSchemaError::DescriptionTooLong(self.description.chars().count()));
        }

        Ok(())
    }

    pub fn to_config(&self, enabled: bool) -> SkillConfig {
        SkillConfig {
            name: self.name.to_string(),
            enabled,
            description: self.description.to_string(),
        }
    }

    pub fn to_markdown(&self) -> String {
        let frontmatter = format!(
            "name: {}\ndescription: {}{}{}",
            self.name,
            self.description,
            if let Some(license) = &self.license { format!("\nlicense: {license}") } else { String::new() },
            if let Some(compatibility) = &self.compatibility { format!("\ncompatibility: {compatibility}") } else { String::new() },
        );
        format!("---\n{frontmatter}\n---\n{}", self.body)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SkillConfig {
    pub name: String,
    pub enabled: bool,
    pub description: String,
}

pub fn init_default_skills(global_index_dir: &str) -> Result<(), Error> {
    let skills_dir = join(global_index_dir, "skills")?;
    let mut global_config = get_global_config(global_index_dir)?;

    if !exists(&skills_dir) {
        create_dir(&skills_dir)?;
    }

    let server_skill_at = join(&skills_dir, "server-development")?;
    if !exists(&server_skill_at) {
        create_dir(&server_skill_at)?;
        write_string(
            &join(&server_skill_at, "SKILL.md")?,
            include_str!("../default-skills/server-development/SKILL.md"),
            WriteMode::CreateOrTruncate,
        )?;
        global_config.add_skill(Skill::load(&server_skill_at).unwrap());
    }

    save_global_config(&global_config, global_index_dir)?;
    Ok(())
}

pub fn load_global_skills(global_index_dir: &str) -> Result<Vec<(String, Result<Skill, SkillSchemaError>)>, Error> {
    let skills_dir = join(global_index_dir, "skills")?;
    let mut result = vec![];

    for entry in read_dir(&skills_dir, true)? {
        let name = basename(&entry)?;

        if !is_dir(&entry) {
            result.push((name, Err(SkillSchemaError::SkillDotMdNotFound)));
            continue;
        }

        result.push((name, Skill::load(&entry)));
    }

    Ok(result)
}

#[derive(Clone, Debug)]
pub enum SkillSchemaError {
    CannotParseFrontmatter,
    FrontmatterNotFound { field: String },
    NameTooShort,
    NameTooLong(usize),
    InvalidCharacter(char),
    CannotStartWithHyphen,
    CannotEndWithHyphen,
    CannotContainConsecutiveHyphens,
    DescriptionTooShort,
    DescriptionTooLong(usize),
    SkillDotMdNotFound,
    DirNameDifferent {
        dir: String,
        frontmatter: String,
    },
}

impl fmt::Display for SkillSchemaError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            SkillSchemaError::CannotParseFrontmatter => write!(fmt, "Cannot parse the frontmatter."),
            SkillSchemaError::FrontmatterNotFound { field } => write!(fmt, "Field `{field}` is not found in the frontmatter."),
            SkillSchemaError::NameTooShort => write!(fmt, "The skill's name is too short."),
            SkillSchemaError::NameTooLong(n) => write!(fmt, "The skill's name is too long. It has to be shorter than 65 characters, but it's {n} characters."),
            SkillSchemaError::InvalidCharacter(ch) => write!(fmt, "You cannot use character {ch:?} in a name of a skill."),
            SkillSchemaError::CannotStartWithHyphen => write!(fmt, "A skill name cannot start with a hyphen ('-')."),
            SkillSchemaError::CannotEndWithHyphen => write!(fmt, "A skill name cannot end with a hyphen ('-')."),
            SkillSchemaError::CannotContainConsecutiveHyphens => write!(fmt, "A skill name cannot contain consecutive hyphens (\"--\")."),
            SkillSchemaError::DescriptionTooShort => write!(fmt, "The skill's description is too short."),
            SkillSchemaError::DescriptionTooLong(n) => write!(fmt, "The skill's description is too long. It has to be shorter than 1025 characters, but it's {n} characters."),
            SkillSchemaError::SkillDotMdNotFound => write!(fmt, "SKILL.md is not found."),
            SkillSchemaError::DirNameDifferent { dir, frontmatter } => write!(fmt, "The directory's name is `{dir}`, but the skill name in the frontmatter is `{frontmatter}`. They have to be the same."),
        }
    }
}
