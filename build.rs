use std::{env, fs, io, path::PathBuf};

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return Ok(());
    }

    let icon_path =
        PathBuf::from(env::var_os("OUT_DIR").expect("Cargo provides OUT_DIR")).join("brewake.ico");
    fs::write(&icon_path, make_icon_file(&[16, 32, 48, 64, 256]))?;

    let manifest = r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
    </windowsSettings>
  </application>
</assembly>
"#;

    winresource::WindowsResource::new()
        .set_icon(icon_path.to_string_lossy().as_ref())
        .set_manifest(manifest)
        .set("InternalName", "Brewake")
        .set(
            "Comments",
            "No telemetry. No administrator rights. No power-plan changes.",
        )
        .compile()?;
    Ok(())
}

fn make_icon_file(sizes: &[usize]) -> Vec<u8> {
    let images: Vec<Vec<u8>> = sizes
        .iter()
        .map(|&size| icon_resource(&draw_mug(size), size))
        .collect();
    let directory_size = 6 + images.len() * 16;
    let mut bytes = Vec::with_capacity(directory_size + images.iter().map(Vec::len).sum::<usize>());

    push_u16(&mut bytes, 0);
    push_u16(&mut bytes, 1);
    push_u16(&mut bytes, images.len() as u16);

    let mut offset = directory_size as u32;
    for (&size, image) in sizes.iter().zip(&images) {
        bytes.push(if size == 256 { 0 } else { size as u8 });
        bytes.push(if size == 256 { 0 } else { size as u8 });
        bytes.push(0);
        bytes.push(0);
        push_u16(&mut bytes, 1);
        push_u16(&mut bytes, 32);
        push_u32(&mut bytes, image.len() as u32);
        push_u32(&mut bytes, offset);
        offset += image.len() as u32;
    }
    for image in images {
        bytes.extend_from_slice(&image);
    }
    bytes
}

fn draw_mug(size: usize) -> Vec<[u8; 4]> {
    let mut pixels = vec![[0u8; 4]; size * size];
    let cream = [245, 234, 217, 255];
    let orange = [230, 132, 44, 255];
    let coffee = [111, 50, 24, 255];
    let dark = [40, 29, 24, 255];
    let green = [28, 139, 104, 255];

    rect(&mut pixels, size, 10, 2, 13, 8, cream);
    rect(&mut pixels, size, 17, 1, 20, 8, cream);
    rect(&mut pixels, size, 6, 9, 24, 27, dark);
    rect(&mut pixels, size, 8, 11, 22, 25, orange);
    rect(&mut pixels, size, 8, 11, 22, 14, cream);
    rect(&mut pixels, size, 9, 13, 21, 16, coffee);
    rect(&mut pixels, size, 23, 12, 29, 23, cream);
    rect(&mut pixels, size, 23, 15, 26, 20, [0, 0, 0, 0]);
    rect(&mut pixels, size, 5, 27, 26, 29, cream);
    rect(&mut pixels, size, 24, 3, 31, 10, green);
    rect(&mut pixels, size, 26, 5, 28, 8, cream);
    rect(&mut pixels, size, 28, 7, 30, 9, cream);
    pixels
}

fn rect(
    pixels: &mut [[u8; 4]],
    size: usize,
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
    color: [u8; 4],
) {
    let left = left * size / 32;
    let top = top * size / 32;
    let right = (right * size).div_ceil(32).min(size);
    let bottom = (bottom * size).div_ceil(32).min(size);
    for y in top..bottom {
        for x in left..right {
            pixels[y * size + x] = color;
        }
    }
}

fn icon_resource(pixels: &[[u8; 4]], size: usize) -> Vec<u8> {
    let color_bytes = size * size * 4;
    let mask_stride = size.div_ceil(32) * 4;
    let mask_bytes = mask_stride * size;
    let mut bytes = Vec::with_capacity(40 + color_bytes + mask_bytes);

    push_u32(&mut bytes, 40);
    push_i32(&mut bytes, size as i32);
    push_i32(&mut bytes, (size * 2) as i32);
    push_u16(&mut bytes, 1);
    push_u16(&mut bytes, 32);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, color_bytes as u32);
    push_i32(&mut bytes, 0);
    push_i32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);

    for y in (0..size).rev() {
        for x in 0..size {
            let [red, green, blue, alpha] = pixels[y * size + x];
            bytes.extend_from_slice(&[blue, green, red, alpha]);
        }
    }
    for y in (0..size).rev() {
        let mut row = vec![0u8; mask_stride];
        for x in 0..size {
            if pixels[y * size + x][3] == 0 {
                row[x / 8] |= 0x80 >> (x % 8);
            }
        }
        bytes.extend_from_slice(&row);
    }
    bytes
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_i32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
