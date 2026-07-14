use async_trait::async_trait;
use serde_json::Value;

use super::GitPlatform;
use crate::error::AppError;
use crate::http_client::HttpClient;
use crate::models::*;

pub struct GitHubAdapter {
    client: HttpClient,
    token: String,
    base_url: String,
}

impl GitHubAdapter {
    pub fn new(client: HttpClient, token: String) -> Self {
        Self { client, token, base_url: "https://api.github.com".to_string() }
    }

    #[allow(dead_code)]
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url.trim_end_matches('/').to_string();
        self
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.token)
    }

    /// Fetch org/enterprise display names via `/orgs/{login}`.
    /// Updates `owner_display_name` in place for org repos.
    async fn resolve_org_display_names(&self, repos: &mut [RepoSummary]) {
        let orgs: Vec<String> = {
            let mut seen = std::collections::HashSet::new();
            repos
                .iter()
                .filter(|r| r.owner_type == "organization" && seen.insert(r.owner.clone()))
                .map(|r| r.owner.clone())
                .collect()
        };
        if orgs.is_empty() {
            return;
        }

        let auth = self.auth_header();
        let client = self.client.clone();
        let base = self.base_url.clone();

        let futs: Vec<_> = orgs
            .into_iter()
            .map(|login| {
                let url = format!("{}/orgs/{}", base, login);
                let c = client.clone();
                let a = auth.clone();
                tokio::spawn(async move {
                    let resp = c
                        .get(&url)
                        .header("Authorization", &a)
                        .header("User-Agent", "mergebeacon")
                        .header("Accept", "application/vnd.github.v3+json")
                        .send()
                        .await
                        .ok()?;
                    let json: serde_json::Value = resp.json().await.ok()?;
                    let name = json["name"].as_str()?.to_string();
                    Some((login, name))
                })
            })
            .collect();

        let results = futures::future::join_all(futs).await;
        let mut name_map = std::collections::HashMap::new();
        for r in results {
            if let Ok(Some((login, name))) = r {
                name_map.insert(login, name);
            }
        }

        for r in repos.iter_mut() {
            if let Some(name) = name_map.get(&r.owner) {
                r.owner_display_name = name.clone();
            }
        }
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, AppError> {
        let resp = self
            .client
            .get(url)
            .header("Authorization", &self.auth_header())
            .header("User-Agent", "mergebeacon")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("GitHub API {} ({}): {}", status, url, body)));
        }
        Ok(resp.json().await?)
    }

    /// Parse the `Link` header to extract the last page number.
    /// GitHub format: `<url?page=5>; rel="last"`
    fn parse_last_page(link: Option<&str>, fallback: u32) -> u32 {
        let Some(header) = link else {
            return fallback;
        };
        // Find the URL with rel="last"
        for part in header.split(',') {
            let part = part.trim();
            if part.contains(r#"rel="last""#) {
                // Extract the page=XX from the URL between < and >
                if let Some(url_start) = part.find('<') {
                    let url_end = part[url_start..].find('>').unwrap_or(part.len() - url_start);
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

    async fn get_text(&self, url: &str) -> Result<String, AppError> {
        let resp = self
            .client
            .get(url)
            .header("Authorization", &self.auth_header())
            .header("User-Agent", "mergebeacon")
            .header("Accept", "application/vnd.github.v3.diff")
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("GitHub API {} ({}): {}", status, url, body)));
        }
        Ok(resp.text().await?)
    }

    async fn post_json(&self, url: &str, body: &Value) -> Result<Value, AppError> {
        let resp = self
            .client
            .post(url)
            .header("Authorization", &self.auth_header())
            .header("User-Agent", "mergebeacon")
            .header("Accept", "application/vnd.github.v3+json")
            .json(body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let error_body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("GitHub API {} ({}): {}", status, url, error_body)));
        }
        Ok(resp.json().await?)
    }

    async fn put_json(&self, url: &str, body: &Value) -> Result<Value, AppError> {
        let resp = self
            .client
            .put(url)
            .header("Authorization", &self.auth_header())
            .header("User-Agent", "mergebeacon")
            .header("Accept", "application/vnd.github.v3+json")
            .json(body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let error_body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("GitHub API {} ({}): {}", status, url, error_body)));
        }
        Ok(resp.json().await?)
    }

    async fn patch_json(&self, url: &str, body: &Value) -> Result<Value, AppError> {
        // PATCH via reqwest::Client
        let resp = self
            .client
            .raw_client()
            .patch(url)
            .header("Authorization", &self.auth_header())
            .header("User-Agent", "mergebeacon")
            .header("Accept", "application/vnd.github.v3+json")
            .json(body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let error_body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("GitHub API {} ({}): {}", status, url, error_body)));
        }
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

    fn map_pr_state(state_str: &str, merged: bool) -> PrState {
        if merged {
            PrState::Merged
        } else {
            match state_str {
                "closed" => PrState::Closed,
                _ => PrState::Open,
            }
        }
    }
}
#[async_trait]
impl GitPlatform for GitHubAdapter {
    fn name(&self) -> &'static str {
        "github"
    }

    async fn current_user(&self) -> Result<User, AppError> {
        let url = format!("{}/user", self.base_url);
        let json = self.get_json::<Value>(&url).await?;
        Ok(Self::map_user(&json))
    }

    async fn list_repos(&self, page: u32) -> Result<Paginated<RepoSummary>, AppError> {
        let url = format!("{}/user/repos?per_page=100&page={}", self.base_url, page);
        let resp = self
            .client
            .raw_client()
            .get(&url)
            .header("Authorization", &self.auth_header())
            .header("User-Agent", "mergebeacon")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;
        let total_pages = Self::parse_last_page(resp.headers().get("link").and_then(|value| value.to_str().ok()), page);
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("GitHub API {status} ({url}): {body}")));
        }
        let items: Vec<Value> = resp.json().await?;
        let total_count =
            if page == total_pages { (page.saturating_sub(1) * 100) + items.len() as u32 } else { total_pages * 100 };

        let mut repos: Vec<RepoSummary> = Vec::with_capacity(items.len());
        for r in &items {
            let full_name = r["full_name"].as_str().unwrap_or("");
            let parts: Vec<&str> = full_name.splitn(2, '/').collect();
            let fork = r["fork"].as_bool().unwrap_or(false);
            let (parent_full_name, parent_owner) = if fork {
                let mut pn = r["parent"]["full_name"].as_str().map(|s| s.to_string());
                let mut po = r["parent"]["owner"]["login"].as_str().map(|s| s.to_string());
                eprintln!("[mergebeacon] fork repo: {} parent_full_name={:?} parent_owner={:?}", full_name, pn, po);
                // Fallback: fetch repo detail if parent info missing from list endpoint
                if pn.is_none() || po.is_none() {
                    let detail_url = format!("{}/repos/{}", self.base_url, full_name);
                    if let Ok(detail) = self.get_json::<Value>(&detail_url).await {
                        pn = detail["parent"]["full_name"].as_str().map(|s| s.to_string());
                        po = detail["parent"]["owner"]["login"].as_str().map(|s| s.to_string());
                        eprintln!(
                            "[mergebeacon] fork repo fallback: {} parent_full_name={:?} parent_owner={:?}",
                            full_name, pn, po
                        );
                    }
                }
                (pn, po)
            } else {
                (None, None)
            };
            let owner_type = r["owner"]["type"].as_str().unwrap_or("user").to_lowercase();
            let owner_login = parts.first().unwrap_or(&"").to_string();
            repos.push(RepoSummary {
                id: r["id"].clone(),
                name: r["name"].as_str().unwrap_or("").to_string(),
                full_name: full_name.to_string(),
                owner: owner_login.clone(),
                owner_type,
                owner_display_name: owner_login,
                description: r["description"].as_str().unwrap_or("").to_string(),
                private: r["private"].as_bool().unwrap_or(false),
                fork,
                parent_full_name,
                parent_owner,
            });
        }

        // Fetch org display names from API
        self.resolve_org_display_names(&mut repos).await;

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
        // GitHub API only supports state=open|closed|all; "merged" is a subset of "closed"
        let api_state = match state {
            PrState::Merged => "closed",
            other => other.as_str(),
        };
        let url = format!(
            "{}/repos/{}/{}/pulls?state={}&per_page={}&page={}",
            self.base_url, owner, repo, api_state, per_page, page
        );

        // Use raw request to read Link header for pagination
        let resp = self
            .client
            .raw_client()
            .get(&url)
            .header("Authorization", &self.auth_header())
            .header("User-Agent", "mergebeacon")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await?;

        let link_header = resp.headers().get("link").and_then(|v| v.to_str().ok());
        let last_page = Self::parse_last_page(link_header, page);

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api(format!("GitHub API {} ({}): {}", status, url, body)));
        }

        let items: Vec<Value> = resp.json().await?;

        let all_prs: Vec<PrSummary> = items
            .iter()
            .map(|pr| PrSummary {
                number: pr["number"].as_u64().unwrap_or(0),
                title: pr["title"].as_str().unwrap_or("").to_string(),
                author: Self::map_user(&pr["user"]),
                state: Self::map_pr_state(pr["state"].as_str().unwrap_or(""), !pr["merged_at"].is_null()),
                created_at: pr["created_at"].as_str().unwrap_or("").to_string(),
                updated_at: pr["updated_at"].as_str().unwrap_or("").to_string(),
                labels: pr["labels"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|l| l["name"].as_str().map(String::from)).collect())
                    .unwrap_or_default(),
            })
            .collect();

        // Filter by requested state (needed because GitHub groups merged into closed)
        let prs: Vec<PrSummary> = match state {
            PrState::Merged => all_prs.into_iter().filter(|p| matches!(p.state, PrState::Merged)).collect(),
            PrState::Closed => all_prs.into_iter().filter(|p| matches!(p.state, PrState::Closed)).collect(),
            _ => all_prs,
        };

        let total_count = if prs.is_empty() {
            0
        } else if (prs.len() as u32) < per_page || page >= last_page {
            (page - 1) * per_page + prs.len() as u32
        } else {
            last_page * per_page
        };

        Ok(Paginated { items: prs, page, total_pages: last_page, total_count })
    }

    async fn get_pull_request(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrDetail, AppError> {
        let url = format!("{}/repos/{}/{}/pulls/{}", self.base_url, owner, repo, pr_number);
        let json = self.get_json::<Value>(&url).await?;

        let summary = PrSummary {
            number: json["number"].as_u64().unwrap_or(0),
            title: json["title"].as_str().unwrap_or("").to_string(),
            author: Self::map_user(&json["user"]),
            state: Self::map_pr_state(json["state"].as_str().unwrap_or(""), !json["merged_at"].is_null()),
            created_at: json["created_at"].as_str().unwrap_or("").to_string(),
            updated_at: json["updated_at"].as_str().unwrap_or("").to_string(),
            labels: json["labels"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|l| l["name"].as_str().map(String::from)).collect())
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

    async fn get_pr_diff(&self, owner: &str, repo: &str, pr_number: u64) -> Result<(String, Vec<PrFile>), AppError> {
        // Get unified diff
        let diff_url = format!("{}/repos/{}/{}/pulls/{}", self.base_url, owner, repo, pr_number);
        let diff = self.get_text(&diff_url).await?;

        // Get files list
        let files_url = format!("{}/repos/{}/{}/pulls/{}/files?per_page=300", self.base_url, owner, repo, pr_number);
        let files_json: Vec<Value> = self.get_json(&files_url).await?;

        let files: Vec<PrFile> = files_json
            .iter()
            .map(|f| PrFile {
                filename: f["filename"].as_str().unwrap_or("").to_string(),
                status: match f["status"].as_str().unwrap_or("") {
                    "added" => FileStatus::Added,
                    "removed" => FileStatus::Removed,
                    "renamed" => FileStatus::Renamed,
                    _ => FileStatus::Modified,
                },
                patch: f["patch"].as_str().unwrap_or("").to_string(),
                additions: f["additions"].as_u64().unwrap_or(0) as u32,
                deletions: f["deletions"].as_u64().unwrap_or(0) as u32,
            })
            .collect();

        Ok((diff, files))
    }

    async fn create_review(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        body: &str,
        event: &ReviewEvent,
        comments: &[ReviewCommentPosition],
    ) -> Result<Review, AppError> {
        let url = format!("{}/repos/{}/{}/pulls/{}/reviews", self.base_url, owner, repo, pr_number);
        let event_str = match event {
            ReviewEvent::Approve => "APPROVE",
            ReviewEvent::Comment => "COMMENT",
            ReviewEvent::RequestChanges => "REQUEST_CHANGES",
        };

        let payload = if comments.is_empty() {
            serde_json::json!({
                "body": body,
                "event": event_str,
            })
        } else {
            let gh_comments: Vec<serde_json::Value> = comments
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "path": c.path,
                        "position": c.position,
                        "body": c.body,
                    })
                })
                .collect();
            serde_json::json!({
                "body": body,
                "event": event_str,
                "comments": gh_comments,
            })
        };

        let json = self.post_json(&url, &payload).await?;

        Ok(Review {
            id: json["id"].clone(),
            body: json["body"].as_str().unwrap_or("").to_string(),
            state: json["state"].as_str().unwrap_or("").to_string(),
            author: Self::map_user(&json["user"]),
            submitted_at: json["submitted_at"].as_str().unwrap_or("").to_string(),
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
        let url = format!("{}/repos/{}/{}/pulls/{}/comments", self.base_url, owner, repo, pr_number);
        let gh_side = match side {
            "left" => "LEFT",
            _ => "RIGHT",
        };
        let payload = if let Some(sl) = start_line {
            serde_json::json!({
                "body": body,
                "commit_id": commit_id,
                "path": path,
                "start_line": sl,
                "line": line,
                "side": gh_side,
            })
        } else {
            serde_json::json!({
                "body": body,
                "commit_id": commit_id,
                "path": path,
                "line": line,
                "side": gh_side,
            })
        };
        let c: Value = self.post_json(&url, &payload).await?;
        Ok(PrComment {
            id: c["id"].clone(),
            body: c["body"].as_str().unwrap_or("").to_string(),
            path: c["path"].as_str().unwrap_or("").to_string(),
            line: c["line"].as_u64().map(|n| n as u32),
            start_line: c["start_line"].as_u64().map(|n| n as u32),
            author: Self::map_user(&c["user"]),
            created_at: c["created_at"].as_str().unwrap_or("").to_string(),
            commit_id: c["commit_id"].as_str().map(|s| s.to_string()),
            original_commit_id: c["original_commit_id"].as_str().map(|s| s.to_string()),
            original_line: c["original_line"].as_u64().map(|n| n as u32),
            original_start_line: c["original_start_line"].as_u64().map(|n| n as u32),
            diff_hunk: c["diff_hunk"].as_str().map(|s| s.to_string()),
        })
    }

    async fn list_pr_comments(&self, owner: &str, repo: &str, pr_number: u64) -> Result<Vec<PrComment>, AppError> {
        let url = format!("{}/repos/{}/{}/pulls/{}/comments?per_page=100", self.base_url, owner, repo, pr_number);
        let items: Vec<Value> = self.get_json(&url).await?;
        let comments = items
            .iter()
            .map(|c| PrComment {
                id: c["id"].clone(),
                body: c["body"].as_str().unwrap_or("").to_string(),
                path: c["path"].as_str().unwrap_or("").to_string(),
                line: c["line"].as_u64().map(|n| n as u32),
                start_line: c["start_line"].as_u64().map(|n| n as u32),
                author: Self::map_user(&c["user"]),
                created_at: c["created_at"].as_str().unwrap_or("").to_string(),
                commit_id: c["commit_id"].as_str().map(|s| s.to_string()),
                original_commit_id: c["original_commit_id"].as_str().map(|s| s.to_string()),
                original_line: c["original_line"].as_u64().map(|n| n as u32),
                original_start_line: c["original_start_line"].as_u64().map(|n| n as u32),
                diff_hunk: c["diff_hunk"].as_str().map(|s| s.to_string()),
            })
            .collect();
        Ok(comments)
    }

    async fn list_reviews(&self, owner: &str, repo: &str, pr_number: u64) -> Result<Vec<Review>, AppError> {
        let url = format!("{}/repos/{}/{}/pulls/{}/reviews", self.base_url, owner, repo, pr_number);
        let items: Vec<Value> = self.get_json(&url).await?;

        let reviews = items
            .iter()
            .map(|r| Review {
                id: r["id"].clone(),
                body: r["body"].as_str().unwrap_or("").to_string(),
                state: r["state"].as_str().unwrap_or("").to_string(),
                author: Self::map_user(&r["user"]),
                submitted_at: r["submitted_at"].as_str().unwrap_or("").to_string(),
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
        let items: Vec<Value> = self.get_json(&url).await?;

        let issues: Vec<IssueSummary> = items
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
                    .map(|arr| arr.iter().filter_map(|l| l["name"].as_str().map(String::from)).collect())
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
        let url = format!("{}/repos/{}/{}/issues", self.base_url, owner, repo);
        let payload = serde_json::json!({
            "title": title,
            "body": body,
            "labels": labels,
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
                .map(|arr| arr.iter().filter_map(|l| l["name"].as_str().map(String::from)).collect())
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
        let url = format!("{}/repos/{}/{}/pulls/{}/merge", self.base_url, owner, repo, pr_number);
        let method = match strategy {
            MergeStrategy::Merge => "merge",
            MergeStrategy::Squash => "squash",
            MergeStrategy::Rebase => "rebase",
        };
        let mut payload = serde_json::json!({
            "merge_method": method,
            "sha": sha,
        });
        if let Some(t) = commit_title {
            payload["commit_title"] = serde_json::Value::String(t);
        }
        if let Some(m) = commit_message {
            payload["commit_message"] = serde_json::Value::String(m);
        }
        let json = self.put_json(&url, &payload).await?;
        Ok(PrMergeResult {
            merged: json["merged"].as_bool().unwrap_or(false),
            sha: json["sha"].as_str().unwrap_or("").to_string(),
            message: json["message"].as_str().unwrap_or("").to_string(),
        })
    }

    async fn close_pull_request(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrState, AppError> {
        let url = format!("{}/repos/{}/{}/pulls/{}", self.base_url, owner, repo, pr_number);
        let payload = serde_json::json!({ "state": "closed" });
        let json = self.patch_json(&url, &payload).await?;
        let state = Self::map_pr_state(json["state"].as_str().unwrap_or(""), !json["merged_at"].is_null());
        Ok(state)
    }

    async fn reopen_pull_request(&self, owner: &str, repo: &str, pr_number: u64) -> Result<PrState, AppError> {
        let url = format!("{}/repos/{}/{}/pulls/{}", self.base_url, owner, repo, pr_number);
        let payload = serde_json::json!({ "state": "open" });
        let json = self.patch_json(&url, &payload).await?;
        let state = Self::map_pr_state(json["state"].as_str().unwrap_or(""), !json["merged_at"].is_null());
        Ok(state)
    }

    async fn close_issue(&self, owner: &str, repo: &str, issue_number: u64) -> Result<(), AppError> {
        let url = format!("{}/repos/{}/{}/issues/{}", self.base_url, owner, repo, issue_number);
        let payload = serde_json::json!({ "state": "closed" });
        self.patch_json(&url, &payload).await?;
        Ok(())
    }
}
