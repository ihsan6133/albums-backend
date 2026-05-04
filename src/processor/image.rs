use std::{error::Error, fs::File, io::BufWriter, time::Instant};

use image::{ExtendedColorType, ImageBuffer, Rgb, codecs::jpeg::JpegEncoder, imageops::{self, FilterType}};

use crate::core::perf::Perf;

pub fn convert_to_thumb(path: &str, dst: &str) ->  Result<Perf, Box<dyn Error>> {
    
    let mut perf = Perf::new();
    
    let total_start = Instant::now();
    
    let mut c  = mozjpeg::Decompress::new_path(path)?;
    c.dct_method(mozjpeg::DctMethod::IntegerFast);
    c.do_fancy_upsampling(false);
    c.do_block_smoothing(false);
    
    let mut c = perf.time("rgb", || {
        c.scale(1);
        c.rgb()
    })?;
        
    
    let (w, h)= (c.width(), c.height());
    
    
    let img = perf.time("scanline", || c.read_scanlines().expect("read scanline"));
        


    let img_buf: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_raw(w as u32, h as u32, img).expect("Error");
    
    
    let n_w: u32;
    let n_h: u32;

    if w > h {
        n_h = 128;
        n_w = (((w as f32)/(h as f32)) * 128.0) as u32;        
    } else {
        n_w = 128;
        n_h = (((h as f32)/(w as f32)) * 128.0) as u32
    }

    let img = perf.time("resize", || 
        imageops::resize(&img_buf, n_w as u32, n_h as u32, FilterType::Nearest)
        // &img_buf
    );
    
    let out_file = File::create(dst)?;
    let writer = BufWriter::new(out_file);

    let mut encoder = JpegEncoder::new_with_quality(writer, 50);
    
    perf.time("encode", ||
        encoder.encode(&img, img.width(), img.height(), ExtendedColorType::Rgb8)    
    )?;
    
    perf.insert("total".to_string(), total_start.elapsed());

    Ok(perf)
    
}
