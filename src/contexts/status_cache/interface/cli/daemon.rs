use crate::contexts::status_cache::application::lifecycle::{self, Port};

/// デーモンを起動する。
pub fn start(port: &impl Port) {
    lifecycle::start(port);
}

/// デーモンを停止する。
pub fn stop(port: &impl Port) {
    if lifecycle::stop(port) {
        println!("daemon stopped");
    } else {
        eprintln!("daemon is not running");
    }
}

/// デーモンのステータスを表示する。
pub fn status(port: &impl Port) {
    match lifecycle::get_status(port) {
        Some(s) => {
            println!(
                "running  pid={}  entries={}  uptime={}s",
                s.pid, s.entries, s.uptime_secs
            );
        }
        None => println!("not running"),
    }
}
