use crate::models::{Paginated, ReviewInboxCategory, ReviewInboxItem};
use crate::state::AppState;
use tauri::State;

use super::auth::build_platform;

fn parse_request(
    category: Option<&str>,
    page: Option<u32>,
    per_page: Option<u32>,
) -> Result<(ReviewInboxCategory, u32, u32), String> {
    let category = match category.unwrap_or("review_requested") {
        "review_requested" => ReviewInboxCategory::ReviewRequested,
        "authored" => ReviewInboxCategory::Authored,
        value => return Err(format!("未知的评审收件箱分类：{value}")),
    };
    let page = page.unwrap_or(1);
    let per_page = per_page.unwrap_or(20);
    if page == 0 {
        return Err("评审收件箱页码必须大于等于 1".into());
    }
    if !(1..=100).contains(&per_page) {
        return Err("评审收件箱每页数量必须在 1 到 100 之间".into());
    }
    Ok((category, page, per_page))
}

#[tauri::command]
pub async fn review_inbox_list(
    state: State<'_, AppState>,
    platform: String,
    category: Option<String>,
    page: Option<u32>,
    per_page: Option<u32>,
) -> Result<Paginated<ReviewInboxItem>, String> {
    let (category, page, per_page) = parse_request(category.as_deref(), page, per_page)?;
    let adapter = build_platform(&platform, &state).map_err(|error| error.to_string())?;
    adapter.list_review_inbox(&category, page, per_page).await.map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::parse_request;
    use crate::models::ReviewInboxCategory;

    #[test]
    fn validates_review_inbox_filters_and_pagination() {
        assert!(matches!(
            parse_request(Some("review_requested"), Some(1), Some(20)),
            Ok((ReviewInboxCategory::ReviewRequested, 1, 20))
        ));
        assert!(matches!(parse_request(Some("authored"), None, None), Ok((ReviewInboxCategory::Authored, 1, 20))));
        assert!(parse_request(Some("mentioned"), Some(1), Some(20)).is_err());
        assert!(parse_request(None, Some(0), Some(20)).is_err());
        assert!(parse_request(None, Some(1), Some(101)).is_err());
    }
}
