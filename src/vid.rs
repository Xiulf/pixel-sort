use super::*;
use ffmpeg_dev::sys::{
    self as ffmpeg, AVCodecContext, AVCodecID_AV_CODEC_ID_H264 as AV_CODEC_ID_H264, AVFrame,
    AVOutputFormat, AVPacket, AVStream, AVFMT_NOFILE, AV_INPUT_BUFFER_PADDING_SIZE,
};

use std::ffi::{CStr, CString};

const NOPTS_VALUE: i64 = -9223372036854775808;
const AVERROR_EAGAIN: i32 = 35;

pub fn sort_file(input: &str, output: &str) {
    let bytes = std::fs::read(input).unwrap();
    let bytes = sort_video(bytes);
}

pub fn sort_video(bytes: Vec<u8>) -> Vec<u8> {
    let frames = unsafe { decode_h264_video(bytes) };

    unimplemented!();
}

fn c_str(s: &str) -> CString {
    CString::new(s).expect("str to cstr")
}

struct RawYuv420p {
    width: u32,
    height: u32,
    bufsize: i32,
    linesize: [i32; 4],
    data: [*mut u8; 4],
}

impl Drop for RawYuv420p {
    fn drop(&mut self) {
        unsafe {
            if !self.data[0].is_null() {
                ffmpeg::av_free(self.data[0] as *mut _);
            }
        }
    }
}

impl RawYuv420p {
    fn luma_size(&self) -> u32 {
        self.width * self.height
    }

    fn chroma_size(&self) -> u32 {
        (self.width * self.height) / 4
    }

    unsafe fn to_vec(&self) -> Vec<u8> {
        let mut output = Vec::new();
        let ptr = self.data[0];

        output.reserve(self.bufsize as usize);

        for i in 0..self.bufsize as usize {
            let val = *ptr.add(i);

            output.push(val);
        }

        output
    }

    unsafe fn save(&self, path: &str) {
        std::fs::write(path, self.to_vec()).unwrap();
    }

    unsafe fn new(width: u32, height: u32) -> Self {
        use ffmpeg::AVPixelFormat_AV_PIX_FMT_YUV420P as AV_PIX_FMT_YUV420P;
        let pix_fmt = AV_PIX_FMT_YUV420P;
        let mut linesize = [0i32; 4];
        let mut data = [std::ptr::null_mut(); 4];
        let bufsize = ffmpeg::av_image_alloc(
            data.as_mut_ptr(),
            linesize.as_mut_ptr(),
            width as i32,
            height as i32,
            pix_fmt,
            1,
        );

        RawYuv420p {
            width,
            height,
            bufsize,
            linesize,
            data,
        }
    }

    unsafe fn fill_from_frame(&mut self, frame: *mut AVFrame) {
        ffmpeg::av_image_copy(
            self.data.as_mut_ptr(),
            self.linesize.as_mut_ptr(),
            (*frame).data.as_mut_ptr() as *mut *const u8,
            (*frame).linesize.as_ptr(),
            (*frame).format,
            (*frame).width,
            (*frame).height,
        );
    }
}

unsafe fn decode_h264_video(source: Vec<u8>) -> Vec<RawYuv420p> {
    unsafe fn decode(
        dec_ctx: *mut AVCodecContext,
        frame: *mut AVFrame,
        pkt: *mut AVPacket,
        output: &mut Vec<RawYuv420p>,
    ) {
        let mut ret = ffmpeg::avcodec_send_packet(dec_ctx, pkt);

        while ret >= 0 {
            ret = ffmpeg::avcodec_receive_frame(dec_ctx, frame);

            println!("decoding frame {}", (*dec_ctx).frame_number);

            if ret < 0 {
                return;
            }

            let done = {
                ret == ffmpeg_dev::extra::defs::averror(ffmpeg_dev::extra::defs::eagain())
                    || ret == ffmpeg_dev::extra::defs::averror_eof()
            };

            if done {
                return;
            }

            let mut decoded = RawYuv420p::new((*frame).width as u32, (*frame).height as u32);

            decoded.fill_from_frame(frame);
            output.push(decoded);
        }
    }

    const INBUF_SIZE: u32 = 4096;

    let mut f = source;
    let codec = ffmpeg::avcodec_find_decoder(AV_CODEC_ID_H264);
    let parser = ffmpeg::av_parser_init((*codec).id as i32);
    let mut c = ffmpeg::avcodec_alloc_context3(codec);
    let mut frame = ffmpeg::av_frame_alloc();
    let mut pkt = ffmpeg::av_packet_alloc();
    let mut output = Vec::new();

    loop {
        let inbuf_size = {
            if f.len() < INBUF_SIZE as usize {
                f.len()
            } else {
                INBUF_SIZE as usize
            }
        };

        let inbuf = f.drain(0..inbuf_size).collect::<Vec<_>>();
        let mut inbuf_size = inbuf_size as isize;

        if inbuf.is_empty() {
            break;
        }

        while inbuf_size > 0 {
            let ret = ffmpeg::av_parser_parse2(
                parser,
                c,
                &mut (*pkt).data,
                &mut (*pkt).size,
                inbuf.as_ptr(),
                inbuf.len() as i32,
                NOPTS_VALUE,
                NOPTS_VALUE,
                0,
            );

            inbuf_size -= ret as isize;

            if (*pkt).size > 0 {
                decode(c, frame, pkt, &mut output);
            }
        }
    }

    decode(c, frame, std::ptr::null_mut(), &mut output);

    ffmpeg::av_parser_close(parser);
    ffmpeg::avcodec_free_context(&mut c);
    ffmpeg::av_frame_free(&mut frame);
    ffmpeg::av_packet_free(&mut pkt);

    output
}
