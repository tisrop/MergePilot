use async_trait::async_trait;
use serde_json::Value;

use super::GitPlatform;
use crate::error::AppError;
use crate::http_client::HttpClient;
use crate::models::*;

/// Gitee adapter — Gitee API v5 is largely compatible with GitHub v3
pub struct GiteeAdapter {
    client: HttpClient,
    token: String,
    base_url: String,
}

impl GiteeAdapter {
    pub fn new(client: HttpClient, token: String) -> Self {
        Self {
            client,
            token,
            base_url: "https://gitee.com/api/v5".to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url.trim_end_matches('/').to_string();
        self
    }

    fn parse_last_page_gitee(link: Option<&str>, fallback: u32) -> u32 {
        let Some(header) = link else {
            return fallback;
        };
        for part in header.split(',') {
            let part = part.trim();
            if part.contains("rel=\"last\"") || part.contains("rel='last'") {
                if let Some(url_start) = part.find('<') {
                    let url_end = part[url_start..]
                        .find('>')
                        .unwrap_or(part.len() - url_start);
                    let url = &part[url_start + 1..url_start + url_end];
                    let query = url.split('?').nth(1).unwrap_or("");
                    for seg in query.split('&') {
                        if let Some(page_str) = seg.strip_prefix("page=") {
                            if let Ok(n) = page_str.parse::<u32>() {
                                return n;
                            }
                        }
                    }
                }
            }
        }
        fallback
    }

    fn auth_query(&self) -> String {
        format!("access_token={}", self.token)
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, AppError> {
        let separator = if url.contains('?') { "&" } else { "?" };
        let full_url = format!("{}{}{}", url, separator, self.auth_query());

        let resp = self
            .client
            .get(&full_url)
            .header("User-Agent", "mergepilot")
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    async fn post_json(&self, url: &str, body: &Value) -> Result<Value, AppError> {
        let separator = if url.contains('?') { "&" } else { "?" };
        let full_url = format!("{}{}{}", url, separator, self.auth_query());

        let resp = self
            .client
            .post(&full_url)
            .header("User-Agent", "mergepilot")
            .json(body)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    async fn patch_json(&self, url: &str, body: &Value) -> Result<Value, AppError> {
        let separator = if url.contains('?') { "&" } else { "?" };
        let full_url = format!("{}{}{}", url, separator, self.auth_query());

        let resp = self
            .client
            .raw_client()
            .patch(&full_url)
            .header("User-Agent", "mergepilot")
            .json(body)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    fn map_user(json: &Value) -> User {
        User {
            id: json["id"].clone(),
            login: json["login"].as_str().unwrap_or("").to_string(),
            name: json["name"].as_str().unwrap_or("").to_string(),
            avatar_url: json["avatar_url"].as_str().unwrap_or("").to_string(),
        }
    }
}
#[async_trait]
impl GitPlatform for GiteeAdapter {
    fn name(&self) -> &'static str {
        "gitee"
    }

    async fn current_user(&self) -> Result<User, AppError> {
        let url = format!("{}/user", self.base_url);
        let json = self.get_json::<Value>(&url).await?;
        Ok(Self::map_user(&json))
    }

    async fn list_repos(&self, page: u32) -> Result<Paginated<RepoSummary>, AppError> {
        let url = format!("{}/user/repos?per_page=100&page={}", self.base_url, page);

        let separator = if url.contains('?') { "&" } else { "?" };
        let full_url = format!("{}{}{}", url, separator, self.auth_query());

        let resp = self
            .client
            .get(&full_url)
            .header("User-Agent", "mergepilot")
            .send()
            .await?;

        let link_header = resp.headers().get("link").and_then(|v| v.to_str().ok());
        let last_page = Self::parse_last_page_gitee(link_header, page);

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!(
                "Gitee API {} ({}): {}",
                status, url, body
            )));
        }

        let items: Vec<Value> = resp.json().await?;

        let repos: Vec<RepoSummary> = items
            .iter()
            .map(|r| {
                let full_name = r["full_name"].as_str().unwrap_or("");
                let parts: Vec<&str> = full_name.splitn(2, '/').collect();
                let fork = r["fork"].as_bool().unwrap_or(false);
                let (parent_full_name, parent_owner) = if fork {
                    let parent_name = r["parent"]["full_name"].as_str().map(|s| s.to_string());
                    let parent_owner = r["parent"]["owner"]["login"]
                        .as_str()
                        .map(|s| s.to_string());
                    (parent_name, parent_owner)
                } else {
                    (None, None)
                };
                RepoSummary {
                    id: r["id"].clone(),
                    name: r["name"].as_str().unwrap_or("").to_string(),
                    full_name: full_name.to_string(),
                    owner: parts.first().unwrap_or(&"").to_string(),
                    description: r["description"].as_str().unwrap_or("").to_string(),
                    private: r["private"].as_bool().unwrap_or(false),
                    fork,
                    parent_full_name,
                    parent_owner,
                }
            })
            .collect();

        Ok(Paginated {
            items: repos,
            page,
            total_pages: last_page,
            total_count: 0,
        })
    }

    async fn list_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        state: &PrState,
        page: u32,
        per_page: u32,
    ) -> Result<Paginated<PrSummary>, AppError> {
        let api_state = state.as_str();
        let url = format!(
            "{}/repos/{}/{}/pulls?state={}&per_page={}&page={}",
            self.base_url, owner, repo, api_state, per_page, page
        );

        let separator = if url.contains('?') { "&" } else { "?" };
        let full_url = format!("{}{}{}", url, separator, self.auth_query());

        let resp = self
            .client
            .get(&full_url)
            .header("User-Agent", "mergepilot")
            .send()
            .await?;

        let header_total_count = resp
            .headers()
            .get("total_count")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok());
        let header_total_page = resp
            .headers()
            .get("total_page")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok());

        let link_header = resp.headers().get("link").and_then(|v| v.to_str().ok());
        let last_page =
            header_total_page.unwrap_or_else(|| Self::parse_last_page_gitee(link_header, page));

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!(
                "Gitee API {} ({}): {}",
                status, url, body
            )));
        }

        let items: Vec<Value> = resp.json().await?;

        let all_prs: Vec<PrSummary> = items
            .iter()
            .map(|pr| {
                let state_str = pr["state"].as_str().unwrap_or("");
                let merged = !pr["merged_at"].is_null();
                PrSummary {
                    number: pr["number"].as_u64().unwrap_or(0),
                    title: pr["title"].as_str().unwrap_or("").to_string(),
                    author: Self::map_user(&pr["user"]),
                    state: if merged {
                        PrState::Merged
                    } else if state_str == "closed" {
                        PrState::Closed
                    } else {
                        PrState::Open
                    },
                    created_at: pr["created_at"].as_str().unwrap_or("").to_string(),
                    updated_at: pr["updated_at"].as_str().unwrap_or("").to_string(),
                    labels: pr["labels"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|l| l["name"].as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                }
            })
            .collect();

        let prs: Vec<PrSummary> = match state {
            PrState::Merged => all_prs
                .into_iter()
                .filter(|p| matches!(p.state, PrState::Merged))
                .collect(),
            PrState::Closed => all_prs
                .into_iter()
                .filter(|p| matches!(p.state, PrState::Closed))
                .collect(),
            _ => all_prs,
        };

        let total_count = if let Some(tc) = header_total_count {
            tc
        } else if prs.is_empty() {
            0
        } else if (prs.len() as u32) < per_page || page >= last_page {
            (page - 1) * per_page + prs.len() as u32
        } else {
            last_page * per_page
        };

        Ok(Paginated {
            items: prs,
            page,
            total_pages: last_page,
            total_count,
        })
    }

    async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<PrDetail, AppError> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}",
            self.base_url, owner, repo, pr_number
        );
        let json = self.get_json::<Value>(&url).await?;

        let summary = PrSummary {
            number: json["number"].as_u64().unwrap_or(0),
            title: json["title"].as_str().unwrap_or("").to_string(),
            author: Self::map_user(&json["user"]),
            state: {
                let st = json["state"].as_str().unwrap_or("");
                let merged = !json["merged_at"].is_null();
                if merged {
                    PrState::Merged
                } else if st == "closed" {
                    PrState::Closed
                } else {
                    PrState::Open
                }
            },
            created_at: json["created_at"].as_str().unwrap_or("").to_string(),
            updated_at: json["updated_at"].as_str().unwrap_or("").to_string(),
            labels: json["labels"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|l| l["name"].as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
        };

        Ok(PrDetail {
            summary,
            body: json["body"].as_str().unwrap_or("").to_string(),
            source_branch: json["head"]["ref"].as_str().unwrap_or("").to_string(),
            target_branch: json["base"]["ref"].as_str().unwrap_or("").to_string(),
            mergeable: json["mergeable"].as_bool(),
            head_sha: json["head"]["sha"].as_str().unwrap_or("").to_string(),
        })
    }

    async fn get_pr_diff(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<(String, Vec<PrFile>), AppError> {
        let files_url = format!(
            "{}/repos/{}/{}/pulls/{}/files?per_page=300",
            self.base_url, owner, repo, pr_number
        );
        let files_json: Vec<Value> = self.get_json(&files_url).await?;

        let files: Vec<PrFile> = files_json
            .iter()
            .map(|f| {
                let patch = f["patch"]["diff"]
                    .as_str()
                    .or_else(|| f["patch"].as_str())
                    .unwrap_or("")
                    .to_string();
                PrFile {
                    filename: f["filename"].as_str().unwrap_or("").to_string(),
                    status: match f["status"].as_str().unwrap_or("") {
                        "added" => FileStatus::Added,
                        "removed" => FileStatus::Removed,
                        "renamed" => FileStatus::Renamed,
                        _ => FileStatus::Modified,
                    },
                    patch,
                    additions: f["additions"].as_u64().unwrap_or(0) as u32,
                    deletions: f["deletions"].as_u64().unwrap_or(0) as u32,
                }
            })
            .collect();

        let diff = files
            .iter()
            .map(|f| {
                let patch = &f.patch;
                if patch.starts_with("diff --git") || patch.starts_with("@@") {
                    if patch.starts_with("@@") {
                        format!(
                            "diff --git a/{} b/{}\n--- a/{}\n+++ b/{}\n{}",
                            f.filename, f.filename, f.filename, f.filename, patch
                        )
                    } else {
                        patch.clone()
                    }
                } else if !patch.is_empty() {
                    format!(
                        "diff --git a/{} b/{}\n--- a/{}\n+++ b/{}\n{}",
                        f.filename, f.filename, f.filename, f.filename, patch
                    )
                } else {
                    String::new()
                }
            })
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("");

        Ok((diff, files))
    }

    async fn create_review(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        body: &str,
        event: &ReviewEvent,
    ) -> Result<Review, AppError> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}/comments",
            self.base_url, owner, repo, pr_number
        );
        let payload = serde_json::json!({
            "body": format!("**Review ({})**\n\n{}",
                match event {
                    ReviewEvent::Approve => "Approve",
                    ReviewEvent::Comment => "Comment",
                    ReviewEvent::RequestChanges => "Request Changes",
                },
                body
            ),
        });
        let json = self.post_json(&url, &payload).await?;

        Ok(Review {
            id: json["id"].clone(),
            body: json["body"].as_str().unwrap_or("").to_string(),
            state: "commented".to_string(),
            author: Self::map_user(&json["user"]),
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
        _side: &str,
        body: &str,
    ) -> Result<(), AppError> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}/comments",
            self.base_url, owner, repo, pr_number
        );
        // Gitee API does not support start_line/end_line for range comments.
        // For multi-line selections, embed the line range in the comment body.
        let final_body = if let Some(sl) = start_line {
            format!("[L{}-L{}]\n{}", sl, line, body)
        } else {
            body.to_string()
        };
        let payload = serde_json::json!({
            "body": final_body,
            "commit_id": commit_id,
            "path": path,
            "position": line,
        });
        self.post_json(&url, &payload).await?;
        Ok(())
    }

    async fn list_pr_comments(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<Vec<PrComment>, AppError> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}/comments?per_page=100",
            self.base_url, owner, repo, pr_number
        );
        let items: Vec<Value> = self.get_json(&url).await?;
        let comments = items
            .iter()
            .filter(|c| c["path"].is_string() && !c["path"].as_str().unwrap_or("").is_empty())
            .map(|c| {
                let line = c["new_line"]
                    .as_u64()
                    .or_else(|| c["position"].as_u64())
                    .map(|n| n as u32);
                PrComment {
                    id: c["id"].clone(),
                    body: c["body"].as_str().unwrap_or("").to_string(),
                    path: c["path"].as_str().unwrap_or("").to_string(),
                    line,
                    start_line: c["start_line"].as_u64().map(|n| n as u32),
                    author: Self::map_user(&c["user"]),
                    created_at: c["created_at"].as_str().unwrap_or("").to_string(),
                }
            })
            .collect();
        Ok(comments)
    }

    async fn list_reviews(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<Vec<Review>, AppError> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}/comments?per_page=100",
            self.base_url, owner, repo, pr_number
        );
        let items: Vec<Value> = self.get_json(&url).await?;

        let reviews = items
            .iter()
            .filter(|c| {
                let path = c["path"].as_str().unwrap_or("");
                path.is_empty()
            })
            .map(|c| Review {
                id: c["id"].clone(),
                body: c["body"].as_str().unwrap_or("").to_string(),
                state: "commented".to_string(),
                author: Self::map_user(&c["user"]),
                submitted_at: c["created_at"].as_str().unwrap_or("").to_string(),
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
        let state_param = state.as_str();
        let url = format!(
            "{}/repos/{}/{}/issues?state={}&per_page=100&page={}",
            self.base_url, owner, repo, state_param, page
        );

        let separator = if url.contains('?') { "&" } else { "?" };
        let full_url = format!("{}{}{}", url, separator, self.auth_query());

        let resp = self
            .client
            .get(&full_url)
            .header("User-Agent", "mergepilot")
            .send()
            .await?;

        let link_header = resp.headers().get("link").and_then(|v| v.to_str().ok());
        let last_page = Self::parse_last_page_gitee(link_header, page);

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!(
                "Gitee API {} ({}): {}",
                status, url, body
            )));
        }

        let items: Vec<Value> = resp.json().await?;

        let issues = items
            .iter()
            .filter(|i| !i["pull_request"].is_object())
            .map(|i| IssueSummary {
                number: i["number"].as_u64().unwrap_or(0),
                title: i["title"].as_str().unwrap_or("").to_string(),
                author: Self::map_user(&i["user"]),
                state: match i["state"].as_str().unwrap_or("") {
                    "closed" => IssueState::Closed,
                    _ => IssueState::Open,
                },
                labels: i["labels"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|l| l["name"].as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                created_at: i["created_at"].as_str().unwrap_or("").to_string(),
            })
            .collect();

        Ok(Paginated {
            items: issues,
            page,
            total_pages: last_page,
            total_count: 0,
        })
    }

    async fn create_issue(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        body: &str,
        labels: &[String],
    ) -> Result<Issue, AppError> {
        let url = format!("{}/repos/{}/{}/issues", self.base_url, owner, repo);
        let payload = serde_json::json!({
            "title": title,
            "body": body,
            "labels": labels.join(","),
        });

        let json = self.post_json(&url, &payload).await?;

        Ok(Issue {
            number: json["number"].as_u64().unwrap_or(0),
            title: json["title"].as_str().unwrap_or("").to_string(),
            body: json["body"].as_str().unwrap_or("").to_string(),
            author: Self::map_user(&json["user"]),
            state: match json["state"].as_str().unwrap_or("") {
                "closed" => IssueState::Closed,
                _ => IssueState::Open,
            },
            labels: json["labels"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|l| l["name"].as_str().map(String::from))
                        .collect()
                })
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
        commit_title: Option<String>,
        commit_message: Option<String>,
        sha: &str,
    ) -> Result<PrMergeResult, AppError> {
        let merge_method = match strategy {
            MergeStrategy::Merge => "merge",
            MergeStrategy::Squash => "squash",
            MergeStrategy::Rebase => "rebase",
        };
        let url = format!(
            "{}/repos/{}/{}/pulls/{}/merge",
            self.base_url, owner, repo, pr_number
        );
        let mut payload = serde_json::json!({
            "merge_method": merge_method,
            "sha": sha,
        });
        if let Some(t) = commit_title {
            payload["title"] = serde_json::Value::String(t);
        }
        if let Some(m) = commit_message {
            payload["description"] = serde_json::Value::String(m);
        }
        let separator = if url.contains('?') { "&" } else { "?" };
        let full_url = format!("{}{}{}", url, separator, self.auth_query());
        let resp = self
            .client
            .put(&full_url)
            .header("User-Agent", "mergepilot")
            .json(&payload)
            .send()
            .await?;
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            let detail = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| v["message"].as_str().map(String::from))
                .unwrap_or_else(|| {
                    if body.is_empty() {
                        "未知错误".to_string()
                    } else {
                        body
                    }
                });
            return Err(AppError::Api(detail));
        }
        let json: Value = resp.json().await?;
        Ok(PrMergeResult {
            merged: json["merged"].as_bool().unwrap_or(false),
            sha: json["sha"].as_str().unwrap_or("").to_string(),
            message: json["message"].as_str().unwrap_or("").to_string(),
        })
    }

    async fn close_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<PrState, AppError> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}",
            self.base_url, owner, repo, pr_number
        );
        let payload = serde_json::json!({ "state": "closed" });
        let json = self.patch_json(&url, &payload).await?;
        let state = if !json["merged_at"].is_null() {
            PrState::Merged
        } else if json["state"].as_str().unwrap_or("") == "closed" {
            PrState::Closed
        } else {
            PrState::Open
        };
        Ok(state)
    }

    async fn reopen_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<PrState, AppError> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}",
            self.base_url, owner, repo, pr_number
        );
        let payload = serde_json::json!({ "state": "open" });
        let json = self.patch_json(&url, &payload).await?;
        let state = if json["state"].as_str().unwrap_or("") == "open" {
            PrState::Open
        } else {
            PrState::Closed
        };
        Ok(state)
    }

    async fn close_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
    ) -> Result<(), AppError> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}",
            self.base_url, owner, repo, issue_number
        );
        let payload = serde_json::json!({ "state": "closed" });
        self.patch_json(&url, &payload).await?;
        Ok(())
    }
}
