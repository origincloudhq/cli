//! Help text: the proto comments, in the words of a command line.
//!
//! - the API linter's directives, `(-- ... --)`, are removed;
//! - a request field in backticks, such as `size_gib`, becomes the flag that
//!   sets it, `--size-gib`;
//! - any other field in backticks becomes its name in the JSON the command
//!   prints, such as `nextPageToken`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::LazyLock;

use heck::ToLowerCamelCase;
use prost_types::FileDescriptorSet;
use regex::Regex;

static LINTER_DIRECTIVE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)\(--.*?--\)").expect("a valid pattern"));
static BACKTICKED_FIELD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"`([a-z][a-z0-9_]*)`").expect("a valid pattern"));

/// The leading comments of the API's files, by file and location path.
pub struct Comments {
    leading: HashMap<(String, Vec<i32>), String>,
}

impl Comments {
    pub fn new(fds: &FileDescriptorSet) -> Self {
        let mut leading = HashMap::new();
        for file in &fds.file {
            let Some(info) = &file.source_code_info else {
                continue;
            };
            for location in &info.location {
                if let Some(text) = &location.leading_comments {
                    leading.insert(
                        (file.name().to_owned(), location.path.clone()),
                        text.clone(),
                    );
                }
            }
        }
        Self { leading }
    }

    /// The leading comment at `path` in `file`; empty when there is none.
    pub fn leading(&self, file: &str, path: &[i32]) -> &str {
        self.leading
            .get(&(file.to_owned(), path.to_vec()))
            .map_or("", String::as_str)
    }
}

/// How a command's help names fields: its flags, and the JSON names of the
/// package's other fields.
pub struct Vocabulary<'a> {
    /// The command's flags, by the request field each sets.
    pub flags: &'a BTreeMap<String, String>,
    /// Every field name of the package.
    pub fields: &'a BTreeSet<String>,
}

/// `comment` as help text: without linter directives and trailing blank
/// lines, its fields named as `vocabulary` says.
pub fn help(comment: &str, vocabulary: &Vocabulary<'_>) -> String {
    let text = LINTER_DIRECTIVE.replace_all(comment, "");
    let text = BACKTICKED_FIELD.replace_all(&text, |caps: &regex::Captures<'_>| {
        let field = &caps[1];
        if let Some(flag) = vocabulary.flags.get(field) {
            format!("`--{flag}`")
        } else if vocabulary.fields.contains(field) {
            format!("`{}`", field.to_lower_camel_case())
        } else {
            caps[0].to_owned()
        }
    });
    let mut lines: Vec<&str> = text
        .lines()
        .map(|l| l.strip_prefix(' ').unwrap_or(l))
        .collect();
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    while lines.first().is_some_and(|l| l.trim().is_empty()) {
        lines.remove(0);
    }
    lines
        .iter()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// `comment` as one line of plain text, without linter directives: the
/// help of a nested message's key, whose backticked names are its sibling
/// keys, not flags.
pub fn plain(comment: &str) -> String {
    LINTER_DIRECTIVE
        .replace_all(comment, "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The first sentence of `text`, on one line and without its full stop:
/// what a command's line in its group's help says.
pub fn summary(text: &str) -> String {
    let paragraph = text.split("\n\n").next().unwrap_or_default();
    let joined = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
    let first = match joined.find(". ") {
        Some(end) => &joined[..end],
        None => joined.as_str(),
    };
    first.trim_end_matches('.').to_owned()
}

/// `text` as `#[doc]` attributes, one per line.
pub fn attributes(text: &str, indent: &str) -> String {
    text.lines()
        .map(|line| {
            if line.is_empty() {
                format!("{indent}#[doc = \"\"]\n")
            } else {
                format!("{indent}#[doc = {:?}]\n", format!(" {line}"))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_summary_is_the_first_sentence() {
        assert_eq!(
            summary("Lists the volumes in a zone, oldest first. Deleted volumes\naren't listed."),
            "Lists the volumes in a zone, oldest first"
        );
        assert_eq!(summary("Returns a volume."), "Returns a volume");
    }

    #[test]
    fn fields_become_flags_or_json_names() {
        let flags = BTreeMap::from([("size_gib".to_owned(), "size-gib".to_owned())]);
        let fields = BTreeSet::from(["size_gib".to_owned(), "next_page_token".to_owned()]);
        let vocabulary = Vocabulary {
            flags: &flags,
            fields: &fields,
        };
        assert_eq!(
            help(
                " A larger `size_gib` grows it; see `next_page_token` and `AVAILABLE`.\n\n (-- api-linter: x\n    y --)\n",
                &vocabulary
            ),
            "A larger `--size-gib` grows it; see `nextPageToken` and `AVAILABLE`."
        );
    }
}
