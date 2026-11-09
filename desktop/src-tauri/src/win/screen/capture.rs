//! Ekran yakalama: Windows.Graphics.Capture (öncelikli, imleç dahil) ve DXGI Desktop Duplication (yedek).
//! Kaynaklar kareyi çağıranın verdiği BGRA D3D11 dokusuna kopyalar (GPU içinde).
use windows::{
    Graphics::{
        Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession},
        DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
    },
    Win32::{
        Foundation::{HMODULE, LPARAM, RECT},
        Graphics::{
            Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0},
            Direct3D11::*,
            Dxgi::{Common::*, *},
            Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW},
        },
        System::WinRT::{
            Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess},
            Graphics::Capture::IGraphicsCaptureItemInterop,
        },
    },
    core::{BOOL, Interface, Result},
};

#[derive(Clone)]
pub struct MonitorDesc {
    pub index: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
    pub left: i32,
    pub top: i32,
    pub hmon: HMONITOR,
}

pub fn monitors() -> Vec<MonitorDesc> {
    unsafe extern "system" fn cb(h: HMONITOR, _: HDC, _: *mut RECT, lp: LPARAM) -> BOOL {
        let v = unsafe { &mut *(lp.0 as *mut Vec<HMONITOR>) };
        v.push(h);
        BOOL(1)
    }
    let mut handles: Vec<HMONITOR> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(cb), LPARAM(&mut handles as *mut _ as isize));
    }
    let mut out = Vec::new();
    for (i, h) in handles.into_iter().enumerate() {
        let mut mi: MONITORINFOEXW = unsafe { std::mem::zeroed() };
        mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if !unsafe { GetMonitorInfoW(h, &mut mi as *mut MONITORINFOEXW as *mut MONITORINFO) }.as_bool() {
            continue;
        }
        let r = mi.monitorInfo.rcMonitor;
        let n = mi.szDevice.iter().position(|c| *c == 0).unwrap_or(mi.szDevice.len());
        out.push(MonitorDesc {
            index: i as u32,
            name: String::from_utf16_lossy(&mi.szDevice[..n]).trim_start_matches("\\\\.\\").to_string(),
            width: (r.right - r.left) as u32,
            height: (r.bottom - r.top) as u32,
            primary: mi.monitorInfo.dwFlags & 1 != 0,
            left: r.left,
            top: r.top,
            hmon: h,
        });
    }
    out
}

pub struct Gpu {
    pub device: ID3D11Device,
    pub ctx: ID3D11DeviceContext,
}

pub fn create_gpu() -> Result<Gpu> {
    let mut device = None;
    let mut ctx = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut ctx),
        )?;
        let device = device.ok_or_else(windows::core::Error::empty)?;
        let ctx = ctx.ok_or_else(windows::core::Error::empty)?;
        // MF/yakalama başka thread'lerden aynı cihazı kullanır.
        if let Ok(mt) = device.cast::<ID3D11Multithread>() {
            let _ = mt.SetMultithreadProtected(true);
        }
        Ok(Gpu { device, ctx })
    }
}

pub fn bgra_texture(device: &ID3D11Device, w: u32, h: u32) -> Result<ID3D11Texture2D> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: w,
        Height: h,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
        ..Default::default()
    };
    let mut t = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut t))? };
    t.ok_or_else(windows::core::Error::empty)
}

pub enum GrabError {
    /// Kaynak boyutu değişti / kayıp: ardışık düzeni yeniden kur.
    Restart,
}

pub trait Source {
    fn size(&self) -> (u32, u32);
    fn name(&self) -> &'static str;
    /// En yeni kareyi `dst`e kopyalar. Ok(true) = yeni kare; Ok(false) = zaman aşımı (değişiklik yok).
    fn grab(&mut self, dst: &ID3D11Texture2D, ctx: &ID3D11DeviceContext, wait_ms: u32) -> std::result::Result<bool, GrabError>;
}

// ---------------------------------------------------------------- Windows.Graphics.Capture

pub struct Wgc {
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    w: u32,
    h: u32,
}

impl Wgc {
    pub fn new(gpu: &Gpu, hmon: HMONITOR) -> Result<Self> {
        if !GraphicsCaptureSession::IsSupported()? {
            return Err(windows::core::Error::new(windows::core::HRESULT(-1), "WGC desteklenmiyor"));
        }
        let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        let item: GraphicsCaptureItem = unsafe { interop.CreateForMonitor(hmon)? };
        let size = item.Size()?;
        let dxgi: IDXGIDevice = gpu.device.cast()?;
        let d3d: IDirect3DDevice = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi)? }.cast()?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(&d3d, DirectXPixelFormat::B8G8R8A8UIntNormalized, 2, size)?;
        let session = pool.CreateCaptureSession(&item)?;
        let _ = session.SetIsCursorCaptureEnabled(true); // imleç videoda görünür
        let _ = session.SetIsBorderRequired(false); // Win11: sarı çerçeve kapalı (desteklenmezse yok sayılır)
        session.StartCapture()?;
        Ok(Self { pool, session, w: size.Width as u32, h: size.Height as u32 })
    }
}

impl Source for Wgc {
    fn size(&self) -> (u32, u32) {
        (self.w, self.h)
    }
    fn name(&self) -> &'static str {
        "WGC"
    }
    fn grab(&mut self, dst: &ID3D11Texture2D, ctx: &ID3D11DeviceContext, wait_ms: u32) -> std::result::Result<bool, GrabError> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(wait_ms as u64);
        loop {
            let mut newest = None;
            while let Ok(f) = self.pool.TryGetNextFrame() {
                newest = Some(f); // eski kareler bırakılır: her zaman en yeni
            }
            if let Some(f) = newest {
                let cs = f.ContentSize().map_err(|_| GrabError::Restart)?;
                if cs.Width as u32 != self.w || cs.Height as u32 != self.h {
                    return Err(GrabError::Restart);
                }
                let surface = f.Surface().map_err(|_| GrabError::Restart)?;
                let access: IDirect3DDxgiInterfaceAccess = surface.cast().map_err(|_| GrabError::Restart)?;
                let tex: ID3D11Texture2D = unsafe { access.GetInterface() }.map_err(|_| GrabError::Restart)?;
                unsafe { ctx.CopyResource(dst, &tex) };
                return Ok(true);
            }
            if std::time::Instant::now() >= deadline {
                return Ok(false);
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

impl Drop for Wgc {
    fn drop(&mut self) {
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}

// ---------------------------------------------------------------- DXGI Desktop Duplication (yedek)

/// Çıktı, GPU cihazından farklı bir bağdaştırıcıdaysa (hibrit dizüstü / ikinci GPU) çoğaltma kendi cihazında çalışır;
/// kare CPU üzerinden ana cihazın dokusuna aktarılır (yedek yol: yavaş ama çalışır).
struct Cross {
    _dev: ID3D11Device,
    ctx: ID3D11DeviceContext,
    staging: ID3D11Texture2D,
}

pub struct Dup {
    dup: IDXGIOutputDuplication,
    w: u32,
    h: u32,
    cross: Option<Cross>,
}

/// `adapters[i]` = i. bağdaştırıcının çıktılarının monitör tanıtıcıları. İlk eşleşen (bağdaştırıcı, çıktı) çifti.
pub(crate) fn find_output(adapters: &[Vec<isize>], hmon: isize) -> Option<(usize, usize)> {
    adapters.iter().enumerate().find_map(|(a, outs)| outs.iter().position(|h| *h == hmon).map(|o| (a, o)))
}

impl Dup {
    pub fn new(gpu: &Gpu, hmon: HMONITOR) -> Result<Self> {
        unsafe {
            // 1) Ana cihazın kendi bağdaştırıcısı (sıfır kopya).
            let dxgi: IDXGIDevice = gpu.device.cast()?;
            let adapter: IDXGIAdapter = dxgi.GetAdapter()?;
            let mut i = 0;
            while let Ok(out) = adapter.EnumOutputs(i) {
                let desc = out.GetDesc()?;
                if desc.Monitor == hmon {
                    let out1: IDXGIOutput1 = out.cast()?;
                    let dup = out1.DuplicateOutput(&gpu.device)?;
                    let r = desc.DesktopCoordinates;
                    return Ok(Self { dup, w: (r.right - r.left) as u32, h: (r.bottom - r.top) as u32, cross: None });
                }
                i += 1;
            }
            // 2) Başka bağdaştırıcılar.
            let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
            let mut adapters: Vec<IDXGIAdapter> = Vec::new();
            let mut handles: Vec<Vec<isize>> = Vec::new();
            let mut ai = 0;
            while let Ok(a1) = factory.EnumAdapters1(ai) {
                ai += 1;
                let a: IDXGIAdapter = a1.cast()?;
                let mut hs = Vec::new();
                let mut oi = 0;
                while let Ok(out) = a.EnumOutputs(oi) {
                    hs.push(out.GetDesc().map(|d| d.Monitor.0 as isize).unwrap_or(0));
                    oi += 1;
                }
                adapters.push(a);
                handles.push(hs);
            }
            let (ai, oi) = find_output(&handles, hmon.0 as isize)
                .ok_or_else(|| windows::core::Error::new(windows::core::HRESULT(-1), "monitör çıktısı bulunamadı"))?;
            let a = &adapters[ai];
            let out = a.EnumOutputs(oi as u32)?;
            let desc = out.GetDesc()?;
            let mut dev = None;
            let mut dctx = None;
            D3D11CreateDevice(
                a,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut dev),
                None,
                Some(&mut dctx),
            )?;
            let dev = dev.ok_or_else(windows::core::Error::empty)?;
            let dctx = dctx.ok_or_else(windows::core::Error::empty)?;
            let out1: IDXGIOutput1 = out.cast()?;
            let dup = out1.DuplicateOutput(&dev)?;
            let r = desc.DesktopCoordinates;
            let (w, h) = ((r.right - r.left) as u32, (r.bottom - r.top) as u32);
            let sd = D3D11_TEXTURE2D_DESC {
                Width: w,
                Height: h,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_STAGING,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                ..Default::default()
            };
            let mut st = None;
            dev.CreateTexture2D(&sd, None, Some(&mut st))?;
            let staging = st.ok_or_else(windows::core::Error::empty)?;
            Ok(Self { dup, w, h, cross: Some(Cross { _dev: dev, ctx: dctx, staging }) })
        }
    }
}

impl Source for Dup {
    fn size(&self) -> (u32, u32) {
        (self.w, self.h)
    }
    fn name(&self) -> &'static str {
        "DXGI-DD"
    }
    fn grab(&mut self, dst: &ID3D11Texture2D, ctx: &ID3D11DeviceContext, wait_ms: u32) -> std::result::Result<bool, GrabError> {
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut res: Option<IDXGIResource> = None;
        match unsafe { self.dup.AcquireNextFrame(wait_ms, &mut info, &mut res) } {
            Ok(()) => {
                let ok = (|| -> Result<()> {
                    let tex: ID3D11Texture2D = res.ok_or_else(windows::core::Error::empty)?.cast()?;
                    match &self.cross {
                        None => unsafe { ctx.CopyResource(dst, &tex) },
                        Some(c) => unsafe {
                            c.ctx.CopyResource(&c.staging, &tex);
                            let mut m = D3D11_MAPPED_SUBRESOURCE::default();
                            c.ctx.Map(&c.staging, 0, D3D11_MAP_READ, 0, Some(&mut m))?;
                            ctx.UpdateSubresource(dst, 0, None, m.pData, m.RowPitch, 0);
                            c.ctx.Unmap(&c.staging, 0);
                        },
                    }
                    Ok(())
                })();
                let _ = unsafe { self.dup.ReleaseFrame() };
                ok.map(|_| true).map_err(|_| GrabError::Restart)
            }
            Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => Ok(false),
            Err(_) => Err(GrabError::Restart), // erişim kaybı (UAC, mod değişimi) -> yeniden kur
        }
    }
}

// ---------------------------------------------------------------- BGRA -> NV12 (GPU, ölçekleme dahil)

pub const RING: usize = 4;

/// D3D11_VIDEO_PROCESSOR_COLOR_SPACE bit alanı: Usage:1 | RGB_Range:1 | YCbCr_Matrix:1 | xvYCC:1 | Nominal_Range:2.
fn color_space_bits(yuv_out: bool) -> u32 {
    if yuv_out {
        (1 << 2) | (1 << 4) // YCbCr_Matrix = BT.709, Nominal_Range = 16_235
    } else {
        0 // RGB_Range = 0: 0-255
    }
}

fn color_space(yuv_out: bool) -> D3D11_VIDEO_PROCESSOR_COLOR_SPACE {
    D3D11_VIDEO_PROCESSOR_COLOR_SPACE { _bitfield: color_space_bits(yuv_out) }
}

/// vdev/en/vp: GPU nesneleri Converter ömrü boyunca canlı tutulur.
#[allow(dead_code)]
pub struct Converter {
    vctx: ID3D11VideoContext,
    vdev: ID3D11VideoDevice,
    vp: ID3D11VideoProcessor,
    en: ID3D11VideoProcessorEnumerator,
    input_view: ID3D11VideoProcessorInputView,
    pub nv12: Vec<ID3D11Texture2D>,
    out_views: Vec<ID3D11VideoProcessorOutputView>,
}

impl Converter {
    pub fn new(gpu: &Gpu, bgra: &ID3D11Texture2D, src_w: u32, src_h: u32, out_w: u32, out_h: u32, fps: u32) -> Result<Self> {
        unsafe {
            let vdev: ID3D11VideoDevice = gpu.device.cast()?;
            let vctx: ID3D11VideoContext = gpu.ctx.cast()?;
            let cd = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
                InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
                InputFrameRate: DXGI_RATIONAL { Numerator: fps, Denominator: 1 },
                InputWidth: src_w,
                InputHeight: src_h,
                OutputFrameRate: DXGI_RATIONAL { Numerator: fps, Denominator: 1 },
                OutputWidth: out_w,
                OutputHeight: out_h,
                Usage: D3D11_VIDEO_USAGE_PLAYBACK_NORMAL,
            };
            let en = vdev.CreateVideoProcessorEnumerator(&cd)?;
            let vp = vdev.CreateVideoProcessor(&en, 0)?;

            let ivd = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
                FourCC: 0,
                ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
                Anonymous: D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0 { Texture2D: D3D11_TEX2D_VPIV { MipSlice: 0, ArraySlice: 0 } },
            };
            let mut iv = None;
            vdev.CreateVideoProcessorInputView(bgra, &en, &ivd, Some(&mut iv))?;
            let input_view = iv.ok_or_else(windows::core::Error::empty)?;

            let desc = D3D11_TEXTURE2D_DESC {
                Width: out_w,
                Height: out_h,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_NV12,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
                ..Default::default()
            };
            let mut nv12 = Vec::new();
            let mut out_views = Vec::new();
            for _ in 0..RING {
                let mut t = None;
                gpu.device.CreateTexture2D(&desc, None, Some(&mut t))?;
                let t = t.ok_or_else(windows::core::Error::empty)?;
                let ovd = D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
                    ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
                    Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 { Texture2D: D3D11_TEX2D_VPOV { MipSlice: 0 } },
                };
                let mut ov = None;
                vdev.CreateVideoProcessorOutputView(&t, &en, &ovd, Some(&mut ov))?;
                out_views.push(ov.ok_or_else(windows::core::Error::empty)?);
                nv12.push(t);
            }
            let src_rect = RECT { left: 0, top: 0, right: src_w as i32, bottom: src_h as i32 };
            let dst_rect = RECT { left: 0, top: 0, right: out_w as i32, bottom: out_h as i32 };
            vctx.VideoProcessorSetStreamSourceRect(&vp, 0, true, Some(&src_rect));
            vctx.VideoProcessorSetStreamDestRect(&vp, 0, true, Some(&dst_rect));
            vctx.VideoProcessorSetOutputTargetRect(&vp, true, Some(&dst_rect));
            // Renk: giriş BGRA tam aralık RGB; çıkış NV12 BT.709, sınırlı aralık (16-235). Kodlayıcı türüyle (encode.rs) aynı.
            // Sürücü bu çağrıları desteklemezse (void döner) varsayılan davranış sürer; hata oluşmaz.
            vctx.VideoProcessorSetStreamColorSpace(&vp, 0, &color_space(false));
            vctx.VideoProcessorSetOutputColorSpace(&vp, &color_space(true));
            Ok(Self { vctx, vdev, vp, en, input_view, nv12, out_views })
        }
    }

    /// BGRA kaynağı `idx` numaralı NV12 dokusuna ölçekleyerek dönüştürür.
    pub fn convert(&self, idx: usize) -> Result<()> {
        let mut stream = D3D11_VIDEO_PROCESSOR_STREAM {
            Enable: BOOL(1),
            pInputSurface: std::mem::ManuallyDrop::new(Some(self.input_view.clone())),
            ..Default::default()
        };
        let r = unsafe { self.vctx.VideoProcessorBlt(&self.vp, &self.out_views[idx], 0, std::slice::from_ref(&stream)) };
        unsafe { std::mem::ManuallyDrop::drop(&mut stream.pInputSurface) }; // klonlanan referansı bırak (sızıntı yok)
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_output_scans_all_adapters() {
        let a = vec![vec![10, 11], vec![], vec![20, 21, 22]];
        assert_eq!(find_output(&a, 10), Some((0, 0)));
        assert_eq!(find_output(&a, 21), Some((2, 1)));
        assert_eq!(find_output(&a, 99), None);
        assert_eq!(find_output(&[], 1), None);
    }

    #[test]
    fn color_space_is_bt709_limited_for_yuv_only() {
        assert_eq!(color_space_bits(false), 0);
        let b = color_space_bits(true);
        assert_eq!(b & (1 << 2), 1 << 2); // BT.709
        assert_eq!((b >> 4) & 3, 1); // 16-235
    }
}
