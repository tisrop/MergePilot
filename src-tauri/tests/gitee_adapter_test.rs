use mergebeacon_lib::http_client::HttpClient;
use mergebeacon_lib::platform::{gitee::GiteeAdapter, GitPlatform};
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

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
                        "merged_at": "2025-01-04T00:00:00Z",
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
    assert_eq!(result.items[1].number, 101);
    assert!(matches!(result.items[1].state, mergebeacon_lib::models::PrState::Merged));
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

    let result = adapter
        .create_pr_comment("octocat", "hello-world", 42, "abc123", "src/main.rs", None, 10, "right", "Good catch!")
        .await;

    assert!(result.is_ok(), "should create PR comment");
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
                "new_line": 42,
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
            }
        ])))
        .mount(&mock_server)
        .await;

    let client = HttpClient::new();
    let adapter =
        GiteeAdapter::new(client, "test-token".to_string()).with_base_url(format!("{}/api/v5", mock_server.uri()));

    let comments = adapter.list_pr_comments("octocat", "hello-world", 42).await.expect("should list PR comments");

    assert_eq!(comments.len(), 3);
    assert_eq!(comments[0].body, "Nice fix!");
    assert_eq!(comments[0].path, "src/main.rs");
    assert_eq!(comments[0].line, Some(15));
    assert_eq!(comments[0].start_line, None);
    assert_eq!(comments[1].body, "Please add tests");
    assert_eq!(comments[1].path, "src/lib.rs");
    assert_eq!(comments[1].line, Some(42));
    assert_eq!(comments[1].start_line, None);
    assert_eq!(comments[2].body, "Multi-line review");
    assert_eq!(comments[2].path, "src/main.rs");
    assert_eq!(comments[2].line, Some(20));
    assert_eq!(comments[2].start_line, Some(10));
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

    let result = adapter
        .create_pr_comment("octocat", "hello-world", 42, "abc123", "src/main.rs", None, 5, "left", "Old code issue")
        .await;

    assert!(result.is_ok(), "should create left-side PR comment");
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
            "mergeable": true
        })))
        .mount(&mock_server)
        .await;

    let adapter = GiteeAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(format!("{}/api/v5", mock_server.uri()));
    let detail = adapter.get_pull_request("octocat", "hello-world", 42).await.expect("PR detail");

    assert_eq!(detail.base_sha, "base-sha");
    assert_eq!(detail.head_sha, "head-sha");
}
