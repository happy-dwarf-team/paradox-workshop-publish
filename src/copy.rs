//! The published copy: which files of a mod go to Steam, and the temporary
//! folder that holds them (spec section 6).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

use crate::plan::{ModPlan, PlanError};

/// What every published copy leaves out, as the spec writes it.
pub const BUILT_IN_EXCLUDES: [&str; 4] = [".git*", ".github/", "workshop.toml", "*.vdf"];

/// [`BUILT_IN_EXCLUDES`] as globs that match at any depth.
const BUILT_IN_GLOBS: [&str; 4] = ["**/.git*", "**/.github", "**/workshop.toml", "**/*.vdf"];

/// The files of a mod folder after the excludes, sorted by path.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Selection {
    /// `(path, size)`, with `/` separators, relative to the mod root.
    pub files: Vec<(String, u64)>,
    /// Excluded files and folders (folders end with `/`); a folder's
    /// contents are not listed.
    pub excluded: Vec<String>,
}

/// Builds the exclude matcher from the built-in rules and the mod's
/// `exclude` globs. Globs are relative to the mod root, `*` stays within
/// one folder and `**` crosses folders; a glob that matches a folder
/// excludes everything in it.
pub fn excludes(patterns: &[String]) -> Result<GlobSet, PlanError> {
    let mut builder = GlobSetBuilder::new();
    // `docs/**` also matches the folder `docs`, so the folder is left out whole.
    let user = patterns.iter().flat_map(|pattern| {
        let glob = pattern.trim_end_matches('/');
        let folder = glob
            .strip_suffix("/**")
            .map(|folder| (pattern.as_str(), folder));
        std::iter::once((pattern.as_str(), glob)).chain(folder)
    });
    let built_in = BUILT_IN_GLOBS.iter().map(|glob| (*glob, *glob));
    for (pattern, glob) in built_in.chain(user) {
        let glob = GlobBuilder::new(glob)
            .literal_separator(true)
            .build()
            .map_err(|error| PlanError::InvalidGlob {
                pattern: pattern.to_owned(),
                message: error.kind().to_string(),
            })?;
        builder.add(glob);
    }
    builder.build().map_err(|error| PlanError::InvalidGlob {
        pattern: patterns.join(", "),
        message: error.to_string(),
    })
}

/// Walks the mod folder and applies the excludes. Symbolic links are
/// errors, so the copy never reaches outside the mod.
pub fn select(dir: &Path, excludes: &GlobSet) -> Result<Selection, PlanError> {
    let mut selection = Selection::default();
    walk(dir, "", excludes, &mut selection)?;
    Ok(selection)
}

fn walk(
    dir: &Path,
    relative: &str,
    excludes: &GlobSet,
    selection: &mut Selection,
) -> Result<(), PlanError> {
    let io_error = |source| PlanError::Walk {
        path: dir.to_owned(),
        source,
    };
    let mut entries = fs::read_dir(dir)
        .and_then(|entries| entries.collect::<io::Result<Vec<_>>>())
        .map_err(io_error)?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry
            .file_name()
            .into_string()
            .map_err(|name| PlanError::NonUtf8Name(dir.join(name)))?;
        let path = if relative.is_empty() {
            name
        } else {
            format!("{relative}/{name}")
        };
        let file_type = entry.file_type().map_err(io_error)?;
        if file_type.is_symlink() {
            return Err(PlanError::Symlink(path));
        }
        if excludes.is_match(&path) {
            if file_type.is_dir() {
                selection.excluded.push(format!("{path}/"));
            } else {
                selection.excluded.push(path);
            }
        } else if file_type.is_dir() {
            walk(&entry.path(), &path, excludes, selection)?;
        } else {
            let size = entry.metadata().map_err(io_error)?.len();
            selection.files.push((path, size));
        }
    }
    Ok(())
}

/// A published copy in a temporary folder, deleted when this is dropped.
#[derive(Debug)]
pub struct PublishedCopy {
    dir: tempfile::TempDir,
}

impl PublishedCopy {
    /// Copies the plan's files into a new temporary folder.
    pub fn create(plan: &ModPlan) -> Result<PublishedCopy, CopyError> {
        let dir = tempfile::Builder::new()
            .prefix("pdx-ws-")
            .tempdir()
            .map_err(|source| CopyError {
                path: std::env::temp_dir(),
                source,
            })?;
        write(plan, dir.path())?;
        Ok(PublishedCopy { dir })
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }
}

/// Writing the published copy failed.
#[derive(Debug, thiserror::Error)]
#[error("cannot write the published copy at {}: {source}", path.display())]
pub struct CopyError {
    pub path: PathBuf,
    pub source: io::Error,
}

/// Writes the plan's files into `dest`, which should be empty.
pub fn write(plan: &ModPlan, dest: &Path) -> Result<(), CopyError> {
    for file in &plan.files {
        let target = dest.join(&file.path);
        let error = |source| CopyError {
            path: target.clone(),
            source,
        };
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(error)?;
        }
        match &file.contents {
            Some(contents) => fs::write(&target, contents).map_err(error)?,
            None => {
                fs::copy(plan.dir.join(&file.path), &target).map_err(error)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(patterns: &[&str], path: &str) -> bool {
        let patterns: Vec<String> = patterns.iter().map(|p| p.to_string()).collect();
        excludes(&patterns).unwrap().is_match(path)
    }

    #[test]
    fn built_in_excludes_match_at_any_depth() {
        for path in [
            ".git",
            ".gitignore",
            ".github",
            "sub/.gitattributes",
            "workshop.toml",
            "sub/workshop.toml",
            "upload.vdf",
            "a/b/item.vdf",
        ] {
            assert!(matches(&[], path), "{path}");
        }
        for path in [
            "descriptor.mod",
            "common/git.txt",
            "vdf.txt",
            "workshop.toml.bak",
        ] {
            assert!(!matches(&[], path), "{path}");
        }
    }

    #[test]
    fn user_globs_are_relative_to_the_mod_root() {
        assert!(matches(&["docs/**"], "docs/a/b.md"));
        assert!(matches(&["docs/**"], "docs"));
        assert!(matches(&["docs"], "docs"));
        assert!(matches(&["docs/"], "docs"));
        assert!(matches(&["*.psd"], "art.psd"));
        assert!(!matches(&["*.psd"], "gfx/art.psd"));
        assert!(matches(&["**/*.psd"], "gfx/art.psd"));
    }

    #[test]
    fn invalid_globs_are_errors() {
        let error = excludes(&["gfx/[".to_owned()]).unwrap_err();
        assert!(error.to_string().contains("`gfx/[`"), "{error}");
    }
}
