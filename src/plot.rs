use std::path::PathBuf;
use std::process::Command;

use std::{fs, io};

#[derive(Clone, Copy)]
pub struct ScalingPlot {
    pub max_plot_temperature: u16,
    pub number_of_steps_for_graph: u16,
}

pub fn plot_maker(session_ids: &[u16], scale: ScalingPlot) {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let session_dir = home.join("Documents").join("Twatch").join("session");

    let paths: Vec<PathBuf> = if session_ids.is_empty() {
        vec![find_latest(&session_dir).expect("No session files found")]
    } else {
        session_ids
            .iter()
            .map(|id| session_dir.join(format!("session_{}.csv", id)))
            .collect()
    };

    let script = find_plot_script();
    let mut cmd = Command::new("python3");
    cmd.arg(&script)
        .arg("--max-temp")
        .arg(scale.max_plot_temperature.to_string())
        .arg("--temp-steps")
        .arg(scale.number_of_steps_for_graph.to_string());

    for p in &paths {
        cmd.arg(p);
    }

    match cmd.spawn() {
        Ok(mut child) => {
            let _ = child.wait();
        }
        Err(e) => {
            eprintln!("Failed to launch plot: {}. Is python3+matplotlib installed?", e);
        }
    }
}

fn find_latest(dir: &PathBuf) -> io::Result<PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(dir)?
        .filter_map(|r| r.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |ext| ext == "csv"))
        .collect();
    paths.sort();
    paths.last().cloned().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no sessions"))
}

fn find_plot_script() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    let exe_dir = exe.parent().unwrap_or_else(|| std::path::Path::new("."));

    let candidates = [
        exe_dir.join("plot.py"),
        exe_dir.join("../plot.py"),
        exe_dir.join("../../plot.py"),
        std::env::current_dir().unwrap_or_default().join("plot.py"),
    ];

    for c in &candidates {
        if c.exists() {
            return c.clone();
        }
    }

    PathBuf::from("plot.py")
}
