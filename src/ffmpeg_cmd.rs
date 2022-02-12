use std::process::{Command, Stdio};

pub(crate) fn ffmpeg_run_version() -> crate::Result<()> {
    let mut cmd = Command::new("ffmpeg");
    cmd.stderr(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.arg("-version");
    match cmd.status() {
        Ok(_) => Ok(()),
        Err(_) => Err(Box::from(bilirust::Error::from(
            "未找到ffmpeg, 请先安装ffmpeg.",
        ))),
    }
}

/// 合并音频视频
pub(crate) fn ffmpeg_merge_file(list: Vec<&str>, output: &str) -> bilirust::Result<()> {
    let mut cmd = Command::new("ffmpeg");
    cmd.stderr(Stdio::null());
    cmd.stdout(Stdio::null());
    for x in list {
        cmd.arg("-i");
        cmd.arg(x);
    }
    cmd.arg("-vcodec");
    cmd.arg("copy");
    cmd.arg("-acodec");
    cmd.arg("copy");
    cmd.arg(output);
    let status = cmd.status().unwrap();
    if status.code().unwrap() == 0 {
        Ok(())
    } else {
        Err(Box::from(bilirust::Error::from(format!(
            "FFMPEG 未能成功运行 : EXIT CODE : {}",
            status.code().unwrap()
        ))))
    }
}
