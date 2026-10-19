#[cfg(windows)]
fn main() {
    use std::time::{Duration, Instant};
    use windows::core::w;
    use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::*;

    unsafe extern "system" fn proc_(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        if msg == WM_PAINT {
            unsafe {
                let mut ps = PAINTSTRUCT::default();
                let dc = BeginPaint(hwnd, &mut ps);
                let mut r = RECT::default();
                let _ = GetClientRect(hwnd, &mut r);
                let half = r.right / 2;
                for (i, c) in [0x0000ffu32, 0x00ff00].iter().enumerate() {
                    let b = CreateSolidBrush(COLORREF(*c));
                    let rr = RECT { left: half * i as i32, top: 0, right: half * (i as i32 + 1), bottom: r.bottom };
                    FillRect(dc, &rr, b);
                    let _ = DeleteObject(b.into());
                }
                let _ = EndPaint(hwnd, &ps);
            }
            return LRESULT(0);
        }
        unsafe { DefWindowProcW(hwnd, msg, w, l) }
    }

    let hwnd = unsafe {
        let inst = GetModuleHandleW(None).unwrap();
        let class = w!("VesperShadeTest");
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(proc_),
            hInstance: inst.into(),
            lpszClassName: class,
            ..Default::default()
        });
        CreateWindowExW(
            WS_EX_TOPMOST,
            class,
            w!("Vesper shader self-test"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            120,
            120,
            420,
            300,
            None,
            None,
            Some(inst.into()),
            None,
        )
        .unwrap()
    };
    let target = hwnd.0 as isize;
    let mut worker = None;

    let sample = |hwnd: HWND| -> Vec<[u8; 3]> {
        unsafe {
            let mut r = RECT::default();
            let _ = GetClientRect(hwnd, &mut r);
            let mut pts = [windows::Win32::Foundation::POINT { x: r.left, y: r.top }];
            let _ = ClientToScreen(hwnd, &mut pts[0]);
            let (w, h) = (r.right, r.bottom);
            let screen = GetDC(None);
            let mem = CreateCompatibleDC(Some(screen));
            let bmp = CreateCompatibleBitmap(screen, w, h);
            let old = SelectObject(mem, bmp.into());
            let _ = BitBlt(mem, 0, 0, w, h, Some(screen), pts[0].x, pts[0].y, SRCCOPY | CAPTUREBLT);
            let mut out = Vec::new();
            for (x, y) in [(w / 4, h / 2), (3 * w / 4, h / 2)] {
                let c = GetPixel(mem, x, y).0;
                out.push([(c & 0xff) as u8, ((c >> 8) & 0xff) as u8, ((c >> 16) & 0xff) as u8]);
            }
            SelectObject(mem, old);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(mem);
            ReleaseDC(None, screen);
            out
        }
    };

    let started = Instant::now();
    let mut before = None;
    let mut during = None;
    while started.elapsed() < Duration::from_millis(4800) {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        let t = started.elapsed();
        if before.is_none() && t > Duration::from_millis(500) {
            before = Some(sample(hwnd));
            worker = Some(std::thread::spawn(move || {
                vesper_lib::shade::run_for(target, vesper_lib::shade::preset("noir").unwrap(), 3.5)
            }));
        }
        if during.is_none() && t > Duration::from_millis(3000) {
            during = Some(sample(hwnd));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let result = worker.unwrap().join().unwrap();
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
    println!("overlay: {result:?}");
    println!("before: {before:?}");
    println!("during: {during:?}");
    let is_grey = |v: &Vec<[u8; 3]>| v.iter().all(|c| c[0].abs_diff(c[1]) < 12 && c[1].abs_diff(c[2]) < 12);
    let ok = !is_grey(&before.unwrap()) && is_grey(&during.unwrap()) && result.is_ok();
    println!("{}", if ok { "PASS: colours before, graded to grey on screen while the overlay ran" } else { "FAIL" });
}

#[cfg(not(windows))]
fn main() {}
