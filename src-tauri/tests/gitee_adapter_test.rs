use mergebeacon_lib::http_client::HttpClient;
use mergebeacon_lib::models::{
    PrCreatePreviewRequest, PrCreateRequest, PrDetail, PrMetadataField, PrMetadataPermissions, PrMetadataUpdate,
    PrMilestone, PrState, PrSummary, ReadinessState, ReviewInboxCategory, ReviewInboxRelationship, User,
};
use mergebeacon_lib::platform::{gitee::GiteeAdapter, GitPlatform};
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn gitee_inline_comment(id: u64) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "body": format!("comment-{id}"),
        "path": "src/main.rs",
        "position": id,
        "new_line": id,
        "commit_id": "abc123",
        "user": { "id": 1, "login": "dev1", "name": "Dev One", "avatar_url": "" },
        "created_at": "2025-01-04T00:00:00Z"
    })
}

#[tokio::test]
async fn test_gitee_lists_branches_and_creates_from_fork() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/team/repo/branches"))
        .and(query_param("per_page", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "name": "master" },
            { "name": "feature" }
        ])))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/team/repo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "default_branch": "feature"
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v5/repos/team/repo/pulls"))
        .and(body_json(serde_json::json!({
            "title": "Add feature",
            "body": "Description",
            "head": "contributor:feature",
            "base": "master"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({ "number": 8 })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/team/repo/compare/master...contributor%3Afeature"))
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
    let adapter = GiteeAdapter::new(HttpClient::new(), "token".into()).with_base_url(mock_server.uri());

    let branch_options = adapter.list_branches("team", "repo").await.unwrap();
    assert_eq!(branch_options.branches, vec!["master", "feature"]);
    assert_eq!(branch_options.default_branch.as_deref(), Some("feature"));
    let preview = adapter
        .preview_pull_request(
            "team",
            "repo",
            &PrCreatePreviewRequest {
                source_owner: "contributor".into(),
                source_repo: "repo".into(),
                source_branch: "feature".into(),
                target_branch: "master".into(),
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
            "team",
            "repo",
            &PrCreateRequest {
                source_owner: "contributor".into(),
                source_repo: "repo".into(),
                source_branch: "feature".into(),
                target_branch: "master".into(),
                title: "Add feature".into(),
                body: "Description".into(),
                draft: false,
                reviewers: vec![],
                assignees: vec![],
                labels: vec![],
            },
        )
        .await
        .unwrap();
    assert_eq!(number, 8);
}

#[tokio::test]
async fn test_gitee_lists_pr_dependency_candidates() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/team/repo/pulls/8"))
        .and(query_param("access_token", "token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "number": 8,
            "title": "Stack child",
            "state": "open",
            "merged_at": null,
            "head": { "ref": "feature-b", "repo": { "full_name": "team/repo" } },
            "base": { "ref": "feature-a", "repo": { "full_name": "team/repo" } }
        })))
        .mount(&mock_server)
        .await;
    for (filter, value) in [("base", "feature-b"), ("head", "team:feature-a")] {
        Mock::given(method("GET"))
            .and(path("/api/v5/repos/team/repo/pulls"))
            .and(query_param("state", "all"))
            .and(query_param(filter, value))
            .and(query_param("per_page", "100"))
            .and(query_param("page", "1"))
            .and(query_param("access_token", "token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(&mock_server)
            .await;
    }
    let adapter =
        GiteeAdapter::new(HttpClient::new(), "token".into()).with_base_url(format!("{}/api/v5", mock_server.uri()));

    let result = adapter.list_pr_dependency_candidates("team", "repo", 8).await.expect("dependency candidates");

    assert!(!result.truncated);
    assert_eq!(result.current.number, 8);
    let candidates = result.items;
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].number, 8);
    assert_eq!(candidates[0].source_branch, "feature-b");
    assert_eq!(candidates[0].target_branch, "feature-a");
}

#[tokio::test]
async fn test_gitee_create_compare_marks_an_incomplete_preview() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/team/repo/compare/master...feature"))
        .and(query_param("per_page", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ahead_by": 2,
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
    let adapter = GiteeAdapter::new(HttpClient::new(), "token".into()).with_base_url(mock_server.uri());

    let preview = adapter
        .preview_pull_request(
            "team",
            "repo",
            &PrCreatePreviewRequest {
                source_owner: "team".into(),
                source_repo: "repo".into(),
                source_branch: "feature".into(),
                target_branch: "master".into(),
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
async fn test_gitee_lists_all_branch_pages() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/team/repo/branches"))
        .and(query_param("per_page", "100"))
        .and(query_param("page", "1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-total-pages", "2")
                .set_body_json(serde_json::json!([{ "name": "master" }])),
        )
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/team/repo/branches"))
        .and(query_param("per_page", "100"))
        .and(query_param("page", "2"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-total-pages", "2")
                .set_body_json(serde_json::json!([{ "name": "feature-101" }])),
        )
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/team/repo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "default_branch": "master"
        })))
        .mount(&mock_server)
        .await;

    let adapter = GiteeAdapter::new(HttpClient::new(), "token".into()).with_base_url(mock_server.uri());
    let branches = adapter.list_branches("team", "repo").await.unwrap();

    assert_eq!(branches.branches, vec!["master", "feature-101"]);
}

#[tokio::test]
async fn test_gitee_lists_repository_labels() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/team/repo/labels"))
        .and(query_param("per_page", "100"))
        .and(query_param("page", "1"))
        .respond_with(
            ResponseTemplate::new(200).insert_header("x-total-pages", "2").set_body_json(
                serde_json::json!([{ "name": "bug", "color": "d73a4a", "description": "Needs fixing" }]),
            ),
        )
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/team/repo/labels"))
        .and(query_param("per_page", "100"))
        .and(query_param("page", "2"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-total-pages", "2")
                .set_body_json(serde_json::json!([{ "name": "feature" }])),
        )
        .mount(&mock_server)
        .await;

    let adapter = GiteeAdapter::new(HttpClient::new(), "token".into()).with_base_url(mock_server.uri());
    let labels = adapter.list_labels("team", "repo").await.unwrap();

    assert_eq!(labels.iter().map(|label| label.name.as_str()).collect::<Vec<_>>(), vec!["bug", "feature"]);
    assert_eq!(labels[0].color.as_deref(), Some("d73a4a"));
    assert_eq!(labels[0].description.as_deref(), Some("Needs fixing"));
}

#[tokio::test]
async fn test_gitee_lists_pr_participant_suggestions() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/team/repo/collaborators"))
        .and(query_param("per_page", "100"))
        .and(query_param("page", "1"))
        .and(query_param("access_token", "token"))
        .respond_with(ResponseTemplate::new(200).insert_header("x-total-pages", "2").set_body_json(
            serde_json::json!([{
                "id": 1,
                "login": "alice",
                "name": "Alice Zhang",
                "avatar_url": "https://example.com/alice.png"
            }]),
        ))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/team/repo/collaborators"))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).insert_header("x-total-pages", "2").set_body_json(
            serde_json::json!([{
                "id": 2,
                "login": "bob",
                "name": "Bob",
                "avatar_url": ""
            }]),
        ))
        .mount(&mock_server)
        .await;

    let adapter = GiteeAdapter::new(HttpClient::new(), "token".into()).with_base_url(mock_server.uri());
    let users = adapter.list_pr_participant_suggestions("team", "repo").await.unwrap();

    assert_eq!(users.iter().map(|user| user.login.as_str()).collect::<Vec<_>>(), vec!["alice", "bob"]);
    assert_eq!(users[0].name, "Alice Zhang");
}

#[tokio::test]
async fn test_gitee_previews_a_single_commit() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/contributor/repo/commits/abc123"))
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
    let adapter = GiteeAdapter::new(HttpClient::new(), "token".into()).with_base_url(mock_server.uri());

    let preview = adapter
        .preview_pull_request(
            "team",
            "repo",
            &PrCreatePreviewRequest {
                source_owner: "contributor".into(),
                source_repo: "repo".into(),
                source_branch: "feature".into(),
                target_branch: "master".into(),
                commit_sha: Some("abc123".into()),
            },
        )
        .await
        .unwrap();

    assert_eq!(preview.commits[0].title, "Only this commit");
    assert_eq!(preview.files[0].filename, "src/commit.rs");
}

#[tokio::test]
async fn test_gitee_current_user() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v5/user"))
        .and(query_param("access_token", "test-token-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1,
            "login": "testuser",
            "name": "Test User",
            "avatar_url": "https://avatars.example.com/u/1"
        })))
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter =
        GiteeAdapter::new(client, "test-token-123".to_string()).with_base_url(format!("{}/api/v5", mock_server.uri()));

    let user = adapter.current_user().await.expect("should fetch user");
    assert_eq!(user.login, "testuser");
    assert_eq!(user.name, "Test User");
}

#[tokio::test]
async fn test_gitee_list_prs_open() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls"))
        .and(query_param("state", "open"))
        .and(query_param("access_token", "test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .append_header("total_count", "1")
                .append_header("total_page", "1")
                .set_body_json(serde_json::json!([
                    {
                        "number": 42,
                        "title": "Fix bug",
                        "state": "open",
                        "merged_at": null,
                        "mergeable": true,
                        "assignees_number": 1,
                        "assignees": [{ "accept": true }],
                        "testers_number": 1,
                        "testers": [{ "accept": true }],
                        "created_at": "2025-01-01T00:00:00Z",
                        "updated_at": "2025-01-02T00:00:00Z",
                        "user": { "id": 1, "login": "dev1", "name": "", "avatar_url": "" },
                        "labels": [{ "name": "bug" }]
                    }
                ])),
        )
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter =
        GiteeAdapter::new(client, "test-token".to_string()).with_base_url(format!("{}/api/v5", mock_server.uri()));

    let result = adapter
        .list_pull_requests("octocat", "hello-world", &mergebeacon_lib::models::PrState::Open, 1, 20)
        .await
        .expect("should list PRs");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].number, 42);
    assert_eq!(result.items[0].title, "Fix bug");
    let status = result.items[0].status.as_ref().expect("open PR should expose status summary");
    assert_eq!(status.status, ReadinessState::Ready);
    assert_eq!(status.approvals_status, ReadinessState::Ready);
    assert_eq!(status.checks_status, ReadinessState::Ready);
    assert_eq!(result.total_count, 1);
    assert_eq!(result.total_pages, 1);
}

#[tokio::test]
async fn test_gitee_list_prs_pagination_headers() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls"))
        .and(query_param("state", "open"))
        .and(query_param("access_token", "test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .append_header("total_count", "25")
                .append_header("total_page", "2")
                .append_header(
                    "link",
                    "<https://gitee.com/api/v5/repos/octocat/hello-world/pulls?page=2&per_page=20&state=open>; rel='next', <https://gitee.com/api/v5/repos/octocat/hello-world/pulls?page=2&per_page=20&state=open>; rel='last'",
                )
                .set_body_json(serde_json::json!([
                    {
                        "number": 42,
                        "title": "Fix bug",
                        "state": "open",
                        "merged_at": null,
                        "created_at": "2025-01-01T00:00:00Z",
                        "updated_at": "2025-01-02T00:00:00Z",
                        "user": { "id": 1, "login": "dev1", "name": "", "avatar_url": "" },
                        "labels": []
                    }
                ])),
        )
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter =
        GiteeAdapter::new(client, "test-token".to_string()).with_base_url(format!("{}/api/v5", mock_server.uri()));

    let result = adapter
        .list_pull_requests("octocat", "hello-world", &mergebeacon_lib::models::PrState::Open, 1, 20)
        .await
        .expect("should list PRs");

    assert_eq!(result.total_count, 25);
    assert_eq!(result.total_pages, 2);
    let status = result.items[0].status.as_ref().expect("open PR should expose status summary");
    assert_eq!(status.status, ReadinessState::Unknown);
    assert_ne!(status.status, ReadinessState::Ready);
}

#[tokio::test]
async fn test_gitee_list_prs_merged() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls"))
        .and(query_param("state", "merged"))
        .and(query_param("access_token", "test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .append_header("total_count", "2")
                .append_header("total_page", "1")
                .set_body_json(serde_json::json!([
                    {
                        "number": 100,
                        "title": "Merged feature",
                        "state": "merged",
                        "merged_at": "2025-01-03T00:00:00Z",
                        "created_at": "2025-01-01T00:00:00Z",
                        "updated_at": "2025-01-03T00:00:00Z",
                        "user": { "id": 1, "login": "dev1", "name": "", "avatar_url": "" },
                        "labels": []
                    },
                    {
                        "number": 101,
                        "title": "Another merge",
                        "state": "merged",
                        "merged_at": null,
                        "created_at": "2025-01-02T00:00:00Z",
                        "updated_at": "2025-01-04T00:00:00Z",
                        "user": { "id": 2, "login": "dev2", "name": "", "avatar_url": "" },
                        "labels": [{ "name": "enhancement" }]
                    }
                ])),
        )
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter =
        GiteeAdapter::new(client, "test-token".to_string()).with_base_url(format!("{}/api/v5", mock_server.uri()));

    let result = adapter
        .list_pull_requests("octocat", "hello-world", &mergebeacon_lib::models::PrState::Merged, 1, 20)
        .await
        .expect("should list merged PRs");

    assert_eq!(result.items.len(), 2);
    assert_eq!(result.items[0].number, 100);
    assert_eq!(result.items[0].title, "Merged feature");
    assert!(matches!(result.items[0].state, mergebeacon_lib::models::PrState::Merged));
    assert!(result.items[0].status.is_none());
    assert_eq!(result.items[1].number, 101);
    assert!(matches!(result.items[1].state, mergebeacon_lib::models::PrState::Merged));
    assert!(result.items[1].status.is_none());
    assert_eq!(result.total_count, 2);
    assert_eq!(result.total_pages, 1);
}

#[tokio::test]
async fn test_gitee_create_pr_comment() {
    let mock_server = MockServer::start().await;

    let expected_body = serde_json::json!({
        "body": "Good catch!",
        "commit_id": "abc123",
        "path": "src/main.rs",
        "position": 10,
    });
    Mock::given(method("POST"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42/comments"))
        .and(query_param("access_token", "test-token"))
        .and(body_json(&expected_body))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": 2001,
            "body": "Good catch!",
            "commit_id": "abc123",
            "path": "src/main.rs",
            "position": 10,
            "new_line": 10,
            "user": { "id": 1, "login": "testuser", "name": "", "avatar_url": "" },
            "created_at": "2025-01-04T00:00:00Z",
            "comment_type": "diff_comment"
        })))
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter =
        GiteeAdapter::new(client, "test-token".to_string()).with_base_url(format!("{}/api/v5", mock_server.uri()));

    let comment = adapter
        .create_pr_comment("octocat", "hello-world", 42, "abc123", "src/main.rs", None, 10, "right", "Good catch!")
        .await
        .expect("should create PR comment");

    assert_eq!(comment.side.as_deref(), Some("right"));
}

#[tokio::test]
async fn test_gitee_create_pr_comment_error() {
    let mock_server = MockServer::start().await;

    // Mock POST to /comments endpoint returning error
    Mock::given(method("POST"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42/comments"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(422).set_body_string(r#"{"message":"Validation failed"}"#))
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter =
        GiteeAdapter::new(client, "test-token".to_string()).with_base_url(format!("{}/api/v5", mock_server.uri()));

    let result = adapter
        .create_pr_comment("octocat", "hello-world", 42, "abc123", "src/main.rs", None, 10, "left", "Bad comment")
        .await;

    assert!(result.is_err(), "should return error for bad request");
}

#[tokio::test]
async fn test_gitee_list_pr_comments() {
    let mock_server = MockServer::start().await;

    // Returns a mix of inline comments (with path) and general comments (without path)
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42/comments"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "id": 100,
                "body": "Nice fix!",
                "path": "src/main.rs",
                "position": 15,
                "new_line": 15,
                "commit_id": "abc123",
                "user": { "id": 1, "login": "dev1", "name": "Dev One", "avatar_url": "" },
                "created_at": "2025-01-04T00:00:00Z"
            },
            {
                "id": 101,
                "body": "Please add tests",
                "path": "src/lib.rs",
                "position": 42,
                "old_line": 42,
                "commit_id": "def456",
                "user": { "id": 2, "login": "dev2", "name": "Dev Two", "avatar_url": "" },
                "created_at": "2025-01-04T01:00:00Z"
            },
            {
                "id": 103,
                "body": "Multi-line review",
                "path": "src/main.rs",
                "position": 20,
                "new_line": 20,
                "start_line": 10,
                "commit_id": "ghi789",
                "user": { "id": 1, "login": "dev1", "name": "Dev One", "avatar_url": "" },
                "created_at": "2025-01-04T03:00:00Z"
            },
            {
                "id": 102,
                "body": "LGTM",
                "path": "",
                "user": { "id": 3, "login": "dev3", "name": "Dev Three", "avatar_url": "" },
                "created_at": "2025-01-04T02:00:00Z"
            },
            {
                "id": 104,
                "in_reply_to_id": 100,
                "body": "Thanks, updated.",
                "path": "src/main.rs",
                "position": 15,
                "new_line": 15,
                "commit_id": "abc123",
                "user": { "id": 4, "login": "author", "name": "PR Author", "avatar_url": "" },
                "created_at": "2025-01-04T04:00:00Z"
            }
        ])))
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter =
        GiteeAdapter::new(client, "test-token".to_string()).with_base_url(format!("{}/api/v5", mock_server.uri()));

    let comments = adapter.list_pr_comments("octocat", "hello-world", 42).await.expect("should list PR comments");

    assert_eq!(comments.len(), 4);
    assert_eq!(comments[0].body, "Nice fix!");
    assert_eq!(comments[0].path, "src/main.rs");
    assert_eq!(comments[0].line, Some(15));
    assert_eq!(comments[0].side.as_deref(), Some("right"));
    assert_eq!(comments[0].start_line, None);
    assert_eq!(comments[0].thread_id, "100");
    assert_eq!(comments[0].reply_to_id, None);
    assert_eq!(comments[0].resolved, None);
    assert!(!comments[0].resolvable);
    assert_eq!(comments[1].body, "Please add tests");
    assert_eq!(comments[1].path, "src/lib.rs");
    assert_eq!(comments[1].line, Some(42));
    assert_eq!(comments[1].side.as_deref(), Some("left"));
    assert_eq!(comments[1].start_line, None);
    assert_eq!(comments[2].body, "Multi-line review");
    assert_eq!(comments[2].path, "src/main.rs");
    assert_eq!(comments[2].line, Some(20));
    assert_eq!(comments[2].start_line, Some(10));
    assert_eq!(comments[3].body, "Thanks, updated.");
    assert_eq!(comments[3].thread_id, "100");
    assert_eq!(comments[3].reply_to_id.as_deref(), Some("100"));
    assert_eq!(comments[3].resolved, None);
    assert!(!comments[3].resolvable);
}

#[tokio::test]
async fn test_gitee_list_pr_comments_empty() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42/comments"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter =
        GiteeAdapter::new(client, "test-token".to_string()).with_base_url(format!("{}/api/v5", mock_server.uri()));

    let comments = adapter.list_pr_comments("octocat", "hello-world", 42).await.expect("should list PR comments");

    assert_eq!(comments.len(), 0);
}

#[tokio::test]
async fn test_gitee_list_pr_comments_fetches_the_page_after_a_full_boundary() {
    let mock_server = MockServer::start().await;
    let first_page: Vec<_> = (1..=100).map(gitee_inline_comment).collect();
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42/comments"))
        .and(query_param("per_page", "100"))
        .and(query_param("page", "1"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(first_page))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42/comments"))
        .and(query_param("per_page", "100"))
        .and(query_param("page", "2"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![gitee_inline_comment(101)]))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));
    let comments =
        adapter.list_pr_comments("octocat", "hello-world", 42).await.expect("should fetch every comment page");

    assert_eq!(comments.len(), 101);
    assert_eq!(comments.first().map(|comment| &comment.id), Some(&serde_json::json!(1)));
    assert_eq!(comments.last().map(|comment| &comment.id), Some(&serde_json::json!(101)));
}

#[tokio::test]
async fn test_gitee_list_pr_comments_reports_rate_limit_errors() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42/comments"))
        .respond_with(ResponseTemplate::new(429).set_body_string(r#"{"message":"rate limit"}"#))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));
    let error = adapter.list_pr_comments("octocat", "hello-world", 42).await.expect_err("rate limits must be returned");

    assert!(error.to_string().contains("Gitee API 429 Too Many Requests"));
    assert!(error.to_string().contains("rate limit"));
}

#[tokio::test]
async fn test_gitee_get_pr_diff() {
    let mock_server = MockServer::start().await;

    // Gitee builds the unified diff from the per-file patches returned by the
    // files endpoint (no .diff URL suffix is supported by Gitee API v5).
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42/files"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "filename": "src/main.rs",
                "status": "modified",
                "patch": "diff --git a/src/main.rs b/src/main.rs\n@@ -1,3 +1,4 @@\n line1\n-line2\n+line2_new\n line3\n",
                "additions": 1,
                "deletions": 1
            },
            {
                "filename": "src/lib.rs",
                "status": "added",
                "patch": "diff --git a/src/lib.rs b/src/lib.rs\n@@ -0,0 +1,3 @@\n+new\n+file\n+content\n",
                "additions": 3,
                "deletions": 0
            }
        ])))
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter =
        GiteeAdapter::new(client, "test-token".to_string()).with_base_url(format!("{}/api/v5", mock_server.uri()));

    let (diff, files) = adapter.get_pr_diff("octocat", "hello-world", 42).await.expect("should get diff");

    assert!(diff.contains("src/main.rs"));
    assert!(diff.contains("src/lib.rs"));
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].filename, "src/main.rs");
    assert_eq!(files[0].additions, 1);
    assert_eq!(files[0].deletions, 1);
    assert_eq!(files[1].filename, "src/lib.rs");
    assert!(matches!(files[1].status, mergebeacon_lib::models::FileStatus::Added));
}

#[tokio::test]
async fn test_gitee_diff_preserves_old_path_for_renamed_text_file() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/44/files"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "filename": "src/new-name.rs",
                "previous_filename": "src/old-name.rs",
                "status": "renamed",
                "patch": "@@ -2 +2 @@\n-old name\n+new name\n",
                "additions": 1,
                "deletions": 1
            }
        ])))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));
    let (diff, files) = adapter.get_pr_diff("octocat", "hello-world", 44).await.expect("renamed file diff");
    let patches = mergebeacon_lib::patch::standardize_patches(&diff, &files);

    assert!(diff
        .starts_with("diff --git a/src/old-name.rs b/src/new-name.rs\n--- a/src/old-name.rs\n+++ b/src/new-name.rs\n"));
    assert_eq!(patches[0].old_path.as_deref(), Some("src/old-name.rs"));
    assert_eq!(patches[0].new_path.as_deref(), Some("src/new-name.rs"));
    assert!(matches!(patches[0].content_kind, mergebeacon_lib::models::PatchContentKind::Text));
}

#[tokio::test]
async fn test_gitee_diff_preserves_metadata_only_rename() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/43/files"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "filename": "src/new-name.rs",
                "previous_filename": "src/old-name.rs",
                "status": "renamed",
                "patch": null,
                "additions": 0,
                "deletions": 0
            },
            {
                "filename": "src/large-new.rs",
                "previous_filename": "src/large-old.rs",
                "status": "renamed",
                "patch": null,
                "additions": 0,
                "deletions": 0,
                "truncated": true
            }
        ])))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));
    let (diff, files) = adapter.get_pr_diff("octocat", "hello-world", 43).await.expect("should load renamed file diff");
    let patches = mergebeacon_lib::patch::standardize_patches(&diff, &files);

    assert!(diff.contains("rename from src/old-name.rs\nrename to src/new-name.rs"));
    assert!(matches!(files[0].status, mergebeacon_lib::models::FileStatus::Renamed));
    assert!(matches!(patches[0].content_kind, mergebeacon_lib::models::PatchContentKind::MetadataOnly));
    assert_eq!(patches[0].old_path.as_deref(), Some("src/old-name.rs"));
    assert_eq!(patches[0].new_path.as_deref(), Some("src/new-name.rs"));
    assert!(matches!(patches[1].content_kind, mergebeacon_lib::models::PatchContentKind::Unavailable));
}

#[tokio::test]
async fn test_gitee_compare_diff_accepts_changes_and_normalizes_rename() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/compare/base-sha...head-sha"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "changes": [
                {
                    "filename": "src/main.rs",
                    "status": "modified",
                    "patch": "@@ -1 +1 @@\n-old\n+new",
                    "additions": 1,
                    "deletions": 1
                },
                {
                    "old_path": "src/old-name.rs",
                    "new_path": "src/new-name.rs",
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

    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));
    let (diff, files) = adapter
        .get_compare_diff("octocat", "hello-world", "base-sha", "head-sha")
        .await
        .expect("should compare commits");
    let patches = mergebeacon_lib::patch::standardize_patches(&diff, &files);

    assert_eq!(files.len(), 2);
    assert!(diff.contains("diff --git a/src/main.rs b/src/main.rs"));
    assert!(diff.contains("rename from src/old-name.rs\nrename to src/new-name.rs"));
    assert!(matches!(files[1].status, mergebeacon_lib::models::FileStatus::Renamed));
    assert_eq!(patches[1].old_path.as_deref(), Some("src/old-name.rs"));
    assert_eq!(patches[1].new_path.as_deref(), Some("src/new-name.rs"));
}

#[tokio::test]
async fn test_gitee_compare_diff_rejects_missing_files_and_changes() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/compare/base...head"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .mount(&mock_server)
        .await;

    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));
    let error = adapter
        .get_compare_diff("octocat", "hello-world", "base", "head")
        .await
        .expect_err("missing compare files must fail");

    assert!(error.to_string().contains("缺少 files/changes 字段"));
}

#[tokio::test]
async fn test_gitee_list_repos_paginated() {
    let mock_server = MockServer::start().await;

    let link_value = "<https://gitee.com/api/v5/user/repos?page=3&per_page=100>; rel=\"last\"";

    Mock::given(method("GET"))
        .and(path("/api/v5/user/repos"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!([
                    {
                        "id": 1,
                        "name": "repo1",
                        "full_name": "user/repo1",
                        "private": false,
                        "fork": false,
                        "description": "First repo",
                        "owner": { "login": "user", "name": "用户昵称", "type": "User" },
                        "namespace": { "type": "personal", "name": "user", "path": "user", "enterprise_id": 0 }
                    }
                ]))
                .insert_header("link", link_value),
        )
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter =
        GiteeAdapter::new(client, "test-token".to_string()).with_base_url(format!("{}/api/v5", mock_server.uri()));

    let result = adapter.list_repos(1).await.expect("should list repos");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].owner, "user");
    assert_eq!(result.items[0].owner_type, "user");
    assert_eq!(result.items[0].owner_display_name, "user");
    assert_eq!(result.total_pages, 3, "should parse last page from Link header");
}

#[tokio::test]
async fn test_gitee_list_repos_single_page() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v5/user/repos"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "id": 1,
                "name": "repo1",
                "full_name": "user/repo1",
                "private": false,
                "fork": false,
                "description": "First repo",
                "owner": { "login": "user", "name": "用户昵称", "type": "User" },
                "namespace": { "type": "personal", "name": "user", "path": "user", "enterprise_id": 0 }
            }
        ])))
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter =
        GiteeAdapter::new(client, "test-token".to_string()).with_base_url(format!("{}/api/v5", mock_server.uri()));

    let result = adapter.list_repos(1).await.expect("should list repos");

    assert_eq!(result.items[0].owner, "user");
    assert_eq!(result.items[0].owner_type, "user");
    assert_eq!(result.total_pages, 1, "should default to 1 with no Link header");
}

#[tokio::test]
async fn test_gitee_list_issues_paginated() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/issues"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!([
                    {
                        "number": 1,
                        "title": "Bug report",
                        "state": "open",
                        "user": { "id": 1, "login": "reporter", "name": "", "avatar_url": "" },
                        "labels": [{ "name": "bug" }],
                        "created_at": "2025-01-01T00:00:00Z"
                    }
                ]))
                .insert_header(
                    "Link",
                    "<https://gitee.com/api/v5/repos/octocat/hello-world/issues?page=5&state=open>; rel=\"last\"",
                ),
        )
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter =
        GiteeAdapter::new(client, "test-token".to_string()).with_base_url(format!("{}/api/v5", mock_server.uri()));

    let result = adapter
        .list_issues("octocat", "hello-world", &mergebeacon_lib::models::IssueState::Open, 1)
        .await
        .expect("should list issues");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.total_pages, 5, "should parse last page from Link header");
}

#[tokio::test]
async fn test_gitee_list_issues_single_page() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/issues"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "number": 1,
                "title": "Bug report",
                "state": "open",
                "user": { "id": 1, "login": "reporter", "name": "", "avatar_url": "" },
                "labels": [{ "name": "bug" }],
                "created_at": "2025-01-01T00:00:00Z"
            }
        ])))
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter =
        GiteeAdapter::new(client, "test-token".to_string()).with_base_url(format!("{}/api/v5", mock_server.uri()));

    let result = adapter
        .list_issues("octocat", "hello-world", &mergebeacon_lib::models::IssueState::Open, 1)
        .await
        .expect("should list issues");

    assert_eq!(result.total_pages, 1, "should default to 1 with no Link header");
}

#[tokio::test]
async fn test_gitee_auth_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v5/user"))
        .and(query_param("access_token", "invalid-token"))
        .respond_with(ResponseTemplate::new(401).set_body_string(r#"{"message":"Bad credentials"}"#))
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter =
        GiteeAdapter::new(client, "invalid-token".to_string()).with_base_url(format!("{}/api/v5", mock_server.uri()));

    let result = adapter.current_user().await;
    assert!(result.is_err(), "should return error for bad credentials");
}

#[tokio::test]
async fn test_gitee_create_issue() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v5/repos/octocat/hello-world/issues"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "number": 99,
            "title": "Memory leak",
            "body": "Steps: 1. Login 2. Logout",
            "state": "open",
            "user": { "id": 1, "login": "reporter", "name": "", "avatar_url": "" },
            "labels": [{ "name": "bug" }],
            "created_at": "2025-01-05T00:00:00Z",
            "updated_at": "2025-01-05T00:00:00Z"
        })))
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter =
        GiteeAdapter::new(client, "test-token".to_string()).with_base_url(format!("{}/api/v5", mock_server.uri()));

    let issue = adapter
        .create_issue("octocat", "hello-world", "Memory leak", "Steps: 1. Login 2. Logout", &["bug".to_string()])
        .await
        .expect("should create issue");

    assert_eq!(issue.number, 99);
    assert_eq!(issue.title, "Memory leak");
}

#[tokio::test]
async fn test_gitee_create_pr_comment_left_side() {
    let mock_server = MockServer::start().await;

    let expected_body = serde_json::json!({
        "body": "Old code issue",
        "commit_id": "abc123",
        "path": "src/main.rs",
        "position": 5,
    });
    Mock::given(method("POST"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42/comments"))
        .and(query_param("access_token", "test-token"))
        .and(body_json(&expected_body))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": 3001, "body": "Old code issue", "commit_id": "abc123",
            "path": "src/main.rs", "position": 5, "new_line": 5,
            "user": { "id": 1, "login": "testuser", "name": "", "avatar_url": "" },
            "created_at": "2025-01-04T00:00:00Z", "comment_type": "diff_comment"
        })))
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter =
        GiteeAdapter::new(client, "test-token".to_string()).with_base_url(format!("{}/api/v5", mock_server.uri()));

    let comment = adapter
        .create_pr_comment("octocat", "hello-world", 42, "abc123", "src/main.rs", None, 5, "left", "Old code issue")
        .await
        .expect("should create left-side PR comment");

    assert_eq!(comment.side.as_deref(), Some("left"));
}

#[tokio::test]
async fn test_gitee_create_pr_comment_multi_line() {
    let mock_server = MockServer::start().await;

    let expected_body = serde_json::json!({
        "body": "[L10-L15]\nMulti-line comment",
        "commit_id": "abc123",
        "path": "src/main.rs",
        "position": 15,
    });

    Mock::given(method("POST"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42/comments"))
        .and(query_param("access_token", "test-token"))
        .and(body_json(&expected_body))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": 4001, "body": "[L10-L15]\nMulti-line comment", "commit_id": "abc123",
            "path": "src/main.rs", "position": 15, "new_line": 15,
            "user": { "id": 1, "login": "testuser", "name": "", "avatar_url": "" },
            "created_at": "2025-01-04T00:00:00Z"
        })))
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter =
        GiteeAdapter::new(client, "test-token".to_string()).with_base_url(format!("{}/api/v5", mock_server.uri()));

    let result = adapter
        .create_pr_comment(
            "octocat",
            "hello-world",
            42,
            "abc123",
            "src/main.rs",
            Some(10),
            15,
            "right",
            "Multi-line comment",
        )
        .await;

    assert!(result.is_ok(), "should create multi-line PR comment");
}

#[tokio::test]
async fn test_gitee_list_repos_prefers_pagination_headers() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v5/user/repos"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-total-pages", "4")
                .insert_header("x-total-count", "321")
                .set_body_json(serde_json::json!([])),
        )
        .mount(&mock_server)
        .await;

    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));
    let result = adapter.list_repos(1).await.expect("should list repos");

    assert_eq!(result.total_pages, 4);
    assert_eq!(result.total_count, 321);
}

#[tokio::test]
async fn test_gitee_rejects_unsupported_review_without_request() {
    let mock_server = MockServer::start().await;
    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));

    let result =
        adapter.create_review("owner", "repo", 1, "approve", &mergebeacon_lib::models::ReviewEvent::Approve, &[]).await;

    assert!(matches!(result, Err(mergebeacon_lib::error::AppError::NotImplemented(_))));
    assert!(mock_server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn test_gitee_mergeable_keeps_unknown_gates_but_infers_no_conflicts() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "state": "open",
            "draft": false,
            "mergeable": true,
            "head": {"sha": "head-sha"}
        })))
        .mount(&mock_server)
        .await;

    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));
    let readiness = adapter.get_merge_readiness("octocat", "hello-world", 42).await.expect("readiness");

    assert_eq!(readiness.status, mergebeacon_lib::models::ReadinessState::Unknown);
    assert_ne!(readiness.status, mergebeacon_lib::models::ReadinessState::Ready);
    assert_eq!(readiness.checks_status, mergebeacon_lib::models::ReadinessState::Unknown);
    assert_eq!(readiness.approvals_status, mergebeacon_lib::models::ReadinessState::Unknown);
    assert_eq!(readiness.has_conflicts, Some(false));
}

#[tokio::test]
async fn test_gitee_merge_readiness_reports_unfinished_review_and_test() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/43"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "state": "open",
            "draft": false,
            "mergeable": true,
            "assignees_number": 1,
            "assignees": [
                {"login": "reviewer", "accept": false}
            ],
            "api_reviewers_number": 1,
            "api_reviewers": [
                {"login": "api-reviewer-1", "accept": true},
                {"login": "api-reviewer-2", "accept": true}
            ],
            "testers_number": 1,
            "testers": [
                {"login": "tester", "accept": false}
            ],
            "head": {"sha": "head-sha"}
        })))
        .mount(&mock_server)
        .await;

    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));
    let readiness = adapter.get_merge_readiness("octocat", "hello-world", 43).await.expect("readiness");

    assert_eq!(readiness.status, mergebeacon_lib::models::ReadinessState::Blocked);
    assert_eq!(readiness.checks_status, mergebeacon_lib::models::ReadinessState::Pending);
    assert_eq!(readiness.approvals_status, mergebeacon_lib::models::ReadinessState::Blocked);
    assert_eq!(readiness.approvals_required, Some(2));
    assert_eq!(readiness.approvals_received, Some(1));
    assert!(readiness.blocking_reasons.iter().any(|reason| {
        reason.code == mergebeacon_lib::models::MergeBlockingReasonCode::ApprovalsRequired
            && reason.message == "还需要 1 个审批"
    }));
    assert!(readiness.blocking_reasons.iter().any(|reason| {
        reason.code == mergebeacon_lib::models::MergeBlockingReasonCode::ChecksPending
            && reason.message == "还需要 1 个测试通过"
    }));
}

#[tokio::test]
async fn test_gitee_merge_readiness_is_ready_after_required_review_and_test() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/44"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "state": "open",
            "draft": false,
            "mergeable": true,
            "assignees_number": 1,
            "assignees": [
                {"login": "reviewer-1", "accept": true},
                {"login": "reviewer-2", "accept": false}
            ],
            "api_reviewers_number": 1,
            "api_reviewers": [
                {"login": "api-reviewer", "accept": true}
            ],
            "testers_number": 1,
            "testers": [
                {"login": "tester", "accept": true}
            ],
            "head": {"sha": "head-sha"}
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "permission": {"admin": false, "push": true, "pull": true}
        })))
        .mount(&mock_server)
        .await;

    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));
    let readiness = adapter.get_merge_readiness("octocat", "hello-world", 44).await.expect("readiness");

    assert_eq!(readiness.status, mergebeacon_lib::models::ReadinessState::Ready);
    assert_eq!(readiness.checks_status, mergebeacon_lib::models::ReadinessState::Ready);
    assert_eq!(readiness.approvals_status, mergebeacon_lib::models::ReadinessState::Ready);
    assert_eq!(readiness.approvals_required, Some(2));
    assert_eq!(readiness.approvals_received, Some(2));
    assert_eq!(readiness.has_merge_permission, Some(true));
    assert!(readiness.blocking_reasons.is_empty());
}

#[tokio::test]
async fn test_gitee_merge_readiness_blocks_user_without_push_permission() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/45"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "state": "open",
            "draft": false,
            "mergeable": true,
            "assignees_number": 0,
            "assignees": [],
            "api_reviewers_number": 0,
            "api_reviewers": [],
            "testers_number": 0,
            "testers": [],
            "head": {"sha": "head-sha"}
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "permissions": {"admin": false, "push": false, "pull": true}
        })))
        .mount(&mock_server)
        .await;

    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));
    let readiness = adapter.get_merge_readiness("octocat", "hello-world", 45).await.expect("readiness");

    assert_eq!(readiness.status, mergebeacon_lib::models::ReadinessState::Blocked);
    assert_eq!(readiness.has_merge_permission, Some(false));
    assert!(readiness
        .blocking_reasons
        .iter()
        .any(|reason| reason.code == mergebeacon_lib::models::MergeBlockingReasonCode::NoMergePermission));
}

#[tokio::test]
async fn test_gitee_file_content_uses_revision_and_auth_query() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/contents/src/lib.rs"))
        .and(query_param("ref", "head-sha"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "encoding": "base64",
            "content": "Zm4gbWFpbigpIHt9Cg==",
            "size": 13
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));
    let content =
        adapter.get_pr_file_content("octocat", "hello-world", "src/lib.rs", "head-sha").await.expect("file content");

    assert_eq!(content.content, "fn main() {}\n");
    assert!(!content.truncated);
}

#[tokio::test]
async fn test_gitee_pr_detail_exposes_base_and_head_revisions() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42"))
        .and(query_param("access_token", "test-token"))
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
            "assignees": [{"id": 2, "login": "reviewer", "name": "Reviewer", "avatar_url": ""}],
            "api_reviewers": [{"id": 3, "login": "api-reviewer", "name": "API Reviewer", "avatar_url": ""}],
            "testers": [{"id": 4, "login": "tester", "name": "Tester", "avatar_url": ""}],
            "milestone": {"id": 9, "number": 4, "title": "0.6.0"}
        })))
        .mount(&mock_server)
        .await;

    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));
    let detail = adapter.get_pull_request("octocat", "hello-world", 42).await.expect("PR detail");

    assert_eq!(detail.base_sha, "base-sha");
    assert_eq!(detail.head_sha, "head-sha");
    assert_eq!(detail.draft, None);
    assert_eq!(
        detail.reviewers.iter().map(|value| value.login.as_str()).collect::<Vec<_>>(),
        vec!["reviewer", "api-reviewer"]
    );
    assert_eq!(detail.assignees.iter().map(|value| value.login.as_str()).collect::<Vec<_>>(), vec!["tester"]);
    assert_eq!(detail.milestone.as_ref().map(|value| value.title.as_str()), Some("0.6.0"));
}

#[tokio::test]
async fn test_gitee_review_inbox_filters_pending_reviewers_and_testers() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/user"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 7,
            "login": "reviewer",
            "name": "Reviewer",
            "avatar_url": ""
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v5/user/repos"))
        .and(query_param("visibility", "all"))
        .and(query_param("sort", "updated"))
        .and(query_param("direction", "desc"))
        .and(query_param("per_page", "100"))
        .and(query_param("page", "1"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 1, "full_name": "team/project" }
        ])))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/team/project/pulls"))
        .and(query_param("state", "open"))
        .and(query_param("sort", "updated"))
        .and(query_param("direction", "desc"))
        .and(query_param("assignee", "reviewer"))
        .and(query_param("per_page", "100"))
        .and(query_param("page", "1"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "number": 11,
                "title": "Pending review",
                "created_at": "2025-01-01T00:00:00Z",
                "updated_at": "2025-01-03T00:00:00Z",
                "user": { "id": 1, "login": "dev", "name": "Dev", "avatar_url": "" },
                "labels": [],
                "mergeable": true,
                "assignees_number": 1,
                "api_reviewers_number": 0,
                "testers_number": 1,
                "assignees": [{ "login": "reviewer", "accept": false }],
                "api_reviewers": [],
                "testers": [{ "login": "reviewer", "accept": false }]
            },
            {
                "number": 12,
                "title": "Already accepted",
                "created_at": "2025-01-01T00:00:00Z",
                "updated_at": "2025-01-02T00:00:00Z",
                "user": { "id": 2, "login": "dev2", "name": "Dev 2", "avatar_url": "" },
                "labels": [],
                "assignees": [{ "login": "reviewer", "accept": true }],
                "api_reviewers": [],
                "testers": []
            },
        ])))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v5/repos/team/project/pulls"))
        .and(query_param("state", "open"))
        .and(query_param("sort", "updated"))
        .and(query_param("direction", "desc"))
        .and(query_param("tester", "reviewer"))
        .and(query_param("per_page", "100"))
        .and(query_param("page", "1"))
        .and(query_param("access_token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "number": 11,
                "title": "Pending review and test",
                "created_at": "2025-01-01T00:00:00Z",
                "updated_at": "2025-01-03T00:00:00Z",
                "user": { "id": 1, "login": "dev", "name": "Dev", "avatar_url": "" },
                "labels": [],
                "mergeable": true,
                "assignees_number": 1,
                "api_reviewers_number": 0,
                "testers_number": 1,
                "assignees": [{ "login": "reviewer", "accept": false }],
                "api_reviewers": [],
                "testers": [{ "login": "reviewer", "accept": false }]
            },
            {
                "number": 13,
                "title": "Pending test",
                "created_at": "2025-01-01T00:00:00Z",
                "updated_at": "2025-01-04T00:00:00Z",
                "head": { "sha": "head-13" },
                "comments_count": 6,
                "user": { "id": 3, "login": "dev3", "name": "Dev 3", "avatar_url": "" },
                "labels": [],
                "mergeable": true,
                "testers_number": 1,
                "assignees": [],
                "api_reviewers": [],
                "testers": [{ "login": "reviewer", "accept": false }]
            },
            {
                "number": 14,
                "title": "Test already accepted",
                "created_at": "2025-01-01T00:00:00Z",
                "updated_at": "2025-01-05T00:00:00Z",
                "user": { "id": 4, "login": "dev4", "name": "Dev 4", "avatar_url": "" },
                "labels": [],
                "assignees": [],
                "api_reviewers": [],
                "testers": [{ "login": "reviewer", "accept": true }]
            }
        ])))
        .mount(&mock_server)
        .await;

    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));
    let result = adapter
        .list_review_inbox(&ReviewInboxCategory::ReviewRequested, 1, 20)
        .await
        .expect("should list review inbox");

    assert_eq!(result.items.len(), 2);
    assert_eq!(result.items.iter().map(|item| item.summary.number).collect::<Vec<_>>(), vec![13, 11]);
    assert_eq!(result.items[0].repository_full_name, "team/project");
    assert_eq!(result.items[0].platform, "gitee");
    assert_eq!(result.items[0].relationships, vec![ReviewInboxRelationship::Tester]);
    assert_eq!(result.items[0].status.status, ReadinessState::Pending);
    assert_eq!(result.items[0].head_sha.as_deref(), Some("head-13"));
    assert_eq!(result.items[0].comments_count, Some(6));
    let combined = result.items.iter().find(|item| item.summary.number == 11).expect("combined PR");
    assert_eq!(combined.relationships, vec![ReviewInboxRelationship::Reviewer, ReviewInboxRelationship::Tester]);
    assert_eq!(combined.status.status, ReadinessState::Blocked);
    assert_eq!(combined.status.checks_status, ReadinessState::Pending);
    assert_eq!(combined.status.approvals_status, ReadinessState::Blocked);
}

#[tokio::test]
async fn test_gitee_review_inbox_aggregates_repositories_and_paginates() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 7, "login": "author", "name": "Author", "avatar_url": ""
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v5/user/repos"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 1, "full_name": "team/newer" },
            { "id": 2, "full_name": "team/older" }
        ])))
        .mount(&mock_server)
        .await;
    for (repo, number, updated_at) in [("newer", 21, "2025-01-03T00:00:00Z"), ("older", 22, "2025-01-01T00:00:00Z")] {
        Mock::given(method("GET"))
            .and(path(format!("/api/v5/repos/team/{repo}/pulls")))
            .and(query_param("author", "author"))
            .and(query_param("state", "open"))
            .and(query_param("access_token", "test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                "number": number,
                "title": format!("PR {number}"),
                "created_at": "2025-01-01T00:00:00Z",
                "updated_at": updated_at,
                "user": { "id": 7, "login": "author", "name": "Author", "avatar_url": "" },
                "labels": [],
                "mergeable": true
            }])))
            .mount(&mock_server)
            .await;
    }

    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));
    let result = adapter
        .list_review_inbox(&ReviewInboxCategory::Authored, 2, 1)
        .await
        .expect("should aggregate and paginate review inbox");

    assert_eq!(result.total_count, 2);
    assert_eq!(result.total_pages, 2);
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].summary.number, 22);
    assert_eq!(result.items[0].repository_full_name, "team/older");
    assert_eq!(result.items[0].relationships, vec![ReviewInboxRelationship::Author]);
    assert_eq!(result.items[0].status.status, ReadinessState::Unknown);
    assert_eq!(result.items[0].status.checks_status, ReadinessState::Unknown);
    assert_eq!(result.items[0].status.approvals_status, ReadinessState::Unknown);
}

#[tokio::test]
async fn test_gitee_review_inbox_sanitizes_html_errors() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 7, "login": "reviewer", "name": "Reviewer", "avatar_url": ""
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v5/user/repos"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 1, "full_name": "team/project" }
        ])))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/team/project/pulls"))
        .respond_with(ResponseTemplate::new(404).set_body_string("<html><body>Not Found</body></html>"))
        .mount(&mock_server)
        .await;

    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));
    let error = adapter
        .list_review_inbox(&ReviewInboxCategory::ReviewRequested, 1, 20)
        .await
        .expect_err("HTML error response must fail");
    let message = error.to_string();

    assert!(message.contains("非 JSON 错误页面"));
    assert!(!message.contains("<html>"));
    assert!(!message.contains("test-token"));
}

#[tokio::test]
async fn test_gitee_replies_edits_and_deletes_review_comment() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42/comments"))
        .and(body_json(serde_json::json!({ "body": "回复", "in_reply_to": 100 })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/comments/100"))
        .and(body_json(serde_json::json!({ "body": "编辑后" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/comments/100"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".into())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));
    adapter
        .reply_to_review_thread("octocat", "hello-world", 42, "100", "100", "回复")
        .await
        .expect("reply should succeed");
    adapter
        .update_review_comment("octocat", "hello-world", 42, "100", "100", "编辑后")
        .await
        .expect("edit should succeed");
    adapter.delete_review_comment("octocat", "hello-world", 42, "100", "100").await.expect("delete should succeed");
}

#[tokio::test]
async fn test_gitee_updates_pull_request_metadata_without_unsupported_fields() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v5/repos/octocat/hello-world/milestones"))
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
    Mock::given(method("DELETE"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42/assignees"))
        .and(body_json(serde_json::json!({ "assignees": "old-reviewer" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42/assignees"))
        .and(body_json(serde_json::json!({ "assignees": "new-reviewer" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42/testers"))
        .and(body_json(serde_json::json!({ "testers": "old-tester" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42/testers"))
        .and(body_json(serde_json::json!({ "testers": "new-tester" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42"))
        .and(body_json(serde_json::json!({
            "title": "New title",
            "body": "New body",
            "labels": "new-label",
            "milestone_number": 5
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
        mergeable: None,
        head_sha: "head".into(),
        base_sha: "base".into(),
        draft: None,
        reviewers: vec![User {
            id: serde_json::json!(2),
            login: "old-reviewer".into(),
            name: "".into(),
            avatar_url: "".into(),
        }],
        assignees: vec![User {
            id: serde_json::json!(8),
            login: "old-tester".into(),
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
        assignees: vec!["new-tester".into()],
        labels: vec!["new-label".into()],
        milestone: Some("0.7.0".into()),
        expected_updated_at: current.summary.updated_at.clone(),
    };
    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));

    let result = adapter
        .update_pull_request_metadata("octocat", "hello-world", 42, &current, &update)
        .await
        .expect("metadata update should succeed");

    assert!(result.failures.is_empty());
    assert_eq!(
        result.updated_fields,
        vec![
            PrMetadataField::TitleBody,
            PrMetadataField::Reviewers,
            PrMetadataField::Assignees,
            PrMetadataField::Labels,
            PrMetadataField::Milestone,
        ]
    );
}

#[tokio::test]
async fn test_gitee_reports_reviewer_success_when_pull_patch_fails() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42/assignees"))
        .and(body_json(serde_json::json!({ "assignees": "new-reviewer" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42"))
        .and(body_json(serde_json::json!({ "title": "New title", "body": "Body" })))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "message": "patch failed"
        })))
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
            labels: Vec::new(),
            status: None,
        },
        body: "Body".into(),
        source_branch: "feature".into(),
        target_branch: "main".into(),
        mergeable: None,
        head_sha: "head".into(),
        base_sha: "base".into(),
        draft: None,
        reviewers: Vec::new(),
        assignees: Vec::new(),
        milestone: None,
        metadata_permissions: PrMetadataPermissions::default(),
    };
    let update = PrMetadataUpdate {
        title: "New title".into(),
        body: current.body.clone(),
        draft: None,
        reviewers: vec!["new-reviewer".into()],
        assignees: Vec::new(),
        labels: Vec::new(),
        milestone: None,
        expected_updated_at: current.summary.updated_at.clone(),
    };
    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));

    let result = adapter
        .update_pull_request_metadata("octocat", "hello-world", 42, &current, &update)
        .await
        .expect("partial metadata update should return an outcome");

    assert_eq!(result.updated_fields, vec![PrMetadataField::Reviewers]);
    assert_eq!(result.failures.len(), 1);
    assert_eq!(result.failures[0].field, PrMetadataField::TitleBody);
}

#[tokio::test]
async fn test_gitee_clears_pull_request_milestone_with_zero_number() {
    let mock_server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/api/v5/repos/octocat/hello-world/pulls/42"))
        .and(body_json(serde_json::json!({ "milestone_number": 0 })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&mock_server)
        .await;

    let current = PrDetail {
        summary: PrSummary {
            number: 42,
            title: "Title".into(),
            author: User { id: serde_json::json!(1), login: "author".into(), name: "".into(), avatar_url: "".into() },
            state: PrState::Open,
            created_at: "2026-07-16T00:00:00Z".into(),
            updated_at: "2026-07-17T00:00:00Z".into(),
            labels: Vec::new(),
            status: None,
        },
        body: "Body".into(),
        source_branch: "feature".into(),
        target_branch: "main".into(),
        mergeable: None,
        head_sha: "head".into(),
        base_sha: "base".into(),
        draft: None,
        reviewers: Vec::new(),
        assignees: Vec::new(),
        milestone: Some(PrMilestone { id: serde_json::json!(9), number: Some(4), title: "0.6.0".into() }),
        metadata_permissions: PrMetadataPermissions::default(),
    };
    let update = PrMetadataUpdate {
        title: current.summary.title.clone(),
        body: current.body.clone(),
        draft: None,
        reviewers: Vec::new(),
        assignees: Vec::new(),
        labels: Vec::new(),
        milestone: None,
        expected_updated_at: current.summary.updated_at.clone(),
    };
    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));

    let result = adapter
        .update_pull_request_metadata("octocat", "hello-world", 42, &current, &update)
        .await
        .expect("milestone removal should succeed");

    assert!(result.failures.is_empty());
    assert_eq!(result.updated_fields, vec![PrMetadataField::Milestone]);
}
