use crate::models::{AiReviewFocus, PrContext};

/// Build the system prompt for AI code review
pub fn build_system_prompt(focus: Option<&AiReviewFocus>, custom_prompt: Option<&str>) -> String {
    if let Some(custom) = custom_prompt {
        return custom.to_string();
    }

    let focus_instruction = match focus.unwrap_or(&AiReviewFocus::All) {
        AiReviewFocus::All => "请全面评审代码，包括逻辑正确性、安全性、性能、代码风格等方面。",
        AiReviewFocus::Security => "请专注于安全漏洞评审：注入攻击、认证授权、敏感信息泄露、加密问题等。",
        AiReviewFocus::Performance => "请专注于性能问题：不必要的分配、阻塞操作、算法复杂度、缓存等。",
        AiReviewFocus::Logic => "请专注于逻辑正确性：边界条件、空值处理、错误处理、并发问题等。",
        AiReviewFocus::CodeStyle => "请专注于代码风格和可读性：命名、注释、结构清晰度等。",
    };

    format!(
        r#"你是一位资深代码评审专家。{}

请分析以下 git diff，给出结构化的评审意见。

对于每个发现的问题，请按以下 JSON 格式输出：

```json
{{
  "suggestions": [
    {{
      "file": "文件路径",
      "line_start": 行号或null,
      "line_end": 行号或null,
      "severity": "critical|major|minor|info",
      "category": "security|performance|logic|style",
      "description": "问题描述",
      "suggestion": "具体修改建议（可选）"
    }}
  ],
  "summary": "总体评审摘要"
}}
```

注意：
- 只输出 JSON，不要有任何额外的文字
- 如果没有发现问题，suggestions 为空数组
- severity 判断标准：
  - critical: 会导致安全漏洞或生产事故
  - major: 可能导致 bug 或严重性能问题
  - minor: 代码风格或可读性改进
  - info: 优化建议
- 最多返回 8 条最重要的建议，按严重程度优先
- description 和 suggestion 各控制在 120 个汉字以内，不要粘贴完整代码或大段 diff
- summary 控制在 200 个汉字以内
- 对每一处建议，给出简洁、可执行的修改方向"#,
        focus_instruction
    )
}

/// Build the user message with the diff content
pub fn build_user_message(diff: &str, context: Option<&PrContext>) -> String {
    let mut msg = String::from("请评审以下代码变更：\n\n");

    if let Some(ctx) = context {
        msg.push_str(&format!("PR 标题: {}\nPR 描述: {}\n\n", ctx.title, ctx.body));
        if let Some(rules) = ctx.repository_rules.as_deref().map(str::trim).filter(|rules| !rules.is_empty()) {
            msg.push_str("仓库级评审规则（评审时必须遵守）：\n");
            msg.push_str(rules);
            msg.push_str("\n\n");
        }
    }

    // Truncate diff if it's too large (max ~64KB for reasonable AI input)
    let diff_content = if diff.len() > 65536 {
        let mut boundary = 65536;
        while !diff.is_char_boundary(boundary) {
            boundary -= 1;
        }
        format!("{}...\n[Diff 内容过长，已截断，仅展示前 64KB]", &diff[..boundary])
    } else {
        diff.to_string()
    };

    msg.push_str("```diff\n");
    msg.push_str(&diff_content);
    msg.push('\n');
    msg.push_str("```");

    msg
}

#[cfg(test)]
mod tests {
    use super::build_user_message;
    use crate::models::PrContext;

    #[test]
    fn truncates_chinese_on_utf8_boundary() {
        let diff = format!("{}中tail", "a".repeat(65_535));
        let message = build_user_message(&diff, None);
        assert!(message.contains(&"a".repeat(65_535)));
        assert!(!message.contains("中tail"));
        assert!(message.contains("已截断"));
    }

    #[test]
    fn truncates_emoji_on_utf8_boundary() {
        let diff = format!("{}🦀tail", "a".repeat(65_534));
        let message = build_user_message(&diff, None);
        assert!(message.contains(&"a".repeat(65_534)));
        assert!(!message.contains("🦀tail"));
        assert!(message.contains("已截断"));
    }

    #[test]
    fn includes_repository_rules_in_user_message() {
        let context = PrContext {
            title: "规则测试".to_string(),
            body: "描述".to_string(),
            repository_rules: Some("禁止在异步任务中持有互斥锁".to_string()),
        };

        let message = build_user_message("+change", Some(&context));

        assert!(message.contains("仓库级评审规则"));
        assert!(message.contains("禁止在异步任务中持有互斥锁"));
    }
}
