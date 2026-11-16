//! The agent skills this `origin` carries: every skill of origincloudhq/skills
//! at the commit SKILLS_REVISION names, copied into crates/cli/skills/ by
//! scripts/generate.sh and embedded by the build script, so that installing
//! one needs no network access.

use super::marker;

include!(concat!(env!("OUT_DIR"), "/skills.rs"));

/// The skill `origin skill install` installs when it is given none.
pub(crate) const DEFAULT: &str = "origin";

/// One skill: its name, which is its directory's, and its files.
#[derive(Debug, Clone)]
pub(crate) struct Skill {
    pub name: &'static str,
    /// Its files by their paths in its directory (`SKILL.md`,
    /// `references/pvs.md`), in byte order.
    pub files: Vec<(&'static str, &'static [u8])>,
}

impl Skill {
    /// What identifies this version of the skill: the digest of its files.
    pub fn digest(&self) -> String {
        marker::digest(self.files.iter().map(|(path, content)| (*path, *content)))
    }
}

/// Every skill, by name.
pub(crate) fn all() -> Vec<Skill> {
    let mut skills: Vec<Skill> = Vec::new();
    for (path, content) in FILES {
        let (name, file) = path
            .split_once('/')
            .expect("a skill's file is in its directory");
        match skills.last_mut() {
            Some(skill) if skill.name == name => skill.files.push((file, content)),
            _ => skills.push(Skill {
                name,
                files: vec![(file, content)],
            }),
        }
    }
    skills
}

/// The skill `name`.
pub(crate) fn skill(name: &str) -> Option<Skill> {
    all().into_iter().find(|s| s.name == name)
}

/// The skills' names, the values a command takes.
pub(crate) fn names() -> Vec<&'static str> {
    all().into_iter().map(|s| s.name).collect()
}

/// The short form of a revision, as the output shows it.
pub(crate) fn short(revision: &str) -> &str {
    &revision[..revision.len().min(12)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_skill_has_its_skill_md_and_no_evals() {
        let skills = all();
        assert!(skills.iter().any(|s| s.name == DEFAULT));
        for skill in &skills {
            assert!(
                skill.files.iter().any(|(p, _)| *p == "SKILL.md"),
                "{}",
                skill.name
            );
            assert!(
                !skill.files.iter().any(|(p, _)| p.starts_with("evals/")),
                "{}",
                skill.name
            );
            let text = std::str::from_utf8(
                skill
                    .files
                    .iter()
                    .find(|(p, _)| *p == "SKILL.md")
                    .unwrap()
                    .1,
            )
            .unwrap();
            assert!(
                text.contains(&format!("\nname: {}\n", skill.name)),
                "{}",
                skill.name
            );
        }
        assert_eq!(
            REVISION,
            include_str!("../../../../../SKILLS_REVISION").trim()
        );
    }
}
