use netpulse_core::{Error, Result};
pub fn read(path: &str) -> Result<Option<String>> {
    use windows_sys::Win32::{
        Graphics::Gdi::*,
        UI::{Shell::*, WindowsAndMessaging::*},
    };
    let file = std::path::Path::new(path);
    if !file.is_absolute()
        || !file.is_file()
        || file
            .extension()
            .is_none_or(|v| !v.eq_ignore_ascii_case("exe"))
    {
        return Ok(None);
    }
    let wide: Vec<_> = path.encode_utf16().chain(Some(0)).collect();
    let mut info: SHFILEINFOW = unsafe { std::mem::zeroed() };
    if unsafe {
        SHGetFileInfoW(
            wide.as_ptr(),
            0,
            &mut info,
            std::mem::size_of_val(&info) as u32,
            SHGFI_ICON | SHGFI_SMALLICON,
        )
    } == 0
        || info.hIcon.is_null()
    {
        return Ok(None);
    }
    struct Icon(windows_sys::Win32::UI::WindowsAndMessaging::HICON);
    impl Drop for Icon {
        fn drop(&mut self) {
            unsafe { DestroyIcon(self.0) };
        }
    }
    let icon = Icon(info.hIcon);
    let dc = unsafe { CreateCompatibleDC(std::ptr::null_mut()) };
    if dc.is_null() {
        return Ok(None);
    }
    struct Dc(HDC);
    impl Drop for Dc {
        fn drop(&mut self) {
            unsafe { DeleteDC(self.0) };
        }
    }
    let dc = Dc(dc);
    let mut bitmap_info: BITMAPINFO = unsafe { std::mem::zeroed() };
    bitmap_info.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: 32,
        biHeight: -32,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB,
        ..Default::default()
    };
    let mut bits = std::ptr::null_mut();
    let bitmap = unsafe {
        CreateDIBSection(
            dc.0,
            &bitmap_info,
            DIB_RGB_COLORS,
            &mut bits,
            std::ptr::null_mut(),
            0,
        )
    };
    if bitmap.is_null() {
        return Ok(None);
    }
    let previous = unsafe { SelectObject(dc.0, bitmap) };
    unsafe {
        std::ptr::write_bytes(bits as *mut u8, 0, 4096);
        DrawIconEx(
            dc.0,
            0,
            0,
            icon.0,
            32,
            32,
            0,
            std::ptr::null_mut(),
            DI_NORMAL,
        );
        GdiFlush();
    }
    let mut rgba = unsafe { std::slice::from_raw_parts(bits as *const u8, 4096) }.to_vec();
    unsafe {
        SelectObject(dc.0, previous);
        DeleteObject(bitmap);
    }
    for pixel in rgba.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    if rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 0) {
        for pixel in rgba.as_chunks_mut::<4>().0 {
            pixel[3] = 255;
        }
    }
    let mut bytes = vec![];
    use image::ImageEncoder;
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(&rgba, 32, 32, image::ExtendedColorType::Rgba8)
        .map_err(|_| {
            Error::new(
                "IconEncodingFailed",
                "applications-icon",
                "Could not encode application icon",
            )
        })?;
    use base64::Engine;
    Ok(Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )))
}
