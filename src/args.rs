use bilirust::{FNVAL_DASH, FNVAL_MP4};
use clap::{arg, Arg, ArgMatches};
use dialoguer::Select;

pub(crate) fn format() -> Arg<'static> {
    arg!(-f --format <format>)
        .required(false)
        .default_value("choose")
        .help("视频格式 只能为 mp4/dash/choose 其中之一")
        .validator(|format| match format {
            "mp4" => Ok(()),
            "dash" => Ok(()),
            "choose" => Ok(()),
            _ => Err("视频格式 只能为 mp4/dash/choose 其中之一"),
        })
}

pub(crate) fn format_value<'a>(matches: &'a ArgMatches) -> &'a str {
    let mut format_str: &'a str = matches.value_of("format").unwrap();
    if "choose" == format_str {
        format_str = ["dash", "mp4"][Select::new()
            .with_prompt("选择视频格式")
            .default(0)
            .items(&["dash (高清)", "mp4 (低清)"])
            .interact()
            .unwrap()];
    }
    format_str
}

pub(crate) fn format_fnval(format_str: &str) -> i64 {
    match format_str {
        "mp4" => FNVAL_MP4,
        "dash" => FNVAL_DASH,
        _ => panic!("格式不正确"),
    }
}
