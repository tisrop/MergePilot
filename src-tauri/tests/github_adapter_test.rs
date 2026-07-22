use mergebeacon_lib::http_client::HttpClient;
use mergebeacon_lib::models::{
    MergeQueueState, PrCreatePreviewRequest, PrCreateRequest, PrDetail, PrMetadataField, PrMetadataPermissions,
    PrMetadataUpdate, PrMilestone, PrState, PrSummary, ReadinessState, ReviewInboxCategory, ReviewInboxRelationship,
    User,
};
use mergebeacon_lib::platform::{github::GitHubAdapter, GitPlatform};
use wiremock::matchers::{body_json, body_string_contains, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn github_search_issue(number: u64, title: &str, updated_at: &str) -> serde_json::Value {
    serde_json::json!({
        "number": number,
        "node_id": format!("PR_node_{number}"),
        "title": title,
        "repository_url": "https://api.github.com/repos/octocat/hello-world",
        "created_at": "2025-01-01T00:00:00Z",
        "updated_at": updated_at,
        "user": { "id": 1, "login": "dev1", "name": "", "avatar_url": "" },
        "labels": [{ "name": "review" }]
    })
}

fn github_review_comment(id: u64, body: &str) -> serde_json::Value {
    serde_json::json!({
        "id": format!("PRRC_{id}"),
        "fullDatabaseId": id.to_string(),
        "body": body,
        "path": "src/lib.rs",
        "line": 8,
        "startLine": null,
        "author": {
            "id": "U_reviewer",
            "login": "reviewer",
            "name": "Reviewer",
            "avatarUrl": "https://avatars.example.com/reviewer"
        },
        "createdAt": "2026-07-16T10:00:00Z",
        "commit": { "oid": "head" },
        "originalCommit": { "oid": "head" },
        "originalLine": 8,
        "originalStartLine": null,
        "diffHunk": "@@ -8 +8 @@",
        "replyTo": null
    })
}

#[tokio::test]
async fn test_github_current_user() {
    let mock_server = MockServer::start().await;

    // Mock /user endpoint
    Mock::given(method("GET"))
        .and(path("/user"))
        .and(header("Authorization", "Bearer test-token-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1,
            "login": "testuser",
            "name": "Test User",
            "avatar_url": "https://avatars.example.com/u/1"
        })))
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter = GitHubAdapter::new(client, "test-token-123".to_string()).with_base_url(mock_server.uri());

    let user = adapter.current_user().await.expect("should fetch user");
    assert_eq!(user.login, "testuser");
    assert_eq!(user.name, "Test User");
}

#[tokio::test]
async fn test_github_lists_branches_and_creates_draft_from_fork() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/branches"))
        .and(query_param("per_page", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "name": "main" },
            { "name": "feature" }
        ])))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "default_branch": "feature"
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/repos/octocat/hello-world/pulls"))
        .and(body_json(serde_json::json!({
            "title": "Add feature",
            "body": "Description",
            "head": "contributor:feature",
            "base": "main",
            "draft": true
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({ "number": 51 })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/compare/main...contributor%3Afeature"))
        .and(query_param("per_page", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "commits": [{
                "sha": "abc123",
                "commit": {
                    "message": "Add feature\n\nDetails",
                    "author": { "name": "Alice", "date": "2026-07-19T10:00:00Z" }
                }
            }],
            "files": [{
                "filename": "src/main.rs",
                "status": "modified",
                "patch": "@@ -1 +1 @@\n-old\n+new",
                "additions": 1,
                "deletions": 1
            }]
        })))
        .mount(&mock_server)
        .await;
    let adapter = GitHubAdapter::new(HttpClient::new(), "token".into()).with_base_url(mock_server.uri());

    let branch_options = adapter.list_branches("octocat", "hello-world").await.unwrap();
    assert_eq!(branch_options.branches, vec!["main", "feature"]);
    assert_eq!(branch_options.default_branch.as_deref(), Some("feature"));
    let preview = adapter
        .preview_pull_request(
            "octocat",
            "hello-world",
            &PrCreatePreviewRequest {
                source_owner: "contributor".into(),
                source_repo: "hello-world".into(),
                source_branch: "feature".into(),
                target_branch: "main".into(),
                commit_sha: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(preview.commits[0].title, "Add feature");
    assert_eq!(preview.files[0].filename, "src/main.rs");
    assert!(!preview.incomplete);
    let number = adapter
        .create_pull_request(
            "octocat",
            "hello-world",
            &PrCreateRequest {
                source_owner: "contributor".into(),
                source_repo: "hello-world".into(),
                source_branch: "feature".into(),
                target_branch: "main".into(),
                title: "Add feature".into(),
                body: "Description".into(),
                draft: true,
                reviewers: vec![],
                assignees: vec![],
                labels: vec![],
            },
        )
        .await
        .unwrap();
    assert_eq!(number, 51);
}

#[tokio::test]
async fn test_github_create_compare_marks_an_incomplete_preview() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/compare/main...feature"))
        .and(query_param("per_page", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_commits": 2,
            "commits": [{
                "sha": "abc123",
                "commit": {
                    "message": "First commit",
                    "author": { "name": "Alice", "date": "2026-07-19T10:00:00Z" }
                }
            }],
            "files": [{
                "filename": "src/main.rs",
                "status": "modified",
                "patch": "@@ -1 +1 @@\n-old\n+new"
            }]
        })))
        .mount(&mock_server)
        .await;
    let adapter = GitHubAdapter::new(HttpClient::new(), "token".into()).with_base_url(mock_server.uri());

    let preview = adapter
        .preview_pull_request(
            "octocat",
            "hello-world",
            &PrCreatePreviewRequest {
                source_owner: "octocat".into(),
                source_repo: "hello-world".into(),
                source_branch: "feature".into(),
                target_branch: "main".into(),
                commit_sha: None,
            },
        )
        .await
        .unwrap();

    assert!(preview.incomplete);
    assert_eq!(preview.commits.len(), 1);
    assert_eq!(preview.files.len(), 1);
}

#[tokio::test]
async fn test_github_create_compare_collects_all_commit_pages() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/compare/main...feature"))
        .and(query_param("per_page", "100"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_commits": 2,
            "commits": [{
                "sha": "abc123",
                "commit": {
                    "message": "First commit",
                    "author": { "name": "Alice", "date": "2026-07-19T10:00:00Z" }
                }
            }],
            "files": [{
                "filename": "src/main.rs",
                "status": "modified",
                "patch": "@@ -1 +1 @@\n-old\n+new"
            }]
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/compare/main...feature"))
        .and(query_param("per_page", "100"))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_commits": 2,
            "commits": [{
                "sha": "def456",
                "commit": {
                    "message": "Second commit",
                    "author": { "name": "Bob", "date": "2026-07-19T11:00:00Z" }
                }
            }]
        })))
        .mount(&mock_server)
        .await;
    let adapter = GitHubAdapter::new(HttpClient::new(), "token".into()).with_base_url(mock_server.uri());

    let preview = adapter
        .preview_pull_request(
            "octocat",
            "hello-world",
            &PrCreatePreviewRequest {
                source_owner: "octocat".into(),
                source_repo: "hello-world".into(),
                source_branch: "feature".into(),
                target_branch: "main".into(),
                commit_sha: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(preview.commits.iter().map(|commit| commit.sha.as_str()).collect::<Vec<_>>(), vec!["abc123", "def456"]);
    assert_eq!(preview.files.len(), 1);
    assert!(!preview.incomplete);
}

#[tokio::test]
async fn test_github_lists_all_branch_pages() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/branches"))
        .and(query_param("per_page", "100"))
        .and(query_param("page", "1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header(
                    "link",
                    "<https://api.github.test/repos/octocat/hello-world/branches?per_page=100&page=2>; rel=\"next\", <https://api.github.test/repos/octocat/hello-world/branches?per_page=100&page=2>; rel=\"last\"",
                )
                .set_body_json(serde_json::json!([{ "name": "main" }])),
        )
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/branches"))
        .and(query_param("per_page", "100"))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([{ "name": "feature-101" }])))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "default_branch": "main"
        })))
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "token".into()).with_base_url(mock_server.uri());
    let branches = adapter.list_branches("octocat", "hello-world").await.unwrap();

    assert_eq!(branches.branches, vec!["main", "feature-101"]);
}

#[tokio::test]
async fn test_github_lists_repository_labels() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/labels"))
        .and(query_param("per_page", "100"))
        .and(query_param("page", "1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header(
                    "link",
                    "<https://api.github.test/repos/octocat/hello-world/labels?per_page=100&page=2>; rel=\"next\", <https://api.github.test/repos/octocat/hello-world/labels?per_page=100&page=2>; rel=\"last\"",
                )
                .set_body_json(serde_json::json!([{ "name": "bug", "color": "d73a4a", "description": "Needs fixing" }])),
        )
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/labels"))
        .and(query_param("per_page", "100"))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([{ "name": "feature" }])))
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "token".into()).with_base_url(mock_server.uri());
    let labels = adapter.list_labels("octocat", "hello-world").await.unwrap();

    assert_eq!(labels.iter().map(|label| label.name.as_str()).collect::<Vec<_>>(), vec!["bug", "feature"]);
    assert_eq!(labels[0].color.as_deref(), Some("d73a4a"));
    assert_eq!(labels[0].description.as_deref(), Some("Needs fixing"));
}

#[tokio::test]
async fn test_github_lists_pr_participant_suggestions() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/assignees"))
        .and(query_param("per_page", "100"))
        .and(query_param("page", "1"))
        .and(header("Authorization", "Bearer token"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header(
                    "link",
                    "<https://api.github.test/repos/octocat/hello-world/assignees?per_page=100&page=2>; rel=\"next\", <https://api.github.test/repos/octocat/hello-world/assignees?per_page=100&page=2>; rel=\"last\"",
                )
                .set_body_json(serde_json::json!([{
                    "id": 1,
                    "login": "alice",
                    "avatar_url": "https://example.com/alice.png"
                }])),
        )
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/assignees"))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([{
            "id": 2,
            "login": "bob",
            "avatar_url": ""
        }])))
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "token".into()).with_base_url(mock_server.uri());
    let users = adapter.list_pr_participant_suggestions("octocat", "hello-world").await.unwrap();

    assert_eq!(users.iter().map(|user| user.login.as_str()).collect::<Vec<_>>(), vec!["alice", "bob"]);
    assert_eq!(users[0].avatar_url, "https://example.com/alice.png");
}

#[tokio::test]
async fn test_github_previews_a_single_commit() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/contributor/hello-world/commits/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "sha": "abc123",
            "commit": {
                "message": "Only this commit\n\nDetails",
                "author": { "name": "Alice", "date": "2026-07-19T10:00:00Z" }
            },
            "files": [{
                "filename": "src/commit.rs",
                "status": "modified",
                "patch": "@@ -1 +1 @@\n-old\n+new",
                "additions": 1,
                "deletions": 1
            }]
        })))
        .mount(&mock_server)
        .await;
    let adapter = GitHubAdapter::new(HttpClient::new(), "token".into()).with_base_url(mock_server.uri());

    let preview = adapter
        .preview_pull_request(
            "octocat",
            "hello-world",
            &PrCreatePreviewRequest {
                source_owner: "contributor".into(),
                source_repo: "hello-world".into(),
                source_branch: "feature".into(),
                target_branch: "main".into(),
                commit_sha: Some("abc123".into()),
            },
        )
        .await
        .unwrap();

    assert_eq!(preview.commits[0].title, "Only this commit");
    assert_eq!(preview.files[0].filename, "src/commit.rs");
}

#[tokio::test]
async fn test_github_list_prs() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls"))
        .and(query_param("state", "open"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "number": 42,
                "node_id": "PR_node_42",
                "title": "Fix bug in parser",
                "state": "open",
                "merged_at": null,
                "created_at": "2025-01-01T00:00:00Z",
                "updated_at": "2025-01-02T00:00:00Z",
                "user": { "id": 1, "login": "dev1", "name": "", "avatar_url": "" },
                "labels": [{ "name": "bug" }, { "name": "priority" }]
            },
            {
                "number": 43,
                "node_id": "PR_node_43",
                "title": "Add new feature",
                "state": "closed",
                "merged_at": "2025-01-03T00:00:00Z",
                "created_at": "2025-01-02T00:00:00Z",
                "updated_at": "2025-01-03T00:00:00Z",
                "user": { "id": 2, "login": "dev2", "name": "", "avatar_url": "" },
                "labels": []
            }
        ])))
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains("query ReviewInboxStatuses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "nodes": [{
                    "id": "PR_node_42",
                    "isDraft": false,
                    "mergeable": "MERGEABLE",
                    "mergeStateStatus": "CLEAN",
                    "reviewDecision": "APPROVED",
                    "commits": { "nodes": [{ "commit": { "statusCheckRollup": { "state": "SUCCESS" } } }] }
                }]
            }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter = GitHubAdapter::new(client, "test-token".to_string()).with_base_url(mock_server.uri());

    let result = adapter
        .list_pull_requests("octocat", "hello-world", &mergebeacon_lib::models::PrState::Open, 1, 20)
        .await
        .expect("should list PRs");

    assert_eq!(result.items.len(), 2);
    assert_eq!(result.items[0].number, 42);
    assert_eq!(result.items[0].title, "Fix bug in parser");
    assert!(matches!(result.items[0].state, mergebeacon_lib::models::PrState::Open));
    let status = result.items[0].status.as_ref().expect("open PR should expose status summary");
    assert_eq!(status.status, ReadinessState::Ready);
    assert_eq!(status.approvals_status, ReadinessState::Ready);
    assert_eq!(status.checks_status, ReadinessState::Ready);
    assert_eq!(result.items[1].number, 43);
    // PR #43 has merged_at set, should be Merged
    assert!(matches!(result.items[1].state, mergebeacon_lib::models::PrState::Merged));
    assert!(result.items[1].status.is_none());

    let requests = mock_server.received_requests().await.expect("requests");
    let graphql = requests.iter().find(|request| request.url.path() == "/graphql").expect("GraphQL request");
    let body: serde_json::Value = serde_json::from_slice(&graphql.body).expect("GraphQL JSON body");
    assert_eq!(body["variables"]["ids"], serde_json::json!(["PR_node_42"]));
}

#[tokio::test]
async fn test_github_lists_pr_dependency_candidates() {
    let mock_server = MockServer::start().await;
    let pull = |number: u64, source: &str, target: &str| {
        serde_json::json!({
            "number": number,
            "title": format!("Stack PR {number}"),
            "state": "open",
            "merged_at": null,
            "head": { "ref": source, "repo": { "full_name": "octocat/hello-world" } },
            "base": { "ref": target, "repo": { "full_name": "octocat/hello-world" } }
        })
    };
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls/2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(pull(2, "feature-b", "feature-a")))
        .mount(&mock_server)
        .await;
    let neighbors = [
        ("base", "feature-b", vec![pull(3, "feature-c", "feature-b")]),
        ("head", "octocat:feature-a", vec![pull(1, "feature-a", "main")]),
        ("base", "feature-c", vec![pull(4, "feature-d", "feature-c")]),
        ("head", "octocat:feature-b", vec![pull(2, "feature-b", "feature-a")]),
        ("base", "feature-a", vec![pull(2, "feature-b", "feature-a")]),
        ("head", "octocat:main", Vec::new()),
        ("base", "feature-d", Vec::new()),
        ("head", "octocat:feature-c", vec![pull(3, "feature-c", "feature-b")]),
    ];
    for (filter, value, response) in neighbors {
        Mock::given(method("GET"))
            .and(path("/repos/octocat/hello-world/pulls"))
            .and(query_param("state", "all"))
            .and(query_param(filter, value))
            .and(query_param("per_page", "100"))
            .and(query_param("page", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .mount(&mock_server)
            .await;
    }
    let adapter = GitHubAdapter::new(HttpClient::new(), "token".into()).with_base_url(mock_server.uri());

    let result =
        adapter.list_pr_dependency_candidates("octocat", "hello-world", 2).await.expect("dependency candidates");
    assert!(!result.truncated);
    assert_eq!(result.current.number, 2);
    let candidates = result.items;
    assert_eq!(candidates.iter().map(|candidate| candidate.number).collect::<Vec<_>>(), vec![1, 2, 3, 4]);
    assert_eq!(candidates[3].source_branch, "feature-d");
    assert_eq!(candidates[3].target_branch, "feature-c");
}

#[tokio::test]
async fn test_github_reads_merge_queue_position_and_state() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains("query MergeQueueStatus"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "repository": {
                    "pullRequest": {
                        "baseRefName": "main",
                        "headRefOid": "head-sha",
                        "mergeQueue": { "entries": { "totalCount": 5 } },
                        "mergeQueueEntry": {
                            "position": 2,
                            "state": "AWAITING_CHECKS",
                            "enqueuedAt": "2026-07-21T01:00:00Z",
                            "estimatedTimeToMerge": 420,
                            "headCommit": { "oid": "queue-sha" }
                        }
                    }
                }
            }
        })))
        .mount(&mock_server)
        .await;
    let adapter = GitHubAdapter::new(HttpClient::new(), "token".into()).with_base_url(mock_server.uri());

    let status = adapter.get_pr_merge_queue_status("octocat", "hello-world", 42).await.unwrap();

    assert!(status.available);
    assert_eq!(status.state, MergeQueueState::Waiting);
    assert_eq!(status.position, Some(2));
    assert_eq!(status.total, Some(5));
    assert_eq!(status.target_branch.as_deref(), Some("main"));
    assert_eq!(status.head_sha.as_deref(), Some("queue-sha"));
    assert_eq!(status.estimated_time_seconds, Some(420));
}

#[tokio::test]
async fn test_github_reports_when_target_branch_has_no_merge_queue() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "repository": {
                    "pullRequest": {
                        "baseRefName": "main",
                        "headRefOid": "head-sha",
                        "mergeQueue": null,
                        "mergeQueueEntry": null
                    }
                }
            }
        })))
        .mount(&mock_server)
        .await;
    let adapter = GitHubAdapter::new(HttpClient::new(), "token".into()).with_base_url(mock_server.uri());

    let status = adapter.get_pr_merge_queue_status("octocat", "hello-world", 42).await.unwrap();

    assert!(!status.available);
    assert_eq!(status.state, MergeQueueState::Unknown);
    assert_eq!(status.failure_reason.as_deref(), Some("目标分支未启用 GitHub Merge Queue"));
}

#[tokio::test]
async fn test_github_reports_configured_queue_when_pr_is_not_queued() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "repository": {
                    "pullRequest": {
                        "baseRefName": "main",
                        "headRefOid": "head-sha",
                        "mergeQueue": { "entries": { "totalCount": 3 } },
                        "mergeQueueEntry": null
                    }
                }
            }
        })))
        .mount(&mock_server)
        .await;
    let adapter = GitHubAdapter::new(HttpClient::new(), "token".into()).with_base_url(mock_server.uri());

    let status = adapter.get_pr_merge_queue_status("octocat", "hello-world", 42).await.unwrap();

    assert!(status.available);
    assert_eq!(status.state, MergeQueueState::NotQueued);
    assert_eq!(status.total, Some(3));
}

#[tokio::test]
async fn test_github_reports_older_enterprise_schema_as_unavailable() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "errors": [{
                "message": "Unknown field MERGEQUEUEENTRY on PullRequest"
            }]
        })))
        .mount(&mock_server)
        .await;
    let adapter = GitHubAdapter::new(HttpClient::new(), "token".into()).with_base_url(mock_server.uri());

    let status = adapter.get_pr_merge_queue_status("octocat", "hello-world", 42).await.unwrap();

    assert!(!status.available);
    assert!(status.failure_reason.as_deref().unwrap_or_default().contains("Enterprise"));
}

#[tokio::test]
async fn test_github_keeps_unrelated_graphql_errors_actionable() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "errors": [{ "message": "Resource not accessible by integration" }]
        })))
        .mount(&mock_server)
        .await;
    let adapter = GitHubAdapter::new(HttpClient::new(), "token".into()).with_base_url(mock_server.uri());

    let error = adapter
        .get_pr_merge_queue_status("octocat", "hello-world", 42)
        .await
        .expect_err("unrelated GraphQL errors must remain visible");

    assert!(error.to_string().contains("Resource not accessible"));
}

#[tokio::test]
async fn test_github_list_prs_keeps_open_items_when_status_batch_fails() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls"))
        .and(query_param("state", "open"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([{
            "number": 42,
            "node_id": "PR_node_42",
            "title": "Fix bug in parser",
            "state": "open",
            "merged_at": null,
            "created_at": "2025-01-01T00:00:00Z",
            "updated_at": "2025-01-02T00:00:00Z",
            "user": { "id": 1, "login": "dev1", "name": "", "avatar_url": "" },
            "labels": []
        }])))
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let result = adapter
        .list_pull_requests("octocat", "hello-world", &mergebeacon_lib::models::PrState::Open, 1, 20)
        .await
        .expect("status failure must not fail PR listing");

    assert_eq!(result.items.len(), 1);
    let status = result.items[0].status.as_ref().expect("open PR should fall back to unknown status");
    assert_eq!(status.status, ReadinessState::Unknown);
    assert_eq!(status.approvals_status, ReadinessState::Unknown);
    assert_eq!(status.checks_status, ReadinessState::Unknown);
}

#[tokio::test]
async fn test_github_create_review() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/repos/octocat/hello-world/pulls/42/reviews"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": 1001,
            "body": "LGTM!",
            "state": "APPROVED",
            "user": { "id": 1, "login": "reviewer", "name": "", "avatar_url": "" },
            "submitted_at": "2025-01-04T00:00:00Z"
        })))
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter = GitHubAdapter::new(client, "test-token".to_string()).with_base_url(mock_server.uri());

    let review = adapter
        .create_review("octocat", "hello-world", 42, "LGTM!", &mergebeacon_lib::models::ReviewEvent::Approve, &[])
        .await
        .expect("should create review");

    assert_eq!(review.id, serde_json::json!(1001));
    assert_eq!(review.body, "LGTM!");
    assert_eq!(review.state, "APPROVED");
}

#[tokio::test]
async fn test_github_list_pr_comments_uses_review_threads_with_resolution_state() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "repository": {
                    "pullRequest": {
                        "reviewThreads": {
                            "nodes": [{
                                "id": "PRRT_thread_1",
                                "isResolved": false,
                                "viewerCanResolve": true,
                                "viewerCanUnresolve": false,
                                "diffSide": "RIGHT",
                                "comments": {
                                    "nodes": [
                                        {
                                            "id": "PRRC_root",
                                            "fullDatabaseId": "100",
                                            "body": "root",
                                            "path": "src/lib.rs",
                                            "line": 8,
                                            "startLine": null,
                                            "author": {
                                                "id": "U_reviewer",
                                                "login": "reviewer",
                                                "name": "Reviewer",
                                                "avatarUrl": "https://avatars.example.com/reviewer"
                                            },
                                            "createdAt": "2026-07-16T10:00:00Z",
                                            "commit": { "oid": "head" },
                                            "originalCommit": { "oid": "head" },
                                            "originalLine": 8,
                                            "originalStartLine": null,
                                            "diffHunk": "@@ -8 +8 @@",
                                            "replyTo": null
                                        },
                                        {
                                            "id": "PRRC_reply",
                                            "fullDatabaseId": "101",
                                            "body": "reply",
                                            "path": "src/lib.rs",
                                            "line": 8,
                                            "startLine": null,
                                            "author": {
                                                "id": "U_author",
                                                "login": "author",
                                                "name": "Author",
                                                "avatarUrl": "https://avatars.example.com/author"
                                            },
                                            "createdAt": "2026-07-16T11:00:00Z",
                                            "commit": { "oid": "head" },
                                            "originalCommit": { "oid": "head" },
                                            "originalLine": 8,
                                            "originalStartLine": null,
                                            "diffHunk": "@@ -8 +8 @@",
                                            "replyTo": {
                                                "id": "PRRC_root",
                                                "fullDatabaseId": "100"
                                            }
                                        }
                                    ],
                                    "pageInfo": { "hasNextPage": false, "endCursor": null }
                                }
                            }],
                            "pageInfo": { "hasNextPage": false, "endCursor": null }
                        }
                    }
                }
            }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let comments = adapter.list_pr_comments("octocat", "hello-world", 42).await.expect("should list review threads");

    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].id, serde_json::json!(100));
    assert_eq!(comments[0].thread_id, "PRRT_thread_1");
    assert_eq!(comments[0].side.as_deref(), Some("right"));
    assert_eq!(comments[0].resolved, Some(false));
    assert!(comments[0].resolvable);
    assert_eq!(comments[1].thread_id, "PRRT_thread_1");
    assert_eq!(comments[1].reply_to_id.as_deref(), Some("100"));
    assert_eq!(comments[1].author.name, "Author");

    let requests = mock_server.received_requests().await.expect("requests");
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).expect("GraphQL JSON body");
    let query = body["query"].as_str().unwrap_or_default();
    assert!(query.contains("reviewThreads"));
    assert!(query.contains("viewerCanUnresolve\n                                        diffSide"));
    assert!(!query.contains("startLine\n                                                diffSide"));
    assert_eq!(body["variables"]["owner"], "octocat");
    assert_eq!(body["variables"]["repo"], "hello-world");
    assert_eq!(body["variables"]["number"], 42);
}

#[tokio::test]
async fn test_github_list_pr_comments_paginates_threads_and_comments() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains("query ReviewThreads"))
        .and(body_string_contains("\"after\":null"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "repository": {
                    "pullRequest": {
                        "reviewThreads": {
                            "nodes": [{
                                "id": "PRRT_thread_1",
                                "isResolved": false,
                                "viewerCanResolve": true,
                                "viewerCanUnresolve": false,
                                "diffSide": "RIGHT",
                                "comments": {
                                    "nodes": [github_review_comment(100, "first comment page")],
                                    "pageInfo": { "hasNextPage": true, "endCursor": "comment_cursor_1" }
                                }
                            }],
                            "pageInfo": { "hasNextPage": true, "endCursor": "thread_cursor_1" }
                        }
                    }
                }
            }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains("query ReviewThreadComments"))
        .and(body_string_contains("\"after\":\"comment_cursor_1\""))
        .and(body_string_contains("\"threadId\":\"PRRT_thread_1\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "node": {
                    "comments": {
                        "nodes": [github_review_comment(101, "second comment page")],
                        "pageInfo": { "hasNextPage": false, "endCursor": null }
                    }
                }
            }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains("query ReviewThreads"))
        .and(body_string_contains("\"after\":\"thread_cursor_1\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "repository": {
                    "pullRequest": {
                        "reviewThreads": {
                            "nodes": [{
                                "id": "PRRT_thread_2",
                                "isResolved": true,
                                "viewerCanResolve": false,
                                "viewerCanUnresolve": true,
                                "diffSide": "LEFT",
                                "comments": {
                                    "nodes": [github_review_comment(200, "second thread page")],
                                    "pageInfo": { "hasNextPage": false, "endCursor": null }
                                }
                            }],
                            "pageInfo": { "hasNextPage": false, "endCursor": null }
                        }
                    }
                }
            }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let comments =
        adapter.list_pr_comments("octocat", "hello-world", 42).await.expect("should paginate review threads");

    assert_eq!(comments.len(), 3);
    assert_eq!(comments[0].id, serde_json::json!(100));
    assert_eq!(comments[1].id, serde_json::json!(101));
    assert_eq!(comments[0].thread_id, "PRRT_thread_1");
    assert_eq!(comments[1].thread_id, "PRRT_thread_1");
    assert_eq!(comments[2].id, serde_json::json!(200));
    assert_eq!(comments[2].thread_id, "PRRT_thread_2");
    assert_eq!(comments[2].resolved, Some(true));
    assert!(comments[2].resolvable);

    let requests = mock_server.received_requests().await.expect("requests");
    let comment_page_query = requests
        .iter()
        .filter_map(|request| serde_json::from_slice::<serde_json::Value>(&request.body).ok())
        .filter_map(|body| body["query"].as_str().map(str::to_string))
        .find(|query| query.contains("query ReviewThreadComments"))
        .expect("paginated comment query");
    assert!(!comment_page_query.contains("diffSide"));
}

#[tokio::test]
async fn test_github_list_pr_comments_reports_graphql_errors() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains("query ReviewThreads"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": null,
            "errors": [{ "message": "Resource not accessible by integration" }]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let error = adapter
        .list_pr_comments("octocat", "hello-world", 42)
        .await
        .expect_err("GraphQL errors must fail the review thread query");

    assert!(error.to_string().contains("GitHub GraphQL 评审线程查询失败"));
    assert!(error.to_string().contains("Resource not accessible by integration"));
}

#[tokio::test]
async fn test_github_resolves_and_reopens_review_thread() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains("mutation ResolveReviewThread"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "resolveReviewThread": {
                    "thread": { "id": "PRRT_thread_1", "isResolved": true }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains("mutation UnresolveReviewThread"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "unresolveReviewThread": {
                    "thread": { "id": "PRRT_thread_1", "isResolved": false }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    adapter
        .set_review_thread_resolved("octocat", "hello-world", 42, "PRRT_thread_1", true)
        .await
        .expect("should resolve review thread");
    adapter
        .set_review_thread_resolved("octocat", "hello-world", 42, "PRRT_thread_1", false)
        .await
        .expect("should reopen review thread");

    let requests = mock_server.received_requests().await.expect("requests");
    assert_eq!(requests.len(), 2);
    let request_bodies = requests
        .iter()
        .map(|request| serde_json::from_slice::<serde_json::Value>(&request.body).expect("GraphQL JSON body"))
        .collect::<Vec<_>>();
    assert!(request_bodies.iter().any(|body| {
        body["query"].as_str().unwrap_or_default().contains("mutation ResolveReviewThread")
            && body["variables"]["threadId"] == "PRRT_thread_1"
    }));
    assert!(request_bodies.iter().any(|body| {
        body["query"].as_str().unwrap_or_default().contains("mutation UnresolveReviewThread")
            && body["variables"]["threadId"] == "PRRT_thread_1"
    }));
}

#[tokio::test]
async fn test_github_replies_edits_and_deletes_review_comment() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v3/repos/octocat/hello-world/pulls/42/comments/100/replies"))
        .and(body_json(serde_json::json!({ "body": "回复" })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/v3/repos/octocat/hello-world/pulls/comments/100"))
        .and(body_json(serde_json::json!({ "body": "编辑后" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v3/repos/octocat/hello-world/pulls/comments/100"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".into())
        .with_base_url(format!("{}/api/v3", mock_server.uri()));
    adapter
        .reply_to_review_thread("octocat", "hello-world", 42, "thread-1", "100", "回复")
        .await
        .expect("reply should succeed");
    adapter
        .update_review_comment("octocat", "hello-world", 42, "thread-1", "100", "编辑后")
        .await
        .expect("edit should succeed");
    adapter
        .delete_review_comment("octocat", "hello-world", 42, "thread-1", "100")
        .await
        .expect("delete should succeed");
}

#[tokio::test]
async fn test_github_lists_viewed_files_with_pagination() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains("query PullRequestViewedFiles"))
        .and(body_string_contains("\"after\":null"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "repository": {
                    "pullRequest": {
                        "files": {
                            "nodes": [
                                { "path": "src/viewed.rs", "viewerViewedState": "VIEWED" },
                                { "path": "src/unviewed.rs", "viewerViewedState": "UNVIEWED" }
                            ],
                            "pageInfo": { "hasNextPage": true, "endCursor": "files_cursor_1" }
                        }
                    }
                }
            }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains("query PullRequestViewedFiles"))
        .and(body_string_contains("\"after\":\"files_cursor_1\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "repository": {
                    "pullRequest": {
                        "files": {
                            "nodes": [
                                { "path": "tests/viewed.rs", "viewerViewedState": "VIEWED" },
                                { "path": "tests/dismissed.rs", "viewerViewedState": "DISMISSED" }
                            ],
                            "pageInfo": { "hasNextPage": false, "endCursor": null }
                        }
                    }
                }
            }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let viewed = adapter.list_viewed_pr_files("octocat", "hello-world", 42).await.expect("should list viewed files");

    assert_eq!(viewed, vec!["src/viewed.rs", "tests/viewed.rs"]);
}

#[tokio::test]
async fn test_github_marks_and_unmarks_file_as_viewed() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains("query PullRequestNodeId"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "repository": {
                    "pullRequest": { "id": "PR_node_42" }
                }
            }
        })))
        .expect(2)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains("mutation MarkFileAsViewed"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "markFileAsViewed": {
                    "pullRequest": { "id": "PR_node_42" }
                }
            }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains("mutation UnmarkFileAsViewed"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "unmarkFileAsViewed": {
                    "pullRequest": { "id": "PR_node_42" }
                }
            }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    adapter
        .set_pr_file_viewed("octocat", "hello-world", 42, "src/lib.rs", true)
        .await
        .expect("should mark file as viewed");
    adapter
        .set_pr_file_viewed("octocat", "hello-world", 42, "src/lib.rs", false)
        .await
        .expect("should mark file as unviewed");

    let requests = mock_server.received_requests().await.expect("requests");
    assert_eq!(requests.len(), 4);
    let bodies = requests
        .iter()
        .map(|request| serde_json::from_slice::<serde_json::Value>(&request.body).expect("GraphQL JSON body"))
        .collect::<Vec<_>>();
    for operation in ["mutation MarkFileAsViewed", "mutation UnmarkFileAsViewed"] {
        let body = bodies
            .iter()
            .find(|body| body["query"].as_str().unwrap_or_default().contains(operation))
            .expect("viewed mutation request");
        assert_eq!(body["variables"]["pullRequestId"], "PR_node_42");
        assert_eq!(body["variables"]["path"], "src/lib.rs");
    }
}

#[tokio::test]
async fn test_github_create_issue() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/repos/octocat/hello-world/issues"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "number": 99,
            "title": "Memory leak in auth module",
            "body": "Steps to reproduce:\n1. Login\n2. Logout\n3. Memory grows",
            "state": "open",
            "user": { "id": 1, "login": "reporter", "name": "", "avatar_url": "" },
            "labels": [{ "name": "bug" }, { "name": "critical" }],
            "created_at": "2025-01-05T00:00:00Z",
            "updated_at": "2025-01-05T00:00:00Z"
        })))
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter = GitHubAdapter::new(client, "test-token".to_string()).with_base_url(mock_server.uri());

    let issue = adapter
        .create_issue(
            "octocat",
            "hello-world",
            "Memory leak in auth module",
            "Steps to reproduce:\n1. Login\n2. Logout\n3. Memory grows",
            &["bug".to_string(), "critical".to_string()],
        )
        .await
        .expect("should create issue");

    assert_eq!(issue.number, 99);
    assert_eq!(issue.title, "Memory leak in auth module");
    assert!(matches!(issue.state, mergebeacon_lib::models::IssueState::Open));
}

#[tokio::test]
async fn test_github_get_pr_diff() {
    let mock_server = MockServer::start().await;

    // Mock the diff text endpoint
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls/42"))
        .and(header("Accept", "application/vnd.github.v3.diff"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "diff --git a/src/main.rs b/src/main.rs\n@@ -1,3 +1,4 @@\n line1\n-line2\n+line2_new\n line3",
        ))
        .mount(&mock_server)
        .await;

    // Mock the files endpoint
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls/42/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "filename": "src/main.rs",
                "status": "modified",
                "patch": "diff --git a/src/main.rs b/src/main.rs\n@@ -1,3 +1,4 @@\n line1\n-line2\n+line2_new\n line3",
                "additions": 1,
                "deletions": 1
            }
        ])))
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter = GitHubAdapter::new(client, "test-token".to_string()).with_base_url(mock_server.uri());

    let (diff, files) = adapter.get_pr_diff("octocat", "hello-world", 42).await.expect("should get diff");

    assert!(diff.contains("src/main.rs"));
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].filename, "src/main.rs");
    assert_eq!(files[0].additions, 1);
    assert_eq!(files[0].deletions, 1);
}

#[tokio::test]
async fn test_github_compare_diff_normalizes_text_and_rename_files() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/compare/base-sha...head-sha"))
        .and(query_param("per_page", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "files": [
                {
                    "filename": "src/main.rs",
                    "status": "modified",
                    "patch": "@@ -1 +1 @@\n-old\n+new",
                    "additions": 1,
                    "deletions": 1
                },
                {
                    "filename": "src/new-name.rs",
                    "previous_filename": "src/old-name.rs",
                    "status": "renamed",
                    "patch": null,
                    "additions": 0,
                    "deletions": 0
                }
            ]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let (diff, files) = adapter
        .get_compare_diff("octocat", "hello-world", "base-sha", "head-sha")
        .await
        .expect("should compare commits");
    let patches = mergebeacon_lib::patch::standardize_patches(&diff, &files);

    assert_eq!(files.len(), 2);
    assert_eq!(files[0].filename, "src/main.rs");
    assert!(diff.contains("diff --git a/src/main.rs b/src/main.rs"));
    assert!(diff.contains("rename from src/old-name.rs\nrename to src/new-name.rs"));
    assert!(matches!(files[1].status, mergebeacon_lib::models::FileStatus::Renamed));
    assert_eq!(patches[1].old_path.as_deref(), Some("src/old-name.rs"));
    assert_eq!(patches[1].new_path.as_deref(), Some("src/new-name.rs"));
}

#[tokio::test]
async fn test_github_compare_diff_rejects_missing_files() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/compare/base...head"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let error =
        adapter.get_compare_diff("octocat", "hello-world", "base", "head").await.expect_err("missing files must fail");

    assert!(error.to_string().contains("缺少 files 字段"));
}

#[tokio::test]
async fn test_github_auth_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(ResponseTemplate::new(401).set_body_string(r#"{"message":"Bad credentials"}"#))
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter = GitHubAdapter::new(client, "invalid-token".to_string()).with_base_url(mock_server.uri());

    let result = adapter.current_user().await;
    assert!(result.is_err(), "should return error for bad credentials");
}

#[tokio::test]
async fn test_github_list_repos_with_fork() {
    let mock_server = wiremock::MockServer::start().await;

    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/user/repos"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "id": 1,
                "name": "linux",
                "full_name": "myname/linux",
                "private": false,
                "fork": true,
                "description": "My Linux fork",
                "owner": {"login": "myname"},
                "parent": {
                    "id": 999,
                    "full_name": "torvalds/linux",
                    "owner": {"login": "torvalds"}
                }
            },
            {
                "id": 2,
                "name": "myproject",
                "full_name": "myname/myproject",
                "private": false,
                "fork": false,
                "description": "My own project",
                "owner": {"login": "myname"}
            }
        ])))
        .mount(&mock_server)
        .await;

    let client = mergebeacon_lib::http_client::HttpClient::new();
    let adapter = mergebeacon_lib::platform::github::GitHubAdapter::new(client, "test-token".to_string())
        .with_base_url(mock_server.uri());

    let result = adapter.list_repos(1).await.expect("should list repos");

    assert_eq!(result.items.len(), 2);

    // Fork repo
    let fork = &result.items[0];
    assert!(fork.fork, "should be marked as fork");
    assert_eq!(fork.full_name, "myname/linux");
    assert_eq!(fork.parent_full_name.as_deref(), Some("torvalds/linux"));
    assert_eq!(fork.parent_owner.as_deref(), Some("torvalds"));

    // Normal repo
    let normal = &result.items[1];
    assert!(!normal.fork, "should not be a fork");
    assert_eq!(normal.parent_full_name, None);
    assert_eq!(normal.parent_owner, None);
}

#[tokio::test]
async fn test_github_list_repos_parses_link_pagination() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/user/repos"))
        .and(query_param("page", "2"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header(
                    "link",
                    format!(
                        "<{}/user/repos?per_page=100&page=3>; rel=\"next\", <{}/user/repos?per_page=100&page=5>; rel=\"last\"",
                        mock_server.uri(),
                        mock_server.uri()
                    ),
                )
                .set_body_json(serde_json::json!([])),
        )
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let result = adapter.list_repos(2).await.expect("should list repos");

    assert_eq!(result.page, 2);
    assert_eq!(result.total_pages, 5);
}

#[tokio::test]
async fn test_github_merge_readiness_reports_checks_failure_without_ready() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "number": 42,
            "state": "open",
            "draft": false,
            "mergeable": true,
            "mergeable_state": "clean",
            "head": {"sha": "head-sha"}
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/commits/head-sha/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"state": "failure"})))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls/42/reviews"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let readiness = adapter.get_merge_readiness("octocat", "hello-world", 42).await.expect("readiness");

    assert_eq!(readiness.status, mergebeacon_lib::models::ReadinessState::Blocked);
    assert_eq!(readiness.checks_status, mergebeacon_lib::models::ReadinessState::Blocked);
    assert!(readiness
        .blocking_reasons
        .iter()
        .any(|reason| { reason.code == mergebeacon_lib::models::MergeBlockingReasonCode::ChecksFailed }));
}

#[tokio::test]
async fn test_github_merge_readiness_keeps_pending_checks_pending() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls/43"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "number": 43,
            "state": "open",
            "draft": false,
            "mergeable": true,
            "mergeable_state": "clean",
            "head": {"sha": "pending-sha"}
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/commits/pending-sha/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"state": "pending"})))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls/43/reviews"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let readiness = adapter.get_merge_readiness("octocat", "hello-world", 43).await.expect("readiness");

    assert_eq!(readiness.status, mergebeacon_lib::models::ReadinessState::Pending);
    assert_eq!(readiness.checks_status, mergebeacon_lib::models::ReadinessState::Pending);
}

#[tokio::test]
async fn test_github_actions_success_overrides_empty_legacy_pending_status() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls/44"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "number": 44,
            "state": "open",
            "draft": false,
            "mergeable": true,
            "mergeable_state": "clean",
            "head": {"sha": "actions-sha"}
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/commits/actions-sha/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "state": "pending",
            "total_count": 0,
            "statuses": []
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/commits/actions-sha/check-runs"))
        .and(query_param("filter", "latest"))
        .and(query_param("per_page", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_count": 2,
            "check_runs": [
                {"name": "test", "status": "completed", "conclusion": "success"},
                {"name": "lint", "status": "completed", "conclusion": "success"}
            ]
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls/44/reviews"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "permissions": {"admin": false, "maintain": false, "push": true}
        })))
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let readiness = adapter.get_merge_readiness("octocat", "hello-world", 44).await.expect("readiness");

    assert_eq!(readiness.status, mergebeacon_lib::models::ReadinessState::Ready);
    assert_eq!(readiness.checks_status, mergebeacon_lib::models::ReadinessState::Ready);
    assert_eq!(readiness.has_merge_permission, Some(true));
    assert!(!readiness
        .blocking_reasons
        .iter()
        .any(|reason| reason.code == mergebeacon_lib::models::MergeBlockingReasonCode::ChecksPending));
}

#[tokio::test]
async fn test_github_merge_readiness_allows_merge_without_configured_checks() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls/48"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "number": 48,
            "state": "open",
            "draft": false,
            "mergeable": true,
            "mergeable_state": "unknown",
            "head": {"sha": "no-checks-sha"}
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/commits/no-checks-sha/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "state": "pending",
            "total_count": 0,
            "statuses": []
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/commits/no-checks-sha/check-runs"))
        .and(query_param("filter", "latest"))
        .and(query_param("per_page", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_count": 0,
            "check_runs": []
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls/48/reviews"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "permissions": {"admin": false, "maintain": false, "push": true}
        })))
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let readiness = adapter.get_merge_readiness("octocat", "hello-world", 48).await.expect("readiness");

    assert_eq!(readiness.status, mergebeacon_lib::models::ReadinessState::Ready);
    assert_eq!(readiness.checks_status, mergebeacon_lib::models::ReadinessState::Ready);
    assert_eq!(readiness.has_conflicts, Some(false));
    assert!(readiness.blocking_reasons.is_empty());
}

#[tokio::test]
async fn test_github_actions_in_progress_remains_pending_without_legacy_statuses() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls/45"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "number": 45,
            "state": "open",
            "draft": false,
            "mergeable": true,
            "mergeable_state": "clean",
            "head": {"sha": "running-sha"}
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/commits/running-sha/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "state": "pending",
            "total_count": 0,
            "statuses": []
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/commits/running-sha/check-runs"))
        .and(query_param("filter", "latest"))
        .and(query_param("per_page", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_count": 1,
            "check_runs": [
                {"name": "test", "status": "in_progress", "conclusion": null}
            ]
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls/45/reviews"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let readiness = adapter.get_merge_readiness("octocat", "hello-world", 45).await.expect("readiness");

    assert_eq!(readiness.status, mergebeacon_lib::models::ReadinessState::Pending);
    assert_eq!(readiness.checks_status, mergebeacon_lib::models::ReadinessState::Pending);
}

#[tokio::test]
async fn test_github_actions_failure_blocks_even_when_legacy_status_succeeds() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls/46"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "number": 46,
            "state": "open",
            "draft": false,
            "mergeable": true,
            "mergeable_state": "clean",
            "head": {"sha": "failed-actions-sha"}
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/commits/failed-actions-sha/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "state": "success",
            "total_count": 1,
            "statuses": [{"state": "success"}]
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/commits/failed-actions-sha/check-runs"))
        .and(query_param("filter", "latest"))
        .and(query_param("per_page", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_count": 1,
            "check_runs": [
                {"name": "test", "status": "completed", "conclusion": "failure"}
            ]
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls/46/reviews"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let readiness = adapter.get_merge_readiness("octocat", "hello-world", 46).await.expect("readiness");

    assert_eq!(readiness.status, mergebeacon_lib::models::ReadinessState::Blocked);
    assert_eq!(readiness.checks_status, mergebeacon_lib::models::ReadinessState::Blocked);
    assert!(readiness
        .blocking_reasons
        .iter()
        .any(|reason| reason.code == mergebeacon_lib::models::MergeBlockingReasonCode::ChecksFailed));
}

#[tokio::test]
async fn test_github_merge_readiness_blocks_user_without_push_permission() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls/47"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "state": "open",
            "draft": false,
            "mergeable": true,
            "mergeable_state": "clean",
            "head": {"sha": "permission-sha"}
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "permissions": {"admin": false, "maintain": false, "push": false, "pull": true}
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/commits/permission-sha/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "state": "success",
            "total_count": 1,
            "statuses": [{"state": "success"}]
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/commits/permission-sha/check-runs"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_count": 0,
            "check_runs": []
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls/47/reviews"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let readiness = adapter.get_merge_readiness("octocat", "hello-world", 47).await.expect("readiness");

    assert_eq!(readiness.status, mergebeacon_lib::models::ReadinessState::Blocked);
    assert_eq!(readiness.has_merge_permission, Some(false));
    assert!(readiness
        .blocking_reasons
        .iter()
        .any(|reason| reason.code == mergebeacon_lib::models::MergeBlockingReasonCode::NoMergePermission));
}

#[tokio::test]
async fn test_github_file_content_decodes_base64_at_revision() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/contents/src/lib.rs"))
        .and(query_param("ref", "head-sha"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "encoding": "base64",
            "content": "Zm4gbWFpbigpIHt9Cg==",
            "size": 13
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let content =
        adapter.get_pr_file_content("octocat", "hello-world", "src/lib.rs", "head-sha").await.expect("file content");

    assert_eq!(content.path, "src/lib.rs");
    assert_eq!(content.revision, "head-sha");
    assert_eq!(content.content, "fn main() {}\n");
    assert!(!content.truncated);
    assert!(!content.binary);
}

#[tokio::test]
async fn test_github_file_content_rejects_invalid_base64() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/contents/src/lib.rs"))
        .and(query_param("ref", "head-sha"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "encoding": "base64",
            "content": "not-base64",
            "size": 8
        })))
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let error = adapter
        .get_pr_file_content("octocat", "hello-world", "src/lib.rs", "head-sha")
        .await
        .expect_err("invalid base64 must fail");

    assert!(error.to_string().contains("不是有效的 base64"));
}

#[tokio::test]
async fn test_github_pr_detail_exposes_base_and_head_revisions() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "number": 42,
            "title": "PR",
            "user": {"id": 1, "login": "dev", "name": "", "avatar_url": ""},
            "state": "open",
            "merged_at": null,
            "created_at": "2026-07-16T00:00:00Z",
            "updated_at": "2026-07-16T00:00:00Z",
            "labels": [],
            "body": "",
            "head": {"ref": "feature", "sha": "head-sha"},
            "base": {"ref": "main", "sha": "base-sha"},
            "mergeable": true,
            "draft": true,
            "requested_reviewers": [{"id": 2, "login": "reviewer", "name": "Reviewer", "avatar_url": ""}],
            "assignees": [{"id": 3, "login": "assignee", "name": "Assignee", "avatar_url": ""}],
            "milestone": {"id": 9, "number": 4, "title": "0.6.0"}
        })))
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let detail = adapter.get_pull_request("octocat", "hello-world", 42).await.expect("PR detail");

    assert_eq!(detail.base_sha, "base-sha");
    assert_eq!(detail.head_sha, "head-sha");
    assert_eq!(detail.draft, Some(true));
    assert_eq!(detail.reviewers[0].login, "reviewer");
    assert_eq!(detail.assignees[0].login, "assignee");
    assert_eq!(detail.milestone.as_ref().map(|value| value.title.as_str()), Some("0.6.0"));
}

#[tokio::test]
async fn test_github_review_inbox_combines_reviewer_and_assignee_without_or_query() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/search/issues"))
        .and(query_param("q", "is:pr is:open review-requested:@me"))
        .and(query_param("sort", "updated"))
        .and(query_param("order", "desc"))
        .and(query_param("page", "1"))
        .and(query_param("per_page", "100"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_count": 2,
            "items": [
                github_search_issue(42, "Reviewer and assignee", "2025-01-02T00:00:00Z"),
                github_search_issue(43, "Reviewer only", "2025-01-04T00:00:00Z")
            ]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/search/issues"))
        .and(query_param("q", "is:pr is:open assignee:@me"))
        .and(query_param("sort", "updated"))
        .and(query_param("order", "desc"))
        .and(query_param("page", "1"))
        .and(query_param("per_page", "100"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_count": 2,
            "items": [
                github_search_issue(44, "Assignee only", "2025-01-03T00:00:00Z"),
                github_search_issue(42, "Reviewer and assignee", "2025-01-02T00:00:00Z")
            ]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let result = adapter
        .list_review_inbox(&ReviewInboxCategory::ReviewRequested, 1, 2)
        .await
        .expect("should combine review requests and assignments");

    assert_eq!(result.total_pages, 2);
    assert_eq!(result.total_count, 3);
    assert_eq!(result.items.iter().map(|item| item.summary.number).collect::<Vec<_>>(), vec![43, 44]);
    assert_eq!(result.items[0].platform, "github");
    assert_eq!(result.items[0].repository_full_name, "octocat/hello-world");
    assert_eq!(result.items[0].categories, vec![ReviewInboxCategory::ReviewRequested]);
    assert_eq!(result.items[0].relationships, vec![ReviewInboxRelationship::Reviewer]);

    let requests = mock_server.received_requests().await.expect("requests");
    assert!(requests.iter().all(|request| request.url.query_pairs().all(|(_, value)| !value.contains(" OR "))));
}

#[tokio::test]
async fn test_github_review_inbox_batches_status_for_current_page() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/search/issues"))
        .and(query_param("q", "is:pr is:open review-requested:@me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_count": 2,
            "items": [
                github_search_issue(42, "Older", "2025-01-02T00:00:00Z"),
                github_search_issue(43, "Current page", "2025-01-04T00:00:00Z")
            ]
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/search/issues"))
        .and(query_param("q", "is:pr is:open assignee:@me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_count": 0,
            "items": []
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "nodes": [{
                    "id": "PR_node_43",
                    "headRefOid": "head-43",
                    "isDraft": false,
                    "mergeable": "CONFLICTING",
                    "mergeStateStatus": "DIRTY",
                    "reviewDecision": "CHANGES_REQUESTED",
                    "commits": { "nodes": [{ "commit": { "statusCheckRollup": { "state": "FAILURE" } } }] },
                    "comments": { "totalCount": 2 },
                    "reviewThreads": { "nodes": [
                        { "comments": { "totalCount": 3 } },
                        { "comments": { "totalCount": 1 } }
                    ] }
                }]
            }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let result = adapter
        .list_review_inbox(&ReviewInboxCategory::ReviewRequested, 1, 1)
        .await
        .expect("should batch status for the current page");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].summary.number, 43);
    assert_eq!(result.items[0].status.status, ReadinessState::Blocked);
    assert_eq!(result.items[0].status.checks_status, ReadinessState::Blocked);
    assert_eq!(result.items[0].status.approvals_status, ReadinessState::Blocked);
    assert_eq!(result.items[0].status.has_conflicts, Some(true));
    assert_eq!(result.items[0].head_sha.as_deref(), Some("head-43"));
    assert_eq!(result.items[0].comments_count, Some(6));

    let requests = mock_server.received_requests().await.expect("requests");
    let graphql = requests.iter().find(|request| request.url.path() == "/graphql").expect("GraphQL request");
    let body: serde_json::Value = serde_json::from_slice(&graphql.body).expect("GraphQL JSON body");
    assert_eq!(body["variables"]["ids"], serde_json::json!(["PR_node_43"]));
}

#[tokio::test]
async fn test_github_review_inbox_keeps_missing_mergeability_unknown() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/search/issues"))
        .and(query_param("q", "is:pr is:open review-requested:@me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_count": 1,
            "items": [github_search_issue(42, "Review", "2025-01-02T00:00:00Z")]
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/search/issues"))
        .and(query_param("q", "is:pr is:open assignee:@me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_count": 0,
            "items": []
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "nodes": [{
                    "id": "PR_node_42",
                    "isDraft": false,
                    "mergeStateStatus": "CLEAN",
                    "reviewDecision": null,
                    "commits": { "nodes": [{ "commit": { "statusCheckRollup": null } }] }
                }]
            }
        })))
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let result = adapter
        .list_review_inbox(&ReviewInboxCategory::ReviewRequested, 1, 20)
        .await
        .expect("missing mergeability must remain unknown");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].status.status, ReadinessState::Unknown);
    assert_eq!(result.items[0].status.checks_status, ReadinessState::Unknown);
    assert_eq!(result.items[0].status.approvals_status, ReadinessState::Unknown);
}

#[tokio::test]
async fn test_github_review_inbox_keeps_items_when_batched_status_fails() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/search/issues"))
        .and(query_param("q", "is:pr is:open review-requested:@me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_count": 1,
            "items": [github_search_issue(42, "Review", "2025-01-02T00:00:00Z")]
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/search/issues"))
        .and(query_param("q", "is:pr is:open assignee:@me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_count": 0,
            "items": []
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({"message": "unavailable"})))
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let result = adapter
        .list_review_inbox(&ReviewInboxCategory::ReviewRequested, 1, 20)
        .await
        .expect("status failure must not fail inbox listing");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].status.status, ReadinessState::Unknown);
}

#[tokio::test]
async fn test_github_review_inbox_fetches_all_remote_pages_before_local_pagination() {
    let mock_server = MockServer::start().await;
    let first_page = (1..=100)
        .map(|number| github_search_issue(number, &format!("PR {number}"), &format!("{number:04}-01-01T00:00:00Z")))
        .collect::<Vec<_>>();
    Mock::given(method("GET"))
        .and(path("/search/issues"))
        .and(query_param("q", "is:pr is:open review-requested:@me"))
        .and(query_param("page", "1"))
        .and(query_param("per_page", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_count": 101,
            "items": first_page
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/search/issues"))
        .and(query_param("q", "is:pr is:open review-requested:@me"))
        .and(query_param("page", "2"))
        .and(query_param("per_page", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_count": 101,
            "items": [github_search_issue(101, "PR 101", "0101-01-01T00:00:00Z")]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/search/issues"))
        .and(query_param("q", "is:pr is:open assignee:@me"))
        .and(query_param("page", "1"))
        .and(query_param("per_page", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_count": 0,
            "items": []
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let result = adapter
        .list_review_inbox(&ReviewInboxCategory::ReviewRequested, 6, 20)
        .await
        .expect("should fetch all GitHub search pages");

    assert_eq!(result.total_count, 101);
    assert_eq!(result.total_pages, 6);
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].summary.number, 1);
}

#[tokio::test]
async fn test_github_review_inbox_fails_when_assignment_search_fails() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/search/issues"))
        .and(query_param("q", "is:pr is:open review-requested:@me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "total_count": 0,
            "items": []
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/search/issues"))
        .and(query_param("q", "is:pr is:open assignee:@me"))
        .respond_with(ResponseTemplate::new(422).set_body_json(serde_json::json!({
            "message": "Validation Failed"
        })))
        .mount(&mock_server)
        .await;

    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let error = adapter
        .list_review_inbox(&ReviewInboxCategory::ReviewRequested, 1, 20)
        .await
        .expect_err("assignment search failure must not be ignored");

    assert!(error.to_string().contains("422 Unprocessable Entity"));
    assert!(error.to_string().contains("Validation Failed"));
    assert!(!error.to_string().contains("test-token"));
}

#[tokio::test]
async fn test_github_updates_pull_request_metadata() {
    let mock_server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/repos/octocat/hello-world/pulls/42"))
        .and(body_json(serde_json::json!({
            "title": "New title",
            "body": "New body"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains("query PullRequestNodeId"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "repository": { "pullRequest": { "id": "PR_node_42" } } }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_string_contains("mutation MarkPullRequestReadyForReview"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "markPullRequestReadyForReview": { "pullRequest": { "id": "PR_node_42", "isDraft": false } } }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/repos/octocat/hello-world/pulls/42/requested_reviewers"))
        .and(body_json(serde_json::json!({ "reviewers": ["old-reviewer"] })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/repos/octocat/hello-world/pulls/42/requested_reviewers"))
        .and(body_json(serde_json::json!({ "reviewers": ["new-reviewer"] })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/milestones"))
        .and(query_param("state", "all"))
        .and(query_param("per_page", "100"))
        .and(query_param("title", "0.7.0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([{
            "id": 10,
            "number": 5,
            "title": "0.7.0"
        }])))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/repos/octocat/hello-world/issues/42"))
        .and(body_json(serde_json::json!({
            "assignees": ["new-assignee"],
            "labels": ["new-label"],
            "milestone": 5
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&mock_server)
        .await;

    let current = PrDetail {
        summary: PrSummary {
            number: 42,
            title: "Old title".into(),
            author: User { id: serde_json::json!(1), login: "author".into(), name: "".into(), avatar_url: "".into() },
            state: PrState::Open,
            created_at: "2026-07-16T00:00:00Z".into(),
            updated_at: "2026-07-17T00:00:00Z".into(),
            labels: vec!["old-label".into()],
            status: None,
        },
        body: "Old body".into(),
        source_branch: "feature".into(),
        target_branch: "main".into(),
        mergeable: Some(true),
        head_sha: "head".into(),
        base_sha: "base".into(),
        draft: Some(true),
        reviewers: vec![User {
            id: serde_json::json!(2),
            login: "old-reviewer".into(),
            name: "".into(),
            avatar_url: "".into(),
        }],
        assignees: vec![User {
            id: serde_json::json!(3),
            login: "old-assignee".into(),
            name: "".into(),
            avatar_url: "".into(),
        }],
        milestone: Some(PrMilestone { id: serde_json::json!(9), number: Some(4), title: "0.6.0".into() }),
        metadata_permissions: PrMetadataPermissions::default(),
    };
    let update = PrMetadataUpdate {
        title: "New title".into(),
        body: "New body".into(),
        draft: Some(false),
        reviewers: vec!["new-reviewer".into()],
        assignees: vec!["new-assignee".into()],
        labels: vec!["new-label".into()],
        milestone: Some("0.7.0".into()),
        expected_updated_at: current.summary.updated_at.clone(),
    };
    let adapter = GitHubAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());

    let result = adapter
        .update_pull_request_metadata("octocat", "hello-world", 42, &current, &update)
        .await
        .expect("metadata update should succeed");

    assert!(result.failures.is_empty());
    assert_eq!(
        result.updated_fields,
        vec![
            PrMetadataField::TitleBody,
            PrMetadataField::Draft,
            PrMetadataField::Reviewers,
            PrMetadataField::Assignees,
            PrMetadataField::Labels,
            PrMetadataField::Milestone,
        ]
    );
}
