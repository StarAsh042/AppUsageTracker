//! 运行日志：按天滚动的文件日志，保留 7 天。
//!
//! 发布版是 `windows_subsystem = "windows"`，没有控制台，所以诊断必须落盘才有
//! 意义。这里只需要「打开文件 + 跨天切换 + 定期清理」，复用已在依赖图里的
//! `log` 与 `chrono` 自己实现，比再引一个插件和一层 IPC 更省。
//!
//! 位置随数据目录走：`<data_directory>/logs/app-YYYY-MM-DD.log`。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{Local, NaiveDate};

/// 保留天数，含今天共 7 个文件。
const KEEP_DAYS: i64 = 7;
const FILE_PREFIX: &str = "app-";
const FILE_SUFFIX: &str = ".log";

/// 日志目录。数据目录可被用户改到任意位置，日志跟着走。
pub(crate) fn log_dir(data_dir: &str) -> PathBuf {
    Path::new(data_dir).join("logs")
}

struct Sink {
    dir: PathBuf,
    /// 当前打开文件对应的日期，用于跨天切换。
    date: String,
    file: Option<std::fs::File>,
}

impl Sink {
    fn needs_rollover(&self, today: &str) -> bool {
        self.file.is_none() || self.date != today
    }

    fn rollover(&mut self, today: &str) -> bool {
        self.file = None;
        let path = self.dir.join(format!("{FILE_PREFIX}{today}{FILE_SUFFIX}"));
        let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        else {
            return false;
        };
        self.file = Some(file);
        self.date = today.to_string();
        prune(&self.dir);
        true
    }

    fn append(&mut self, bytes: &[u8]) {
        if let Some(file) = self.file.as_mut() {
            let _ = file.write_all(bytes);
            let _ = file.flush();
        }
    }
}

/// 全局写状态。`None` 表示尚未初始化，此时记录直接丢弃。
static SINK: Mutex<Option<Sink>> = Mutex::new(None);

static FILE_LOGGER: FileLogger = FileLogger;

struct FileLogger;

impl log::Log for FileLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        write_record(record);
    }

    fn flush(&self) {
        let Ok(mut guard) = SINK.lock() else {
            return;
        };
        if let Some(file) = guard.as_mut().and_then(|s| s.file.as_mut()) {
            let _ = file.flush();
        }
    }
}

fn write_record(record: &log::Record) {
    let now = Local::now();
    let line = format!(
        "{} {:<5} {}: {}\n",
        now.format("%Y-%m-%d %H:%M:%S%.3f"),
        record.level(),
        record.target(),
        record.args()
    );
    // 开发时同时留在 stderr，免得 cargo run 还要去翻文件。
    if cfg!(debug_assertions) {
        eprint!("{line}");
    }
    let today = now.format("%Y-%m-%d").to_string();
    let Ok(mut guard) = SINK.lock() else {
        return;
    };
    let Some(sink) = guard.as_mut() else {
        return;
    };
    if sink.needs_rollover(&today) && !sink.rollover(&today) {
        return;
    }
    sink.append(line.as_bytes());
}

/// 安装全局 logger。重复调用安全，只会重置写入目录。
pub(crate) fn init(data_dir: &str) {
    let dir = log_dir(data_dir);
    let _ = std::fs::create_dir_all(&dir);
    prune(&dir);
    if let Ok(mut guard) = SINK.lock() {
        *guard = Some(Sink {
            dir,
            date: String::new(),
            file: None,
        });
    }
    if log::set_logger(&FILE_LOGGER).is_err() {
        return;
    }
    log::set_max_level(if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    });
}

/// 数据目录变更后把日志挪到新目录，下一次写入时重新打开文件。
pub(crate) fn set_dir(data_dir: &str) {
    let dir = log_dir(data_dir);
    let _ = std::fs::create_dir_all(&dir);
    let Ok(mut guard) = SINK.lock() else {
        return;
    };
    if let Some(sink) = guard.as_mut().filter(|s| s.dir != dir) {
        sink.dir = dir;
        sink.file = None;
        sink.date.clear();
    }
}

/// 删掉超过 `KEEP_DAYS` 天的日志文件，其他文件一律不动。
fn prune(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let today = Local::now().date_naive();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(date) = file_date(name.to_string_lossy().as_ref()) else {
            continue;
        };
        if (today - date).num_days() >= KEEP_DAYS {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// 从 `app-YYYY-MM-DD.log` 里取出日期，不是本模块写的日志就返回 None。
fn file_date(name: &str) -> Option<NaiveDate> {
    let stem = name.strip_prefix(FILE_PREFIX)?.strip_suffix(FILE_SUFFIX)?;
    NaiveDate::parse_from_str(stem, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("aut-log-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("创建临时目录");
        dir
    }

    fn day_file(dir: &Path, offset_days: i64) -> PathBuf {
        let date = Local::now().date_naive() - chrono::Duration::days(offset_days);
        dir.join(format!(
            "{FILE_PREFIX}{}{FILE_SUFFIX}",
            date.format("%Y-%m-%d")
        ))
    }

    #[test]
    fn prune_keeps_last_seven_days() {
        let dir = scratch("prune");
        let kept: Vec<PathBuf> = (0..KEEP_DAYS).map(|d| day_file(&dir, d)).collect();
        for p in &kept {
            std::fs::write(p, b"x").expect("写测试日志");
        }
        let expired = day_file(&dir, KEEP_DAYS);
        std::fs::write(&expired, b"x").expect("写过期日志");
        let other = dir.join("notes.txt");
        std::fs::write(&other, b"x").expect("写无关文件");

        prune(&dir);

        for p in &kept {
            assert!(p.exists(), "{} 在保留期内不该被删", p.display());
        }
        assert!(!expired.exists(), "超过 7 天的日志应被删除");
        assert!(other.exists(), "非本模块命名的文件不该被删");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn records_land_in_the_daily_file() {
        let dir = scratch("write");
        init(&dir.to_string_lossy());

        log::error!("日志落盘验证 boom");

        let text = std::fs::read_to_string(day_file(&log_dir(&dir.to_string_lossy()), 0))
            .expect("日志文件应存在");
        assert!(text.contains("boom"), "日志文件里应能读到这条记录");
        assert!(text.contains("ERROR"), "记录应带级别");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
