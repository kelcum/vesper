//! Shaders for Roblox without touching Roblox.
//!
//! Roblox's anti-cheat blocks anything that loads into the game (ReShade
//! included), so Vesper never does. Instead it works like a screen recorder:
//! Windows Graphics Capture (what OBS and Discord use) hands over each frame
//! Roblox draws, a pixel shader colour-grades it on the GPU, and the result
//! is shown in a click-through window laid exactly over Roblox's. Mouse and
//! keyboard go straight to Roblox; the overlay never takes focus and hides
//! whenever Roblox isn't the active window.
//!
//! The cost is one extra frame of latency and a little GPU time.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShaderParams {
    /// Stops of exposure, -1..1.
    pub exposure: f32,
    /// 0.5..1.5, 1 is unchanged.
    pub contrast: f32,
    /// 0..2, 1 is unchanged.
    pub saturation: f32,
    /// Boosts muted colours more than vivid ones, -1..1.
    pub vibrance: f32,
    /// Cool to warm, -1..1.
    pub temperature: f32,
    /// Green to magenta, -1..1.
    pub tint: f32,
    /// 0..1.
    pub sharpen: f32,
    /// Darkened corners, 0..1.
    pub vignette: f32,
    /// Glow around bright areas, 0..1.
    pub bloom: f32,
    /// Film-like highlight roll-off, 0..1.
    pub tonemap: f32,
}

impl Default for ShaderParams {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            vibrance: 0.0,
            temperature: 0.0,
            tint: 0.0,
            sharpen: 0.0,
            vignette: 0.0,
            bloom: 0.0,
            tonemap: 0.0,
        }
    }
}

impl ShaderParams {
    pub fn clamped(mut self) -> Self {
        let c = |v: f32, lo: f32, hi: f32| if v.is_finite() { v.clamp(lo, hi) } else { (lo + hi) / 2.0 };
        self.exposure = c(self.exposure, -1.0, 1.0);
        self.contrast = c(self.contrast, 0.5, 1.5);
        self.saturation = c(self.saturation, 0.0, 2.0);
        self.vibrance = c(self.vibrance, -1.0, 1.0);
        self.temperature = c(self.temperature, -1.0, 1.0);
        self.tint = c(self.tint, -1.0, 1.0);
        self.sharpen = c(self.sharpen, 0.0, 1.0);
        self.vignette = c(self.vignette, 0.0, 1.0);
        self.bloom = c(self.bloom, 0.0, 1.0);
        self.tonemap = c(self.tonemap, 0.0, 1.0);
        self
    }
}

pub const PRESETS: &[(&str, &str)] = &[
    ("vivid", "Vivid"),
    ("cinematic", "Cinematic"),
    ("crisp", "Crisp"),
    ("warm", "Golden hour"),
    ("cool", "Moonlight"),
    ("soft", "Soft glow"),
    ("noir", "Noir"),
];

pub fn preset(id: &str) -> Option<ShaderParams> {
    let d = ShaderParams::default();
    Some(match id {
        "vivid" => ShaderParams { contrast: 1.08, saturation: 1.12, vibrance: 0.35, sharpen: 0.25, ..d },
        "cinematic" => ShaderParams {
            contrast: 1.12,
            saturation: 0.92,
            temperature: 0.12,
            tint: 0.04,
            vignette: 0.35,
            tonemap: 0.7,
            bloom: 0.15,
            ..d
        },
        "crisp" => ShaderParams { contrast: 1.05, vibrance: 0.15, sharpen: 0.6, ..d },
        "warm" => ShaderParams { exposure: 0.05, temperature: 0.45, saturation: 1.08, bloom: 0.25, vignette: 0.2, ..d },
        "cool" => ShaderParams { exposure: -0.1, temperature: -0.45, tint: 0.05, contrast: 1.06, vignette: 0.3, ..d },
        "soft" => ShaderParams { bloom: 0.55, contrast: 0.95, vibrance: 0.2, tonemap: 0.4, ..d },
        "noir" => ShaderParams { saturation: 0.0, contrast: 1.25, vignette: 0.45, sharpen: 0.2, tonemap: 0.3, ..d },
        _ => return None,
    })
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state", content = "detail")]
pub enum ShaderStatus {
    Off,
    /// On, waiting for a Roblox window.
    Waiting,
    Running,
    Failed(String),
}

/// Runs the overlay while it exists. Dropping it removes the overlay.
pub struct Overlay {
    stop: Arc<AtomicBool>,
    params: Arc<Mutex<ShaderParams>>,
}

impl Drop for Overlay {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Overlay {
    /// `find_target` returns the window to grade (Roblox's main window).
    pub fn start(
        params: ShaderParams,
        find_target: impl Fn() -> Option<isize> + Send + 'static,
        on_status: impl Fn(ShaderStatus) + Send + Sync + 'static,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let shared = Arc::new(Mutex::new(params.clamped()));
        let (flag, p) = (stop.clone(), shared.clone());
        let spawned = std::thread::Builder::new().name("vesper-shaders".into()).spawn(move || {
            let on_status = Arc::new(on_status);
            while !flag.load(Ordering::Relaxed) {
                let Some(hwnd) = find_target() else {
                    on_status(ShaderStatus::Waiting);
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    continue;
                };
                match imp::run(hwnd, &p, &flag, true, on_status.clone()) {
                    Ok(()) => {}
                    Err(e) => {
                        log::warn!("Shaders stopped: {e}");
                        on_status(ShaderStatus::Failed(e));
                        // Don't spin on a persistent failure.
                        for _ in 0..50 {
                            if flag.load(Ordering::Relaxed) {
                                break;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        }
                    }
                }
            }
            on_status(ShaderStatus::Off);
        });
        if let Err(e) = spawned {
            log::warn!("Couldn't start the shader thread: {e}");
        }
        Self { stop, params: shared }
    }

    pub fn set_params(&self, params: ShaderParams) {
        *self.params.lock().unwrap_or_else(|e| e.into_inner()) = params.clamped();
    }
}

/// Grades `hwnd` for `seconds` without waiting for focus, then returns.
/// Used by the self-test.
pub fn run_for(hwnd: isize, params: ShaderParams, seconds: f32) -> Result<(), String> {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs_f32(seconds));
        flag.store(true, Ordering::Relaxed);
    });
    imp::run(hwnd, &Arc::new(Mutex::new(params.clamped())), &stop, false, Arc::new(|_| {}))
}

/// The main window of a running Roblox player, if any.
pub fn roblox_window() -> Option<isize> {
    let pids = crate::roblox::process::roblox_pids();
    if pids.is_empty() {
        return None;
    }
    imp::main_window_of(&pids)
}

#[cfg(windows)]
const SHADER: &str = r#"
Texture2D src : register(t0);
SamplerState samp : register(s0);
cbuffer Params : register(b0) {
    float4 rect;      // xy: offset, zw: scale of the client area within the capture (uv)
    float2 texel;     // one pixel of the capture, in uv
    float exposure, contrast;
    float saturation, vibrance, temperature, tint;
    float sharpen, vignette, bloom, tonemap;
};

struct VsOut { float4 pos : SV_Position; float2 uv : TEXCOORD0; };

VsOut vs(uint id : SV_VertexID) {
    VsOut o;
    o.uv = float2((id << 1) & 2, id & 2);
    o.pos = float4(o.uv * float2(2, -2) + float2(-1, 1), 0, 1);
    return o;
}

float3 tap(float2 uv) { return src.SampleLevel(samp, uv, 0).rgb; }

float4 ps(VsOut i) : SV_Target {
    float2 t = rect.xy + i.uv * rect.zw;
    float3 c = tap(t);
    if (sharpen > 0) {
        float3 n = tap(t + float2(texel.x, 0)) + tap(t - float2(texel.x, 0)) + tap(t + float2(0, texel.y)) + tap(t - float2(0, texel.y));
        c = saturate(c + (c - n * 0.25) * sharpen * 1.5);
    }
    if (bloom > 0) {
        float3 b = 0;
        [unroll] for (int k = 0; k < 12; k++) {
            float a = k * 0.5236;
            float2 o = float2(cos(a), sin(a)) * texel * (6 + (k % 3) * 6);
            float3 s = tap(t + o);
            b += max(s - 0.6, 0);
        }
        c += b / 12 * bloom * 2.5;
    }
    c *= exp2(exposure);
    c *= float3(1 + temperature * 0.12 + tint * 0.04, 1 - tint * 0.08, 1 - temperature * 0.12 + tint * 0.04);
    if (tonemap > 0) {
        float3 f = (c * (2.51 * c + 0.03)) / (c * (2.43 * c + 0.59) + 0.14);
        c = lerp(c, f, tonemap);
    }
    c = (c - 0.5) * contrast + 0.5;
    float l = dot(c, float3(0.2126, 0.7152, 0.0722));
    c = lerp(l.xxx, c, saturation);
    float s = max(c.r, max(c.g, c.b)) - min(c.r, min(c.g, c.b));
    c = lerp(l.xxx, c, 1 + vibrance * (1 - saturate(s)));
    float2 d = i.uv - 0.5;
    c *= 1 - vignette * saturate(dot(d, d) * 2.2);
    return float4(saturate(c), 1);
}
"#;

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Constants {
    rect: [f32; 4],
    texel: [f32; 2],
    exposure: f32,
    contrast: f32,
    saturation: f32,
    vibrance: f32,
    temperature: f32,
    tint: f32,
    sharpen: f32,
    vignette: f32,
    bloom: f32,
    tonemap: f32,
}

#[cfg(windows)]
mod imp {
    use super::{Constants, ShaderParams, ShaderStatus, SHADER};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use windows::core::{factory, w, Interface, BOOL, PCSTR};
    use windows::Foundation::TypedEventHandler;
    use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession};
    use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
    use windows::Graphics::DirectX::DirectXPixelFormat;
    use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
    use windows::Win32::Graphics::Direct3D::{ID3DBlob, D3D_DRIVER_TYPE_HARDWARE, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST};
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::*;
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::WinRT::Direct3D11::{
        CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
    };
    use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
    use windows::Win32::UI::WindowsAndMessaging::*;

    fn err(what: &str, e: windows::core::Error) -> String {
        format!("{what}: {}", e.message())
    }

    pub fn main_window_of(pids: &[u32]) -> Option<isize> {
        struct Search<'a> {
            pids: &'a [u32],
            best: Option<(i64, isize)>,
        }
        unsafe extern "system" fn each(hwnd: HWND, data: LPARAM) -> BOOL {
            let s = unsafe { &mut *(data.0 as *mut Search) };
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
            if s.pids.contains(&pid) && unsafe { IsWindowVisible(hwnd) }.as_bool() {
                let owner = unsafe { GetWindow(hwnd, GW_OWNER) }.unwrap_or_default();
                let mut r = RECT::default();
                if owner.is_invalid() && unsafe { GetClientRect(hwnd, &mut r) }.is_ok() {
                    let area = (r.right - r.left) as i64 * (r.bottom - r.top) as i64;
                    if area > 0 && s.best.is_none_or(|(a, _)| area > a) {
                        s.best = Some((area, hwnd.0 as isize));
                    }
                }
            }
            BOOL(1)
        }
        let mut s = Search { pids, best: None };
        let _ = unsafe { EnumWindows(Some(each), LPARAM(&mut s as *mut Search as isize)) };
        s.best.map(|(_, h)| h)
    }

    /// Where Roblox's client area is on screen, and within the captured frame.
    #[derive(Clone, Copy, Default, PartialEq)]
    struct Placement {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        /// Client origin relative to the captured window bounds.
        off_x: i32,
        off_y: i32,
    }

    fn placement(target: HWND) -> Option<Placement> {
        let mut client = RECT::default();
        unsafe { GetClientRect(target, &mut client) }.ok()?;
        let mut origin = POINT::default();
        unsafe { ClientToScreen(target, &mut origin) }.ok().ok()?;
        let mut frame = RECT::default();
        unsafe {
            DwmGetWindowAttribute(
                target,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut frame as *mut RECT as *mut _,
                std::mem::size_of::<RECT>() as u32,
            )
        }
        .ok()?;
        Some(Placement {
            x: origin.x,
            y: origin.y,
            w: client.right - client.left,
            h: client.bottom - client.top,
            off_x: origin.x - frame.left,
            off_y: origin.y - frame.top,
        })
    }

    struct Renderer {
        device: ID3D11Device,
        context: ID3D11DeviceContext,
        swapchain: IDXGISwapChain1,
        rtv: Option<ID3D11RenderTargetView>,
        vs: ID3D11VertexShader,
        ps: ID3D11PixelShader,
        sampler: ID3D11SamplerState,
        cbuffer: ID3D11Buffer,
        copy: Option<(ID3D11Texture2D, ID3D11ShaderResourceView, u32, u32)>,
        size: (u32, u32),
        frames: u64,
        /// The same device, as Windows Graphics Capture wants it.
        capture_device: Option<IDirect3DDevice>,
    }

    // The device is free-threaded; the context is only used under the mutex.
    unsafe impl Send for Renderer {}

    impl Renderer {
        fn resize(&mut self, w: u32, h: u32) -> Result<(), String> {
            if (w, h) == self.size || w == 0 || h == 0 {
                return Ok(());
            }
            self.rtv = None;
            unsafe {
                self.swapchain
                    .ResizeBuffers(0, w, h, DXGI_FORMAT_UNKNOWN, DXGI_SWAP_CHAIN_FLAG(0))
                    .map_err(|e| err("resizing", e))?;
            }
            self.size = (w, h);
            Ok(())
        }

        fn target(&mut self) -> Result<ID3D11RenderTargetView, String> {
            if let Some(r) = &self.rtv {
                return Ok(r.clone());
            }
            unsafe {
                let buffer: ID3D11Texture2D = self.swapchain.GetBuffer(0).map_err(|e| err("back buffer", e))?;
                let mut rtv = None;
                self.device
                    .CreateRenderTargetView(&buffer, None, Some(&mut rtv))
                    .map_err(|e| err("render target", e))?;
                let rtv = rtv.ok_or("no render target")?;
                self.rtv = Some(rtv.clone());
                Ok(rtv)
            }
        }

        fn draw(&mut self, frame: &ID3D11Texture2D, place: Placement, p: ShaderParams) -> Result<(), String> {
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            unsafe { frame.GetDesc(&mut desc) };
            let (fw, fh) = (desc.Width, desc.Height);
            if self.copy.as_ref().is_none_or(|c| (c.2, c.3) != (fw, fh)) {
                let d = D3D11_TEXTURE2D_DESC {
                    Width: fw,
                    Height: fh,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                    ..Default::default()
                };
                unsafe {
                    let mut tex = None;
                    self.device.CreateTexture2D(&d, None, Some(&mut tex)).map_err(|e| err("frame copy", e))?;
                    let tex = tex.ok_or("no frame copy")?;
                    let mut srv = None;
                    self.device
                        .CreateShaderResourceView(&tex, None, Some(&mut srv))
                        .map_err(|e| err("frame view", e))?;
                    self.copy = Some((tex, srv.ok_or("no frame view")?, fw, fh));
                }
            }
            let (tex, srv) = {
                let c = self.copy.as_ref().expect("created above");
                (c.0.clone(), c.1.clone())
            };
            let rtv = self.target()?;
            let constants = Constants {
                rect: [
                    place.off_x as f32 / fw as f32,
                    place.off_y as f32 / fh as f32,
                    place.w as f32 / fw as f32,
                    place.h as f32 / fh as f32,
                ],
                texel: [1.0 / fw as f32, 1.0 / fh as f32],
                exposure: p.exposure,
                contrast: p.contrast,
                saturation: p.saturation,
                vibrance: p.vibrance,
                temperature: p.temperature,
                tint: p.tint,
                sharpen: p.sharpen,
                vignette: p.vignette,
                bloom: p.bloom,
                tonemap: p.tonemap,
            };
            unsafe {
                let ctx = &self.context;
                ctx.CopyResource(&tex, frame);
                ctx.UpdateSubresource(&self.cbuffer, 0, None, &constants as *const Constants as *const _, 0, 0);
                ctx.OMSetRenderTargets(Some(&[Some(rtv)]), None);
                ctx.RSSetViewports(Some(&[D3D11_VIEWPORT {
                    TopLeftX: 0.0,
                    TopLeftY: 0.0,
                    Width: self.size.0 as f32,
                    Height: self.size.1 as f32,
                    MinDepth: 0.0,
                    MaxDepth: 1.0,
                }]));
                ctx.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                ctx.IASetInputLayout(None);
                ctx.VSSetShader(&self.vs, None);
                ctx.PSSetShader(&self.ps, None);
                ctx.PSSetShaderResources(0, Some(&[Some(srv)]));
                ctx.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
                ctx.PSSetConstantBuffers(0, Some(&[Some(self.cbuffer.clone())]));
                ctx.Draw(3, 0);
                ctx.PSSetShaderResources(0, Some(&[None]));
                self.swapchain.Present(0, DXGI_PRESENT(0)).ok().map_err(|e| err("presenting", e))?;
            }
            self.frames += 1;
            Ok(())
        }
    }

    fn compile(entry: PCSTR, target: PCSTR) -> Result<Vec<u8>, String> {
        unsafe {
            let mut blob: Option<ID3DBlob> = None;
            let mut errors: Option<ID3DBlob> = None;
            let r = D3DCompile(
                SHADER.as_ptr() as *const _,
                SHADER.len(),
                PCSTR::null(),
                None,
                None,
                entry,
                target,
                0,
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
                return Err(format!("shader didn't compile: {} {detail}", e.message()));
            }
            let b = blob.ok_or("no shader")?;
            Ok(std::slice::from_raw_parts(b.GetBufferPointer() as *const u8, b.GetBufferSize()).to_vec())
        }
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        match msg {
            // Never take clicks or focus.
            WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            _ => unsafe { DefWindowProcW(hwnd, msg, w, l) },
        }
    }

    fn create_window() -> Result<HWND, String> {
        unsafe {
            let instance = GetModuleHandleW(None).map_err(|e| err("module", e))?;
            let class = w!("VesperShaderOverlay");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                lpszClassName: class,
                hCursor: HCURSOR::default(),
                ..Default::default()
            };
            // Registering twice fails harmlessly.
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST
                    | WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_NOACTIVATE
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOREDIRECTIONBITMAP,
                class,
                w!("Vesper shaders"),
                WS_POPUP,
                0,
                0,
                16,
                16,
                None,
                None,
                Some(instance.into()),
                None,
            )
            .map_err(|e| err("overlay window", e))?;
            let _ = SetLayeredWindowAttributes(hwnd, windows::Win32::Foundation::COLORREF(0), 255, LWA_ALPHA);
            Ok(hwnd)
        }
    }

    fn setup(target: HWND, overlay: HWND) -> Result<(Renderer, GraphicsCaptureItem, IDirect3DDevice), String> {
        unsafe {
            let mut device = None;
            let mut context = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
            .map_err(|e| err("Direct3D", e))?;
            let device: ID3D11Device = device.ok_or("no Direct3D device")?;
            let context = context.ok_or("no Direct3D context")?;
            let dxgi: IDXGIDevice = device.cast().map_err(|e| err("DXGI", e))?;
            let winrt: IDirect3DDevice = CreateDirect3D11DeviceFromDXGIDevice(&dxgi)
                .and_then(|i| i.cast())
                .map_err(|e| err("capture device", e))?;
            let interop =
                factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().map_err(|e| err("capture", e))?;
            let item: GraphicsCaptureItem =
                interop.CreateForWindow(target).map_err(|e| err("capturing Roblox's window", e))?;

            let adapter = dxgi.GetAdapter().map_err(|e| err("adapter", e))?;
            let factory: IDXGIFactory2 = adapter.GetParent().map_err(|e| err("DXGI factory", e))?;
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: 16,
                Height: 16,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                AlphaMode: DXGI_ALPHA_MODE_IGNORE,
                ..Default::default()
            };
            let swapchain = factory
                .CreateSwapChainForHwnd(&device, overlay, &desc, None, None)
                .map_err(|e| err("swap chain", e))?;

            let vs_code = compile(windows::core::s!("vs"), windows::core::s!("vs_5_0"))?;
            let ps_code = compile(windows::core::s!("ps"), windows::core::s!("ps_5_0"))?;
            let mut vs = None;
            device.CreateVertexShader(&vs_code, None, Some(&mut vs)).map_err(|e| err("vertex shader", e))?;
            let mut ps = None;
            device.CreatePixelShader(&ps_code, None, Some(&mut ps)).map_err(|e| err("pixel shader", e))?;
            let sd = D3D11_SAMPLER_DESC {
                Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                MaxLOD: f32::MAX,
                ..Default::default()
            };
            let mut sampler = None;
            device.CreateSamplerState(&sd, Some(&mut sampler)).map_err(|e| err("sampler", e))?;
            let bd = D3D11_BUFFER_DESC {
                ByteWidth: (std::mem::size_of::<Constants>() as u32).div_ceil(16) * 16,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                ..Default::default()
            };
            let mut cbuffer = None;
            device.CreateBuffer(&bd, None, Some(&mut cbuffer)).map_err(|e| err("constants", e))?;
            Ok((
                Renderer {
                    device,
                    context,
                    swapchain,
                    rtv: None,
                    vs: vs.ok_or("no vertex shader")?,
                    ps: ps.ok_or("no pixel shader")?,
                    sampler: sampler.ok_or("no sampler")?,
                    cbuffer: cbuffer.ok_or("no constants")?,
                    copy: None,
                    size: (16, 16),
                    frames: 0,
                    capture_device: None,
                },
                item,
                winrt,
            ))
        }
    }

    /// Grades `target` until it closes or `stop` is set.
    pub fn run(
        target: isize,
        params: &Arc<Mutex<ShaderParams>>,
        stop: &AtomicBool,
        follow_focus: bool,
        on_status: Arc<dyn Fn(ShaderStatus) + Send + Sync>,
    ) -> Result<(), String> {
        let target = HWND(target as *mut _);
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }
        let overlay = create_window()?;
        let result = (|| {
            let (renderer, item, device) = setup(target, overlay)?;
            let size = item.Size().map_err(|e| err("capture size", e))?;
            let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
                &device,
                DirectXPixelFormat::B8G8R8A8UIntNormalized,
                2,
                size,
            )
            .map_err(|e| err("frame pool", e))?;
            let session: GraphicsCaptureSession =
                pool.CreateCaptureSession(&item).map_err(|e| err("capture session", e))?;
            let _ = session.SetIsCursorCaptureEnabled(false);
            // No yellow frame around Roblox (Windows 11; older versions keep it).
            let _ = session.SetIsBorderRequired(false);

            let mut renderer = renderer;
            renderer.capture_device = Some(device.clone());
            let renderer = Arc::new(Mutex::new(renderer));
            let place = Arc::new(Mutex::new(placement(target).unwrap_or_default()));
            let last_size = Arc::new(Mutex::new(size));
            {
                let (renderer, place, params, last_size) =
                    (renderer.clone(), place.clone(), params.clone(), last_size.clone());
                pool.FrameArrived(&TypedEventHandler::new(
                    move |pool: windows::core::Ref<Direct3D11CaptureFramePool>, _| {
                        let Some(pool) = pool.as_ref() else { return Ok(()) };
                        let frame = pool.TryGetNextFrame()?;
                        let content = frame.ContentSize()?;
                        {
                            let mut last = last_size.lock().unwrap_or_else(|e| e.into_inner());
                            if content.Width != last.Width || content.Height != last.Height {
                                *last = content;
                                let r = renderer.lock().unwrap_or_else(|e| e.into_inner());
                                if let Some(device) = &r.capture_device {
                                    pool.Recreate(device, DirectXPixelFormat::B8G8R8A8UIntNormalized, 2, content)?;
                                }
                                return Ok(());
                            }
                        }
                        let access: IDirect3DDxgiInterfaceAccess = frame.Surface()?.cast()?;
                        let tex: ID3D11Texture2D = unsafe { access.GetInterface()? };
                        let p = *params.lock().unwrap_or_else(|e| e.into_inner());
                        let pl = *place.lock().unwrap_or_else(|e| e.into_inner());
                        let mut r = renderer.lock().unwrap_or_else(|e| e.into_inner());
                        if pl.w > 0 && pl.h > 0 {
                            if let Err(e) = r.resize(pl.w as u32, pl.h as u32).and_then(|()| r.draw(&tex, pl, p)) {
                                log::debug!("Shader frame skipped: {e}");
                            }
                        }
                        Ok(())
                    },
                ))
                .map_err(|e| err("frame events", e))?;
            }
            session.StartCapture().map_err(|e| err("starting capture", e))?;
            on_status(ShaderStatus::Running);
            log::info!("Shaders running over Roblox's window");

            let mut shown = false;
            let mut last_place = Placement::default();
            let started = Instant::now();
            while !stop.load(Ordering::Relaxed) {
                let mut msg = MSG::default();
                while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
                    unsafe {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
                if !unsafe { IsWindow(Some(target)) }.as_bool() {
                    break;
                }
                let pl = placement(target).unwrap_or_default();
                *place.lock().unwrap_or_else(|e| e.into_inner()) = pl;
                let drawn = renderer.lock().unwrap_or_else(|e| e.into_inner()).frames > 0;
                let active = !follow_focus || unsafe { GetForegroundWindow() } == target;
                let visible = drawn && active && !unsafe { IsIconic(target) }.as_bool() && pl.w > 0 && pl.h > 0;
                if visible && (!shown || pl != last_place) {
                    unsafe {
                        let _ = SetWindowPos(
                            overlay,
                            Some(HWND_TOPMOST),
                            pl.x,
                            pl.y,
                            pl.w,
                            pl.h,
                            SWP_NOACTIVATE | SWP_SHOWWINDOW,
                        );
                    }
                    shown = true;
                    last_place = pl;
                } else if !visible && shown {
                    unsafe {
                        let _ = ShowWindow(overlay, SW_HIDE);
                    }
                    shown = false;
                }
                if !drawn && started.elapsed() > Duration::from_secs(10) {
                    return Err("Roblox's window didn't send any frames to capture".into());
                }
                std::thread::sleep(Duration::from_millis(8));
            }
            let _ = session.Close();
            let _ = pool.Close();
            Ok(())
        })();
        unsafe {
            let _ = DestroyWindow(overlay);
        }
        result
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{ShaderParams, ShaderStatus};
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};

    pub fn main_window_of(_pids: &[u32]) -> Option<isize> {
        None
    }

    pub fn run(
        _target: isize,
        _params: &Arc<Mutex<ShaderParams>>,
        _stop: &AtomicBool,
        _follow_focus: bool,
        _on_status: Arc<dyn Fn(ShaderStatus) + Send + Sync>,
    ) -> Result<(), String> {
        Err("Shaders need Windows".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_exist_and_stay_in_range() {
        for (id, _) in PRESETS {
            let p = preset(id).unwrap();
            assert_eq!(p, p.clamped(), "{id} is within the sliders' ranges");
        }
        assert!(preset("nope").is_none());
    }

    #[test]
    fn odd_values_are_clamped() {
        let p = ShaderParams { exposure: 9.0, contrast: f32::NAN, saturation: -3.0, ..Default::default() }.clamped();
        assert_eq!(p.exposure, 1.0);
        assert_eq!(p.contrast, 1.0);
        assert_eq!(p.saturation, 0.0);
    }

    #[cfg(windows)]
    #[test]
    fn constants_match_the_shader_layout() {
        // float4 + float2 + 2 floats + 4 floats + 4 floats = 64 bytes.
        assert_eq!(std::mem::size_of::<Constants>(), 64);
    }
}
