//! Media Foundation H.264 kodlayıcı (düşük gecikme, CBR, B karesi yok, GOP ~2 sn).
//! Önce donanım MFT (D3D11 dokusu doğrudan, sıfır kopya); olmazsa/başarısızsa yazılım MFT (NV12 CPU örneği).
use std::mem::ManuallyDrop;

use windows::{
    Win32::{
        Graphics::Direct3D11::*,
        Media::MediaFoundation::*,
        System::{
            Com::CoTaskMemFree,
            Variant::{VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BOOL, VT_UI4},
        },
    },
    core::{Interface, Result},
};

use super::capture::Gpu;

pub struct EncodedFrame {
    pub data: Vec<u8>,
    pub key: bool,
    pub pts_100ns: i64,
}

const EV_NEED_INPUT: u32 = 601; // METransformNeedInput
const EV_HAVE_OUTPUT: u32 = 602; // METransformHaveOutput

fn var_u32(v: u32) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 { vt: VT_UI4, wReserved1: 0, wReserved2: 0, wReserved3: 0, Anonymous: VARIANT_0_0_0 { ulVal: v } }),
        },
    }
}

fn var_bool(b: bool) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_BOOL,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 { boolVal: windows::Win32::Foundation::VARIANT_BOOL(if b { -1 } else { 0 }) },
            }),
        },
    }
}

fn pack(hi: u32, lo: u32) -> u64 {
    ((hi as u64) << 32) | lo as u64
}

/// Annex-B akışında (IDR var mı, SPS var mı). Saf fonksiyon: birim testli.
pub fn analyze_annexb(d: &[u8]) -> (bool, bool) {
    let (mut idr, mut sps) = (false, false);
    let mut i = 0;
    while i + 3 < d.len() {
        if d[i] == 0 && d[i + 1] == 0 && (d[i + 2] == 1 || (d[i + 2] == 0 && d[i + 3] == 1 && i + 4 < d.len())) {
            let h = if d[i + 2] == 1 { i + 3 } else { i + 4 };
            if h < d.len() {
                match d[h] & 0x1F {
                    5 => idr = true,
                    7 => sps = true,
                    _ => {}
                }
            }
            i = h;
        } else {
            i += 1;
        }
    }
    (idr, sps)
}

pub struct Encoder {
    mft: IMFTransform,
    codec: Option<ICodecAPI>,
    events: Option<IMFMediaEventGenerator>,
    _mgr: Option<IMFDXGIDeviceManager>,
    zero_copy: bool,
    pub hardware: bool,
    seq_header: Vec<u8>,
    w: u32,
    h: u32,
    fps: u32,
    need_input: u32,
    staging: Option<ID3D11Texture2D>,
    provides_samples: bool,
    out_size: u32,
}

fn enum_mfts(hw: bool) -> Vec<IMFActivate> {
    let flags = if hw {
        MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER
    } else {
        MFT_ENUM_FLAG_SYNCMFT | MFT_ENUM_FLAG_SORTANDFILTER
    };
    let out_info = MFT_REGISTER_TYPE_INFO { guidMajorType: MFMediaType_Video, guidSubtype: MFVideoFormat_H264 };
    let mut p: *mut Option<IMFActivate> = std::ptr::null_mut();
    let mut n = 0u32;
    let mut v = Vec::new();
    unsafe {
        if MFTEnumEx(MFT_CATEGORY_VIDEO_ENCODER, flags, None, Some(&out_info), &mut p, &mut n).is_err() || p.is_null() {
            return v;
        }
        for i in 0..n as usize {
            if let Some(a) = (*p.add(i)).take() {
                v.push(a);
            }
        }
        CoTaskMemFree(Some(p as *const _));
    }
    v
}

impl Encoder {
    /// COM + MFStartup çağıran thread'de başlatılmış olmalı.
    pub fn new(gpu: &Gpu, w: u32, h: u32, fps: u32, kbps: u32) -> Result<Self> {
        let mut last = windows::core::Error::new(windows::core::HRESULT(-1), "H.264 kodlayıcı bulunamadı");
        for hw in [true, false] {
            for act in enum_mfts(hw) {
                match unsafe { Self::setup(&act, gpu, hw, w, h, fps, kbps) } {
                    Ok(e) => return Ok(e),
                    Err(e) => last = e,
                }
            }
        }
        Err(last)
    }

    unsafe fn setup(act: &IMFActivate, gpu: &Gpu, hw: bool, w: u32, h: u32, fps: u32, kbps: u32) -> Result<Self> {
        unsafe {
            let mft: IMFTransform = act.ActivateObject()?;
            let attrs = mft.GetAttributes().ok();
            let is_async = attrs.as_ref().is_some_and(|a| a.GetUINT32(&MF_TRANSFORM_ASYNC).unwrap_or(0) != 0);
            if is_async {
                attrs.as_ref().unwrap().SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1)?;
            }
            if let Some(a) = &attrs {
                let _ = a.SetUINT32(&MF_LOW_LATENCY, 1);
            }

            // Donanım MFT'ye D3D11 cihazını ver (sıfır kopya); kabul etmezse CPU örneği kullanılır.
            let mut mgr_keep = None;
            let mut zero_copy = false;
            if hw {
                let mut token = 0u32;
                let mut mgr = None;
                MFCreateDXGIDeviceManager(&mut token, &mut mgr)?;
                if let Some(m) = mgr {
                    m.ResetDevice(&gpu.device, token)?;
                    zero_copy = mft.ProcessMessage(MFT_MESSAGE_SET_D3D_MANAGER, m.as_raw() as usize).is_ok();
                    mgr_keep = Some(m);
                }
            }

            let codec: Option<ICodecAPI> = mft.cast().ok();
            if let Some(c) = &codec {
                let _ = c.SetValue(&CODECAPI_AVEncCommonRateControlMode, &var_u32(0)); // CBR
                let _ = c.SetValue(&CODECAPI_AVEncCommonMeanBitRate, &var_u32(kbps * 1000));
                let _ = c.SetValue(&CODECAPI_AVLowLatencyMode, &var_bool(true));
                let _ = c.SetValue(&CODECAPI_AVEncMPVGOPSize, &var_u32(fps * 2));
                let _ = c.SetValue(&CODECAPI_AVEncMPVDefaultBPictureCount, &var_u32(0));
            }

            let out = MFCreateMediaType()?;
            out.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            out.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
            out.SetUINT32(&MF_MT_AVG_BITRATE, kbps * 1000)?;
            out.SetUINT64(&MF_MT_FRAME_SIZE, pack(w, h))?;
            out.SetUINT64(&MF_MT_FRAME_RATE, pack(fps, 1))?;
            out.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))?;
            out.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
            out.SetUINT32(&MF_MT_MPEG2_PROFILE, 77)?; // Main
            mft.SetOutputType(0, &out, 0)?;

            let mut set_in = false;
            for i in 0..32 {
                let Ok(t) = mft.GetInputAvailableType(0, i) else { break };
                if t.GetGUID(&MF_MT_SUBTYPE).ok() == Some(MFVideoFormat_NV12) {
                    t.SetUINT64(&MF_MT_FRAME_SIZE, pack(w, h))?;
                    t.SetUINT64(&MF_MT_FRAME_RATE, pack(fps, 1))?;
                    t.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))?;
                    t.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
                    mft.SetInputType(0, &t, 0)?;
                    set_in = true;
                    break;
                }
            }
            if !set_in {
                return Err(windows::core::Error::new(windows::core::HRESULT(-1), "NV12 girişi kabul edilmedi"));
            }

            let mut seq_header = Vec::new();
            if let Ok(cur) = mft.GetOutputCurrentType(0) {
                if let Ok(sz) = cur.GetBlobSize(&MF_MT_MPEG_SEQUENCE_HEADER) {
                    let mut buf = vec![0u8; sz as usize];
                    if cur.GetBlob(&MF_MT_MPEG_SEQUENCE_HEADER, &mut buf, None).is_ok() {
                        seq_header = buf;
                    }
                }
            }

            let info = mft.GetOutputStreamInfo(0)?;
            let provides_samples = info.dwFlags & MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 != 0;
            mft.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0)?;
            mft.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
            mft.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;

            let events = if is_async { mft.cast::<IMFMediaEventGenerator>().ok() } else { None };
            Ok(Self {
                mft,
                codec,
                events,
                _mgr: mgr_keep,
                zero_copy,
                hardware: hw,
                seq_header,
                w,
                h,
                fps,
                need_input: 0,
                staging: None,
                provides_samples,
                out_size: info.cbSize.max(w * h * 2),
            })
        }
    }

    pub fn describe(&self) -> String {
        format!("{} ({})", if self.hardware { "donanım H.264" } else { "yazılım H.264" }, if self.zero_copy { "sıfır kopya" } else { "CPU örneği" })
    }

    pub fn force_key(&self) {
        if let Some(c) = &self.codec {
            let _ = unsafe { c.SetValue(&CODECAPI_AVEncVideoForceKeyFrame, &var_u32(1)) };
        }
    }

    pub fn set_bitrate(&self, kbps: u32) {
        if let Some(c) = &self.codec {
            let _ = unsafe { c.SetValue(&CODECAPI_AVEncCommonMeanBitRate, &var_u32(kbps * 1000)) };
        }
    }

    fn make_sample(&mut self, gpu: &Gpu, nv12: &ID3D11Texture2D, pts: i64) -> Result<IMFSample> {
        unsafe {
            let sample = MFCreateSample()?;
            if self.zero_copy {
                let buf = MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, nv12, 0, false)?;
                let b2: IMF2DBuffer = buf.cast()?;
                buf.SetCurrentLength(b2.GetContiguousLength()?)?;
                sample.AddBuffer(&buf)?;
            } else {
                // NV12 dokusunu CPU'ya oku (hazırlama dokusu) ve bitişik NV12 tampona yaz.
                if self.staging.is_none() {
                    let mut desc = D3D11_TEXTURE2D_DESC::default();
                    nv12.GetDesc(&mut desc);
                    desc.Usage = D3D11_USAGE_STAGING;
                    desc.BindFlags = 0;
                    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
                    let mut t = None;
                    gpu.device.CreateTexture2D(&desc, None, Some(&mut t))?;
                    self.staging = t;
                }
                let st = self.staging.as_ref().unwrap();
                gpu.ctx.CopyResource(st, nv12);
                let mut m = D3D11_MAPPED_SUBRESOURCE::default();
                gpu.ctx.Map(st, 0, D3D11_MAP_READ, 0, Some(&mut m))?;
                let (w, h) = (self.w as usize, self.h as usize);
                let size = w * h * 3 / 2;
                let buf = MFCreateMemoryBuffer(size as u32)?;
                let mut dst = std::ptr::null_mut();
                buf.Lock(&mut dst, None, None)?;
                let src = m.pData as *const u8;
                let pitch = m.RowPitch as usize;
                for y in 0..h {
                    std::ptr::copy_nonoverlapping(src.add(y * pitch), dst.add(y * w), w);
                }
                let uv_src = src.add(pitch * h);
                for y in 0..h / 2 {
                    std::ptr::copy_nonoverlapping(uv_src.add(y * pitch), dst.add(w * h + y * w), w);
                }
                buf.Unlock()?;
                gpu.ctx.Unmap(st, 0);
                buf.SetCurrentLength(size as u32)?;
                sample.AddBuffer(&buf)?;
            }
            sample.SetSampleTime(pts)?;
            sample.SetSampleDuration(10_000_000 / self.fps.max(1) as i64)?;
            Ok(sample)
        }
    }

    /// Bir NV12 dokusunu kodlar; hazır çıktıları döner (async MFT'de gecikmeli olabilir).
    pub fn encode(&mut self, gpu: &Gpu, nv12: &ID3D11Texture2D, pts_100ns: i64) -> Result<Vec<EncodedFrame>> {
        let sample = self.make_sample(gpu, nv12, pts_100ns)?;
        let mut out = Vec::new();
        if let Some(ev) = self.events.clone() {
            self.pump(&ev, &mut out)?;
            if self.need_input == 0 {
                std::thread::sleep(std::time::Duration::from_millis(4));
                self.pump(&ev, &mut out)?;
                if self.need_input == 0 {
                    return Ok(out); // kodlayıcı meşgul: bu kare atlanır (gecikme birikmesin)
                }
            }
            unsafe { self.mft.ProcessInput(0, &sample, 0)? };
            self.need_input -= 1;
            let end = std::time::Instant::now() + std::time::Duration::from_millis(30);
            while out.is_empty() && std::time::Instant::now() < end {
                self.pump(&ev, &mut out)?;
                if out.is_empty() {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }
        } else {
            unsafe { self.mft.ProcessInput(0, &sample, 0)? };
            while self.read_output(&mut out)? {}
        }
        Ok(out)
    }

    fn pump(&mut self, ev: &IMFMediaEventGenerator, out: &mut Vec<EncodedFrame>) -> Result<()> {
        loop {
            let Ok(e) = (unsafe { ev.GetEvent(MF_EVENT_FLAG_NO_WAIT) }) else { return Ok(()) };
            match unsafe { e.GetType()? } {
                EV_NEED_INPUT => self.need_input += 1,
                EV_HAVE_OUTPUT => {
                    self.read_output(out)?;
                }
                _ => {}
            }
        }
    }

    /// Bir çıktı örneği okur. Ok(true) = örnek alındı (tekrar dene), Ok(false) = giriş bekliyor.
    fn read_output(&mut self, out: &mut Vec<EncodedFrame>) -> Result<bool> {
        unsafe {
            let own = if self.provides_samples {
                None
            } else {
                let s = MFCreateSample()?;
                s.AddBuffer(&MFCreateMemoryBuffer(self.out_size)?)?;
                Some(s)
            };
            let mut buf = MFT_OUTPUT_DATA_BUFFER { dwStreamID: 0, pSample: ManuallyDrop::new(own), dwStatus: 0, pEvents: ManuallyDrop::new(None) };
            let mut status = 0u32;
            let r = self.mft.ProcessOutput(0, std::slice::from_mut(&mut buf), &mut status);
            let sample = ManuallyDrop::take(&mut buf.pSample);
            let _ = ManuallyDrop::take(&mut buf.pEvents);
            match r {
                Ok(()) => {
                    let Some(sample) = sample else { return Ok(false) };
                    let pts = sample.GetSampleTime().unwrap_or(0);
                    let cb = sample.ConvertToContiguousBuffer()?;
                    let mut p = std::ptr::null_mut();
                    let mut len = 0u32;
                    cb.Lock(&mut p, None, Some(&mut len))?;
                    let mut data = std::slice::from_raw_parts(p, len as usize).to_vec();
                    cb.Unlock()?;
                    let (idr, has_sps) = analyze_annexb(&data);
                    if idr && !has_sps && !self.seq_header.is_empty() {
                        let mut v = self.seq_header.clone();
                        v.extend_from_slice(&data);
                        data = v;
                    }
                    if !data.is_empty() {
                        out.push(EncodedFrame { data, key: idr, pts_100ns: pts });
                    }
                    Ok(true)
                }
                Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => Ok(false),
                Err(e) if e.code() == MF_E_TRANSFORM_STREAM_CHANGE => Ok(true),
                Err(e) => Err(e),
            }
        }
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe {
            let _ = self.mft.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0);
            let _ = self.mft.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0);
            let _ = self.mft.ProcessMessage(MFT_MESSAGE_SET_D3D_MANAGER, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annexb_analysis() {
        let sps_idr = [0, 0, 0, 1, 0x67, 1, 2, 0, 0, 1, 0x68, 3, 0, 0, 0, 1, 0x65, 9, 9];
        assert_eq!(analyze_annexb(&sps_idr), (true, true));
        let p = [0, 0, 1, 0x41, 1, 2, 3];
        assert_eq!(analyze_annexb(&p), (false, false));
        let idr_only = [0, 0, 1, 0x65, 1];
        assert_eq!(analyze_annexb(&idr_only), (true, false));
        assert_eq!(analyze_annexb(&[]), (false, false));
    }
}
