use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::error_log::{ErrorLogInput, ErrorLogStore};

#[tauri::command]
pub fn error_log_record(store: State<'_, ErrorLogStore>, record: ErrorLogInput) -> CommandResult<()> {
    store.record_input(record).map_err(|_| CommandError::from("本地错误日志写入失败"))
}
