//! How an installed skill says which version it is, and that `origin`
//! installed it: two keys `origin skill install` adds to the `metadata` of
//! its `SKILL.md`'s frontmatter, as `gh skill install` adds its `github-*`
//! keys there.
//!
//! - `origin-skills-revision`: the origincloudhq/skills commit the skill
//!   was copied from;
//! - `origin-skills-digest`: the digest of the skill's files as that commit
//!   has them, [`digest`].
//!
//! The keys are added as lines of their own, so that taking them out again
//! ([`strip`]) gives the file as it was, byte for byte: a copy whose files,
//! with that file so restored, have the digest its marker names is one
//! `origin` wrote and no one changed since. Anything else in a skill's
//! directory, a file added or changed or a marker taken out, makes it
//! someone else's, which only `--force` replaces.

use sha2::{Digest, Sha256};

pub(crate) const REVISION_KEY: &str = "origin-skills-revision";
pub(crate) const DIGEST_KEY: &str = "origin-skills-digest";

/// What an installed skill's marker says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Marker {
    pub revision: String,
    pub digest: String,
}

/// The digest of a skill's files, each given by its path in the skill's
/// directory (`/` between its parts) and its content, in any order:
/// `sha256:` and the SHA-256 of every path, its length and its content, in
/// the paths' byte order.
pub(crate) fn digest<'a>(files: impl IntoIterator<Item = (&'a str, &'a [u8])>) -> String {
    let mut files: Vec<(&str, &[u8])> = files.into_iter().collect();
    files.sort();
    let mut hash = Sha256::new();
    for (path, content) in files {
        hash.update(path.as_bytes());
        hash.update([0]);
        hash.update(content.len().to_string().as_bytes());
        hash.update([0]);
        hash.update(content);
    }
    let digest = hash.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

/// A `SKILL.md` that has no frontmatter, or whose frontmatter has no end.
#[derive(Debug, thiserror::Error)]
#[error("SKILL.md has no frontmatter between `---` lines")]
pub(crate) struct NoFrontmatter;

/// The lines of `text`, each with its line break, and the indices of the
/// lines that open and close its frontmatter.
fn frontmatter(text: &str) -> Result<(Vec<&str>, usize, usize), NoFrontmatter> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let fence = |line: &str| line.trim_end_matches(['\n', '\r']) == "---";
    if !lines.first().is_some_and(|l| fence(l)) {
        return Err(NoFrontmatter);
    }
    let end = lines
        .iter()
        .skip(1)
        .position(|l| fence(l))
        .ok_or(NoFrontmatter)?
        + 1;
    Ok((lines, 0, end))
}

/// Whether `line` is one of the marker's.
fn is_marker(line: &str) -> bool {
    line.starts_with([' ', '\t']) && {
        let key = line.trim_start();
        key.starts_with(&format!("{REVISION_KEY}:")) || key.starts_with(&format!("{DIGEST_KEY}:"))
    }
}

/// `text`, a `SKILL.md`, with the marker of `revision` and `digest` in its
/// frontmatter's `metadata`, after the keys it has; in a `metadata` added
/// for it, when it has none.
pub(crate) fn mark(text: &str, revision: &str, digest: &str) -> Result<String, NoFrontmatter> {
    let (lines, _, end) = frontmatter(text)?;
    let newline = if lines[0].ends_with("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let metadata = (1..end).find(|&i| lines[i].trim_end() == "metadata:");
    let (at, indent, header) = match metadata {
        Some(start) => {
            let children: Vec<usize> = (start + 1..end)
                .take_while(|&i| lines[i].starts_with([' ', '\t']) || lines[i].trim().is_empty())
                .collect();
            let last = children
                .iter()
                .rev()
                .find(|&&i| !lines[i].trim().is_empty())
                .copied();
            let indent = last
                .map(|i| {
                    let line = lines[i];
                    line[..line.len() - line.trim_start().len()].to_owned()
                })
                .unwrap_or_else(|| "  ".to_owned());
            (last.unwrap_or(start) + 1, indent, false)
        }
        None => (end, "  ".to_owned(), true),
    };
    let mut out = String::with_capacity(text.len() + 200);
    for line in &lines[..at] {
        out.push_str(line);
    }
    if header {
        out.push_str("metadata:");
        out.push_str(newline);
    }
    out.push_str(&format!("{indent}{REVISION_KEY}: \"{revision}\"{newline}"));
    out.push_str(&format!("{indent}{DIGEST_KEY}: \"{digest}\"{newline}"));
    for line in &lines[at..] {
        out.push_str(line);
    }
    Ok(out)
}

/// The marker in `text`, a `SKILL.md`, if it has one.
pub(crate) fn read(text: &str) -> Option<Marker> {
    let (lines, _, end) = frontmatter(text).ok()?;
    let value = |key: &str| {
        lines[1..end].iter().find_map(|line| {
            let rest = line.trim_start().strip_prefix(key)?.strip_prefix(':')?;
            line.starts_with([' ', '\t'])
                .then(|| rest.trim().trim_matches('"').to_owned())
        })
    };
    Some(Marker {
        revision: value(REVISION_KEY)?,
        digest: value(DIGEST_KEY)?,
    })
}

/// `text` without the marker [`mark`] added: the file as the skill has it.
pub(crate) fn strip(text: &str) -> String {
    let Ok((lines, _, end)) = frontmatter(text) else {
        return text.to_owned();
    };
    let mut kept: Vec<&str> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        if i > 0 && i < end && is_marker(line) {
            continue;
        }
        kept.push(line);
    }
    // The `metadata` the marker came with, which [`mark`] adds last when
    // the frontmatter has none: left with nothing in it.
    let removed = lines.len() - kept.len();
    let end = end - removed;
    if removed > 0 && end > 1 && kept[end - 1].trim_end() == "metadata:" {
        kept.remove(end - 1);
    }
    kept.concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SKILL: &str = "---\nname: demo\ndescription: >-\n  Does things.\nlicense: Apache-2.0\nmetadata:\n  api-revision: \"abc\"\n  cli-version: \"1.18.0\"\n---\n# Demo\n\nmetadata:\n  not: frontmatter\n";

    #[test]
    fn the_marker_goes_after_the_metadata_and_comes_out_again() {
        let marked = mark(SKILL, "r1", "sha256:d1").unwrap();
        assert_eq!(
            marked,
            "---\nname: demo\ndescription: >-\n  Does things.\nlicense: Apache-2.0\nmetadata:\n  api-revision: \"abc\"\n  cli-version: \"1.18.0\"\n  origin-skills-revision: \"r1\"\n  origin-skills-digest: \"sha256:d1\"\n---\n# Demo\n\nmetadata:\n  not: frontmatter\n"
        );
        assert_eq!(
            read(&marked),
            Some(Marker {
                revision: "r1".to_owned(),
                digest: "sha256:d1".to_owned()
            })
        );
        assert_eq!(strip(&marked), SKILL);
        assert_eq!(read(SKILL), None);
        assert_eq!(strip(SKILL), SKILL);
    }

    #[test]
    fn a_skill_without_metadata_gets_one_and_loses_it_again() {
        for skill in [
            "---\nname: demo\ndescription: d\n---\nBody.\n",
            "---\nname: demo\nmetadata:\n  a: \"1\"\ndescription: d\n---\nBody.\n",
            "---\r\nname: demo\r\ndescription: d\r\n---\r\nBody.\r\n",
        ] {
            let marked = mark(skill, "r", "sha256:x").unwrap();
            assert!(read(&marked).is_some(), "{marked}");
            assert_eq!(strip(&marked), skill);
        }
        assert_eq!(
            mark("---\nname: demo\n---\n", "r", "d").unwrap(),
            "---\nname: demo\nmetadata:\n  origin-skills-revision: \"r\"\n  origin-skills-digest: \"d\"\n---\n"
        );
    }

    #[test]
    fn a_file_without_frontmatter_has_no_marker() {
        assert!(mark("# Demo\n", "r", "d").is_err());
        assert!(mark("---\nname: demo\n", "r", "d").is_err());
        assert_eq!(read("# Demo\n"), None);
    }

    #[test]
    fn the_digest_is_of_the_paths_and_contents_in_any_order() {
        let a = digest([("SKILL.md", &b"x"[..]), ("references/a.md", &b"y"[..])]);
        let b = digest([("references/a.md", &b"y"[..]), ("SKILL.md", &b"x"[..])]);
        assert_eq!(a, b);
        assert!(a.starts_with("sha256:") && a.len() == 7 + 64);
        assert_ne!(
            a,
            digest([("SKILL.md", &b"x"[..]), ("references/b.md", &b"y"[..])])
        );
        // A boundary moved between a path and its content changes it.
        assert_ne!(digest([("ab", &b"c"[..])]), digest([("a", &b"bc"[..])]));
    }
}
