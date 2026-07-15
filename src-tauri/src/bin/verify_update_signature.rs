use base64::{engine::general_purpose::STANDARD, Engine as _};
use minisign_verify::{PublicKey, Signature};
use serde::Deserialize;
use std::{env, fs, path::PathBuf};

#[derive(Deserialize)]
struct TauriConfig {
    plugins: Plugins,
}

#[derive(Deserialize)]
struct Plugins {
    updater: Updater,
}

#[derive(Deserialize)]
struct Updater {
    pubkey: String,
}

struct Arguments {
    config: PathBuf,
    signature: String,
    file: PathBuf,
}

fn next_value(arguments: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    arguments.next().ok_or_else(|| format!("{name} 缺少参数值"))
}

fn parse_arguments() -> Result<Arguments, String> {
    let mut values = env::args().skip(1);
    let mut config = None;
    let mut signature = None;
    let mut file = None;
    while let Some(name) = values.next() {
        match name.as_str() {
            "--config" => config = Some(PathBuf::from(next_value(&mut values, "--config")?)),
            "--signature" => signature = Some(next_value(&mut values, "--signature")?),
            "--file" => file = Some(PathBuf::from(next_value(&mut values, "--file")?)),
            _ => return Err(format!("未知参数：{name}")),
        }
    }
    Ok(Arguments {
        config: config.ok_or("缺少 --config")?,
        signature: signature.ok_or("缺少 --signature")?,
        file: file.ok_or("缺少 --file")?,
    })
}

fn decode_base64_text(value: &str, label: &str) -> Result<String, String> {
    let decoded = STANDARD.decode(value).map_err(|error| format!("{label} Base64 无效：{error}"))?;
    String::from_utf8(decoded).map_err(|_| format!("{label}不是 UTF-8 文本"))
}

fn run() -> Result<(), String> {
    let arguments = parse_arguments()?;
    let config: TauriConfig =
        serde_json::from_slice(&fs::read(&arguments.config).map_err(|error| format!("读取 Tauri 配置失败：{error}"))?)
            .map_err(|error| format!("解析 Tauri 配置失败：{error}"))?;
    let public_key = PublicKey::decode(&decode_base64_text(&config.plugins.updater.pubkey, "updater 公钥")?)
        .map_err(|error| format!("解析 updater 公钥失败：{error}"))?;
    let signature = Signature::decode(&decode_base64_text(&arguments.signature, "updater 签名")?)
        .map_err(|error| format!("解析 updater 签名失败：{error}"))?;
    let artifact = fs::read(&arguments.file).map_err(|error| format!("读取更新资源失败：{error}"))?;
    public_key.verify(&artifact, &signature, true).map_err(|error| format!("updater 签名验证失败：{error}"))
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
