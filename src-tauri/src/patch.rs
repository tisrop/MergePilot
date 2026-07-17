use crate::models::{FileStatus, PatchContentKind, PatchHunk, PatchLine, PatchLineKind, PrFile, StandardPatchFile};

pub const PATCH_SCHEMA_VERSION: u32 = 1;
const MAX_STANDARD_PATCH_BYTES: usize = 4 * 1024 * 1024;

pub(crate) fn metadata_only_rename_patch(old_path: &str, new_path: &str) -> String {
    format!(
        "diff --git a/{old_path} b/{new_path}\nsimilarity index 100%\nrename from {old_path}\nrename to {new_path}\n"
    )
}

#[derive(Debug, Clone, Copy)]
struct HunkRange {
    start: u32,
    count: u32,
}

pub fn standardize_patches(diff: &str, files: &[PrFile]) -> Vec<StandardPatchFile> {
    let sections = split_file_patches(diff);
    files
        .iter()
        .map(|file| {
            let patch = sections
                .iter()
                .find(|section| section_matches_file(section, &file.filename))
                .copied()
                .unwrap_or(&file.patch);
            standardize_file_patch(file, patch)
        })
        .collect()
}

fn standardize_file_patch(file: &PrFile, patch: &str) -> StandardPatchFile {
    if patch.len() > MAX_STANDARD_PATCH_BYTES {
        return unavailable_patch(file, "文件 patch 超过 4 MiB，未生成结构化内容");
    }

    if patch.trim().is_empty() {
        return unavailable_patch(file, "平台未返回该文件的文本 patch");
    }

    let normalized_patch = normalize_patch(file, patch);
    let (old_path, new_path) = patch_paths(&normalized_patch, file);

    if is_binary_patch(&normalized_patch) {
        return StandardPatchFile {
            filename: file.filename.clone(),
            old_path,
            new_path,
            status: file.status.clone(),
            additions: file.additions,
            deletions: file.deletions,
            content_kind: PatchContentKind::Binary,
            patch: normalized_patch,
            hunks: Vec::new(),
            message: Some("二进制文件不提供文本 Diff".to_string()),
        };
    }

    match parse_hunks(&normalized_patch) {
        Ok(hunks) if !hunks.is_empty() => StandardPatchFile {
            filename: file.filename.clone(),
            old_path,
            new_path,
            status: file.status.clone(),
            additions: file.additions,
            deletions: file.deletions,
            content_kind: PatchContentKind::Text,
            patch: normalized_patch,
            hunks,
            message: None,
        },
        Ok(_) => StandardPatchFile {
            filename: file.filename.clone(),
            old_path,
            new_path,
            status: file.status.clone(),
            additions: file.additions,
            deletions: file.deletions,
            content_kind: PatchContentKind::MetadataOnly,
            patch: normalized_patch,
            hunks: Vec::new(),
            message: Some("该文件仅包含重命名、权限或其他元数据变更".to_string()),
        },
        Err(message) => StandardPatchFile {
            filename: file.filename.clone(),
            old_path,
            new_path,
            status: file.status.clone(),
            additions: file.additions,
            deletions: file.deletions,
            content_kind: PatchContentKind::Unavailable,
            patch: normalized_patch,
            hunks: Vec::new(),
            message: Some(message),
        },
    }
}

fn unavailable_patch(file: &PrFile, message: &str) -> StandardPatchFile {
    let (old_path, new_path) = fallback_paths(file);
    StandardPatchFile {
        filename: file.filename.clone(),
        old_path,
        new_path,
        status: file.status.clone(),
        additions: file.additions,
        deletions: file.deletions,
        content_kind: PatchContentKind::Unavailable,
        patch: String::new(),
        hunks: Vec::new(),
        message: Some(message.to_string()),
    }
}

fn split_file_patches(diff: &str) -> Vec<&str> {
    let starts: Vec<usize> = diff
        .match_indices("diff --git ")
        .filter_map(|(index, _)| {
            (index == 0 || diff.as_bytes().get(index.wrapping_sub(1)) == Some(&b'\n')).then_some(index)
        })
        .collect();

    starts
        .iter()
        .enumerate()
        .map(|(index, start)| {
            let end = starts.get(index + 1).copied().unwrap_or(diff.len());
            &diff[*start..end]
        })
        .collect()
}

fn section_matches_file(section: &str, filename: &str) -> bool {
    let new_marker = format!("+++ b/{filename}");
    let old_marker = format!("--- a/{filename}");
    let header_suffix = format!(" b/{filename}");

    section.lines().any(|line| {
        line == new_marker
            || line == old_marker
            || line.strip_prefix("diff --git ").is_some_and(|header| header.ends_with(&header_suffix))
    })
}

fn normalize_patch(file: &PrFile, patch: &str) -> String {
    let patch = patch.replace("\r\n", "\n").replace('\r', "\n");
    let mut normalized =
        if patch.starts_with("diff --git ") { patch } else { format!("{}{}", file_headers(file), patch) };

    let first_hunk = normalized.lines().position(|line| line.starts_with("@@ "));
    let has_old_marker = normalized.lines().take(first_hunk.unwrap_or(usize::MAX)).any(|line| line.starts_with("--- "));
    let has_new_marker = normalized.lines().take(first_hunk.unwrap_or(usize::MAX)).any(|line| line.starts_with("+++ "));

    if first_hunk.is_some() && (!has_old_marker || !has_new_marker) {
        let hunk_offset = normalized.find("@@ ").unwrap_or(normalized.len());
        let markers = file_markers(file);
        normalized.insert_str(hunk_offset, &markers);
    }

    if !normalized.ends_with('\n') {
        normalized.push('\n');
    }
    normalized
}

fn file_headers(file: &PrFile) -> String {
    format!("diff --git a/{0} b/{0}\n{1}", file.filename, file_markers(file))
}

fn file_markers(file: &PrFile) -> String {
    let old_marker =
        if matches!(file.status, FileStatus::Added) { "/dev/null".to_string() } else { format!("a/{}", file.filename) };
    let new_marker = if matches!(file.status, FileStatus::Removed) {
        "/dev/null".to_string()
    } else {
        format!("b/{}", file.filename)
    };
    format!("--- {old_marker}\n+++ {new_marker}\n")
}

fn patch_paths(patch: &str, file: &PrFile) -> (Option<String>, Option<String>) {
    let mut old_path = None;
    let mut new_path = None;
    for line in patch.lines() {
        if line.starts_with("@@ ") {
            break;
        }
        if let Some(value) = line.strip_prefix("--- ") {
            old_path = marker_path(value);
        } else if let Some(value) = line.strip_prefix("+++ ") {
            new_path = marker_path(value);
        } else if let Some(value) = line.strip_prefix("rename from ") {
            old_path = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("rename to ") {
            new_path = Some(value.to_string());
        }
    }

    let (fallback_old, fallback_new) = fallback_paths(file);
    (old_path.or(fallback_old), new_path.or(fallback_new))
}

fn marker_path(marker: &str) -> Option<String> {
    let marker = marker.trim();
    if marker == "/dev/null" {
        None
    } else {
        Some(marker.strip_prefix("a/").or_else(|| marker.strip_prefix("b/")).unwrap_or(marker).to_string())
    }
}

fn fallback_paths(file: &PrFile) -> (Option<String>, Option<String>) {
    match file.status {
        FileStatus::Added => (None, Some(file.filename.clone())),
        FileStatus::Removed => (Some(file.filename.clone()), None),
        _ => (Some(file.filename.clone()), Some(file.filename.clone())),
    }
}

fn is_binary_patch(patch: &str) -> bool {
    patch.lines().any(|line| line == "GIT binary patch" || line.starts_with("Binary files "))
}

fn parse_hunks(patch: &str) -> Result<Vec<PatchHunk>, String> {
    let lines: Vec<&str> = patch.lines().collect();
    let mut hunks = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let header = lines[index];
        if !header.starts_with("@@ ") {
            index += 1;
            continue;
        }

        let (old_range, new_range, section_header) = parse_hunk_header(header)?;
        let mut old_line = old_range.start;
        let mut new_line = new_range.start;
        let mut old_consumed: u32 = 0;
        let mut new_consumed: u32 = 0;
        let mut patch_lines = Vec::new();
        index += 1;

        while index < lines.len() && !lines[index].starts_with("@@ ") && !lines[index].starts_with("diff --git ") {
            let line = lines[index];
            let parsed = match line.as_bytes().first().copied() {
                Some(b' ') => {
                    let parsed = PatchLine {
                        kind: PatchLineKind::Context,
                        content: line[1..].to_string(),
                        old_line: Some(old_line),
                        new_line: Some(new_line),
                    };
                    old_line = increment_line(old_line)?;
                    new_line = increment_line(new_line)?;
                    old_consumed = old_consumed.checked_add(1).ok_or_else(|| "patch 行数超过支持范围".to_string())?;
                    new_consumed = new_consumed.checked_add(1).ok_or_else(|| "patch 行数超过支持范围".to_string())?;
                    parsed
                }
                Some(b'+') => {
                    let parsed = PatchLine {
                        kind: PatchLineKind::Addition,
                        content: line[1..].to_string(),
                        old_line: None,
                        new_line: Some(new_line),
                    };
                    new_line = increment_line(new_line)?;
                    new_consumed = new_consumed.checked_add(1).ok_or_else(|| "patch 行数超过支持范围".to_string())?;
                    parsed
                }
                Some(b'-') => {
                    let parsed = PatchLine {
                        kind: PatchLineKind::Deletion,
                        content: line[1..].to_string(),
                        old_line: Some(old_line),
                        new_line: None,
                    };
                    old_line = increment_line(old_line)?;
                    old_consumed = old_consumed.checked_add(1).ok_or_else(|| "patch 行数超过支持范围".to_string())?;
                    parsed
                }
                Some(b'\\') if line == "\\ No newline at end of file" => PatchLine {
                    kind: PatchLineKind::NoNewline,
                    content: "No newline at end of file".to_string(),
                    old_line: None,
                    new_line: None,
                },
                _ => return Err(format!("无法解析 patch 行：{line}")),
            };
            patch_lines.push(parsed);
            index += 1;
        }

        if old_consumed != old_range.count || new_consumed != new_range.count {
            return Err(format!("hunk 行数与声明不一致：{header}（实际旧行 {old_consumed}、新行 {new_consumed}）",));
        }

        hunks.push(PatchHunk {
            header: header.to_string(),
            old_start: old_range.start,
            old_count: old_range.count,
            new_start: new_range.start,
            new_count: new_range.count,
            section_header,
            lines: patch_lines,
        });
    }

    Ok(hunks)
}

fn parse_hunk_header(header: &str) -> Result<(HunkRange, HunkRange, Option<String>), String> {
    let body = header.strip_prefix("@@ ").ok_or_else(|| format!("无效的 hunk 头：{header}"))?;
    let (ranges, section) = body.split_once(" @@").ok_or_else(|| format!("无效的 hunk 头：{header}"))?;
    let mut ranges = ranges.split_whitespace();
    let old_range = parse_range(ranges.next(), '-', header)?;
    let new_range = parse_range(ranges.next(), '+', header)?;
    if ranges.next().is_some() {
        return Err(format!("无效的 hunk 头：{header}"));
    }
    let section_header = section.strip_prefix(' ').filter(|value| !value.is_empty()).map(str::to_string);
    Ok((old_range, new_range, section_header))
}

fn parse_range(value: Option<&str>, prefix: char, header: &str) -> Result<HunkRange, String> {
    let value =
        value.and_then(|value| value.strip_prefix(prefix)).ok_or_else(|| format!("无效的 hunk 行号范围：{header}"))?;
    let (start, count) = value.split_once(',').map_or((value, "1"), |(start, count)| (start, count));
    let start = start.parse::<u32>().map_err(|_| format!("无效的 hunk 起始行：{header}"))?;
    let count = count.parse::<u32>().map_err(|_| format!("无效的 hunk 行数：{header}"))?;
    Ok(HunkRange { start, count })
}

fn increment_line(line: u32) -> Result<u32, String> {
    line.checked_add(1).ok_or_else(|| "patch 行号超过支持范围".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(filename: &str, status: FileStatus, patch: &str) -> PrFile {
        PrFile { filename: filename.to_string(), status, patch: patch.to_string(), additions: 2, deletions: 1 }
    }

    #[test]
    fn standardizes_bare_hunks_and_assigns_line_numbers() {
        let files = vec![file(
            "src/main.rs",
            FileStatus::Modified,
            "@@ -10,2 +10,3 @@ fn main()\n old\n-removed\n+added\n+second",
        )];

        let result = standardize_patches("", &files);

        assert_eq!(result.len(), 1);
        assert!(result[0]
            .patch
            .starts_with("diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n"));
        assert!(matches!(result[0].content_kind, PatchContentKind::Text));
        assert_eq!(result[0].hunks[0].section_header.as_deref(), Some("fn main()"));
        assert_eq!(result[0].hunks[0].lines[0].old_line, Some(10));
        assert_eq!(result[0].hunks[0].lines[0].new_line, Some(10));
        assert_eq!(result[0].hunks[0].lines[1].old_line, Some(11));
        assert_eq!(result[0].hunks[0].lines[1].new_line, None);
        assert_eq!(result[0].hunks[0].lines[3].new_line, Some(12));
    }

    #[test]
    fn handles_multiple_hunks_and_no_newline_marker() {
        let files = vec![file(
            "src/lib.rs",
            FileStatus::Modified,
            "@@ -1 +1 @@\n-old\n+new\n\\ No newline at end of file\n@@ -20,0 +21,1 @@\n+tail\n",
        )];

        let result = standardize_patches("", &files);

        assert_eq!(result[0].hunks.len(), 2);
        assert!(matches!(result[0].hunks[0].lines[2].kind, PatchLineKind::NoNewline));
        assert_eq!(result[0].hunks[1].old_count, 0);
        assert_eq!(result[0].hunks[1].new_start, 21);
    }

    #[test]
    fn prefers_the_complete_diff_section_over_provider_file_patch() {
        let files = vec![file("src/main.rs", FileStatus::Modified, "@@ -1 +1 @@\n-truncated\n+patch")];
        let diff = "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,2 +1,2 @@\n first\n-old\n+new\n";

        let result = standardize_patches(diff, &files);

        assert_eq!(result[0].hunks[0].lines.len(), 3);
        assert_eq!(result[0].hunks[0].lines[0].content, "first");
    }

    #[test]
    fn exposes_added_deleted_and_renamed_paths() {
        let added = file("new.rs", FileStatus::Added, "@@ -0,0 +1 @@\n+new");
        let removed = file("old.rs", FileStatus::Removed, "@@ -1 +0,0 @@\n-old");
        let renamed = file(
            "new-name.rs",
            FileStatus::Renamed,
            "diff --git a/old-name.rs b/new-name.rs\n--- a/old-name.rs\n+++ b/new-name.rs\n@@ -1 +1 @@\n-old\n+new",
        );

        let result = standardize_patches("", &[added, removed, renamed]);

        assert_eq!(result[0].old_path, None);
        assert_eq!(result[0].new_path.as_deref(), Some("new.rs"));
        assert_eq!(result[1].old_path.as_deref(), Some("old.rs"));
        assert_eq!(result[1].new_path, None);
        assert_eq!(result[2].old_path.as_deref(), Some("old-name.rs"));
        assert_eq!(result[2].new_path.as_deref(), Some("new-name.rs"));
    }

    #[test]
    fn standardizes_metadata_only_rename_with_distinct_paths() {
        let renamed = file(
            "src/new-name.rs",
            FileStatus::Renamed,
            &metadata_only_rename_patch("src/old-name.rs", "src/new-name.rs"),
        );

        let result = standardize_patches("", &[renamed]);

        assert!(matches!(result[0].content_kind, PatchContentKind::MetadataOnly));
        assert_eq!(result[0].old_path.as_deref(), Some("src/old-name.rs"));
        assert_eq!(result[0].new_path.as_deref(), Some("src/new-name.rs"));
        assert_eq!(result[0].message.as_deref(), Some("该文件仅包含重命名、权限或其他元数据变更"));
    }

    #[test]
    fn reports_binary_empty_and_malformed_patches_without_panicking() {
        let binary = file("image.png", FileStatus::Modified, "Binary files a/image.png and b/image.png differ");
        let empty = file("large.txt", FileStatus::Modified, "");
        let malformed = file("bad.rs", FileStatus::Modified, "@@ invalid @@\n+line");

        let result = standardize_patches("", &[binary, empty, malformed]);

        assert!(matches!(result[0].content_kind, PatchContentKind::Binary));
        assert!(matches!(result[1].content_kind, PatchContentKind::Unavailable));
        assert!(matches!(result[2].content_kind, PatchContentKind::Unavailable));
        assert!(result[2].message.as_deref().is_some_and(|message| message.contains("hunk")));
    }

    #[test]
    fn rejects_hunks_whose_declared_ranges_do_not_match_lines() {
        let files = vec![file("bad.rs", FileStatus::Modified, "@@ -1,2 +1,2 @@\n-old\n+new")];

        let result = standardize_patches("", &files);

        assert!(matches!(result[0].content_kind, PatchContentKind::Unavailable));
        assert!(result[0].message.as_deref().is_some_and(|message| message.contains("行数")));
    }

    #[test]
    fn normalizes_crlf_input() {
        let files = vec![file("src/main.rs", FileStatus::Modified, "@@ -1 +1 @@\r\n-old\r\n+new\r\n")];

        let result = standardize_patches("", &files);

        assert!(!result[0].patch.contains('\r'));
        assert_eq!(result[0].hunks[0].lines[1].content, "new");
    }
}
