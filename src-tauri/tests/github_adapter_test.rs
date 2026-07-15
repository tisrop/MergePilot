use mergebeacon_lib::http_client::HttpClient;
use mergebeacon_lib::platform::{github::GitHubAdapter, GitPlatform};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

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
async fn test_github_list_prs() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/octocat/hello-world/pulls"))
        .and(query_param("state", "open"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "number": 42,
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
    assert_eq!(result.items[1].number, 43);
    // PR #43 has merged_at set, should be Merged
    assert!(matches!(result.items[1].state, mergebeacon_lib::models::PrState::Merged));
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
