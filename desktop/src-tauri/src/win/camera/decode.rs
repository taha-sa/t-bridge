//! Telefon kamerasından gelen Annex-B H.264'ü Media Foundation H.264 çözücü MFT'siyle (Microsoft; D3D yöneticisi
//! verilirse DXVA, burada yazılım yolu) NV12'ye çözer ve boşluksuz (stride'sız) NV12 olarak döner.
use windows::{
    Win32::{
        Media::MediaFoundation::*,
        System::{
            Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
            Variant::{VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BOOL},
        },
    },
    core::{Interface, Result},
};

pub struct Decoded {
    pub nv12: Vec<u8>,
    pub w: u32,
    pub h: u32,
}

pub struct Decoder {
    mft: IMFTransform,
    codec: Option<ICodecAPI>,
    out_ready: bool,
    stride: usize,
    buf_h: usize,
    vis_w: usize,
    vis_h: usize,
    provides: bool,
    out_size: u32,
}

fn pack(hi: u32, lo: u32) -> u64 {
    ((hi as u64) << 32) | lo as u64
}

impl Decoder {
    /// COM + MFStartup çağıran thread'de başlatılmış olmalı.
    pub fn new() -> Result<Self> {
        unsafe {
            let mft: IMFTransform = CoCreateInstance(&CLSID_MSH264DecoderMFT, None, CLSCTX_INPROC_SERVER)?;
            if let Ok(a) = mft.GetAttributes() {
                let _ = a.SetUINT32(&MF_LOW_LATENCY, 1);
            }
            let codec: Option<ICodecAPI> = mft.cast().ok();
            if let Some(c) = &codec {
                let v = VARIANT {
                    Anonymous: VARIANT_0 {
                        Anonymous: std::mem::ManuallyDrop::new(VARIANT_0_0 {
                            vt: VT_BOOL,
                            wReserved1: 0,
                            wReserved2: 0,
                            wReserved3: 0,
                            Anonymous: VARIANT_0_0_0 { boolVal: windows::Win32::Foundation::VARIANT_BOOL(-1) },
                        }),
                    },
                };
                let _ = c.SetValue(&CODECAPI_AVLowLatencyMode, &v);
            }
            let t = MFCreateMediaType()?;
            t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            t.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
            t.SetUINT64(&MF_MT_FRAME_SIZE, pack(1280, 720))?; // ipucu; gerçek boyut SPS'ten gelir
            t.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
            mft.SetInputType(0, &t, 0)?;
            mft.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
            mft.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
            let mut d = Self { mft, codec, out_ready: false, stride: 0, buf_h: 0, vis_w: 0, vis_h: 0, provides: false, out_size: 0 };
            let _ = d.negotiate_output(); // SPS gelmeden başarısız olabilir; STREAM_CHANGE'de yeniden denenir
            Ok(d)
        }
    }

    /// Çıkış türünü NV12'ye ayarlar ve boyut/stride bilgisini okur.
    fn negotiate_output(&mut self) -> Result<()> {
        unsafe {
            for i in 0..64 {
                let Ok(t) = self.mft.GetOutputAvailableType(0, i) else { break };
                if t.GetGUID(&MF_MT_SUBTYPE).ok() == Some(MFVideoFormat_NV12) {
                    self.mft.SetOutputType(0, &t, 0)?;
                    let cur = self.mft.GetOutputCurrentType(0)?;
                    let fs = cur.GetUINT64(&MF_MT_FRAME_SIZE)?;
                    let (fw, fh) = ((fs >> 32) as usize, fs as u32 as usize);
                    self.stride = cur.GetUINT32(&MF_MT_DEFAULT_STRIDE).map(|s| (s as i32).unsigned_abs() as usize).unwrap_or(fw).max(fw);
                    self.buf_h = fh;
                    // Görünen alan: MFVideoArea (OffsetX, OffsetY: 4+4 bayt; Area: cx,cy i32)
                    let (mut vw, mut vh) = (fw, fh);
                    let mut blob = [0u8; 16];
                    if cur.GetBlob(&MF_MT_MINIMUM_DISPLAY_APERTURE, &mut blob, None).is_ok() {
                        let cx = i32::from_le_bytes(blob[8..12].try_into().unwrap());
                        let cy = i32::from_le_bytes(blob[12..16].try_into().unwrap());
                        if cx > 0 && cy > 0 {
                            vw = cx as usize;
                            vh = cy as usize;
                        }
                    }
                    self.vis_w = vw & !1;
                    self.vis_h = vh & !1;
                    let info = self.mft.GetOutputStreamInfo(0)?;
                    self.provides = info.dwFlags & MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 != 0;
                    self.out_size = info.cbSize.max((self.stride * self.buf_h * 3 / 2) as u32);
                    self.out_ready = true;
                    return Ok(());
                }
            }
        }
        Err(windows::core::Error::new(windows::core::HRESULT(-1), "NV12 çıkışı yok"))
    }

    /// Bir Annex-B erişim birimi (kare) çözer; hazır kareleri döner (çoğunlukla 0 veya 1).
    pub fn decode(&mut self, data: &[u8], pts_100ns: i64) -> Result<Vec<Decoded>> {
        let mut out = Vec::new();
        unsafe {
            let buf = MFCreateMemoryBuffer(data.len() as u32)?;
            let mut p = std::ptr::null_mut();
            buf.Lock(&mut p, None, None)?;
            std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
            buf.Unlock()?;
            buf.SetCurrentLength(data.len() as u32)?;
            let sample = MFCreateSample()?;
            sample.AddBuffer(&buf)?;
            sample.SetSampleTime(pts_100ns)?;
            match self.mft.ProcessInput(0, &sample, 0) {
                Ok(()) => {}
                Err(e) if e.code() == MF_E_NOTACCEPTING => {
                    self.drain(&mut out)?;
                    self.mft.ProcessInput(0, &sample, 0)?;
                }
                Err(e) => return Err(e),
            }
            self.drain(&mut out)?;
        }
        Ok(out)
    }

    fn drain(&mut self, out: &mut Vec<Decoded>) -> Result<()> {
        unsafe {
            loop {
                if !self.out_ready && self.negotiate_output().is_err() {
                    return Ok(());
                }
                let own = if self.provides {
                    None
                } else {
                    let s = MFCreateSample()?;
                    s.AddBuffer(&MFCreateMemoryBuffer(self.out_size)?)?;
                    Some(s)
                };
                let mut db = MFT_OUTPUT_DATA_BUFFER { dwStreamID: 0, pSample: std::mem::ManuallyDrop::new(own), dwStatus: 0, pEvents: std::mem::ManuallyDrop::new(None) };
                let mut status = 0u32;
                let r = self.mft.ProcessOutput(0, std::slice::from_mut(&mut db), &mut status);
                let sample = std::mem::ManuallyDrop::take(&mut db.pSample);
                let _ = std::mem::ManuallyDrop::take(&mut db.pEvents);
                match r {
                    Ok(()) => {
                        let Some(sample) = sample else { return Ok(()) };
                        let cb = sample.ConvertToContiguousBuffer()?;
                        let mut p = std::ptr::null_mut();
                        let mut len = 0u32;
                        cb.Lock(&mut p, None, Some(&mut len))?;
                        let src = std::slice::from_raw_parts(p, len as usize);
                        if let Some(d) = self.compact(src) {
                            out.push(d);
                        }
                        cb.Unlock()?;
                    }
                    Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => return Ok(()),
                    Err(e) if e.code() == MF_E_TRANSFORM_STREAM_CHANGE => {
                        self.out_ready = false;
                        self.negotiate_output()?;
                    }
                    Err(e) => return Err(e),
                }
            }
        }
    }

    /// stride/hizalı yükseklikli NV12 tamponunu görünen alana göre boşluksuz NV12'ye kopyalar.
    fn compact(&self, src: &[u8]) -> Option<Decoded> {
        let (w, h, s, bh) = (self.vis_w, self.vis_h, self.stride, self.buf_h);
        if w == 0 || h == 0 || src.len() < s * bh + s * bh / 2 || w > s || h > bh {
            return None;
        }
        let mut nv12 = vec![0u8; w * h * 3 / 2];
        for y in 0..h {
            nv12[y * w..y * w + w].copy_from_slice(&src[y * s..y * s + w]);
        }
        let uv = s * bh;
        for y in 0..h / 2 {
            nv12[w * h + y * w..w * h + y * w + w].copy_from_slice(&src[uv + y * s..uv + y * s + w]);
        }
        Some(Decoded { nv12, w: w as u32, h: h as u32 })
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe {
            let _ = self.mft.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0);
            let _ = self.mft.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0);
        }
        let _ = &self.codec;
    }
}
