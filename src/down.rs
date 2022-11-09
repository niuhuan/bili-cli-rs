use anyhow::Context;
use bilirust::{Audio, Ss, SsState, Video, FNVAL_DASH, VIDEO_QUALITY_4K};
use dialoguer::Select;
use futures::stream::TryStreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use lazy_static::lazy_static;
use std::env::current_dir;
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio_util::io::StreamReader;

use crate::local::{allowed_file_name, join_paths};
use crate::{app, ffmpeg_cmd, login_client};

lazy_static! {
    static ref SHORT_PATTERN: regex::Regex =
        regex::Regex::new(r"//b\d+\.tv/([0-9a-zA-Z]+)$").unwrap();
    static ref BV_PATTERN: regex::Regex = regex::Regex::new(r"BV[0-9a-zA-Z]{10}").unwrap();
    static ref SERIES_PATTERN: regex::Regex = regex::Regex::new(r"((ep)|(ss))[0-9]+").unwrap();
    static ref USER_COLLECTION_DETAIL_PATTERN: regex::Regex =
        regex::Regex::new(r"/([0-9]+)/channel/collectiondetail\?sid=([0-9]+)").unwrap();
}

// 新下载
pub(crate) async fn down() -> crate::Result<()> {
    let mut url = app::url_value();
    let ss = app::ss_value();
    if let Some(_) = SHORT_PATTERN.find(url.as_str()) {
        let rsp = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()?
            .get(&url)
            .send()
            .await?;
        match rsp.status().as_u16() {
            302 => {
                let headers = rsp.headers();
                let location = headers.get("location");
                if let Some(location) = location {
                    url = location.to_str()?.to_owned();
                }
            }
            _ => return Err(anyhow::Error::msg("resolve short links error")),
        }
    }
    if let Some(find) = BV_PATTERN.find(url.as_str()) {
        return down_bv((&(url[find.start()..find.end()])).to_owned()).await;
    }
    if let Some(find) = SERIES_PATTERN.find(url.as_str()) {
        return down_series((&(url[find.start()..find.end()])).to_owned(), url, ss).await;
    }
    if let Some(find) = USER_COLLECTION_DETAIL_PATTERN.captures(url.as_str()) {
        let mid: i64 = find.get(1).unwrap().as_str().parse().unwrap();
        let sid: i64 = find.get(2).unwrap().as_str().parse().unwrap();
        return down_collection_detail(mid, sid).await;
    }
    Ok(())
}

async fn down_bv(bv: String) -> crate::Result<()> {
    let client = login_client().await?;
    // 获取基本信息
    println!();
    println!("匹配到 : {}", bv.clone());
    let info = client.bv_info(bv.clone()).await.unwrap();
    println!("  {}", &info.title);
    // 获取格式+获取清晰度
    let format_str = app::format_value();
    let format = app::format_fnval(format_str);
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
            // 文件名
            let name = allowed_file_name(&info.title);
            let audio_file = format!("{}.audio", name);
            let video_file = format!("{}.video", name);
            let mix_file = format!("{}.mp4", name);
            println!("下载到文件 : {}", &mix_file);
            if Path::new(&mix_file).exists() {
                panic!("文件已存在");
            }
            // 下载
            down_file_to(&audio.base_url, &audio_file, "下载音频").await;
            println!(" > 下载音频");
            down_file_to(&video.base_url, &video_file, "下载视频").await;
            println!(" > 下载视频");
            println!(" > 合并视频");
            let mix_result =
                ffmpeg_cmd::ffmpeg_merge_file(vec![&video_file, &audio_file], &mix_file);
            mix_result.unwrap();
            println!(" > 清理合并前的数据");
            let _ = std::fs::remove_file(&audio_file);
            let _ = std::fs::remove_file(&video_file);
        }
        "mp4" => {
            let file = format!("{}.mp4", info.title);
            println!("下载到文件 : {}", &file);
            if Path::new(&file).exists() {
                panic!("文件夹已存在");
            }
            down_file_to(&(vu.durl.first().unwrap().url), &file, "下载中").await;
            println!("下载完成");
        }
        &_ => panic!("e2"),
    };
    Ok(())
}

/// 下载一系列视频
async fn down_series(id: String, url: String, ss: bool) -> crate::Result<()> {
    let client = login_client().await?;
    println!();
    println!("匹配到合集 : {}", id);
    let ss_state = if ss {
        client.videos_info_by_url(url).await.unwrap()
    } else {
        client.videos_info(id.clone()).await.unwrap()
    };
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
        allowed_file_name(ss_state.media_info.series.as_str()).as_str(),
    ]);
    println!("  保存位置 : {}", project_dir.as_str());
    println!();
    // todo
    if Path::new(project_dir.as_str()).exists() {
        //panic!("文件夹已存在, 请使用continue");
    }
    std::fs::create_dir_all(project_dir.as_str()).unwrap();

    //
    let fetch_ids = if app::choose_ep_value() {
        let titles = (&ss_state)
            .ss_list
            .iter()
            .map(|x| format!("{} ({})", x.id, x.title.as_str(),))
            .collect_vec();
        let default_selects = (&titles).iter().map(|_| true).collect_vec();
        let selects = dialoguer::MultiSelect::new()
            .with_prompt("请选择要下载的合集")
            .items(&titles)
            .defaults(&default_selects)
            .interact()
            .unwrap();
        let mut id_list: Vec<i64> = vec![];
        for i in 0..titles.len() {
            if selects.contains(&i) {
                id_list.push(ss_state.ss_list[i].id);
            }
        }
        id_list
    } else {
        (&ss_state).ss_list.iter().map(|x| x.id).collect_vec()
    };

    // 找到所有的ss
    // 找到所有ss的bv
    println!("搜索视频");
    let mut sss: Vec<(Ss, SsState, String)> = vec![];
    for x in ss_state.ss_list {
        if !fetch_ids.contains(&x.id) {
            continue;
        }
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
    for x in &sss {
        let ss_dir = join_paths(vec![project_dir.as_str(), x.2.as_str()]);
        std::fs::create_dir_all(ss_dir.as_str()).unwrap();
        for ep in &x.1.ep_list {
            let name = format!("{}. ({}) {}", ep.i, ep.title_format, ep.long_title);
            let name = allowed_file_name(&name);
            println!();
            println!("{}", &name);
            let audio_name = format!("{}.audio", name);
            let video_name = format!("{}.video", name);
            let final_name = format!("{}.mp4", name);
            let audio_file = join_paths(vec![&ss_dir, &audio_name]);
            let video_file = join_paths(vec![&ss_dir, &video_name]);
            let final_file = join_paths(vec![&ss_dir, &final_name]);
            if Path::new(&&final_file).exists() {
                continue;
            }
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
            //
            down_file_to(audio_url, &audio_file, "下载音频").await;
            println!(" > 下载音频");
            down_file_to(video_url, &video_file, "下载视频").await;
            println!(" > 下载视频");
            println!(" > 合并视频");
            ffmpeg_cmd::ffmpeg_merge_file(vec![&video_file, &audio_file], &final_file).unwrap();
            println!(" > 清理合并前的数据");
            let _ = std::fs::remove_file(&audio_file);
            let _ = std::fs::remove_file(&video_file);
        }
    }
    println!();
    println!("全部完成");
    Ok(())
}

async fn down_collection_detail(mid: i64, sid: i64) -> crate::Result<()> {
    let client = login_client().await?;
    // 获取第一页
    let mut current_page = 1;
    let mut page = client
        .collection_video_page(mid, sid, false, current_page, 20)
        .await
        .unwrap();
    println!();
    println!("获取到合集 : {}", page.meta.name);
    println!();
    let folder = allowed_file_name(page.meta.name.as_str());
    std::fs::create_dir_all(folder.as_str()).unwrap();
    loop {
        // 下载视频
        for archive in page.archives {
            println!();
            println!("{}", archive.title);
            let name = allowed_file_name(&archive.title);
            let audio_name = format!("{}.audio", name);
            let video_name = format!("{}.video", name);
            let final_name = format!("{}.mp4", name);
            let audio_file = join_paths(vec![folder.as_str(), audio_name.as_str()]);
            let video_file = join_paths(vec![folder.as_str(), video_name.as_str()]);
            let final_file = join_paths(vec![folder.as_str(), final_name.as_str()]);
            if Path::new(&&final_file).exists() {
                continue;
            }
            //
            let info = client.bv_info(archive.bvid).await.unwrap();
            let video_url = client
                .bv_download_url(info.bvid, info.cid, FNVAL_DASH, VIDEO_QUALITY_4K)
                .await
                .unwrap();
            let audio_url = video_url.dash.audio.first().unwrap().base_url.as_str();
            let video_url = video_url.dash.video.first().unwrap().base_url.as_str();
            //
            down_file_to(audio_url, &audio_file, "下载音频").await;
            println!(" > 下载音频");
            down_file_to(video_url, &video_file, "下载视频").await;
            println!(" > 下载视频");
            println!(" > 合并视频");
            ffmpeg_cmd::ffmpeg_merge_file(vec![&video_file, &audio_file], &final_file).unwrap();
            println!(" > 清理合并前的数据");
            let _ = std::fs::remove_file(&audio_file);
            let _ = std::fs::remove_file(&video_file);
        }
        // 获取下一页
        if page.page.page_size * page.page.page_num >= page.page.total {
            break;
        }
        current_page += 1;
        page = client
            .collection_video_page(mid, sid, false, current_page, 20)
            .await
            .unwrap();
    }
    println!();
    println!("全部完成");
    Ok(())
}

async fn down_file_to(url: &str, path: &str, title: &str) {
    let path = Path::new(path);
    let checkpoint = if app::resume_download_value() && path.exists() {
        path.metadata().unwrap().len()
    } else {
        0
    };
    let rsp = request_resource(url).await;
    let size = content_length(&rsp).unwrap();
    let (rsp, file) = if checkpoint == 0 {
        (rsp, tokio::fs::File::create(path).await.unwrap())
    } else {
        if size == checkpoint {
            return;
        }
        drop(rsp);
        (
            request_resource_rang(url, checkpoint).await,
            tokio::fs::OpenOptions::new()
                .append(true)
                .open(path)
                .await
                .unwrap(),
        )
    };
    let mut file = BufWriter::with_capacity(1 << 18, file);
    let mut buf = Box::new([0; 1 << 18]);
    let mut reader = BufReader::with_capacity(
        1 << 18,
        StreamReader::new(rsp.bytes_stream().map_err(convert_error)),
    );
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<Vec<u8>>(1 << 10);
    let sjb = tokio::spawn(async move {
        loop {
            let read = reader.read(buf.as_mut()).await.unwrap();
            if read == 0 {
                break;
            }
            sender.send(buf[0..read].to_vec()).await.unwrap();
        }
    });
    let title = title.to_string();
    let rjb = tokio::spawn(async move {
        let pb = ProgressBar::new(size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    &*("".to_owned()
                        + "{spinner:.green}  "
                        + title.as_str()
                        + " [{wide_bar:.cyan/blue}] {bytes}/{total_bytes}"),
                )
                .unwrap()
                .progress_chars("#>-"),
        );
        let mut down_count: u64 = checkpoint;
        pb.set_position(down_count);
        while let Some(msg) = receiver.recv().await {
            file.write(&msg).await.unwrap();
            down_count = down_count + msg.len() as u64;
            pb.set_position(down_count);
        }
        pb.finish_and_clear();
        file.flush().await.unwrap();
    });
    let (s, r) = tokio::join!(sjb, rjb);
    s.unwrap();
    r.unwrap();
}

fn convert_error(err: reqwest::Error) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, err)
}

async fn request_resource(url: &str) -> reqwest::Response {
    reqwest::Client::new().get(url).header(
        "user-agent",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/98.0.4758.80 Safari/537.36",
    ).header("referer", "https://www.bilibili.com").send().await.unwrap().error_for_status().unwrap()
}

async fn request_resource_rang(url: &str, begin: u64) -> reqwest::Response {
    reqwest::Client::new().get(url).header(
        "user-agent",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/98.0.4758.80 Safari/537.36",
    ).header("referer", "https://www.bilibili.com").header("Range",format!("bytes={}-",begin)).send().await.unwrap().error_for_status().unwrap()
}

fn content_length(rsp: &reqwest::Response) -> crate::Result<u64> {
    Ok(rsp
        .headers()
        .get("content-length")
        .with_context(|| "未能取得文件长度, HEADER不存在")?
        .to_str()
        .with_context(|| "未能取得文件长度, HEADER不能使用")?
        .parse()
        .with_context(|| "未能取得文件长度, HEADER不能识别未数字")?)
}
