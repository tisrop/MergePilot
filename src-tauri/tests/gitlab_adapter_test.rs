use mergepilot_lib::error::AppError;
use mergepilot_lib::http_client::HttpClient;
use mergepilot_lib::models::{PrState, ReviewEvent};
use mergepilot_lib::platform::{gitlab::GitLabAdapter, GitPlatform};
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

    let adapter = GitLabAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(mock_server.uri());
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
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-next-page", "2")
                .set_body_json(serde_json::json!([])),
        )
        .mount(&mock_server)
        .await;

    let adapter = GitLabAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(mock_server.uri());
    let result = adapter.list_repos(1).await.expect("should list repos");

    assert_eq!(result.total_pages, 2);
}

#[tokio::test]
async fn test_gitlab_nested_subgroup_project_path_is_fully_encoded() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(
            "/api/v4/projects/group%2Fsubgroup%2Frepo/merge_requests",
        ))
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

    let adapter = GitLabAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(mock_server.uri());
    let result = adapter
        .list_pull_requests("group/subgroup", "repo", &PrState::Open, 1, 20)
        .await
        .expect("should list merge requests");

    assert!(result.items.is_empty());
}

#[tokio::test]
async fn test_gitlab_rejects_unsupported_review_without_request() {
    let mock_server = MockServer::start().await;
    let adapter = GitLabAdapter::new(HttpClient::new(), "test-token".to_string())
        .with_base_url(mock_server.uri());

    let result = adapter
        .create_review("group", "repo", 1, "approve", &ReviewEvent::Approve, &[])
        .await;

    assert!(matches!(result, Err(AppError::NotImplemented(_))));
    assert!(mock_server.received_requests().await.unwrap().is_empty());
}
