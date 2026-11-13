use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{HumanBytes, ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use swiftdl_engine::header::{HeaderMap, HeaderName, HeaderValue};
use swiftdl_engine::{CancellationToken, Outcome, Progress, Request, SpeedLimiter};

/// swiftdl：高速多连接下载器，支持断点续传。
#[derive(Parser)]
#[command(version, about, help_template = "{about-with-newline}\n用法：{usage}\n\n参数与选项：\n{all-args}{after-help}")]
struct Args {
    /// 要下载的链接
    url: String,

    /// 输出文件或目录（默认使用服务器提供的文件名，保存到当前目录）
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// 并行连接数
    #[arg(short, long, default_value_t = 8, value_parser = clap::value_parser!(u16).range(1..=64))]
    connections: u16,

    /// 附加请求头，例如 -H "Cookie: session=abc"（可多次指定）
    #[arg(short = 'H', long = "header")]
    headers: Vec<String>,

    /// 速度限制，单位 KiB/s（0 表示不限速）
    #[arg(short, long, default_value_t = 0)]
    limit: u64,

    /// 忽略已保存的进度，从头下载
    #[arg(long)]
    restart: bool,
}

fn parse_headers(raw: &[String]) -> Result<HeaderMap> {
    let mut map = HeaderMap::new();
    for h in raw {
        let (name, value) = h.split_once(':').with_context(|| format!("请求头格式应为“名称: 值”，实际为 {h:?}"))?;
        map.append(
            HeaderName::from_bytes(name.trim().as_bytes()).with_context(|| format!("{h:?} 中的请求头名称无效"))?,
            HeaderValue::from_str(value.trim()).with_context(|| format!("{h:?} 中的请求头值无效"))?,
        );
    }
    Ok(map)
}

async fn run(args: Args) -> Result<Option<PathBuf>> {
    let headers = parse_headers(&args.headers)?;
    let client = swiftdl_engine::build_client()?;
    let remote = swiftdl_engine::probe(&client, &args.url, &headers).await?;
    let dest = match args.output {
        Some(p) if p.is_dir() => swiftdl_engine::resolve_dest(&p, &remote.filename),
        Some(p) => p,
        None => swiftdl_engine::resolve_dest(&std::env::current_dir()?, &remote.filename),
    };

    eprintln!("文件：{}", dest.display());
    match remote.size {
        Some(size) => eprintln!("大小：{}（{size} 字节）", HumanBytes(size)),
        None => eprintln!("大小：未知"),
    }
    if !remote.resumable {
        eprintln!("服务器不支持分段请求：将使用单连接下载，无法断点续传。");
    }

    let progress = Arc::new(Progress::default());
    let cancel = CancellationToken::new();
    let bar = ProgressBar::new(remote.size.unwrap_or(0));
    bar.set_style(
        ProgressStyle::with_template(
            "{bar:40.cyan/blue} {bytes:>10}/{total_bytes} {binary_bytes_per_sec:>12}  剩余 {eta:>4}  {msg}",
        )?
        .progress_chars("█▉▊▋▌▍▎▏ "),
    );
    let ticker = {
        let (progress, bar) = (progress.clone(), bar.clone());
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_millis(200));
            let mut first = true;
            loop {
                tick.tick().await;
                let done = progress.downloaded.load(Ordering::Relaxed);
                if first && done > 0 {
                    // Resumed: start the bar where we left off so the speed
                    // estimate isn't skewed by the jump.
                    bar.reset();
                    first = false;
                }
                bar.set_position(done);
                bar.set_message(format!("{} 个连接", progress.connections.load(Ordering::Relaxed)));
            }
        })
    };
    {
        let cancel = cancel.clone();
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            cancel.cancel();
        });
    }

    let req = Request {
        mirrors: vec![],
        url: args.url,
        dest: dest.clone(),
        connections: args.connections as usize,
        headers,
        restart: args.restart,
    };
    let limiter = Arc::new(SpeedLimiter::new(args.limit * 1024));
    let outcome = swiftdl_engine::run(&client, &req, progress.clone(), cancel, limiter).await;
    ticker.abort();
    bar.set_position(progress.downloaded.load(Ordering::Relaxed));
    match outcome {
        Ok(Outcome::Completed) => {
            bar.finish_with_message("完成");
            Ok(Some(dest))
        }
        Ok(Outcome::Paused) => {
            bar.abandon_with_message("已暂停");
            Ok(None)
        }
        Err(e) => {
            bar.abandon_with_message("失败");
            Err(e.context("下载已停止，进度已保存；再次运行相同命令即可继续"))
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Args::parse()).await {
        Ok(Some(path)) => {
            eprintln!("已保存到 {}", path.display());
            ExitCode::SUCCESS
        }
        Ok(None) => {
            eprintln!("已暂停。再次运行相同命令即可继续下载。");
            ExitCode::from(130)
        }
        Err(e) => {
            eprintln!("错误：{e:#}");
            ExitCode::FAILURE
        }
    }
}
