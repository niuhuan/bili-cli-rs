use std::env::current_dir;
use std::fmt::format;
use std::io::Write;
use std::path::Path;

use bilirust::{Audio, Ss, SsState, Video, FNVAL_DASH, VIDEO_QUALITY_4K};
use clap::ArgMatches;
use dialoguer::Select;
use futures::stream::TryStreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use lazy_static::lazy_static;
use tokio::io::AsyncReadExt;
use tokio_util::io::StreamReader;

use crate::entities::*;
use crate::local::{connect_db, create_table_if_not_exists, join_paths, save_property_from_db};
use crate::{args, ffmpeg_cmd, login_client};

lazy_static! {
    static ref BV_PATTERN: regex::Regex = regex::Regex::new(r"BV[0-9a-zA-Z]{10}").unwrap();
    static ref COLLECTION_PATTERN: regex::Regex = regex::Regex::new(r"(ep)|(ss)[0-9]+").unwrap();
}

// 新下载
pub(crate) async fn down(matches: &ArgMatches) {
    let url = args::url_value(&matches);
    let find = BV_PATTERN.find(url.as_str());
    if find.is_some() {
        let find = find.unwrap();
        down_bv(&matches, (&(url[find.start()..find.end()])).to_owned()).await;
        return;
    }
    let find = COLLECTION_PATTERN.find(url.as_str());
    if find.is_some() {
        let find = find.unwrap();
        down_coll(&matches, (&(url[find.start()..find.end()])).to_owned()).await;
        return;
    }
}

async fn down_bv(matches: &ArgMatches, bv: String) {
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
                format!("{}_{}_{}_{}", bv, format_str, quality_video, quality_audio).as_str(),
            ]);
            println!("下载到文件夹 : {}", dir.clone());
            if Path::new(dir.clone().as_str()).exists() {
                panic!("文件夹已存在, 请使用continue");
            }
            std::fs::create_dir_all(dir.clone()).unwrap();
            let audio_file = join_paths(vec![dir.as_str(), format!("{}.audio", bv).as_str()]);
            let video_file = join_paths(vec![dir.as_str(), format!("{}.video", bv).as_str()]);
            let mix_file = join_paths(vec![dir.as_str(), format!("{}.mp4", bv).as_str()]);

            println!("初始化...");
            let db = connect_db(join_paths(vec![dir.as_str(), "task.db"]).as_str()).await;
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
            save_property_from_db(
                &db,
                "quality_video".to_owned(),
                format!("{}", quality_video),
            )
            .await
            .unwrap();
            save_property_from_db(
                &db,
                "quality_audio".to_owned(),
                format!("{}", quality_audio),
            )
            .await
            .unwrap();

            // 下载音频
            println!("下载音频 : {}", audio_file.clone());
            let audio_rsp = request_resource(audio.base_url.as_str()).await;
            let audio_length = content_length(&audio_rsp);
            save_property_from_db(&db, "audio_length".to_owned(), format!("{}", audio_length))
                .await
                .unwrap();
            let mut down_count: u64 = 0;
            let mut file = std::fs::File::create(audio_file.clone()).unwrap();
            let mut buf = [0; 8192];
            let mut reader = StreamReader::new(audio_rsp.bytes_stream().map_err(convert_error));
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
                        save_property_from_db(
                            &db,
                            "audio_down_count".to_owned(),
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
            pb.finish_with_message("Audio Done");
            println!("音频下载完成");
            // 下载视频
            println!("下载视频 : {}", video_file.clone());
            let video_rsp = request_resource(video.base_url.as_str()).await;
            let video_length = content_length(&video_rsp);
            save_property_from_db(&db, "video_length".to_owned(), format!("{}", video_length))
                .await
                .unwrap();
            let mut down_count: u64 = 0;
            let mut file = std::fs::File::create(video_file.clone()).unwrap();
            let mut buf = [0; 8192];
            let mut reader = StreamReader::new(video_rsp.bytes_stream().map_err(convert_error));
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
                        save_property_from_db(
                            &db,
                            "video_down_count".to_owned(),
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
            pb.finish_with_message("Video Done");
            println!("视频下载完成");
            // 合并
            println!("合并中...");
            let mix_result =
                ffmpeg_cmd::ffmpeg_merge_file(vec![&video_file, &audio_file], &mix_file);
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
            let rsp = request_resource(vu.durl.first().unwrap().url.as_str()).await;
            let length = content_length(&rsp);
            std::fs::create_dir_all(dir.clone()).unwrap();
            println!("初始化...");
            let db = connect_db(join_paths(vec![dir.as_str(), "task.db"]).as_str()).await;
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

async fn down_coll(_matches: &ArgMatches, id: String) {
    let client = login_client().await;
    println!();
    println!("匹配到合集 : {}", id);
    let ss_state = client.videos_info(id.clone()).await.unwrap();
    println!("  系列名称 : {}", ss_state.media_info.series.clone());
    println!(
        "  包含番剧 : {} ",
        (&ss_state.ss_list)
            .iter()
            .map(|i| i.title.as_str())
            .join(" / ")
    );
    let project_dir = join_paths(vec![
        current_dir().unwrap().to_str().unwrap(),
        format!("{}", ss_state.media_info.series.as_str()).as_str(),
    ]);
    println!("  保存位置 : {}", project_dir.as_str());
    println!();
    // todo
    if Path::new(project_dir.as_str()).exists() {
        panic!("文件夹已存在, 请使用continue");
    }
    std::fs::create_dir_all(project_dir.as_str()).unwrap();

    // 找到所有的ss
    // 找到所有ss的bv
    println!("搜索视频");
    let mut sss: Vec<(Ss, SsState, String)> = vec![];
    for x in ss_state.ss_list {
        let videos_info = client.videos_info(format!("ss{}", x.id)).await.unwrap();
        let x_dir_name = format!(
            "{} ({}) {}",
            x.id,
            x.title.as_str(),
            videos_info.media_info.season_title.as_str(),
        );
        println!(
            "  {} : 共 {} 个视频",
            x_dir_name.as_str(),
            videos_info.ep_list.len()
        );
        sss.push((x, videos_info, x_dir_name));
    }
    println!();

    println!("下载视频");
    println!();
    for x in &sss {
        let ss_dir = join_paths(vec![project_dir.as_str(), x.2.as_str()]);
        std::fs::create_dir_all(ss_dir.as_str()).unwrap();
        for ep in &x.1.ep_list {
            let name = format!("{}. ({}) {}", ep.i, ep.title_format, ep.long_title);
            println!("{}", &name);
            let audio_name = format!("{}.audio", name);
            let video_name = format!("{}.video", name);
            let final_name = format!("{}.mp4", name);
            let bv = client
                .bv_download_url(
                    ep.bvid.clone(),
                    ep.cid.clone(),
                    FNVAL_DASH,
                    VIDEO_QUALITY_4K,
                )
                .await
                .unwrap();
            let audio_url: &str = bv.dash.audio.first().unwrap().base_url.as_str();
            let video_url = bv.dash.video.first().unwrap().base_url.as_str();
            let audio_file = join_paths(vec![ss_dir.as_str(), audio_name.as_str()]);
            let video_file = join_paths(vec![&ss_dir, &video_name]);
            let final_file = join_paths(vec![&ss_dir, &final_name]);
            //
            down_file_to(audio_url, &audio_file, "下载音频").await;
            down_file_to(video_url, &video_file, "下载视频").await;
            ffmpeg_cmd::ffmpeg_merge_file(vec![&video_file, &audio_file], &final_file).unwrap();
            println!("合并完成");
            println!();
        }
    }
    println!("全部完成");
}

async fn down_file_to(url: &str, path: &str, title: &str) {
    let rsp = request_resource(url).await;
    let size = content_length(&rsp);
    let pb = ProgressBar::new(size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                &*("".to_owned()
                    + "{spinner:.green}  "
                    + title
                    + " [{wide_bar:.cyan/blue}] {bytes}/{total_bytes}"),
            )
            .progress_chars("#>-"),
    );
    let mut down_count: u64 = 0;
    let mut file = std::fs::File::create(path).unwrap();
    let mut buf = [0; 8192];
    let mut reader = StreamReader::new(rsp.bytes_stream().map_err(convert_error));
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
            }
            Err(err) => {
                panic!("{}", err)
            }
        }
    }
    drop(file);
    drop(reader);
    pb.finish_and_clear();
    println!("{} Done", title);
}

pub(crate) async fn continue_fn(_matches: &ArgMatches) {
    println!("还在建设中");
}

fn convert_error(err: reqwest::Error) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, err)
}

async fn request_resource(url: &str) -> reqwest::Response {
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
