//! Renders each built-in sky as a horizon strip and a cross layout, for checking by eye.
//!
//! `cargo run --example sky_previews -- <output dir>`

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).expect("output directory"));
    std::fs::create_dir_all(&out).unwrap();
    for (id, _) in vesper_lib::sky::PRESETS {
        let colour = vesper_lib::sky::preset_color(id).unwrap();
        let faces = vesper_lib::sky::render(256, &colour);
        vesper_lib::sky::preview_strip(&faces, 120).save(out.join(format!("sky-{id}.png"))).unwrap();
        let get = |f: &str| faces.iter().find(|(n, _)| *n == f).unwrap().1.clone();
        let mut cross = image::RgbaImage::from_pixel(1024, 768, image::Rgba([40, 40, 40, 255]));
        let up = image::imageops::rotate90(&get("up"));
        let dn = image::imageops::rotate270(&get("dn"));
        for (img, x, y) in [
            (up, 256, 0),
            (get("rt"), 0, 256),
            (get("ft"), 256, 256),
            (get("lf"), 512, 256),
            (get("bk"), 768, 256),
            (dn, 256, 512),
        ] {
            image::imageops::overlay(&mut cross, &img, x, y);
        }
        cross.save(out.join(format!("cross-{id}.png"))).unwrap();
    }
    println!("written to {}", out.display());
}
