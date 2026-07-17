use async_trait::async_trait;
use serde_json::Value;
use sha1::{Digest, Sha1};

use super::GitPlatform;
use crate::error::AppError;
use crate::http_client::HttpClient;
use crate::models::*;

pub struct GitLabAdapter {
    client: HttpClient,
    token: String,
    base_url: String,
}

impl GitLabAdapter {
    pub fn new(client: HttpClient, token: String) -> Self {
        Self { client, token, base_url: "https://gitlab.com/api/v4".to_string() }
    }

    #[allow(dead_code)]
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = super::normalize_api_base("gitlab", &url);
        self
    }

    #[allow(dead_code)]
    fn auth_header(&self) -> String {
        format!("PRIVATE-TOKEN: {}", self.token)
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, AppError> {
        let resp = self
            .client
            .get(url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("User-Agent", "mergebeacon")
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    async fn post_json(&self, url: &str, body: &Value) -> Result<Value, AppError> {
        let resp = self
            .client
            .post(url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("User-Agent", "mergebeacon")
            .json(body)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    async fn put_json(&self, url: &str, body: &Value) -> Result<Value, AppError> {
        let resp = self
            .client
            .put(url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("User-Agent", "mergebeacon")
            .json(body)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    fn map_user(json: &Value) -> User {
        User {
            id: json["id"].clone(),
            login: json["username"].as_str().unwrap_or("").to_string(),
            name: json["name"].as_str().unwrap_or("").to_string(),
            avatar_url: json["avatar_url"].as_str().unwrap_or("").to_string(),
        }
    }

    fn required_string(json: &Value, field: &str, label: &str) -> Result<String, AppError> {
        json[field]
            .as_str()
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| AppError::Api(format!("GitLab {label} 缺少 {field}")))
    }

    fn unified_diff(change: &Value) -> String {
        let patch = change["diff"].as_str().unwrap_or("");
        let old_path = change["old_path"].as_str().unwrap_or("");
        let new_path = change["new_path"].as_str().unwrap_or("");
        if patch.trim().is_empty() {
            let is_metadata_only_rename = change["renamed_file"].as_bool() == Some(true)
                && change["collapsed"].as_bool() != Some(true)
                && change["too_large"].as_bool() != Some(true)
                && change["additions"].as_u64().unwrap_or(0) == 0
                && change["deletions"].as_u64().unwrap_or(0) == 0
                && !old_path.is_empty()
                && !new_path.is_empty()
                && old_path != new_path;
            return if is_metadata_only_rename {
                crate::patch::metadata_only_rename_patch(old_path, new_path)
            } else {
                String::new()
            };
        }

        let mut diff = if patch.starts_with("diff --git ") {
            patch.to_string()
        } else {
            let old_marker = if change["new_file"].as_bool() == Some(true) {
                "/dev/null".to_string()
            } else {
                format!("a/{old_path}")
            };
            let new_marker = if change["deleted_file"].as_bool() == Some(true) {
                "/dev/null".to_string()
            } else {
                format!("b/{new_path}")
            };

            format!("diff --git a/{old_path} b/{new_path}\n--- {old_marker}\n+++ {new_marker}\n{patch}")
        };
        if !diff.ends_with('\n') {
            diff.push('\n');
        }
        diff
    }

    fn line_code(path: &str, old_line: Option<u32>, new_line: Option<u32>) -> String {
        let path_hash = format!("{:x}", Sha1::digest(path.as_bytes()));
        format!("{path_hash}_{}_{}", old_line.unwrap_or(0), new_line.unwrap_or(0))
    }

    fn line_position(side: &str, line: u32) -> Result<(Option<u32>, Option<u32>, &'static str), AppError> {
        match side {
            "left" => Ok((Some(line), None, "old")),
            "right" => Ok((None, Some(line), "new")),
            _ => Err(AppError::Api("GitLab 行级评论 side 必须为 left 或 right".into())),
        }
    }

    fn map_pr_comment(note: &Value) -> Option<PrComment> {
        let position = note.get("position")?.as_object()?;
        let position = Value::Object(position.clone());
        let path = position["new_path"].as_str().or_else(|| position["old_path"].as_str())?.to_string();
        let line = position["new_line"].as_u64().or_else(|| position["old_line"].as_u64()).map(|line| line as u32);
        let start_line = position["line_range"]["start"]["new_line"]
            .as_u64()
            .or_else(|| position["line_range"]["start"]["old_line"].as_u64())
            .map(|line| line as u32)
            .filter(|start| Some(*start) != line);
        let original = note.get("original_position").filter(|original| original.is_object()).unwrap_or(&position);

        Some(PrComment {
            id: note["id"].clone(),
            body: note["body"].as_str().unwrap_or("").to_string(),
            path,
            line,
            start_line,
            author: Self::map_user(&note["author"]),
            created_at: note["created_at"].as_str().unwrap_or("").to_string(),
            commit_id: position["head_sha"].as_str().map(str::to_string),
            original_commit_id: original["head_sha"].as_str().map(str::to_string),
            original_line: original["new_line"]
                .as_u64()
                .or_else(|| original["old_line"].as_u64())
                .map(|line| line as u32),
            original_start_line: original["line_range"]["start"]["new_line"]
                .as_u64()
                .or_else(|| original["line_range"]["start"]["old_line"].as_u64())
                .map(|line| line as u32),
            diff_hunk: None,
        })
    }
}
#[async_trait]
impl GitPlatform for GitLabAdapter {
    fn name(&self) -> &'static str {
        "gitlab"
    }

    async fn current_user(&self) -> Result<User, AppError> {
        let url = format!("{}/user", self.base_url);
        let json = self.get_json::<Value>(&url).await?;
        Ok(Self::map_user(&json))
    }

    async fn list_repos(&self, page: u32) -> Result<Paginated<RepoSummary>, AppError> {
        let url = format!("{}/projects?membership=true&per_page=100&page={}", self.base_url, page);
        let resp = self
            .client
            .get(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("User-Agent", "mergebeacon")
            .send()
            .await?;
        let total_count = resp
            .headers()
            .get("x-total")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
        let next_page = resp
            .headers()
            .get("x-next-page")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u32>().ok());
        let total_pages = resp
            .headers()
            .get("x-total-pages")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u32>().ok())
            .or_else(|| (total_count > 0).then(|| total_count.div_ceil(100)))
            .or(next_page)
            .unwrap_or(page)
            .max(page);
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("GitLab API {status} ({url}): {body}")));
        }
        let items: Vec<Value> = resp.json().await?;

        let repos: Vec<RepoSummary> = items
            .iter()
            .map(|r| {
                let path = r["path_with_namespace"].as_str().unwrap_or("");
                let fork = r["forked_from_project"].is_object();
                let (parent_full_name, parent_owner) = if fork {
                    let parent_name = r["forked_from_project"]["path_with_namespace"].as_str().map(|s| s.to_string());
                    let parent_owner = r["forked_from_project"]["namespace"]["path"].as_str().map(|s| s.to_string());
                    (parent_name, parent_owner)
                } else {
                    (None, None)
                };
                let owner_type = match r["namespace"]["kind"].as_str() {
                    Some("group") => "organization",
                    _ => "user",
                };
                let owner_path = r["namespace"]["path"].as_str().unwrap_or("").to_string();
                let owner_display_name = r["namespace"]["name"].as_str().unwrap_or(&owner_path).to_string();
                RepoSummary {
                    id: r["id"].clone(),
                    name: r["name"].as_str().unwrap_or("").to_string(),
                    full_name: path.to_string(),
                    owner: owner_path,
                    owner_type: owner_type.to_string(),
                    owner_display_name,
                    description: r["description"].as_str().unwrap_or("").to_string(),
                    private: r["visibility"].as_str().unwrap_or("") != "public",
                    fork,
                    parent_full_name,
                    parent_owner,
                }
            })
            .collect();

        Ok(Paginated { items: repos, page, total_pages, total_count })
    }

    async fn list_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        state: &PrState,
        page: u32,
        per_page: u32,
    ) -> Result<Paginated<PrSummary>, AppError> {
        let project_id = urlencoding(owner, repo);
        let state_param = match state {
            PrState::All => "all",
            PrState::Merged => "merged",
            PrState::Closed => "closed",
            PrState::Open => "opened",
        };
        let url = format!(
            "{}/projects/{}/merge_requests?state={}&per_page={}&page={}",
            self.base_url, project_id, state_param, per_page, page
        );

        let resp = self
            .client
            .raw_client()
            .get(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("User-Agent", "mergebeacon")
            .send()
            .await?
            .error_for_status()?;

        let total_pages = resp
            .headers()
            .get("x-total-pages")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(page);

        let total_count = resp
            .headers()
            .get("x-total")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(total_pages * per_page);

        let items: Vec<Value> = resp.json().await?;

        let mrs: Vec<PrSummary> = items
            .iter()
            .map(|mr| PrSummary {
                number: mr["iid"].as_u64().unwrap_or(0),
                title: mr["title"].as_str().unwrap_or("").to_string(),
                author: Self::map_user(&mr["author"]),
                state: match (mr["state"].as_str().unwrap_or(""), mr["merged_at"].is_null()) {
                    (_, false) => PrState::Merged,
                    ("closed", _) => PrState::Closed,
                    _ => PrState::Open,
                },
                created_at: mr["created_at"].as_str().unwrap_or("").to_string(),
                updated_at: mr["updated_at"].as_str().unwrap_or("").to_string(),
                labels: mr["labels"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|l| l.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
            })
            .collect();

        Ok(Paginated { items: mrs, page, total_pages, total_count })
    }

    async fn get_pull_request(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrDetail, AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!("{}/projects/{}/merge_requests/{}", self.base_url, project_id, pr_number);
        let json = self.get_json::<Value>(&url).await?;

        let summary = PrSummary {
            number: json["iid"].as_u64().unwrap_or(0),
            title: json["title"].as_str().unwrap_or("").to_string(),
            author: Self::map_user(&json["author"]),
            state: match (json["state"].as_str().unwrap_or(""), json["merged_at"].is_null()) {
                (_, false) => PrState::Merged,
                ("closed", _) => PrState::Closed,
                _ => PrState::Open,
            },
            created_at: json["created_at"].as_str().unwrap_or("").to_string(),
            updated_at: json["updated_at"].as_str().unwrap_or("").to_string(),
            labels: json["labels"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|l| l.as_str().map(String::from)).collect())
                .unwrap_or_default(),
        };

        Ok(PrDetail {
            summary,
            body: json["description"].as_str().unwrap_or("").to_string(),
            source_branch: json["source_branch"].as_str().unwrap_or("").to_string(),
            target_branch: json["target_branch"].as_str().unwrap_or("").to_string(),
            mergeable: None,
            head_sha: json["sha"].as_str().or_else(|| json["diff_refs"]["head_sha"].as_str()).unwrap_or("").to_string(),
            base_sha: json["diff_refs"]["base_sha"].as_str().unwrap_or("").to_string(),
        })
    }

    async fn get_merge_readiness(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrMergeReadiness, AppError> {
        let project_id = urlencoding(owner, repo);
        let mr_url = format!("{}/projects/{}/merge_requests/{}", self.base_url, project_id, pr_number);
        let mr = self.get_json::<Value>(&mr_url).await?;
        let head_sha = mr["sha"].as_str().or_else(|| mr["diff_refs"]["head_sha"].as_str()).unwrap_or("").to_string();
        let draft = mr["draft"].as_bool().or_else(|| mr["work_in_progress"].as_bool());
        let merge_status = mr["detailed_merge_status"].as_str().or_else(|| mr["merge_status"].as_str()).unwrap_or("");
        let has_conflicts = mr["has_conflicts"].as_bool().or(match merge_status {
            "conflict" | "cannot_be_merged" => Some(true),
            "mergeable" | "can_be_merged" => Some(false),
            _ => None,
        });
        let branch_behind = matches!(merge_status, "need_rebase" | "behind").then_some(true);
        let has_merge_permission = mr["user"]["can_merge"].as_bool();
        let mut reasons = Vec::new();
        if mr["state"].as_str() != Some("opened") {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::NotOpen,
                message: "MR 不是打开状态".into(),
            });
        }
        if draft == Some(true) {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::Draft,
                message: "MR 仍处于 Draft 状态".into(),
            });
        }
        if has_conflicts == Some(true) {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::Conflicts,
                message: "源分支存在合并冲突".into(),
            });
        }
        if branch_behind == Some(true) {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::BranchBehind,
                message: "源分支需要 Rebase".into(),
            });
        }
        if merge_status == "discussions_not_resolved" || mr["blocking_discussions_resolved"].as_bool() == Some(false) {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::DiscussionsUnresolved,
                message: "仍有未解决的阻塞讨论".into(),
            });
        }

        if merge_status == "requested_changes" {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::ChangesRequested,
                message: "已有评审请求修改".into(),
            });
        }
        if merge_status == "not_approved" {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::ApprovalsRequired,
                message: "MR 尚未满足审批要求".into(),
            });
        }
        if matches!(merge_status, "blocked_status" | "broken_status") {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::PlatformBlocked,
                message: "GitLab 合并规则阻止当前 MR 合并".into(),
            });
        }
        if has_merge_permission == Some(false) {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::NoMergePermission,
                message: "当前账号没有该 MR 的合并权限".into(),
            });
        }

        let pipelines_url =
            format!("{}/projects/{}/merge_requests/{}/pipelines?per_page=1", self.base_url, project_id, pr_number);
        let checks_status = match self.get_json::<Vec<Value>>(&pipelines_url).await {
            Ok(pipelines) if pipelines.is_empty() => {
                if matches!(merge_status, "mergeable" | "can_be_merged") {
                    // GitLab 已确认 MR 可合并，且没有关联 Pipeline，表示当前项目没有需要等待的 CI。
                    ReadinessState::Ready
                } else {
                    ReadinessState::Unknown
                }
            }
            Ok(pipelines) => match pipelines.first().and_then(|pipeline| pipeline["status"].as_str()) {
                Some("success") => ReadinessState::Ready,
                Some("failed") | Some("canceled") | Some("canceling") | Some("skipped") => {
                    reasons.push(MergeBlockingReason {
                        code: MergeBlockingReasonCode::ChecksFailed,
                        message: "Pipeline 未通过".into(),
                    });
                    ReadinessState::Blocked
                }
                Some("pending")
                | Some("running")
                | Some("created")
                | Some("waiting_for_resource")
                | Some("preparing") => {
                    reasons.push(MergeBlockingReason {
                        code: MergeBlockingReasonCode::ChecksPending,
                        message: "Pipeline 仍在进行中".into(),
                    });
                    ReadinessState::Pending
                }
                Some(_) => ReadinessState::Unknown,
                None => ReadinessState::Unknown,
            },
            Err(_) => ReadinessState::Unknown,
        };

        let checks_status = if checks_status == ReadinessState::Unknown {
            match merge_status {
                "ci_still_running" | "pipelines_must_succeed" => ReadinessState::Pending,
                "broken_status" => ReadinessState::Blocked,
                _ => checks_status,
            }
        } else {
            checks_status
        };
        if checks_status == ReadinessState::Pending
            && !reasons.iter().any(|reason| reason.code == MergeBlockingReasonCode::ChecksPending)
        {
            reasons.push(MergeBlockingReason {
                code: MergeBlockingReasonCode::ChecksPending,
                message: "Pipeline 仍在进行中".into(),
            });
        }

        let approvals_url = format!("{}/projects/{}/merge_requests/{}/approvals", self.base_url, project_id, pr_number);
        let (approvals_status, approvals_required, approvals_received) =
            match self.get_json::<Value>(&approvals_url).await {
                Ok(approvals) => {
                    let required = approvals["approvals_required"].as_u64().map(|n| n as u32);
                    let received = approvals["approved_by"].as_array().map(|items| items.len() as u32);
                    let left = approvals["approvals_left"].as_u64();
                    if left.unwrap_or(0) > 0 || (required.is_some() && received.unwrap_or(0) < required.unwrap_or(0)) {
                        reasons.push(MergeBlockingReason {
                            code: MergeBlockingReasonCode::ApprovalsRequired,
                            message: format!(
                                "还需要 {} 个审批",
                                left.unwrap_or_else(|| u64::from(
                                    required.unwrap_or(0).saturating_sub(received.unwrap_or(0))
                                ))
                            ),
                        });
                        (ReadinessState::Blocked, required, received)
                    } else {
                        (ReadinessState::Ready, required, received)
                    }
                }
                Err(_) => (ReadinessState::Unknown, None, None),
            };

        let mergeable = match merge_status {
            "mergeable" | "can_be_merged" => Some(true),
            "conflict" | "cannot_be_merged" => Some(false),
            _ => None,
        };
        let has_hard_blocker = reasons.iter().any(|reason| reason.code != MergeBlockingReasonCode::ChecksPending);
        let status = if has_hard_blocker
            || checks_status == ReadinessState::Blocked
            || approvals_status == ReadinessState::Blocked
        {
            ReadinessState::Blocked
        } else if checks_status == ReadinessState::Pending {
            ReadinessState::Pending
        } else if mergeable == Some(true)
            && draft == Some(false)
            && has_conflicts == Some(false)
            && checks_status == ReadinessState::Ready
            && approvals_status == ReadinessState::Ready
            && has_merge_permission == Some(true)
        {
            ReadinessState::Ready
        } else {
            ReadinessState::Unknown
        };
        Ok(PrMergeReadiness {
            status,
            head_sha,
            mergeable,
            draft,
            has_conflicts,
            checks_status,
            approvals_status,
            approvals_required,
            approvals_received,
            has_merge_permission,
            branch_behind,
            blocking_reasons: reasons,
        })
    }

    async fn get_pr_diff(&self, owner: &str, repo: &str, pr_number: u64) -> Result<(String, Vec<PrFile>), AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!("{}/projects/{}/merge_requests/{}/changes", self.base_url, project_id, pr_number);
        let json = self.get_json::<Value>(&url).await?;

        let changes: Vec<PrFile> = json["changes"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|c| PrFile {
                        filename: c["new_path"].as_str().unwrap_or("").to_string(),
                        status: match c["new_file"].as_bool() {
                            Some(true) => FileStatus::Added,
                            _ => match c["deleted_file"].as_bool() {
                                Some(true) => FileStatus::Removed,
                                _ => match c["renamed_file"].as_bool() {
                                    Some(true) => FileStatus::Renamed,
                                    _ => FileStatus::Modified,
                                },
                            },
                        },
                        patch: c["diff"].as_str().unwrap_or("").to_string(),
                        additions: c["additions"].as_u64().unwrap_or(0) as u32,
                        deletions: c["deletions"].as_u64().unwrap_or(0) as u32,
                    })
                    .collect()
            })
            .unwrap_or_default();

        // GitLab returns bare hunks in `changes[].diff`; diff2html requires file headers.
        let diff = json["changes"]
            .as_array()
            .map(|items| items.iter().map(Self::unified_diff).collect::<String>())
            .unwrap_or_default();

        Ok((diff, changes))
    }

    async fn get_pr_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        revision: &str,
    ) -> Result<PrFileContent, AppError> {
        crate::file_content::validate_request(path, revision)?;
        let project_id = urlencoding(owner, repo);
        let encoded_path = urlencoding::encode(path);
        let encoded_revision = urlencoding::encode(revision);
        let url = format!(
            "{}/projects/{}/repository/files/{}?ref={}",
            self.base_url, project_id, encoded_path, encoded_revision
        );
        let json = self.get_json::<Value>(&url).await?;
        crate::file_content::decode_response("GitLab", path, revision, &json)
    }

    async fn create_review(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        body: &str,
        event: &ReviewEvent,
        _comments: &[ReviewCommentPosition],
    ) -> Result<Review, AppError> {
        if !matches!(event, ReviewEvent::Comment) {
            return Err(AppError::NotImplemented("该平台仅支持评论评审".to_string()));
        }
        let project_id = urlencoding(owner, repo);

        // GitLab uses notes for reviews; approval API is separate
        let url = format!("{}/projects/{}/merge_requests/{}/notes", self.base_url, project_id, pr_number);
        let payload = serde_json::json!({
            "body": body,
        });

        let json = self.post_json(&url, &payload).await?;

        Ok(Review {
            id: json["id"].clone(),
            body: json["body"].as_str().unwrap_or("").to_string(),
            state: "commented".to_string(),
            author: Self::map_user(&json["author"]),
            submitted_at: json["created_at"].as_str().unwrap_or("").to_string(),
        })
    }

    async fn create_pr_comment(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        commit_id: &str,
        path: &str,
        start_line: Option<u32>,
        line: u32,
        side: &str,
        body: &str,
    ) -> Result<PrComment, AppError> {
        if path.is_empty() || body.trim().is_empty() || line == 0 {
            return Err(AppError::Api("GitLab 行级评论的文件、行号和内容不能为空".into()));
        }
        if start_line.is_some_and(|start| start == 0 || start > line) {
            return Err(AppError::Api("GitLab 多行评论起始行无效".into()));
        }
        let (old_line, new_line, line_type) = Self::line_position(side, line)?;

        let project_id = urlencoding(owner, repo);
        let merge_request_url = format!("{}/projects/{project_id}/merge_requests/{pr_number}", self.base_url);
        let merge_request: Value = self.get_json(&merge_request_url).await?;
        let diff_refs = &merge_request["diff_refs"];
        let base_sha = Self::required_string(diff_refs, "base_sha", "MR diff refs")?;
        let start_sha = Self::required_string(diff_refs, "start_sha", "MR diff refs")?;
        let head_sha = Self::required_string(diff_refs, "head_sha", "MR diff refs")?;
        if commit_id != head_sha {
            return Err(AppError::Api("GitLab MR 已更新，请刷新 Diff 后重新评论".into()));
        }

        let mut position = serde_json::json!({
            "position_type": "text",
            "base_sha": base_sha,
            "start_sha": start_sha,
            "head_sha": head_sha,
            "old_path": path,
            "new_path": path,
        });
        if let Some(old_line) = old_line {
            position["old_line"] = old_line.into();
        }
        if let Some(new_line) = new_line {
            position["new_line"] = new_line.into();
        }
        if let Some(start_line) = start_line.filter(|start| *start != line) {
            let (start_old_line, start_new_line, _) = Self::line_position(side, start_line)?;
            position["line_range"] = serde_json::json!({
                "start": {
                    "line_code": Self::line_code(path, start_old_line, start_new_line),
                    "type": line_type,
                    "old_line": start_old_line,
                    "new_line": start_new_line,
                },
                "end": {
                    "line_code": Self::line_code(path, old_line, new_line),
                    "type": line_type,
                    "old_line": old_line,
                    "new_line": new_line,
                },
            });
        }

        let url = format!("{merge_request_url}/discussions");
        let discussion = self.post_json(&url, &serde_json::json!({ "body": body, "position": position })).await?;
        let note = discussion["notes"]
            .as_array()
            .and_then(|notes| notes.iter().find(|note| note["position"].is_object()))
            .ok_or_else(|| AppError::Api("GitLab 创建行级评论后未返回有效 Note".into()))?;
        Self::map_pr_comment(note).ok_or_else(|| AppError::Api("GitLab 行级评论响应缺少位置信息".into()))
    }

    async fn list_pr_comments(&self, owner: &str, repo: &str, pr_number: u64) -> Result<Vec<PrComment>, AppError> {
        let project_id = urlencoding(owner, repo);
        let url =
            format!("{}/projects/{project_id}/merge_requests/{pr_number}/discussions?per_page=100", self.base_url);
        let discussions: Vec<Value> = self.get_json(&url).await?;
        Ok(discussions
            .iter()
            .flat_map(|discussion| discussion["notes"].as_array().into_iter().flatten())
            .filter(|note| !note["system"].as_bool().unwrap_or(false))
            .filter_map(Self::map_pr_comment)
            .collect())
    }

    async fn list_reviews(&self, owner: &str, repo: &str, pr_number: u64) -> Result<Vec<Review>, AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!("{}/projects/{}/merge_requests/{}/notes?per_page=100", self.base_url, project_id, pr_number);
        let items: Vec<Value> = self.get_json(&url).await?;

        let reviews = items
            .iter()
            .filter(|n| !n["system"].as_bool().unwrap_or(false))
            .filter(|n| !n["position"].is_object())
            .map(|n| Review {
                id: n["id"].clone(),
                body: n["body"].as_str().unwrap_or("").to_string(),
                state: "commented".to_string(),
                author: Self::map_user(&n["author"]),
                submitted_at: n["created_at"].as_str().unwrap_or("").to_string(),
            })
            .collect();

        Ok(reviews)
    }

    async fn list_issues(
        &self,
        owner: &str,
        repo: &str,
        state: &IssueState,
        page: u32,
    ) -> Result<Paginated<IssueSummary>, AppError> {
        let project_id = urlencoding(owner, repo);
        let state_param = state.as_str();
        let url = format!(
            "{}/projects/{}/issues?state={}&per_page=100&page={}",
            self.base_url, project_id, state_param, page
        );
        let items: Vec<Value> = self.get_json(&url).await?;

        let issues = items
            .iter()
            .map(|i| IssueSummary {
                number: i["iid"].as_u64().unwrap_or(0),
                title: i["title"].as_str().unwrap_or("").to_string(),
                author: Self::map_user(&i["author"]),
                state: match i["state"].as_str().unwrap_or("") {
                    "closed" => IssueState::Closed,
                    _ => IssueState::Open,
                },
                labels: i["labels"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|l| l.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
                created_at: i["created_at"].as_str().unwrap_or("").to_string(),
            })
            .collect();

        Ok(Paginated { items: issues, page, total_pages: 1, total_count: 0 })
    }

    async fn create_issue(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        body: &str,
        labels: &[String],
    ) -> Result<Issue, AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!("{}/projects/{}/issues", self.base_url, project_id);
        let payload = serde_json::json!({
            "title": title,
            "description": body,
            "labels": labels.join(","),
        });

        let json = self.post_json(&url, &payload).await?;

        Ok(Issue {
            number: json["iid"].as_u64().unwrap_or(0),
            title: json["title"].as_str().unwrap_or("").to_string(),
            body: json["description"].as_str().unwrap_or("").to_string(),
            author: Self::map_user(&json["author"]),
            state: match json["state"].as_str().unwrap_or("") {
                "closed" => IssueState::Closed,
                _ => IssueState::Open,
            },
            labels: json["labels"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|l| l.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            created_at: json["created_at"].as_str().unwrap_or("").to_string(),
            updated_at: json["updated_at"].as_str().unwrap_or("").to_string(),
        })
    }

    async fn merge_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        strategy: &MergeStrategy,
        _commit_title: Option<String>,
        commit_message: Option<String>,
        sha: &str,
    ) -> Result<PrMergeResult, AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!("{}/projects/{}/merge_requests/{}/merge", self.base_url, project_id, pr_number);
        let squash = matches!(strategy, MergeStrategy::Squash);
        let mut payload = serde_json::json!({
            "squash": squash,
            "sha": sha,
        });
        if let Some(m) = commit_message {
            payload["merge_commit_message"] = serde_json::Value::String(m);
        }
        let json = self.put_json(&url, &payload).await?;
        Ok(PrMergeResult { merged: true, sha: json["id"].as_str().unwrap_or("").to_string(), message: String::new() })
    }

    async fn close_pull_request(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrState, AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!("{}/projects/{}/merge_requests/{}", self.base_url, project_id, pr_number);
        let payload = serde_json::json!({ "state_event": "close" });
        self.put_json(&url, &payload).await?;
        Ok(PrState::Closed)
    }

    async fn reopen_pull_request(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrState, AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!("{}/projects/{}/merge_requests/{}", self.base_url, project_id, pr_number);
        let payload = serde_json::json!({ "state_event": "reopen" });
        self.put_json(&url, &payload).await?;
        Ok(PrState::Open)
    }

    async fn close_issue(&self, owner: &str, repo: &str, issue_number: u64) -> Result<(), AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!("{}/projects/{}/issues/{}", self.base_url, project_id, issue_number);
        let payload = serde_json::json!({ "state_event": "close" });
        self.put_json(&url, &payload).await?;
        Ok(())
    }
}

fn urlencoding(owner: &str, repo: &str) -> String {
    urlencoding::encode(&format!("{owner}/{repo}")).into_owned()
}
