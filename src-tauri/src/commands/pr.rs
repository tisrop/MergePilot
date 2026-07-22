use crate::models::*;
use crate::patch::{standardize_patches, PATCH_SCHEMA_VERSION};
use crate::platform::capabilities_for;
use crate::state::AppState;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use tauri::State;

use super::auth::build_platform;

fn validate_compare_request(owner: &str, repo: &str, base_sha: &str, head_sha: &str) -> Result<(), String> {
    if owner.trim().is_empty() || repo.trim().is_empty() {
        return Err("仓库 owner 和名称不能为空".into());
    }
    if base_sha.trim().is_empty() || head_sha.trim().is_empty() {
        return Err("增量评审缺少 base/head 提交版本".into());
    }
    if base_sha == head_sha {
        return Err("base 和 head 提交版本相同，没有可评审的新增改动".into());
    }
    if base_sha.contains(['\0', '\n', '\r']) || head_sha.contains(['\0', '\n', '\r']) {
        return Err("提交版本包含非法字符".into());
    }
    Ok(())
}

fn normalized_values(values: Vec<String>, label: &str) -> Result<Vec<String>, String> {
    if values.len() > 100 {
        return Err(format!("{label}数量不能超过 100 个"));
    }
    let mut normalized = Vec::new();
    let mut seen = BTreeSet::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        if value.chars().count() > 256 || value.contains(['\0', '\n', '\r']) {
            return Err(format!("{label}包含无效内容"));
        }
        let key = value.to_lowercase();
        if seen.insert(key) {
            normalized.push(value.to_string());
        }
    }
    Ok(normalized)
}

fn validate_metadata_update(mut update: PrMetadataUpdate) -> Result<PrMetadataUpdate, String> {
    update.title = update.title.trim().to_string();
    if update.title.is_empty() {
        return Err("PR 标题不能为空".into());
    }
    if update.title.chars().count() > 1024 || update.title.contains(['\0', '\n', '\r']) {
        return Err("PR 标题过长或包含非法字符".into());
    }
    if update.body.len() > 1_048_576 || update.body.contains('\0') {
        return Err("PR 描述过长或包含非法字符".into());
    }
    update.reviewers = normalized_values(update.reviewers, "评审者")?;
    update.assignees = normalized_values(update.assignees, "Assignee")?;
    update.labels = normalized_values(update.labels, "标签")?;
    update.milestone = update.milestone.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    });
    if update.milestone.as_ref().is_some_and(|value| value.chars().count() > 256) {
        return Err("Milestone 名称不能超过 256 个字符".into());
    }
    Ok(update)
}

fn validate_create_reference(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{label}不能为空"));
    }
    if value.chars().count() > 512 || value.contains(['\0', '\n', '\r']) {
        return Err(format!("{label}过长或包含非法字符"));
    }
    Ok(())
}

fn validate_create_preview_request(mut request: PrCreatePreviewRequest) -> Result<PrCreatePreviewRequest, String> {
    request.source_owner = request.source_owner.trim().to_string();
    request.source_repo = request.source_repo.trim().to_string();
    request.source_branch = request.source_branch.trim().to_string();
    request.target_branch = request.target_branch.trim().to_string();
    request.commit_sha = request.commit_sha.and_then(|sha| {
        let sha = sha.trim().to_string();
        (!sha.is_empty()).then_some(sha)
    });
    for (value, label) in [
        (&request.source_owner, "源仓库 owner"),
        (&request.source_repo, "源仓库名称"),
        (&request.source_branch, "源分支"),
        (&request.target_branch, "目标分支"),
    ] {
        validate_create_reference(value, label)?;
    }
    if let Some(commit_sha) = request.commit_sha.as_deref() {
        validate_create_reference(commit_sha, "提交 SHA")?;
    }
    Ok(request)
}

fn validate_create_request(mut request: PrCreateRequest) -> Result<PrCreateRequest, String> {
    let references = validate_create_preview_request(PrCreatePreviewRequest {
        source_owner: request.source_owner,
        source_repo: request.source_repo,
        source_branch: request.source_branch,
        target_branch: request.target_branch,
        commit_sha: None,
    })?;
    request.source_owner = references.source_owner;
    request.source_repo = references.source_repo;
    request.source_branch = references.source_branch;
    request.target_branch = references.target_branch;
    request.title = request.title.trim().to_string();
    if request.title.is_empty() {
        return Err("PR 标题不能为空".into());
    }
    if request.title.chars().count() > 1024 || request.title.contains(['\0', '\n', '\r']) {
        return Err("PR 标题过长或包含非法字符".into());
    }
    if request.body.len() > 1_048_576 || request.body.contains('\0') {
        return Err("PR 描述过长或包含非法字符".into());
    }
    request.reviewers = normalized_values(request.reviewers, "评审者")?;
    request.assignees = normalized_values(request.assignees, "Assignee")?;
    request.labels = normalized_values(request.labels, "标签")?;
    Ok(request)
}

fn normalized_user_logins(users: &[User]) -> BTreeSet<String> {
    users.iter().map(|user| user.login.trim().to_lowercase()).filter(|login| !login.is_empty()).collect()
}

fn normalized_string_set(values: &[String]) -> BTreeSet<String> {
    values.iter().map(|value| value.trim().to_lowercase()).filter(|value| !value.is_empty()).collect()
}

fn metadata_changed_fields(current: &PrDetail, update: &PrMetadataUpdate) -> Vec<PrMetadataField> {
    let mut fields = Vec::new();
    if current.summary.title != update.title || current.body != update.body {
        fields.push(PrMetadataField::TitleBody);
    }
    if update.draft.is_some() && current.draft != update.draft {
        fields.push(PrMetadataField::Draft);
    }
    if normalized_user_logins(&current.reviewers) != normalized_string_set(&update.reviewers) {
        fields.push(PrMetadataField::Reviewers);
    }
    if normalized_user_logins(&current.assignees) != normalized_string_set(&update.assignees) {
        fields.push(PrMetadataField::Assignees);
    }
    if normalized_string_set(&current.summary.labels) != normalized_string_set(&update.labels) {
        fields.push(PrMetadataField::Labels);
    }
    if current.milestone.as_ref().map(|milestone| milestone.title.trim()) != update.milestone.as_deref() {
        fields.push(PrMetadataField::Milestone);
    }
    fields
}

fn ensure_metadata_field_available(
    field: PrMetadataField,
    capabilities: &crate::platform::PlatformCapabilities,
    permissions: &PrMetadataPermissions,
) -> Result<(), String> {
    let (supported, permission, label) = match field {
        PrMetadataField::TitleBody => {
            (capabilities.supports_pr_title_body_edit, permissions.can_edit_title_body, "修改标题和描述")
        }
        PrMetadataField::Draft => {
            (capabilities.supports_pr_draft_toggle, permissions.can_toggle_draft, "切换 Draft / Ready")
        }
        PrMetadataField::Reviewers => {
            (capabilities.supports_pr_reviewer_management, permissions.can_manage_reviewers, "管理评审者")
        }
        PrMetadataField::Assignees => {
            (capabilities.supports_pr_assignee_management, permissions.can_manage_assignees, "管理 Assignees")
        }
        PrMetadataField::Labels => {
            (capabilities.supports_pr_label_management, permissions.can_manage_labels, "管理标签")
        }
        PrMetadataField::Milestone => {
            (capabilities.supports_pr_milestone_management, permissions.can_manage_milestone, "管理 Milestone")
        }
        PrMetadataField::Refresh => return Ok(()),
    };
    if !supported {
        return Err(format!("当前平台不支持{label}"));
    }
    if permission == Some(false) {
        return Err(format!("当前 Token 或账号没有权限{label}"));
    }
    Ok(())
}

fn extract_issue_refs(body: &str) -> Vec<u64> {
    let keywords = ["close", "closes", "closed", "fix", "fixes", "fixed", "resolve", "resolves", "resolved"];
    let mut issues = Vec::new();
    let words: Vec<&str> = body.split(|c: char| c.is_whitespace() || c == ',').collect();
    for (i, word) in words.iter().enumerate() {
        let lower = word.to_lowercase();
        if keywords.contains(&lower.as_str()) {
            if let Some(next) = words.get(i + 1) {
                if let Some(num_str) = next.strip_prefix('#') {
                    if let Ok(num) = num_str.parse::<u64>() {
                        issues.push(num);
                    }
                }
            }
        }
    }
    issues
}

fn dependency_key(repository: &str, branch: &str) -> (String, String) {
    (repository.trim().to_lowercase(), branch.trim().to_string())
}

fn build_pr_dependency_graph(
    candidates: Vec<PrDependencyCandidate>,
    current_number: u64,
    current_fallback: Option<PrDependencyCandidate>,
    truncated: bool,
) -> Result<PrDependencyGraph, String> {
    let mut candidates_by_number = BTreeMap::new();
    for candidate in candidates {
        if candidate.number == 0
            || candidate.source_branch.trim().is_empty()
            || candidate.target_branch.trim().is_empty()
        {
            continue;
        }
        candidates_by_number.entry(candidate.number).or_insert(candidate);
    }
    if let std::collections::btree_map::Entry::Vacant(entry) = candidates_by_number.entry(current_number) {
        if let Some(candidate) = current_fallback.filter(|candidate| candidate.number == current_number) {
            entry.insert(candidate);
        } else {
            return Err("当前 PR / MR 缺少依赖分析所需的基础信息，请刷新后重试".into());
        }
    }

    let mut targets = HashMap::<(String, String), Vec<u64>>::new();
    for candidate in candidates_by_number.values() {
        targets
            .entry(dependency_key(&candidate.target_repository, &candidate.target_branch))
            .or_default()
            .push(candidate.number);
    }

    let mut all_edges = BTreeSet::<(u64, u64)>::new();
    for parent in candidates_by_number.values() {
        let source = dependency_key(&parent.source_repository, &parent.source_branch);
        for child_number in targets.get(&source).into_iter().flatten() {
            if parent.number != *child_number {
                all_edges.insert((parent.number, *child_number));
            }
        }
    }

    let mut parents = HashMap::<u64, Vec<u64>>::new();
    let mut children = HashMap::<u64, Vec<u64>>::new();
    for (parent, child) in &all_edges {
        parents.entry(*child).or_default().push(*parent);
        children.entry(*parent).or_default().push(*child);
    }

    let mut ancestors = HashSet::new();
    let mut ancestor_queue = VecDeque::from([current_number]);
    while let Some(number) = ancestor_queue.pop_front() {
        for parent in parents.get(&number).into_iter().flatten() {
            if *parent != current_number && ancestors.insert(*parent) {
                ancestor_queue.push_back(*parent);
            }
        }
    }

    let mut visible = HashSet::from([current_number]);
    visible.extend(ancestors.iter().copied());
    let mut descendant_queue = VecDeque::from([current_number]);
    while let Some(number) = descendant_queue.pop_front() {
        for child in children.get(&number).into_iter().flatten() {
            let is_open =
                candidates_by_number.get(child).is_some_and(|candidate| matches!(candidate.state, PrState::Open));
            if is_open && visible.insert(*child) {
                descendant_queue.push_back(*child);
            }
        }
    }

    let edges = all_edges
        .into_iter()
        .filter(|(parent, child)| visible.contains(parent) && visible.contains(child))
        .map(|(parent_number, child_number)| PrDependencyEdge { parent_number, child_number })
        .collect::<Vec<_>>();

    let mut indegree = visible.iter().map(|number| (*number, 0_usize)).collect::<HashMap<_, _>>();
    for edge in &edges {
        if let Some(value) = indegree.get_mut(&edge.child_number) {
            *value += 1;
        }
    }
    let mut ready =
        indegree.iter().filter_map(|(number, degree)| (*degree == 0).then_some(*number)).collect::<BTreeSet<_>>();
    let mut suggested_merge_order = Vec::with_capacity(visible.len());
    while let Some(number) = ready.iter().next().copied() {
        ready.remove(&number);
        suggested_merge_order.push(number);
        for child in children.get(&number).into_iter().flatten() {
            if !visible.contains(child) {
                continue;
            }
            if let Some(degree) = indegree.get_mut(child) {
                *degree = degree.saturating_sub(1);
                if *degree == 0 {
                    ready.insert(*child);
                }
            }
        }
    }
    let has_cycle = suggested_merge_order.len() != visible.len();
    if has_cycle {
        let ordered = suggested_merge_order.iter().copied().collect::<HashSet<_>>();
        let remaining = visible.iter().filter(|number| !ordered.contains(number)).copied().collect::<BTreeSet<_>>();
        suggested_merge_order.extend(remaining);
    }

    let current_is_open =
        candidates_by_number.get(&current_number).is_some_and(|candidate| matches!(candidate.state, PrState::Open));
    let blocking_parent_numbers = if current_is_open {
        suggested_merge_order
            .iter()
            .filter(|number| ancestors.contains(number))
            .filter(|number| {
                candidates_by_number.get(number).is_some_and(|candidate| !matches!(candidate.state, PrState::Merged))
            })
            .copied()
            .collect()
    } else {
        Vec::new()
    };
    let nodes = suggested_merge_order
        .iter()
        .filter_map(|number| candidates_by_number.get(number))
        .map(|candidate| PrDependencyNode {
            number: candidate.number,
            title: candidate.title.clone(),
            state: candidate.state.clone(),
            source_branch: candidate.source_branch.clone(),
            target_branch: candidate.target_branch.clone(),
        })
        .collect();

    Ok(PrDependencyGraph {
        current_number,
        nodes,
        edges,
        suggested_merge_order,
        blocking_parent_numbers,
        has_cycle,
        truncated,
    })
}

#[tauri::command]
pub async fn pr_list(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    state_filter: Option<String>,
    page: Option<u32>,
    per_page: Option<u32>,
) -> Result<Paginated<PrSummary>, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    let pr_state = match state_filter.as_deref() {
        Some("closed") => PrState::Closed,
        Some("merged") => PrState::Merged,
        Some("all") => PrState::All,
        _ => PrState::Open,
    };
    p.list_pull_requests(&owner, &repo, &pr_state, page.unwrap_or(1), per_page.unwrap_or(20))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pr_detail(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    number: u64,
) -> Result<PrDetail, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    p.get_pull_request(&owner, &repo, number).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pr_dependencies(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    number: u64,
) -> Result<PrDependencyGraph, String> {
    let owner = owner.trim();
    let repo = repo.trim();
    if owner.is_empty() || repo.is_empty() || number == 0 {
        return Err("仓库和 PR / MR 编号不能为空".into());
    }
    let p = build_platform(&platform, &state).map_err(|error| error.to_string())?;
    let candidates = p.list_pr_dependency_candidates(owner, repo, number).await.map_err(|error| error.to_string())?;
    build_pr_dependency_graph(candidates.items, number, Some(candidates.current), candidates.truncated)
}

#[tauri::command]
pub async fn pr_merge_queue_status(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    number: u64,
) -> Result<PrMergeQueueStatus, String> {
    let owner = owner.trim();
    let repo = repo.trim();
    if owner.is_empty() || repo.is_empty() || number == 0 {
        return Err("仓库和 PR / MR 编号不能为空".into());
    }
    let capabilities = capabilities_for(&platform).ok_or_else(|| format!("不支持的平台：{platform}"))?;
    if capabilities.merge_queue_kind.is_none() {
        return Err("当前平台不支持原生 Merge Queue / Merge Train".into());
    }
    let adapter = build_platform(&platform, &state).map_err(|error| error.to_string())?;
    adapter.get_pr_merge_queue_status(owner, repo, number).await.map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn pr_branches(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
) -> Result<PrBranchOptions, String> {
    let owner = owner.trim().to_string();
    let repo = repo.trim().to_string();
    if owner.is_empty() || repo.is_empty() {
        return Err("仓库 owner 和名称不能为空".into());
    }
    let p = build_platform(&platform, &state).map_err(|error| error.to_string())?;
    p.list_branches(&owner, &repo).await.map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn pr_labels(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
) -> Result<Vec<PrLabel>, String> {
    let owner = owner.trim().to_string();
    let repo = repo.trim().to_string();
    if owner.is_empty() || repo.is_empty() {
        return Err("仓库 owner 和名称不能为空".into());
    }
    let p = build_platform(&platform, &state).map_err(|error| error.to_string())?;
    p.list_labels(&owner, &repo).await.map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn pr_participant_suggestions(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
) -> Result<Vec<User>, String> {
    let owner = owner.trim().to_string();
    let repo = repo.trim().to_string();
    if owner.is_empty() || repo.is_empty() {
        return Err("仓库 owner 和名称不能为空".into());
    }
    let p = build_platform(&platform, &state).map_err(|error| error.to_string())?;
    p.list_pr_participant_suggestions(&owner, &repo).await.map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn pr_create_preview(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    request: PrCreatePreviewRequest,
) -> Result<PrCreatePreview, String> {
    let owner = owner.trim().to_string();
    let repo = repo.trim().to_string();
    validate_create_reference(&owner, "目标仓库 owner")?;
    validate_create_reference(&repo, "目标仓库名称")?;
    let request = validate_create_preview_request(request)?;
    if request.source_owner == owner && request.source_repo == repo && request.source_branch == request.target_branch {
        return Err("同一仓库的源分支和目标分支不能相同".into());
    }
    let capabilities = capabilities_for(&platform).ok_or_else(|| format!("不支持的平台：{platform}"))?;
    if !capabilities.supports_pr_creation {
        return Err("当前平台不支持创建 PR / MR".into());
    }

    let p = build_platform(&platform, &state).map_err(|error| error.to_string())?;
    let preview = p.preview_pull_request(&owner, &repo, &request).await.map_err(|error| error.to_string())?;
    let patches = standardize_patches(&preview.diff, &preview.files);
    Ok(PrCreatePreview {
        commits: preview.commits,
        incomplete: preview.incomplete,
        incomplete_reasons: preview.incomplete_reasons,
        diff: DiffResult {
            diff: preview.diff,
            files: preview.files,
            patch_schema_version: PATCH_SCHEMA_VERSION,
            patches,
        },
    })
}

#[tauri::command]
pub async fn pr_create(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    request: PrCreateRequest,
) -> Result<PrCreateOutcome, String> {
    let owner = owner.trim().to_string();
    let repo = repo.trim().to_string();
    if owner.is_empty() || repo.is_empty() {
        return Err("目标仓库 owner 和名称不能为空".into());
    }
    let request = validate_create_request(request)?;
    if request.source_owner == owner && request.source_repo == repo && request.source_branch == request.target_branch {
        return Err("同一仓库的源分支和目标分支不能相同".into());
    }
    let capabilities = capabilities_for(&platform).ok_or_else(|| format!("不支持的平台：{platform}"))?;
    if !capabilities.supports_pr_creation {
        return Err("当前平台不支持创建 PR / MR".into());
    }
    if request.draft && !capabilities.supports_pr_draft_toggle {
        return Err("当前平台不支持创建 Draft PR / MR".into());
    }

    let p = build_platform(&platform, &state).map_err(|error| error.to_string())?;
    let number = p.create_pull_request(&owner, &repo, &request).await.map_err(|error| error.to_string())?;
    let mut detail = match p.get_pull_request(&owner, &repo, number).await {
        Ok(detail) => detail,
        Err(error) => {
            return Ok(PrCreateOutcome {
                number,
                detail: None,
                updated_fields: Vec::new(),
                failures: vec![PrMetadataUpdateFailure {
                    field: PrMetadataField::Refresh,
                    message: format!("PR / MR 已创建，但刷新详情失败：{error}"),
                }],
            });
        }
    };

    let update = PrMetadataUpdate {
        title: detail.summary.title.clone(),
        body: detail.body.clone(),
        draft: detail.draft,
        reviewers: request.reviewers,
        assignees: request.assignees,
        labels: request.labels,
        milestone: detail.milestone.as_ref().map(|milestone| milestone.title.clone()),
        expected_updated_at: detail.summary.updated_at.clone(),
    };
    let changed_fields = metadata_changed_fields(&detail, &update);
    let mut failures = Vec::new();
    for field in &changed_fields {
        if let Err(message) = ensure_metadata_field_available(*field, &capabilities, &detail.metadata_permissions) {
            failures.push(PrMetadataUpdateFailure { field: *field, message });
        }
    }
    let writable_fields = changed_fields
        .iter()
        .copied()
        .filter(|field| !failures.iter().any(|failure| failure.field == *field))
        .collect::<Vec<_>>();
    let mut safe_update = update;
    if !writable_fields.contains(&PrMetadataField::Reviewers) {
        safe_update.reviewers = detail.reviewers.iter().map(|user| user.login.clone()).collect();
    }
    if !writable_fields.contains(&PrMetadataField::Assignees) {
        safe_update.assignees = detail.assignees.iter().map(|user| user.login.clone()).collect();
    }
    if !writable_fields.contains(&PrMetadataField::Labels) {
        safe_update.labels = detail.summary.labels.clone();
    }

    let mut updated_fields = Vec::new();
    if !writable_fields.is_empty() {
        match p.update_pull_request_metadata(&owner, &repo, number, &detail, &safe_update).await {
            Ok(mutation) => {
                updated_fields = mutation.updated_fields;
                failures.extend(mutation.failures);
                match p.get_pull_request(&owner, &repo, number).await {
                    Ok(refreshed) => detail = refreshed,
                    Err(error) => failures.push(PrMetadataUpdateFailure {
                        field: PrMetadataField::Refresh,
                        message: format!("参与者或标签写入后刷新详情失败：{error}"),
                    }),
                }
            }
            Err(error) => {
                for field in writable_fields {
                    failures.push(PrMetadataUpdateFailure { field, message: error.to_string() });
                }
            }
        }
    }

    Ok(PrCreateOutcome { number, detail: Some(detail), updated_fields, failures })
}

#[tauri::command]
pub async fn pr_metadata_update(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    number: u64,
    update: PrMetadataUpdate,
) -> Result<PrMetadataUpdateOutcome, String> {
    if owner.trim().is_empty() || repo.trim().is_empty() {
        return Err("仓库 owner 和名称不能为空".into());
    }
    let update = validate_metadata_update(update)?;
    let capabilities = capabilities_for(&platform).ok_or_else(|| format!("不支持的平台：{platform}"))?;
    let p = build_platform(&platform, &state).map_err(|error| error.to_string())?;
    let current = p.get_pull_request(&owner, &repo, number).await.map_err(|error| error.to_string())?;
    if !update.expected_updated_at.trim().is_empty() && current.summary.updated_at != update.expected_updated_at {
        return Err("PR 元数据已在远端更新，请刷新详情后重试".into());
    }

    let changed_fields = metadata_changed_fields(&current, &update);
    for field in &changed_fields {
        ensure_metadata_field_available(*field, &capabilities, &current.metadata_permissions)?;
    }
    if changed_fields.is_empty() {
        return Ok(PrMetadataUpdateOutcome { detail: Some(current), updated_fields: Vec::new(), failures: Vec::new() });
    }

    let mut mutation = p
        .update_pull_request_metadata(&owner, &repo, number, &current, &update)
        .await
        .map_err(|error| error.to_string())?;
    match p.get_pull_request(&owner, &repo, number).await {
        Ok(detail) => Ok(PrMetadataUpdateOutcome {
            detail: Some(detail),
            updated_fields: mutation.updated_fields,
            failures: mutation.failures,
        }),
        Err(error) => {
            mutation.failures.push(PrMetadataUpdateFailure {
                field: PrMetadataField::Refresh,
                message: format!("元数据写入后刷新详情失败：{error}"),
            });
            Ok(PrMetadataUpdateOutcome {
                detail: None,
                updated_fields: mutation.updated_fields,
                failures: mutation.failures,
            })
        }
    }
}

#[tauri::command]
pub async fn pr_merge_readiness(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    number: u64,
) -> Result<PrMergeReadiness, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    p.get_merge_readiness(&owner, &repo, number).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pr_diff(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    number: u64,
) -> Result<DiffResult, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    let (diff, files) = p.get_pr_diff(&owner, &repo, number).await.map_err(|e| e.to_string())?;
    let patches = standardize_patches(&diff, &files);
    Ok(DiffResult { diff, files, patch_schema_version: PATCH_SCHEMA_VERSION, patches })
}

#[tauri::command]
pub async fn pr_compare_diff(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    base_sha: String,
    head_sha: String,
) -> Result<DiffResult, String> {
    validate_compare_request(&owner, &repo, &base_sha, &head_sha)?;

    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    let (diff, files) = p.get_compare_diff(&owner, &repo, &base_sha, &head_sha).await.map_err(|e| e.to_string())?;
    let patches = standardize_patches(&diff, &files);
    Ok(DiffResult { diff, files, patch_schema_version: PATCH_SCHEMA_VERSION, patches })
}

#[tauri::command]
pub async fn pr_file_content(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    path: String,
    revision: String,
) -> Result<PrFileContent, String> {
    if owner.trim().is_empty() || repo.trim().is_empty() {
        return Err("仓库 owner 和名称不能为空".into());
    }
    crate::file_content::validate_request(&path, &revision).map_err(|error| error.to_string())?;
    let p = build_platform(&platform, &state).map_err(|error| error.to_string())?;
    p.get_pr_file_content(&owner, &repo, &path, &revision).await.map_err(|error| error.to_string())
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn pr_merge(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    number: u64,
    strategy: String,
    commit_title: Option<String>,
    commit_message: Option<String>,
    close_issues: Option<bool>,
) -> Result<PrMergeOutcome, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    let pr_detail = p.get_pull_request(&owner, &repo, number).await.map_err(|e| e.to_string())?;
    let merge_strategy = match strategy.as_str() {
        "squash" => MergeStrategy::Squash,
        "rebase" => MergeStrategy::Rebase,
        _ => MergeStrategy::Merge,
    };
    let result = p
        .merge_pull_request(&owner, &repo, number, &merge_strategy, commit_title, commit_message, &pr_detail.head_sha)
        .await
        .map_err(|e| e.to_string())?;

    let mut closed_issues = Vec::new();
    let mut issue_close_failures = Vec::new();
    if close_issues.unwrap_or(false) {
        for issue_num in extract_issue_refs(&pr_detail.body) {
            match p.close_issue(&owner, &repo, issue_num).await {
                Ok(()) => closed_issues.push(issue_num),
                Err(error) => {
                    issue_close_failures.push(IssueCloseFailure { number: issue_num, error: error.to_string() })
                }
            }
        }
    }

    Ok(PrMergeOutcome { merge: result, closed_issues, issue_close_failures })
}

#[tauri::command]
pub async fn pr_close(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    number: u64,
) -> Result<PrState, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    p.close_pull_request(&owner, &repo, number).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pr_reopen(
    state: State<'_, AppState>,
    platform: String,
    owner: String,
    repo: String,
    number: u64,
) -> Result<PrState, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    p.reopen_pull_request(&owner, &repo, number).await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        build_pr_dependency_graph, ensure_metadata_field_available, metadata_changed_fields, validate_compare_request,
        validate_create_preview_request, validate_create_request, validate_metadata_update,
    };
    use crate::models::{
        PrCreatePreviewRequest, PrCreateRequest, PrDependencyCandidate, PrDetail, PrMetadataField,
        PrMetadataPermissions, PrMetadataUpdate, PrMilestone, PrState, PrSummary, User,
    };
    use crate::platform::capabilities_for;

    fn detail() -> PrDetail {
        PrDetail {
            summary: PrSummary {
                number: 42,
                title: "原始标题".into(),
                author: User {
                    id: serde_json::json!(1),
                    login: "author".into(),
                    name: "Author".into(),
                    avatar_url: String::new(),
                },
                state: PrState::Open,
                created_at: String::new(),
                updated_at: "2026-07-18T00:00:00Z".into(),
                labels: vec!["bug".into()],
                status: None,
            },
            body: "原始描述".into(),
            source_branch: "feature".into(),
            target_branch: "main".into(),
            mergeable: Some(true),
            head_sha: "head".into(),
            base_sha: "base".into(),
            draft: Some(false),
            reviewers: vec![User {
                id: serde_json::json!(2),
                login: "Reviewer".into(),
                name: "Reviewer".into(),
                avatar_url: String::new(),
            }],
            assignees: Vec::new(),
            milestone: Some(PrMilestone { id: serde_json::json!(3), number: Some(3), title: "0.6.0".into() }),
            metadata_permissions: PrMetadataPermissions::default(),
        }
    }

    fn update() -> PrMetadataUpdate {
        PrMetadataUpdate {
            title: "原始标题".into(),
            body: "原始描述".into(),
            draft: Some(false),
            reviewers: vec!["reviewer".into()],
            assignees: Vec::new(),
            labels: vec!["BUG".into()],
            milestone: Some("0.6.0".into()),
            expected_updated_at: "2026-07-18T00:00:00Z".into(),
        }
    }

    fn create_request() -> PrCreateRequest {
        PrCreateRequest {
            source_owner: " contributor ".into(),
            source_repo: " repo ".into(),
            source_branch: " feature ".into(),
            target_branch: " main ".into(),
            title: " Add feature ".into(),
            body: "Description".into(),
            draft: true,
            reviewers: vec![" Alice ".into(), "alice".into()],
            assignees: vec![],
            labels: vec!["feature".into(), "FEATURE".into()],
        }
    }

    fn dependency_candidate(
        number: u64,
        source_branch: &str,
        target_branch: &str,
        state: PrState,
    ) -> PrDependencyCandidate {
        PrDependencyCandidate {
            number,
            title: format!("PR {number}"),
            state,
            source_branch: source_branch.into(),
            target_branch: target_branch.into(),
            source_repository: "team/repo".into(),
            target_repository: "team/repo".into(),
        }
    }

    #[test]
    fn dependency_graph_builds_connected_stack_and_merge_order() {
        let graph = build_pr_dependency_graph(
            vec![
                dependency_candidate(1, "feature-a", "main", PrState::Merged),
                dependency_candidate(2, "feature-b", "feature-a", PrState::Open),
                dependency_candidate(3, "feature-c", "feature-b", PrState::Open),
                dependency_candidate(9, "unrelated", "main", PrState::Open),
            ],
            2,
            None,
            false,
        )
        .expect("dependency graph");

        assert_eq!(graph.nodes.iter().map(|node| node.number).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(graph.suggested_merge_order, vec![1, 2, 3]);
        assert!(graph.blocking_parent_numbers.is_empty());
        assert_eq!(graph.edges.len(), 2);
        assert!(!graph.has_cycle);
        assert!(!graph.truncated);
    }

    #[test]
    fn dependency_graph_marks_unmerged_ancestors_and_cycles() {
        let blocked = build_pr_dependency_graph(
            vec![
                dependency_candidate(1, "feature-a", "main", PrState::Open),
                dependency_candidate(2, "feature-b", "feature-a", PrState::Open),
            ],
            2,
            None,
            false,
        )
        .expect("blocked dependency graph");
        assert_eq!(blocked.blocking_parent_numbers, vec![1]);

        let cycle = build_pr_dependency_graph(
            vec![
                dependency_candidate(1, "feature-a", "feature-b", PrState::Open),
                dependency_candidate(2, "feature-b", "feature-a", PrState::Open),
            ],
            1,
            None,
            false,
        )
        .expect("cyclic dependency graph");
        assert!(cycle.has_cycle);
        assert_eq!(cycle.nodes.len(), 2);
    }

    #[test]
    fn dependency_graph_keeps_closed_ancestors_but_hides_inactive_descendants_and_siblings() {
        let graph = build_pr_dependency_graph(
            vec![
                dependency_candidate(1, "feature-a", "main", PrState::Closed),
                dependency_candidate(2, "feature-b", "feature-a", PrState::Open),
                dependency_candidate(3, "closed-child", "feature-b", PrState::Closed),
                dependency_candidate(4, "open-child", "feature-b", PrState::Open),
                dependency_candidate(5, "open-grandchild", "open-child", PrState::Open),
                dependency_candidate(6, "closed-grandchild", "closed-child", PrState::Open),
                dependency_candidate(7, "sibling", "feature-a", PrState::Open),
                dependency_candidate(8, "merged-child", "feature-b", PrState::Merged),
            ],
            2,
            None,
            false,
        )
        .expect("filtered dependency graph");

        assert_eq!(graph.nodes.iter().map(|node| node.number).collect::<Vec<_>>(), vec![1, 2, 4, 5]);
        assert_eq!(graph.suggested_merge_order, vec![1, 2, 4, 5]);
        assert_eq!(graph.blocking_parent_numbers, vec![1]);
    }

    #[test]
    fn dependency_graph_does_not_mark_ancestors_as_blocking_when_current_item_is_closed() {
        let graph = build_pr_dependency_graph(
            vec![
                dependency_candidate(1, "feature-a", "main", PrState::Closed),
                dependency_candidate(2, "feature-b", "feature-a", PrState::Closed),
                dependency_candidate(3, "feature-c", "feature-b", PrState::Open),
            ],
            2,
            None,
            false,
        )
        .expect("closed current dependency graph");

        assert_eq!(graph.nodes.iter().map(|node| node.number).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert!(graph.blocking_parent_numbers.is_empty());
    }

    #[test]
    fn dependency_graph_does_not_link_same_branch_across_repositories() {
        let mut fork_parent = dependency_candidate(1, "feature-a", "main", PrState::Open);
        fork_parent.source_repository = "contributor/repo".into();
        let graph = build_pr_dependency_graph(
            vec![fork_parent, dependency_candidate(2, "feature-b", "feature-a", PrState::Open)],
            2,
            None,
            false,
        )
        .expect("repository-isolated dependency graph");

        assert_eq!(graph.suggested_merge_order, vec![2]);
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn dependency_graph_falls_back_to_the_current_pr_when_candidates_omit_it() {
        let current = dependency_candidate(2, "feature-b", "feature-a", PrState::Open);

        let graph = build_pr_dependency_graph(
            vec![dependency_candidate(9, "unrelated", "main", PrState::Open)],
            2,
            Some(current),
            false,
        )
        .expect("single-node fallback graph");

        assert_eq!(graph.nodes.iter().map(|node| node.number).collect::<Vec<_>>(), vec![2]);
        assert_eq!(graph.suggested_merge_order, vec![2]);
        assert!(graph.edges.is_empty());
        assert!(graph.blocking_parent_numbers.is_empty());
    }

    #[test]
    fn compare_request_accepts_distinct_commit_versions() {
        assert!(validate_compare_request("owner", "repo", "abc123", "def456").is_ok());
    }

    #[test]
    fn compare_request_rejects_missing_or_equal_versions() {
        assert!(validate_compare_request("", "repo", "abc123", "def456").is_err());
        assert!(validate_compare_request("owner", "repo", " ", "def456").is_err());
        assert!(validate_compare_request("owner", "repo", "abc123", "abc123").is_err());
    }

    #[test]
    fn compare_request_rejects_control_characters() {
        for invalid in ["abc\0def", "abc\ndef", "abc\rdef"] {
            let error = validate_compare_request("owner", "repo", invalid, "def456")
                .expect_err("control characters must be rejected");
            assert!(error.contains("非法字符"));
        }
    }
    #[test]
    fn metadata_update_normalizes_lists_and_empty_milestone() {
        let mut candidate = update();
        candidate.title = "  新标题  ".into();
        candidate.reviewers = vec![" Alice ".into(), "alice".into(), String::new(), "Bob".into()];
        candidate.labels = vec!["bug".into(), "BUG".into()];
        candidate.milestone = Some("   ".into());

        let normalized = validate_metadata_update(candidate).expect("metadata should be valid");
        assert_eq!(normalized.title, "新标题");
        assert_eq!(normalized.reviewers, vec!["Alice", "Bob"]);
        assert_eq!(normalized.labels, vec!["bug"]);
        assert_eq!(normalized.milestone, None);
    }

    #[test]
    fn create_request_normalizes_core_fields_and_lists() {
        let normalized = validate_create_request(create_request()).expect("create request should be valid");
        assert_eq!(normalized.source_owner, "contributor");
        assert_eq!(normalized.source_branch, "feature");
        assert_eq!(normalized.target_branch, "main");
        assert_eq!(normalized.title, "Add feature");
        assert_eq!(normalized.reviewers, vec!["Alice"]);
        assert_eq!(normalized.labels, vec!["feature"]);
    }

    #[test]
    fn create_request_rejects_missing_and_unsafe_fields() {
        let mut candidate = create_request();
        candidate.source_branch = " ".into();
        assert!(validate_create_request(candidate).unwrap_err().contains("分支"));

        let mut candidate = create_request();
        candidate.title = "bad\ntitle".into();
        assert!(validate_create_request(candidate).unwrap_err().contains("标题"));
    }

    #[test]
    fn create_preview_request_normalizes_and_validates_references() {
        let normalized = validate_create_preview_request(PrCreatePreviewRequest {
            source_owner: " contributor ".into(),
            source_repo: " repo ".into(),
            source_branch: " feature ".into(),
            target_branch: " main ".into(),
            commit_sha: None,
        })
        .expect("preview references should be valid");
        assert_eq!(normalized.source_owner, "contributor");
        assert_eq!(normalized.target_branch, "main");

        let error = validate_create_preview_request(PrCreatePreviewRequest {
            source_owner: "contributor".into(),
            source_repo: "repo".into(),
            source_branch: "bad\nbranch".into(),
            target_branch: "main".into(),
            commit_sha: None,
        })
        .unwrap_err();
        assert!(error.contains("源分支"));
    }

    #[test]
    fn metadata_update_rejects_invalid_title_body_and_oversized_lists() {
        let mut candidate = update();
        candidate.title = "   ".into();
        assert!(validate_metadata_update(candidate).unwrap_err().contains("标题不能为空"));

        let mut candidate = update();
        candidate.body = "invalid\0body".into();
        assert!(validate_metadata_update(candidate).unwrap_err().contains("描述"));

        let mut candidate = update();
        candidate.reviewers = (0..101).map(|index| format!("user-{index}")).collect();
        assert!(validate_metadata_update(candidate).unwrap_err().contains("不能超过 100"));
    }

    #[test]
    fn metadata_change_detection_is_case_insensitive_for_set_fields() {
        assert!(metadata_changed_fields(&detail(), &update()).is_empty());

        let mut candidate = update();
        candidate.body = "新描述".into();
        candidate.draft = Some(true);
        candidate.assignees = vec!["owner".into()];
        candidate.milestone = None;
        assert_eq!(
            metadata_changed_fields(&detail(), &candidate),
            vec![
                PrMetadataField::TitleBody,
                PrMetadataField::Draft,
                PrMetadataField::Assignees,
                PrMetadataField::Milestone,
            ]
        );
    }

    #[test]
    fn metadata_field_availability_respects_platform_and_runtime_permission() {
        let gitee = capabilities_for("gitee").expect("gitee capabilities");
        assert!(
            ensure_metadata_field_available(PrMetadataField::Assignees, &gitee, &PrMetadataPermissions::default(),)
                .is_ok()
        );

        let github = capabilities_for("github").expect("github capabilities");
        let permissions = PrMetadataPermissions { can_manage_labels: Some(false), ..Default::default() };
        assert!(ensure_metadata_field_available(PrMetadataField::Labels, &github, &permissions)
            .unwrap_err()
            .contains("没有权限"));
    }
}
