use crate::app::init_app;
pub(crate) use anyhow::Result;
use bilirust::WebToken;
use image::Luma;
use local::{join_paths, load_property, save_property, template_dir};
use qrcode::QrCode;
use serde_json::{from_str, to_string};
use std::process::exit;
use std::thread::sleep;
use std::time::Duration;

mod app;
mod down;
mod entities;
mod ffmpeg;
mod local;

#[tokio::main]
async fn main() {
    init_app();
    run_app().await.unwrap();
}

async fn run_app() -> crate::Result<()> {
    ffmpeg::ffmpeg_run_version()?;
    match app::subcommand() {
        None => app::print_help()?,
        Some(subcommand) => match subcommand.as_str() {
            "login" => login().await?,
            "user" => user().await?,
            "down" => down::down().await?,
            _ => app::print_help()?,
        },
    }
    Ok(())
}

async fn login() -> Result<()> {
    let client = bilirust::Client::new();
    let qr_data = client.login_qr().await.unwrap();
    if app::qr_console_value() {
        qr2term::print_qr(qr_data.url.clone().as_str()).unwrap();
    } else {
        let code = QrCode::new(qr_data.url.clone().as_str().as_bytes()).unwrap();
        let image = code.render::<Luma<u8>>().build();
        let path = join_paths(vec![
            &template_dir(),
            &(uuid::Uuid::new_v4().to_string() + ".png"),
        ]);
        image.save(path.as_str()).unwrap();
        opener::open(path).unwrap();
    }

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
