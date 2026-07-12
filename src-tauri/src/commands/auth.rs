use crate::models::*;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn auth_login(
    state: State<'_, AppState>,
    platform: String,
    token: String,
    custom_url: Option<String>,
) -> Result<AuthLoginResult, String> {
    use crate::platform::{
        gitee::GiteeAdapter, github::GitHubAdapter, gitlab::GitLabAdapter, GitPlatform,
    };

    let client = state.http_client.as_ref().clone();
    let custom_url = custom_url.map(|url| crate::platform::normalize_api_base(&platform, &url));

    // Build adapter with custom URL if provided
    let p: Box<dyn GitPlatform> = match platform.as_str() {
        "github" => {
            let adapter = GitHubAdapter::new(client.clone(), token.clone());
            if let Some(ref url) = custom_url {
                Box::new(adapter.with_base_url(url.clone()))
            } else {
                Box::new(adapter)
            }
        }
        "gitlab" => {
            let adapter = GitLabAdapter::new(client.clone(), token.clone());
            if let Some(ref url) = custom_url {
                Box::new(adapter.with_base_url(url.clone()))
            } else {
                Box::new(adapter)
            }
        }
        "gitee" => {
            let adapter = GiteeAdapter::new(client.clone(), token.clone());
            if let Some(ref url) = custom_url {
                Box::new(adapter.with_base_url(url.clone()))
            } else {
                Box::new(adapter)
            }
        }
        _ => return Err(format!("Unknown platform: {}", platform)),
    };

    // Verify token by fetching current user
    let user = p.current_user().await.map_err(|e| e.to_string())?;

    // Store token
    let credential_storage = state
        .token_vault
        .store_token(&platform, &token)
        .map_err(|e| e.to_string())?;

    // Store custom URL if provided
    if let Some(ref url) = custom_url {
        state
            .token_vault
            .store_custom_url(&platform, url)
            .map_err(|e| e.to_string())?;
    }

    Ok(AuthLoginResult {
        user,
        credential_storage,
    })
}

#[tauri::command]
pub async fn auth_logout(state: State<'_, AppState>, platform: String) -> Result<(), String> {
    state
        .token_vault
        .delete_token(&platform)
        .map_err(|e| e.to_string())?;
    state
        .token_vault
        .delete_custom_url(&platform)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn auth_has_any_token(state: State<'_, AppState>) -> Result<bool, String> {
    let platforms = ["github", "gitlab", "gitee"];
    for platform in platforms {
        if state
            .token_vault
            .get_token(platform)
            .map_err(|e| e.to_string())?
            .is_some()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

#[tauri::command]
pub async fn auth_has_token(state: State<'_, AppState>, platform: String) -> Result<bool, String> {
    Ok(state
        .token_vault
        .get_token(&platform)
        .map_err(|e| e.to_string())?
        .is_some())
}

#[tauri::command]
pub async fn auth_check(
    state: State<'_, AppState>,
    platform: String,
) -> Result<Option<User>, String> {
    let p = match build_platform(&platform, &state) {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };
    match p.current_user().await {
        Ok(user) => Ok(Some(user)),
        Err(_) => Ok(None),
    }
}

#[tauri::command]
pub async fn repo_list(
    state: State<'_, AppState>,
    platform: String,
    page: u32,
) -> Result<Paginated<RepoSummary>, String> {
    let p = build_platform(&platform, &state).map_err(|e| e.to_string())?;
    p.list_repos(page).await.map_err(|e| e.to_string())
}

/// Build a platform adapter from state + token.
/// Reads custom URL from vault if one was configured for this platform.
pub(crate) fn build_platform(
    platform: &str,
    state: &AppState,
) -> Result<Box<dyn crate::platform::GitPlatform>, crate::error::AppError> {
    use crate::platform::{gitee::GiteeAdapter, github::GitHubAdapter, gitlab::GitLabAdapter};

    let token = state
        .token_vault
        .get_token(platform)?
        .ok_or_else(|| crate::error::AppError::NotAuthenticated(platform.to_string()))?;
    let client = state.http_client.as_ref().clone();
    let custom_url = state.token_vault.get_custom_url(platform);

    match platform {
        "github" => {
            let adapter = GitHubAdapter::new(client, token);
            Ok(Box::new(if let Some(url) = custom_url {
                adapter.with_base_url(url)
            } else {
                adapter
            }))
        }
        "gitlab" => {
            let adapter = GitLabAdapter::new(client, token);
            Ok(Box::new(if let Some(url) = custom_url {
                adapter.with_base_url(url)
            } else {
                adapter
            }))
        }
        "gitee" => {
            let adapter = GiteeAdapter::new(client, token);
            Ok(Box::new(if let Some(url) = custom_url {
                adapter.with_base_url(url)
            } else {
                adapter
            }))
        }
        _ => Err(crate::error::AppError::Unknown(format!(
            "Unknown platform: {}",
            platform
        ))),
    }
}
