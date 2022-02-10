use std::env::current_dir;
use std::io::Write;
use std::path::Path;
use std::process::exit;
use std::thread::sleep;
use std::time::Duration;

use bilirust::{Audio, Video, WebToken, VIDEO_QUALITY_4K};
use clap::ArgMatches;
use clap::{arg, App};
use dialoguer::Select;
use futures::stream::TryStreamExt;
use image::Luma;
use indicatif::ProgressBar;
use lazy_static::lazy_static;
use qrcode::QrCode;
use serde_json::{from_str, to_string};
use tokio::io::AsyncReadExt;
use tokio_util::io::StreamReader;

use entities::*;
use local::{
    connect_db, create_table_if_not_exists, join_paths, load_property, save_property,
    save_property_from_db, template_dir,
};

mod args;
mod entities;
mod ffmpeg_cmd;
mod local;
mod types;

fn app() -> App<'static> {
    App::new("bili-cli")
        .subcommand(App::new("login").about("使用二维码登录"))
        .subcommand(App::new("user").about("用户信息"))
        .subcommand(
            App::new("down")
                .about("下载视频")
                .arg(args::format())
                .arg(arg!(<url>).help("需要下载的url")),
        )
        .subcommand(
            App::new("continue")
                .about("继续下载视频")
                .arg(arg!(<dir>).help("上次的文件夹")),
        )
}

#[tokio::main]
async fn main() {
    ffmpeg_cmd::ffmpeg_run_version();
    let matches = app().get_matches();
    match matches.subcommand() {
        None => app().print_help().unwrap(),
        Some((subcommand, matches)) => match subcommand {
            "login" => login().await,
            "user" => user().await,
            "down" => down(matches).await,
            "continue" => continue_fn(matches).await,
            _ => app().print_help().unwrap(),
        },
    };
}

lazy_static! {
    static ref BV_PATTERN: regex::Regex = regex::Regex::new(r"BV[0-9a-zA-Z]{10}").unwrap();
}

async fn login() {
    let client = bilirust::Client::new();
    let qr_data = client.login_qr().await.unwrap();
    let code = QrCode::new(qr_data.url.clone().as_str().as_bytes()).unwrap();
    let image = code.render::<Luma<u8>>().build();
    let path = join_paths(vec![
        template_dir().as_str(),
        (uuid::Uuid::new_v4().to_string() + ".png").as_str(),
    ]);
    image.save(path.as_str()).unwrap();
    opener::open(path).unwrap();
    loop {
        sleep(Duration::new(3, 0));
        let info = client.login_qr_info(qr_data.oauth_key.clone()).await;
        match info {
            Ok(info) => {
                // -1：密钥错误
                // -2：密钥超时
                // -4：未扫描
                // -5：未确认
                match info.error_data {
                    0 => {
                        let web_token = client
                            .login_qr_info_parse_token(info.url.to_string())
                            .unwrap();
                        let web_token_string = to_string(&web_token).unwrap();
                        save_property("web_token".to_owned(), web_token_string)
                            .await
                            .unwrap();
                        println!("OK");
                        break;
                    }
                    -4 => continue,
                    -5 => continue,
                    -2 => panic!("time out"),
                    other => panic!("ERROR : {}", other),
                }
            }
            Err(err) => {
                panic!("{}", err);
            }
        }
    }
}

async fn login_client() -> bilirust::Client {
    let property = load_property("web_token".to_owned()).await.unwrap();
    if property.clone().as_str() == "" {
        println!("需要登录");
        exit(1);
    }
    let token: WebToken = from_str(property.as_str()).unwrap();
    let mut client = bilirust::Client::new();
    client.login_set_sess_data(token.sessdata);
    client
}

async fn user() {
    println!("{:?}", login_client().await.my_info().await.unwrap());
}

// 新下载
async fn down(matches: &ArgMatches) {
    let url = matches.value_of("url").unwrap();
    let find = BV_PATTERN.find(url);
    match find {
        Some(find) => {
            // 提取BV
            let bv = (&(url[find.start()..find.end()])).to_owned();
            println!("匹配到 : {}", bv.clone());
            // 提取
            let client = login_client().await;
            // 获取基本信息
            let info = client.bv_info(bv.clone()).await.unwrap();
            // 获取格式+获取清晰度
            let format_str = args::format_value(&matches);
            let format = args::format_fnval(format_str);
            let vu = client
                .bv_download_url(bv.clone(), info.cid, format, VIDEO_QUALITY_4K)
                .await
                .unwrap();
            match format_str {
                "dash" => {
                    // 选择清晰度
                    if vu.support_formats.len() == 0 {
                        panic!("未找到");
                    }
                    let mut choose_string: Vec<String> = vec![];
                    let mut choose_int: Vec<i64> = vec![];
                    for i in 0..vu.support_formats.len() {
                        let f = &vu.support_formats[i];
                        choose_string.push(f.new_description.clone());
                        choose_int.push(f.quality);
                    }
                    let choose = Select::new()
                        .with_prompt("选择视频质量")
                        .default(0)
                        .items(&choose_string)
                        .interact()
                        .unwrap();
                    let quality_video = choose_int[choose];
                    // 音频
                    let mut choose_string: Vec<String> = vec![];
                    let mut choose_int: Vec<i64> = vec![];
                    for i in 0..vu.dash.audio.len() {
                        let f = &vu.dash.audio[i];
                        match f.id {
                            30216 => {
                                choose_string.push("64K".to_owned());
                                choose_int.push(f.id);
                            }
                            30232 => {
                                choose_string.push("132K".to_owned());
                                choose_int.push(f.id);
                            }
                            30280 => {
                                choose_string.push("192K".to_owned());
                                choose_int.push(f.id);
                            }
                            _ => {
                                choose_string.push(format!("AUDIO-{}", f.id));
                                choose_int.push(f.id);
                            }
                        }
                    }
                    let choose = Select::new()
                        .with_prompt("选择音频质量")
                        .default(0)
                        .items(&choose_string)
                        .interact()
                        .unwrap();
                    let quality_audio = choose_int[choose];
                    // 下载
                    let mut video: Option<Video> = None;
                    for x in vu.dash.video {
                        if x.id == quality_video {
                            video = Some(x);
                            break;
                        }
                    }
                    let mut audio: Option<Audio> = None;
                    for x in vu.dash.audio {
                        if x.id == quality_audio {
                            audio = Some(x);
                            break;
                        }
                    }
                    let video = video.unwrap();
                    let audio = audio.unwrap();
                    // 文件夹
                    let dir = join_paths(vec![
                        current_dir().unwrap().to_str().unwrap(),
                        format!("{}_{}_{}_{}", bv, format_str, quality_video, quality_audio)
                            .as_str(),
                    ]);
                    println!("下载到文件夹 : {}", dir.clone());
                    if Path::new(dir.clone().as_str()).exists() {
                        panic!("文件夹已存在, 请使用continue");
                    }
                    std::fs::create_dir_all(dir.clone()).unwrap();
                    let audio_file =
                        join_paths(vec![dir.clone().as_str(), format!("{}.audio", bv).as_str()]);
                    let video_file =
                        join_paths(vec![dir.clone().as_str(), format!("{}.video", bv).as_str()]);
                    let mix_file =
                        join_paths(vec![dir.clone().as_str(), format!("{}.mp4", bv).as_str()]);

                    // 下载音频
                    println!("下载音频 : {}", audio_file.clone());
                    let audio_rsp = request_resource(audio.base_url).await;
                    let audio_length = content_length(&audio_rsp);
                    let mut down_count: u64 = 0;
                    let mut file = std::fs::File::create(audio_file.clone()).unwrap();
                    let mut buf = [0; 8192];
                    let mut reader =
                        StreamReader::new(audio_rsp.bytes_stream().map_err(convert_error));
                    let pb = ProgressBar::new(audio_length);
                    loop {
                        pb.set_position(down_count);
                        let read = reader.read(&mut buf);
                        let read = read.await;
                        match read {
                            Ok(read) => {
                                if read == 0 {
                                    break;
                                }
                                file.write(&buf[0..read]).unwrap();
                                down_count = down_count + read as u64;
                                // save_property_from_db(
                                //     &db,
                                //     "audio_down_count".to_owned(),
                                //     format!("{}", down_count),
                                // )
                                // .await
                                // .unwrap();
                            }
                            Err(err) => {
                                panic!("{}", err)
                            }
                        }
                    }
                    drop(file);
                    drop(reader);
                    pb.finish_with_message("Audio Done");
                    println!("Audio Done");
                    // 下载视频
                    println!("下载视频 : {}", video_file.clone());
                    let video_rsp = request_resource(video.base_url).await;
                    let video_length = content_length(&video_rsp);
                    let mut down_count: u64 = 0;
                    let mut file = std::fs::File::create(video_file.clone()).unwrap();
                    let mut buf = [0; 8192];
                    let mut reader =
                        StreamReader::new(video_rsp.bytes_stream().map_err(convert_error));
                    let pb = ProgressBar::new(video_length);
                    loop {
                        pb.set_position(down_count);
                        let read = reader.read(&mut buf);
                        let read = read.await;
                        match read {
                            Ok(read) => {
                                if read == 0 {
                                    break;
                                }
                                file.write(&buf[0..read]).unwrap();
                                down_count = down_count + read as u64;
                                // save_property_from_db(
                                //     &db,
                                //     "video_down_count".to_owned(),
                                //     format!("{}", down_count),
                                // )
                                // .await
                                // .unwrap();
                            }
                            Err(err) => {
                                panic!("{}", err)
                            }
                        }
                    }
                    drop(file);
                    drop(reader);
                    pb.finish_with_message("Video Done");
                    println!("Video Done");
                    // 合并
                    println!("合并中...");
                    let mix_result = ffmpeg_cmd::ffmpeg_merge_file(
                        vec![video_file.clone(), audio_file.clone()],
                        mix_file.clone(),
                    );
                    mix_result.unwrap();
                    println!("合并完成");
                }
                "mp4" => {
                    let dir = join_paths(vec![
                        current_dir().unwrap().to_str().unwrap(),
                        format!("{}_{}", bv, format_str).as_str(),
                    ]);
                    println!("下载到文件夹 : {}", dir.clone());
                    if Path::new(dir.clone().as_str()).exists() {
                        panic!("文件夹已存在, 请使用continue");
                    }
                    println!("链接中...");
                    let rsp = request_resource(vu.durl.first().unwrap().clone().url).await;
                    let length = content_length(&rsp);
                    std::fs::create_dir_all(dir.clone()).unwrap();
                    println!("初始化...");
                    let db = connect_db(join_paths(vec![dir.clone().as_str(), "task.db"]).as_str())
                        .await;
                    create_table_if_not_exists(&db, property::Entity).await;
                    property::init_indexes(&db).await;
                    save_property_from_db(&db, "download_type".to_owned(), "bv".to_owned())
                        .await
                        .unwrap();
                    save_property_from_db(&db, "bv".to_owned(), bv.clone())
                        .await
                        .unwrap();
                    save_property_from_db(&db, "format_str".to_owned(), vu.format.clone())
                        .await
                        .unwrap();
                    save_property_from_db(&db, "quality_str".to_owned(), format!("{}", vu.quality))
                        .await
                        .unwrap();
                    save_property_from_db(&db, "content_length".to_owned(), format!("{}", length))
                        .await
                        .unwrap();
                    let file = join_paths(vec![
                        dir.clone().as_str(),
                        format!("{}.{}", bv, format_str).as_str(),
                    ]);
                    println!("下载到文件 : {}", file.clone());
                    let mut down_count: u64 = 0;
                    let mut file = std::fs::File::create(file).unwrap();
                    let mut buf = [0; 8192];
                    let mut reader = StreamReader::new(rsp.bytes_stream().map_err(convert_error));
                    let pb = ProgressBar::new(length);
                    loop {
                        pb.set_position(down_count);
                        let read = reader.read(&mut buf);
                        let read = read.await;
                        match read {
                            Ok(read) => {
                                if read == 0 {
                                    break;
                                }
                                file.write(&buf[0..read]).unwrap();
                                down_count = down_count + read as u64;
                                save_property_from_db(
                                    &db,
                                    "down_count".to_owned(),
                                    format!("{}", down_count),
                                )
                                .await
                                .unwrap();
                            }
                            Err(err) => {
                                panic!("{}", err)
                            }
                        }
                    }
                    drop(file);
                    drop(reader);
                    pb.finish_with_message("Done");
                    println!("Done");
                }
                &_ => panic!("e2"),
            }
        }
        None => {}
    }
}

async fn continue_fn(_matches: &ArgMatches) {}

fn convert_error(err: reqwest::Error) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, err)
}

async fn request_resource(url: String) -> reqwest::Response {
    reqwest::Client::new().get(url).header(
        "user-agent",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/98.0.4758.80 Safari/537.36",
    ).header("referer","https://www.bilibili.com").send().await.unwrap().error_for_status().unwrap()
}

fn content_length(rsp: &reqwest::Response) -> u64 {
    rsp.headers()
        .get("content-length")
        .unwrap()
        .to_str()
        .unwrap()
        .parse::<u64>()
        .unwrap()
}
