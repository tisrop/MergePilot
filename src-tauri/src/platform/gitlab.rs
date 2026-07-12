use async_trait::async_trait;
use serde_json::Value;

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
        Self {
            client,
            token,
            base_url: "https://gitlab.com/api/v4".to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url.trim_end_matches('/').to_string();
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
            .header("User-Agent", "mergepilot")
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
            .header("User-Agent", "mergepilot")
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
            login: json["username"].as_str().unwrap_or("").to_string(),
            name: json["name"].as_str().unwrap_or("").to_string(),
            avatar_url: json["avatar_url"].as_str().unwrap_or("").to_string(),
        }
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
        let url = format!(
            "{}/projects?membership=true&per_page=100&page={}",
            self.base_url, page
        );
        let items: Vec<Value> = self.get_json(&url).await?;

        let repos: Vec<RepoSummary> = items
            .iter()
            .map(|r| {
                let path = r["path_with_namespace"].as_str().unwrap_or("");
                let fork = r["forked_from_project"].is_object();
                let (parent_full_name, parent_owner) = if fork {
                    let parent_name = r["forked_from_project"]["path_with_namespace"]
                        .as_str()
                        .map(|s| s.to_string());
                    let parent_owner = r["forked_from_project"]["namespace"]["path"]
                        .as_str()
                        .map(|s| s.to_string());
                    (parent_name, parent_owner)
                } else {
                    (None, None)
                };
                RepoSummary {
                    id: r["id"].clone(),
                    name: r["name"].as_str().unwrap_or("").to_string(),
                    full_name: path.to_string(),
                    owner: r["namespace"]["path"].as_str().unwrap_or("").to_string(),
                    description: r["description"].as_str().unwrap_or("").to_string(),
                    private: r["visibility"].as_str().unwrap_or("") != "public",
                    fork,
                    parent_full_name,
                    parent_owner,
                }
            })
            .collect();

        Ok(Paginated {
            items: repos,
            page,
            total_pages: 1,
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
            .header("User-Agent", "mergepilot")
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
                state: match (
                    mr["state"].as_str().unwrap_or(""),
                    mr["merged_at"].is_null(),
                ) {
                    (_, false) => PrState::Merged,
                    ("closed", _) => PrState::Closed,
                    _ => PrState::Open,
                },
                created_at: mr["created_at"].as_str().unwrap_or("").to_string(),
                updated_at: mr["updated_at"].as_str().unwrap_or("").to_string(),
                labels: mr["labels"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|l| l.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
            .collect();

        Ok(Paginated {
            items: mrs,
            page,
            total_pages,
            total_count,
        })
    }

    async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<PrDetail, AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!(
            "{}/projects/{}/merge_requests/{}",
            self.base_url, project_id, pr_number
        );
        let json = self.get_json::<Value>(&url).await?;

        let summary = PrSummary {
            number: json["iid"].as_u64().unwrap_or(0),
            title: json["title"].as_str().unwrap_or("").to_string(),
            author: Self::map_user(&json["author"]),
            state: match (
                json["state"].as_str().unwrap_or(""),
                json["merged_at"].is_null(),
            ) {
                (_, false) => PrState::Merged,
                ("closed", _) => PrState::Closed,
                _ => PrState::Open,
            },
            created_at: json["created_at"].as_str().unwrap_or("").to_string(),
            updated_at: json["updated_at"].as_str().unwrap_or("").to_string(),
            labels: json["labels"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|l| l.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
        };

        Ok(PrDetail {
            summary,
            body: json["description"].as_str().unwrap_or("").to_string(),
            source_branch: json["source_branch"].as_str().unwrap_or("").to_string(),
            target_branch: json["target_branch"].as_str().unwrap_or("").to_string(),
            mergeable: None,
            head_sha: String::new(),
        })
    }

    async fn get_pr_diff(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<(String, Vec<PrFile>), AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!(
            "{}/projects/{}/merge_requests/{}/changes",
            self.base_url, project_id, pr_number
        );
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

        // Build unified diff from individual file diffs
        let diff = changes
            .iter()
            .map(|f| f.patch.clone())
            .collect::<Vec<_>>()
            .join("");

        Ok((diff, changes))
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
        let project_id = urlencoding(owner, repo);

        // GitLab uses notes for reviews; approval API is separate
        let url = format!(
            "{}/projects/{}/merge_requests/{}/notes",
            self.base_url, project_id, pr_number
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
            author: Self::map_user(&json["author"]),
            submitted_at: json["created_at"].as_str().unwrap_or("").to_string(),
        })
    }

    async fn create_pr_comment(
        &self,
        _owner: &str,
        _repo: &str,
        _pr_number: u64,
        _commit_id: &str,
        _path: &str,
        _start_line: Option<u32>,
        _line: u32,
        _side: &str,
        _body: &str,
    ) -> Result<PrComment, AppError> {
        // TODO: implement GitLab MR comment
        Err(AppError::NotImplemented("GitLab inline comments".into()))
    }

    async fn list_pr_comments(
        &self,
        _owner: &str,
        _repo: &str,
        _pr_number: u64,
    ) -> Result<Vec<PrComment>, AppError> {
        Ok(Vec::new())
    }

    async fn list_reviews(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<Vec<Review>, AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!(
            "{}/projects/{}/merge_requests/{}/notes?per_page=100",
            self.base_url, project_id, pr_number
        );
        let items: Vec<Value> = self.get_json(&url).await?;

        let reviews = items
            .iter()
            .filter(|n| !n["system"].as_bool().unwrap_or(false))
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
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|l| l.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                created_at: i["created_at"].as_str().unwrap_or("").to_string(),
            })
            .collect();

        Ok(Paginated {
            items: issues,
            page,
            total_pages: 1,
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
                .map(|arr| {
                    arr.iter()
                        .filter_map(|l| l.as_str().map(String::from))
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
        _commit_title: Option<String>,
        commit_message: Option<String>,
        sha: &str,
    ) -> Result<PrMergeResult, AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!(
            "{}/projects/{}/merge_requests/{}/merge",
            self.base_url, project_id, pr_number
        );
        let squash = matches!(strategy, MergeStrategy::Squash);
        let mut payload = serde_json::json!({
            "squash": squash,
            "sha": sha,
        });
        if let Some(m) = commit_message {
            payload["merge_commit_message"] = serde_json::Value::String(m);
        }
        let json = self.put_json(&url, &payload).await?;
        Ok(PrMergeResult {
            merged: true,
            sha: json["id"].as_str().unwrap_or("").to_string(),
            message: String::new(),
        })
    }

    async fn close_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<PrState, AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!(
            "{}/projects/{}/merge_requests/{}",
            self.base_url, project_id, pr_number
        );
        let payload = serde_json::json!({ "state_event": "close" });
        self.put_json(&url, &payload).await?;
        Ok(PrState::Closed)
    }

    async fn reopen_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<PrState, AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!(
            "{}/projects/{}/merge_requests/{}",
            self.base_url, project_id, pr_number
        );
        let payload = serde_json::json!({ "state_event": "reopen" });
        self.put_json(&url, &payload).await?;
        Ok(PrState::Open)
    }

    async fn close_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
    ) -> Result<(), AppError> {
        let project_id = urlencoding(owner, repo);
        let url = format!(
            "{}/projects/{}/issues/{}",
            self.base_url, project_id, issue_number
        );
        let payload = serde_json::json!({ "state_event": "close" });
        self.put_json(&url, &payload).await?;
        Ok(())
    }
}

fn urlencoding(owner: &str, repo: &str) -> String {
    format!("{}%2F{}", owner, repo)
}
