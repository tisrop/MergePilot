use mergebeacon_lib::error::AppError;
use mergebeacon_lib::http_client::HttpClient;
use mergebeacon_lib::models::{PrState, ReviewEvent};
use mergebeacon_lib::platform::{gitlab::GitLabAdapter, GitPlatform};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_gitlab_list_repos_parses_pagination_headers() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects"))
        .and(query_param("page", "2"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-total-pages", "7")
                .insert_header("x-total", "612")
                .set_body_json(serde_json::json!([{
                    "id": 1,
                    "name": "repo",
                    "path_with_namespace": "group/repo",
                    "description": "",
                    "visibility": "private",
                    "namespace": { "kind": "group", "path": "group", "name": "Group" }
                }])),
        )
        .mount(&mock_server)
        .await;

    let adapter = GitLabAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let result = adapter.list_repos(2).await.expect("should list repos");

    assert_eq!(result.page, 2);
    assert_eq!(result.total_pages, 7);
    assert_eq!(result.total_count, 612);
}

#[tokio::test]
async fn test_gitlab_list_repos_uses_next_page_when_total_pages_is_missing() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects"))
        .respond_with(ResponseTemplate::new(200).insert_header("x-next-page", "2").set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;

    let adapter = GitLabAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let result = adapter.list_repos(1).await.expect("should list repos");

    assert_eq!(result.total_pages, 2);
}

#[tokio::test]
async fn test_gitlab_nested_subgroup_project_path_is_fully_encoded() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fsubgroup%2Frepo/merge_requests"))
        .and(query_param("state", "opened"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-total-pages", "1")
                .insert_header("x-total", "0")
                .set_body_json(serde_json::json!([])),
        )
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GitLabAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let result = adapter
        .list_pull_requests("group/subgroup", "repo", &PrState::Open, 1, 20)
        .await
        .expect("should list merge requests");

    assert!(result.items.is_empty());
}

#[tokio::test]
async fn test_gitlab_pr_detail_exposes_head_sha_for_inline_comments() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Frepo/merge_requests/3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "iid": 3,
            "title": "MR",
            "author": gitlab_user(),
            "state": "opened",
            "merged_at": null,
            "created_at": "2026-07-15T10:00:00Z",
            "updated_at": "2026-07-15T10:00:00Z",
            "labels": [],
            "description": "",
            "source_branch": "feature",
            "target_branch": "main",
            "sha": "head-sha"
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GitLabAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let detail = adapter.get_pull_request("group", "repo", 3).await.expect("should load merge request");

    assert_eq!(detail.head_sha, "head-sha");
}

#[tokio::test]
async fn test_gitlab_diff_wraps_bare_hunks_with_unified_diff_headers() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fsubgroup%2Frepo/merge_requests/4/changes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "changes": [
                {
                    "old_path": "src/old.rs",
                    "new_path": "src/new.rs",
                    "diff": "@@ -1 +1 @@\n-old\n+new",
                    "new_file": false,
                    "deleted_file": false,
                    "renamed_file": true
                },
                {
                    "old_path": "src/added.rs",
                    "new_path": "src/added.rs",
                    "diff": "@@ -0,0 +1 @@\n+added\n",
                    "new_file": true,
                    "deleted_file": false,
                    "renamed_file": false
                },
                {
                    "old_path": "src/deleted.rs",
                    "new_path": "src/deleted.rs",
                    "diff": "@@ -1 +0,0 @@\n-deleted",
                    "new_file": false,
                    "deleted_file": true,
                    "renamed_file": false
                }
            ]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GitLabAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let (diff, files) = adapter.get_pr_diff("group/subgroup", "repo", 4).await.expect("should load merge request diff");

    assert_eq!(files.len(), 3);
    assert_eq!(files[0].filename, "src/new.rs");
    assert!(diff.contains("diff --git a/src/old.rs b/src/new.rs\n--- a/src/old.rs\n+++ b/src/new.rs\n@@ -1 +1 @@"));
    assert!(diff.contains("diff --git a/src/added.rs b/src/added.rs\n--- /dev/null\n+++ b/src/added.rs\n@@ -0,0 +1 @@"));
    assert!(diff
        .contains("diff --git a/src/deleted.rs b/src/deleted.rs\n--- a/src/deleted.rs\n+++ /dev/null\n@@ -1 +0,0 @@"));
    assert!(diff.contains("+new\ndiff --git a/src/added.rs"));
    assert!(diff.ends_with('\n'));
}

#[tokio::test]
async fn test_gitlab_rejects_unsupported_review_without_request() {
    let mock_server = MockServer::start().await;
    let adapter = GitLabAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());

    let result = adapter.create_review("group", "repo", 1, "approve", &ReviewEvent::Approve, &[]).await;

    assert!(matches!(result, Err(AppError::NotImplemented(_))));
    assert!(mock_server.received_requests().await.unwrap().is_empty());
}

fn gitlab_user() -> serde_json::Value {
    serde_json::json!({
        "id": 7,
        "username": "reviewer",
        "name": "Reviewer",
        "avatar_url": "https://gitlab.example/avatar.png"
    })
}

fn gitlab_position() -> serde_json::Value {
    serde_json::json!({
        "position_type": "text",
        "base_sha": "base-sha",
        "start_sha": "start-sha",
        "head_sha": "head-sha",
        "old_path": "src/lib.rs",
        "new_path": "src/lib.rs",
        "new_line": 12
    })
}

#[tokio::test]
async fn test_gitlab_create_single_line_comment() {
    use wiremock::matchers::body_json;

    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Fsubgroup%2Frepo/merge_requests/3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "diff_refs": {
                "base_sha": "base-sha",
                "start_sha": "start-sha",
                "head_sha": "head-sha"
            }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let position = gitlab_position();
    Mock::given(method("POST"))
        .and(path("/api/v4/projects/group%2Fsubgroup%2Frepo/merge_requests/3/discussions"))
        .and(body_json(serde_json::json!({
            "body": "please fix",
            "position": position
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": "discussion-1",
            "notes": [{
                "id": 42,
                "body": "please fix",
                "author": gitlab_user(),
                "created_at": "2026-07-15T10:00:00Z",
                "position": gitlab_position()
            }]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GitLabAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let comment = adapter
        .create_pr_comment("group/subgroup", "repo", 3, "head-sha", "src/lib.rs", None, 12, "right", "please fix")
        .await
        .expect("should create GitLab discussion comment");

    assert_eq!(comment.id, serde_json::json!(42));
    assert_eq!(comment.path, "src/lib.rs");
    assert_eq!(comment.line, Some(12));
    assert_eq!(comment.commit_id.as_deref(), Some("head-sha"));
}

#[tokio::test]
async fn test_gitlab_create_multiline_old_side_comment() {
    use sha1::{Digest, Sha1};
    use wiremock::matchers::body_json;

    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Frepo/merge_requests/5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "diff_refs": {
                "base_sha": "base-sha",
                "start_sha": "start-sha",
                "head_sha": "head-sha"
            }
        })))
        .mount(&mock_server)
        .await;

    let hash = format!("{:x}", Sha1::digest(b"src/old.rs"));
    let position = serde_json::json!({
        "position_type": "text",
        "base_sha": "base-sha",
        "start_sha": "start-sha",
        "head_sha": "head-sha",
        "old_path": "src/old.rs",
        "new_path": "src/old.rs",
        "old_line": 9,
        "line_range": {
            "start": {
                "line_code": format!("{hash}_7_0"),
                "type": "old",
                "old_line": 7,
                "new_line": null
            },
            "end": {
                "line_code": format!("{hash}_9_0"),
                "type": "old",
                "old_line": 9,
                "new_line": null
            }
        }
    });
    Mock::given(method("POST"))
        .and(path("/api/v4/projects/group%2Frepo/merge_requests/5/discussions"))
        .and(body_json(serde_json::json!({ "body": "old lines", "position": position })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "notes": [{
                "id": 43,
                "body": "old lines",
                "author": gitlab_user(),
                "created_at": "2026-07-15T10:00:00Z",
                "position": position
            }]
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GitLabAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let comment = adapter
        .create_pr_comment("group", "repo", 5, "head-sha", "src/old.rs", Some(7), 9, "left", "old lines")
        .await
        .expect("should create multiline comment");

    assert_eq!(comment.line, Some(9));
    assert_eq!(comment.start_line, Some(7));
}

#[tokio::test]
async fn test_gitlab_rejects_stale_comment_revision_without_posting() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Frepo/merge_requests/8"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "diff_refs": {
                "base_sha": "base-sha",
                "start_sha": "start-sha",
                "head_sha": "new-head-sha"
            }
        })))
        .mount(&mock_server)
        .await;

    let adapter = GitLabAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let result =
        adapter.create_pr_comment("group", "repo", 8, "old-head-sha", "src/lib.rs", None, 3, "right", "stale").await;

    assert!(matches!(result, Err(AppError::Api(message)) if message.contains("刷新 Diff")));
    let requests = mock_server.received_requests().await.expect("requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method.as_str(), "GET");
}

#[tokio::test]
async fn test_gitlab_list_inline_comments_filters_regular_and_system_notes() {
    let mock_server = MockServer::start().await;
    let mut old_position = gitlab_position();
    old_position["new_line"] = serde_json::json!(10);
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Frepo/merge_requests/9/discussions"))
        .and(query_param("per_page", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "id": "inline",
                "notes": [{
                    "id": 50,
                    "body": "inline note",
                    "system": false,
                    "author": gitlab_user(),
                    "created_at": "2026-07-15T10:00:00Z",
                    "position": gitlab_position(),
                    "original_position": old_position
                }]
            },
            {
                "id": "regular",
                "notes": [{
                    "id": 51,
                    "body": "regular note",
                    "system": false,
                    "author": gitlab_user(),
                    "created_at": "2026-07-15T10:00:00Z",
                    "position": null
                }]
            },
            {
                "id": "system",
                "notes": [{
                    "id": 52,
                    "body": "system note",
                    "system": true,
                    "author": gitlab_user(),
                    "created_at": "2026-07-15T10:00:00Z",
                    "position": gitlab_position()
                }]
            }
        ])))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GitLabAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let comments = adapter.list_pr_comments("group", "repo", 9).await.expect("should list inline comments");

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].id, serde_json::json!(50));
    assert_eq!(comments[0].line, Some(12));
    assert_eq!(comments[0].original_line, Some(10));
}

#[tokio::test]
async fn test_gitlab_rejects_invalid_comment_input_before_request() {
    let mock_server = MockServer::start().await;
    let adapter = GitLabAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());

    let invalid_side =
        adapter.create_pr_comment("group", "repo", 1, "head", "src/lib.rs", None, 1, "middle", "comment").await;
    let invalid_range =
        adapter.create_pr_comment("group", "repo", 1, "head", "src/lib.rs", Some(3), 2, "right", "comment").await;

    assert!(matches!(invalid_side, Err(AppError::Api(_))));
    assert!(matches!(invalid_range, Err(AppError::Api(_))));
    assert!(mock_server.received_requests().await.expect("requests").is_empty());
}

#[tokio::test]
async fn test_gitlab_list_reviews_excludes_inline_discussion_notes() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Frepo/merge_requests/10/notes"))
        .and(query_param("per_page", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "id": 60,
                "body": "general review",
                "system": false,
                "author": gitlab_user(),
                "created_at": "2026-07-15T10:00:00Z",
                "position": null
            },
            {
                "id": 61,
                "body": "inline note",
                "system": false,
                "author": gitlab_user(),
                "created_at": "2026-07-15T10:00:00Z",
                "position": gitlab_position()
            }
        ])))
        .expect(1)
        .mount(&mock_server)
        .await;

    let adapter = GitLabAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let reviews = adapter.list_reviews("group", "repo", 10).await.expect("should list reviews");

    assert_eq!(reviews.len(), 1);
    assert_eq!(reviews[0].id, serde_json::json!(60));
}

#[tokio::test]
async fn test_gitlab_merge_readiness_keeps_failed_pipeline_lookup_from_being_ready() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Frepo/merge_requests/9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "iid": 9,
            "state": "opened",
            "draft": false,
            "detailed_merge_status": "mergeable",
            "has_conflicts": false,
            "sha": "head-sha",
            "diff_refs": {"head_sha": "head-sha"},
            "user": {"can_merge": true}
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Frepo/merge_requests/9/pipelines"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Frepo/merge_requests/9/approvals"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "approvals_required": 0,
            "approvals_left": 0,
            "approved_by": []
        })))
        .mount(&mock_server)
        .await;

    let adapter = GitLabAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let readiness = adapter.get_merge_readiness("group", "repo", 9).await.expect("readiness");

    assert_eq!(readiness.status, mergebeacon_lib::models::ReadinessState::Unknown);
    assert_eq!(readiness.checks_status, mergebeacon_lib::models::ReadinessState::Unknown);
    assert_eq!(readiness.head_sha, "head-sha");
}

#[tokio::test]
async fn test_gitlab_merge_readiness_allows_merge_without_configured_pipeline() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Frepo/merge_requests/12"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "iid": 12,
            "state": "opened",
            "draft": false,
            "detailed_merge_status": "mergeable",
            "has_conflicts": false,
            "sha": "head-sha",
            "user": {"can_merge": true}
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Frepo/merge_requests/12/pipelines"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Frepo/merge_requests/12/approvals"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "approvals_required": 0,
            "approvals_left": 0,
            "approved_by": []
        })))
        .mount(&mock_server)
        .await;

    let adapter = GitLabAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let readiness = adapter.get_merge_readiness("group", "repo", 12).await.expect("readiness");

    assert_eq!(readiness.status, mergebeacon_lib::models::ReadinessState::Ready);
    assert_eq!(readiness.checks_status, mergebeacon_lib::models::ReadinessState::Ready);
    assert_eq!(readiness.has_merge_permission, Some(true));
}

#[tokio::test]
async fn test_gitlab_merge_readiness_blocks_user_without_merge_permission() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Frepo/merge_requests/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "iid": 10,
            "state": "opened",
            "draft": false,
            "detailed_merge_status": "mergeable",
            "has_conflicts": false,
            "sha": "head-sha",
            "user": {"can_merge": false}
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Frepo/merge_requests/10/pipelines"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"status": "success"}
        ])))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Frepo/merge_requests/10/approvals"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "approvals_required": 0,
            "approvals_left": 0,
            "approved_by": []
        })))
        .mount(&mock_server)
        .await;

    let adapter = GitLabAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let readiness = adapter.get_merge_readiness("group", "repo", 10).await.expect("readiness");

    assert_eq!(readiness.status, mergebeacon_lib::models::ReadinessState::Blocked);
    assert_eq!(readiness.has_merge_permission, Some(false));
    assert!(readiness
        .blocking_reasons
        .iter()
        .any(|reason| reason.code == mergebeacon_lib::models::MergeBlockingReasonCode::NoMergePermission));
}

#[tokio::test]
async fn test_gitlab_merge_readiness_is_ready_with_merge_permission() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Frepo/merge_requests/11"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "iid": 11,
            "state": "opened",
            "draft": false,
            "detailed_merge_status": "mergeable",
            "has_conflicts": false,
            "sha": "head-sha",
            "user": {"can_merge": true}
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Frepo/merge_requests/11/pipelines"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"status": "success"}
        ])))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v4/projects/group%2Frepo/merge_requests/11/approvals"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "approvals_required": 0,
            "approvals_left": 0,
            "approved_by": []
        })))
        .mount(&mock_server)
        .await;

    let adapter = GitLabAdapter::new(HttpClient::new(), "test-token".to_string()).with_base_url(mock_server.uri());
    let readiness = adapter.get_merge_readiness("group", "repo", 11).await.expect("readiness");

    assert_eq!(readiness.status, mergebeacon_lib::models::ReadinessState::Ready);
    assert_eq!(readiness.has_merge_permission, Some(true));
    assert!(readiness.blocking_reasons.is_empty());
}
