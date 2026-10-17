//! Compiles the shader overlay's colour grade (src/shade/grade.hlsl) into the
//! files Vesper ships, so the app itself never needs Microsoft's shader
//! compiler. Run it after changing the HLSL, then commit the results:
//!
//! `cargo run --example compile_shaders`

#[cfg(windows)]
fn main() {
    use sha2::{Digest, Sha256};
    use windows::core::{s, PCSTR};
    use windows::Win32::Graphics::Direct3D::Fxc::{D3DCompile, D3DCOMPILE_OPTIMIZATION_LEVEL3};
    use windows::Win32::Graphics::Direct3D::ID3DBlob;

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/shade");
    let source = std::fs::read_to_string(dir.join("grade.hlsl")).expect("grade.hlsl");
    for (entry, target, file) in [(s!("vs"), s!("vs_5_0"), "grade.vs.cso"), (s!("ps"), s!("ps_5_0"), "grade.ps.cso")] {
        let bytes = unsafe {
            let mut blob: Option<ID3DBlob> = None;
            let mut errors: Option<ID3DBlob> = None;
            let r = D3DCompile(
                source.as_ptr() as *const _,
                source.len(),
                PCSTR::null(),
                None,
                None,
                entry,
                target,
                D3DCOMPILE_OPTIMIZATION_LEVEL3,
                0,
                &mut blob,
                Some(&mut errors),
            );
            if let Err(e) = r {
                let detail = errors
                    .map(|b| {
                        String::from_utf8_lossy(std::slice::from_raw_parts(
                            b.GetBufferPointer() as *const u8,
                            b.GetBufferSize(),
                        ))
                        .into_owned()
                    })
                    .unwrap_or_default();
                panic!("{file} didn't compile: {} {detail}", e.message());
            }
            let b = blob.expect("compiled shader");
            std::slice::from_raw_parts(b.GetBufferPointer() as *const u8, b.GetBufferSize()).to_vec()
        };
        std::fs::write(dir.join(file), &bytes).expect("write shader");
        println!("{file}: {} bytes", bytes.len());
    }
    // Line endings don't change the shader, so they don't change the hash either.
    let hash = format!("{:x}", Sha256::digest(source.replace('\r', "").as_bytes()));
    std::fs::write(dir.join("grade.sha256"), &hash).expect("write hash");
    println!("grade.sha256: {hash}");
}

#[cfg(not(windows))]
fn main() {
    eprintln!("The shaders are compiled on Windows.");
}
