#[cfg(not(feature = "ffmpeg_api"))]
use std::process::{Command, Stdio};

#[cfg(feature = "ffmpeg_api")]
pub(crate) fn ffmpeg_run_version() -> crate::Result<()> {
    Ok(())
}

#[cfg(not(feature = "ffmpeg_api"))]
pub(crate) fn ffmpeg_run_version() -> crate::Result<()> {
    let mut cmd = Command::new("ffmpeg");
    cmd.stderr(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.arg("-version");
    match cmd.status() {
        Ok(_) => Ok(()),
        Err(_) => Err(anyhow::Error::msg("未找到ffmpeg, 请先安装ffmpeg.")),
    }
}

#[cfg(feature = "ffmpeg_api")]
pub(crate) fn ffmpeg_merge_file(list: Vec<&str>, output: &str) -> bilirust::Result<()> {
    ffmpeg_api::ffmpeg_merge_files(list, output)
}

/// 合并音频视频
#[cfg(not(feature = "ffmpeg_api"))]
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
        Err(anyhow::Error::msg(format!(
            "FFMPEG 未能成功运行 : EXIT CODE : {}",
            status.code().unwrap()
        )))
    }
}

#[cfg(feature = "ffmpeg_api")]
mod ffmpeg_api {
    use anyhow::{anyhow, Context};
    use rsmpeg::{
        self,
        avcodec::{AVCodec, AVCodecContext},
        avformat::{AVFormatContextInput, AVFormatContextOutput},
    };
    use std::collections::HashMap;
    use std::ffi::CString;
    use std::os::raw::c_int;

    pub fn ffmpeg_merge_files(list: Vec<&str>, output: &str) -> anyhow::Result<()> {
        let output = CString::new(output)?;
        let mut output_format_context = AVFormatContextOutput::create(&output, None)?;
        let mut inputs = vec![];
        for input in list {
            let input = CString::new(input).unwrap();
            let input_format_context = AVFormatContextInput::open(&input)?;
            let mut stream_index_map = HashMap::new();
            for av_stream_ref in input_format_context.streams() {
                let stream_codecpar = av_stream_ref.codecpar();
                let codec_id = stream_codecpar.codec_id;
                let decoder = AVCodec::find_decoder(codec_id)
                    .with_context(|| anyhow!("video decoder ({}) not found.", codec_id))?;
                let mut decode_context = AVCodecContext::new(&decoder);
                decode_context.apply_codecpar(&stream_codecpar)?;
                decode_context.set_time_base(av_stream_ref.time_base);
                if let Some(framerate) = av_stream_ref.guess_framerate() {
                    decode_context.set_framerate(framerate);
                }
                let mut out_stream = output_format_context.new_stream();
                out_stream.set_codecpar(decode_context.extract_codecpar());
                out_stream.set_time_base(decode_context.time_base);
                stream_index_map.insert(av_stream_ref.index as i32, out_stream.index as i32);
            }
            inputs.push((input_format_context, stream_index_map));
        }
        let mut dict = None;
        output_format_context.write_header(&mut dict)?;
        for (mut input_format_context, stream_index_map) in inputs {
            loop {
                let mut packet = match input_format_context.read_packet()? {
                    Some(x) => x,
                    None => break,
                };
                packet.set_stream_index(
                    stream_index_map
                        .get(&(packet.stream_index as i32))
                        .unwrap()
                        .clone() as c_int,
                );
                output_format_context
                    .interleaved_write_frame(&mut packet)
                    .unwrap();
            }
        }
        output_format_context.write_trailer()?;
        Ok(())
    }
}
