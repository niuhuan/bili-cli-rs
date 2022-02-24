use std::process::exit;
use std::thread::sleep;
use std::time::Duration;

pub(crate) use bilirust::Result;
use bilirust::WebToken;
use clap::{arg, Command};
use image::Luma;
use qrcode::QrCode;
use serde_json::{from_str, to_string};

use local::{join_paths, load_property, save_property, template_dir};

mod args;
mod down;
mod entities;
mod ffmpeg_cmd;
mod local;

fn app() -> Command<'static> {
    Command::new("bili-cli")
        .subcommand(Command::new("login").about("使用二维码登录"))
        .subcommand(Command::new("user").about("用户信息"))
        .subcommand(
            Command::new("down")
                .about("下载视频")
                .arg(args::format())
                .arg(args::url()),
        )
        .subcommand(
            Command::new("continue")
                .about("继续下载视频")
                .arg(arg!(<dir>).help("上次的文件夹")),
        )
}

#[tokio::main]
async fn main() {
    run_app().await.unwrap();
}

async fn run_app() -> crate::Result<()> {
    ffmpeg_cmd::ffmpeg_run_version()?;
    let matches = app().get_matches();
    match matches.subcommand() {
        None => app().print_help()?,
        Some((subcommand, matches)) => match subcommand {
            "login" => login().await?,
            "user" => user().await?,
            "down" => down::down(matches).await?,
            "continue" => down::continue_fn(matches).await?,
            _ => app().print_help()?,
        },
    }
    Ok(())
}

async fn login() -> Result<()> {
    let client = bilirust::Client::new();
    let qr_data = client.login_qr().await.unwrap();
    let code = QrCode::new(qr_data.url.clone().as_str().as_bytes()).unwrap();
    let image = code.render::<Luma<u8>>().build();
    let path = join_paths(vec![
        &template_dir(),
        &(uuid::Uuid::new_v4().to_string() + ".png"),
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
                        save_property("web_token".to_owned(), web_token_string).await?;
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
    Ok(())
}

async fn login_client() -> Result<bilirust::Client> {
    let property = load_property("web_token".to_owned()).await?;
    if &property == "" {
        println!("需要登录");
        exit(1);
    }
    let token: WebToken = from_str(property.as_str())?;
    let mut client = bilirust::Client::new();
    client.login_set_sess_data(token.sessdata);
    Ok(client)
}

async fn user() -> Result<()> {
    println!("{:?}", login_client().await?.my_info().await?);
    Ok(())
}
