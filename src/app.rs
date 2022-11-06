use bilirust::{FNVAL_DASH, FNVAL_MP4};
use clap::{arg, Arg, ArgAction, ArgMatches, Command};
use dialoguer::{Input, Select};
use once_cell::sync::OnceCell;

pub(crate) static MATCHES: OnceCell<ArgMatches> = OnceCell::new();

pub fn app() -> Command {
    Command::new("bili-cli")
        .subcommand(
            Command::new("login")
                .about("使用二维码登录")
                .arg(qr_console()),
        )
        .subcommand(Command::new("user").about("用户信息"))
        .subcommand(
            Command::new("down")
                .about("下载视频")
                .arg(format())
                .arg(url())
                .arg(ss())
                .arg(choose_ep())
                .arg(resume_download()),
        )
}

pub(crate) fn init_app() {
    MATCHES.set(app().get_matches()).unwrap();
}

pub(crate) fn print_help() -> crate::Result<()> {
    app().print_help()?;
    Ok(())
}

fn args() -> &'static ArgMatches {
    MATCHES.get().unwrap()
}

pub(crate) fn subcommand() -> Option<String> {
    if let Some((str, _)) = args().subcommand() {
        Some(str.to_string())
    } else {
        None
    }
}

/// 控制台输出二维码参数
pub(crate) fn qr_console() -> Arg {
    arg!(<console_qrcode>)
        .short('c')
        .long("console")
        .action(ArgAction::SetTrue)
        .help("在控制台输出二维码")
}

/// 控制台输出二维码参数
pub(crate) fn qr_console_value() -> bool {
    args().subcommand().unwrap().1.get_flag("console_qrcode")
}

/// 格式参数, 下载bv的时候可以指定格式
/// -f mp4 默认使用mp4不再确认
pub(crate) fn format() -> Arg {
    arg!(<format>)
        .short('f')
        .long("format")
        .required(false)
        .default_value("choose")
        .help("视频格式 只能为 mp4/dash/choose 其中之一")
        .value_parser(format_v)
}

fn format_v(format: &str) -> Result<String, String> {
    match format {
        "mp4" => Ok("mp4".to_string()),
        "dash" => Ok("dash".to_string()),
        "choose" => Ok("choose".to_string()),
        _ => Err("视频格式 只能为 mp4/dash/choose 其中之一".to_string()),
    }
}

/// 获取格式的值
pub(crate) fn format_value() -> &'static str {
    let format_string = args()
        .subcommand()
        .unwrap()
        .1
        .get_one::<String>("format")
        .unwrap();
    let mut format_str: &str = format_string.as_str();
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
pub(crate) fn url() -> Arg {
    arg!(<url>).required(false).help("需要下载的url")
}

/// 获取URL参数的值
pub(crate) fn url_value() -> String {
    let url: &str = if let Some(str) = args().subcommand().unwrap().1.get_one::<String>("url") {
        str
    } else {
        ""
    };
    if "" == url {
        return Input::new()
            .with_prompt("请输入视频网址")
            .interact_text()
            .unwrap();
    }
    url.to_string()
}

/// 下载的url, 如果指定的次参数则不需要再输入
pub(crate) fn ss() -> Arg {
    arg!(<ss>)
        .short('s')
        .long("ss")
        .required(false)
        .action(ArgAction::SetTrue)
        .help("使用url解析剧集数据而不是id, 有的剧集下不了加上这个试试")
}

pub(crate) fn ss_value() -> bool {
    args().subcommand().unwrap().1.get_flag("ss")
}

/// 获取EP
pub(crate) fn choose_ep() -> Arg {
    arg!(<ce>)
        .short('c')
        .long("ce")
        .required(false)
        .action(ArgAction::SetTrue)
        .help("加上这个可以选择要下载的ep, 而不是全部EP")
}

pub(crate) fn choose_ep_value() -> bool {
    args().subcommand().unwrap().1.get_flag("ce")
}

/// 断点续传
pub(crate) fn resume_download() -> Arg {
    arg!(<resume_download>)
        .short('r')
        .long("resume")
        .required(false)
        .action(ArgAction::SetTrue)
        .help("断点续传，您必须选择和上次一样的清晰度，否则会出现视频无法使用的情况")
}

pub(crate) fn resume_download_value() -> bool {
    args().subcommand().unwrap().1.get_flag("resume_download")
}
