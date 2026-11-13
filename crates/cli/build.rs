//! Embeds the agent skills `origin skill install` installs: the copy of
//! origincloudhq/skills at the commit SKILLS_REVISION names, which
//! scripts/generate.sh keeps in skills/, one directory per skill. It writes
//! `skills.rs` into OUT_DIR, every file of every skill with its path under
//! skills/, in byte order, and the revision; `handler/skill/embedded.rs`
//! includes it.

use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let skills = manifest.join("skills");
    let revision_file = manifest.join("../../SKILLS_REVISION");
    println!("cargo:rerun-if-changed={}", skills.display());
    println!("cargo:rerun-if-changed={}", revision_file.display());

    let revision = std::fs::read_to_string(&revision_file)
        .expect("SKILLS_REVISION is at the repository's root")
        .trim()
        .to_owned();
    assert!(
        revision.len() == 40 && revision.bytes().all(|b| b.is_ascii_hexdigit()),
        "SKILLS_REVISION must hold a full commit hash"
    );

    let mut files = Vec::new();
    walk(&skills, &skills, &mut files);
    files.sort();
    assert!(
        !files.is_empty(),
        "skills/ holds no skill: run scripts/generate.sh"
    );

    let mut out = format!(
        "/// The origincloudhq/skills commit the skills are copied from.\n\
         pub(crate) const REVISION: &str = {revision:?};\n\n\
         /// Every file of every skill, by its path under skills/ (`origin/SKILL.md`).\n\
         pub(crate) const FILES: &[(&str, &[u8])] = &[\n"
    );
    for (relative, path) in &files {
        out.push_str(&format!(
            "    ({relative:?}, include_bytes!({:?})),\n",
            path.display().to_string()
        ));
    }
    out.push_str("];\n");
    let generated =
        PathBuf::from(std::env::var_os("OUT_DIR").expect("set by cargo")).join("skills.rs");
    std::fs::write(generated, out).expect("writing skills.rs");
}

/// Every regular file under `dir`, by its path relative to `root` with `/`
/// between its parts. Anything else fails the build: scripts/generate.sh
/// copies regular files only.
fn walk(root: &Path, dir: &Path, files: &mut Vec<(String, PathBuf)>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
    {
        let path = entry.expect("a directory entry").path();
        let kind = std::fs::symlink_metadata(&path)
            .expect("its metadata")
            .file_type();
        if kind.is_dir() {
            walk(root, &path, files);
        } else if kind.is_file() {
            let relative = path
                .strip_prefix(root)
                .expect("under the root")
                .components()
                .map(|c| c.as_os_str().to_str().expect("a skill's paths are UTF-8"))
                .collect::<Vec<_>>()
                .join("/");
            files.push((relative, path));
        } else {
            panic!("{} isn't a regular file or a directory", path.display());
        }
    }
}
