use bilirust::{FNVAL_DASH, FNVAL_MP4};
use clap::{arg, Arg, ArgMatches};
use dialoguer::{Input, Select};

/// 格式参数, 下载bv的时候可以指定格式
/// -f mp4 默认使用mp4不再确认
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

/// 获取格式的值
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

/// 根据格式的值获取参数
pub(crate) fn format_fnval(format_str: &str) -> i64 {
    match format_str {
        "mp4" => FNVAL_MP4,
        "dash" => FNVAL_DASH,
        _ => panic!("格式不正确"),
    }
}

/// 下载的url, 如果指定的次参数则不需要再输入
pub(crate) fn url() -> Arg<'static> {
    arg!(<url>).required(false).help("需要下载的url")
}

/// 获取URL参数的值
pub(crate) fn url_value(matches: &ArgMatches) -> String {
    let url: &str = matches.value_of("url").unwrap_or("");
    if "" == url {
        return Input::new()
            .with_prompt("请输入视频网址")
            .interact_text()
            .unwrap();
    }
    url.to_string()
}
