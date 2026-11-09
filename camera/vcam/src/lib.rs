//! T-Bridge sanal kamera: Frame Server (svchost, LocalService) içinde çalışan Media Foundation medya kaynağı.
//!
//! Mimari (Microsoft Windows-Camera "VirtualCamera" örneğindeki yapıya dayanır; kod özgündür):
//!   MFCreateVirtualCamera(sourceId = CLSID)  ->  Frame Server  ->  CoCreateInstance(CLSID) = [`Activate`] (IMFActivate)
//!   ActivateObject  ->  [`Source`] (IMFMediaSourceEx, IMFGetService, IKsControl)  ->  [`Stream`] (IMFMediaStream2)
//! Kare kaynağı: T-Bridge uygulamasının yazdığı NV12 paylaşımlı bellek halkası (`kopru-camring`). Bölümü bu DLL yaratır
//! (hizmet hesabında SeCreateGlobalPrivilege vardır), uygulama açıp yazar. 1 sn taze kare yoksa yer tutucu kare üretilir.
#![allow(non_snake_case)]
mod frames;

use std::ffi::c_void;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use kopru_camring::{Ring, STALE_MS};
use windows::{
    Win32::{
        Foundation::*,
        Media::{KernelStreaming::*, MediaFoundation::*},
        Security::{Authorization::*, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES},
        System::{
            Com::*,
            Memory::*,
            SystemInformation::GetTickCount64,
        },
    },
    core::*,
};

/// Sanal kamera medya kaynağının CLSID'si (kayıt defteri ve MFCreateVirtualCamera ile aynı).
pub const CLSID_TBRIDGE_VCAM: GUID = GUID::from_u128(0x7b3f1c52_4d2e_4c71_9a0b_5e2d8f6a1c34);

const MODES: [(u32, u32); 2] = [(1280, 720), (1920, 1080)];
const FPS: u32 = 30;

fn pack(hi: u32, lo: u32) -> u64 {
    ((hi as u64) << 32) | lo as u64
}

// ---------------------------------------------------------------- paylaşımlı bellek (kaynak yaratır)

struct Shm {
    _h: HANDLE,
    ring: Ring,
}
unsafe impl Send for Shm {}
unsafe impl Sync for Shm {}

/// Kamerayı etkinleştiren kullanıcının SID'i: kayıt sırasında HKLM\SOFTWARE\Classes\CLSID\{..} altına `OwnerSid` olarak yazılır.
fn read_owner_sid() -> Option<String> {
    use windows::Win32::System::Registry::*;
    unsafe {
        let key: Vec<u16> = "SOFTWARE\\Classes\\CLSID\\{7B3F1C52-4D2E-4C71-9A0B-5E2D8F6A1C34}".encode_utf16().chain(std::iter::once(0)).collect();
        let name: Vec<u16> = "OwnerSid".encode_utf16().chain(std::iter::once(0)).collect();
        let mut buf = [0u16; 256];
        let mut len = (buf.len() * 2) as u32;
        let r = RegGetValueW(HKEY_LOCAL_MACHINE, PCWSTR(key.as_ptr()), PCWSTR(name.as_ptr()), RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr() as *mut c_void), Some(&mut len));
        if r.0 != 0 {
            return None;
        }
        let n = (len as usize / 2).saturating_sub(1).min(buf.len());
        Some(String::from_utf16_lossy(&buf[..n]))
    }
}

fn open_shm() -> Option<Shm> {
    unsafe {
        let mut sd = PSECURITY_DESCRIPTOR::default();
        let sddl: Vec<u16> = kopru_camring::build_sddl(read_owner_sid().as_deref()).encode_utf16().chain(std::iter::once(0)).collect();
        ConvertStringSecurityDescriptorToSecurityDescriptorW(PCWSTR(sddl.as_ptr()), SDDL_REVISION_1, &mut sd, None).ok()?;
        let sa = SECURITY_ATTRIBUTES { nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32, lpSecurityDescriptor: sd.0, bInheritHandle: false.into() };
        let name: Vec<u16> = kopru_camring::SECTION_NAME.encode_utf16().chain(std::iter::once(0)).collect();
        let size = kopru_camring::TOTAL_SIZE as u64;
        let h = CreateFileMappingW(INVALID_HANDLE_VALUE, Some(&sa), PAGE_READWRITE, (size >> 32) as u32, size as u32, PCWSTR(name.as_ptr())).ok()?;
        let existed = GetLastError() == ERROR_ALREADY_EXISTS;
        let view = MapViewOfFile(h, FILE_MAP_ALL_ACCESS, 0, 0, 0);
        if view.Value.is_null() {
            let _ = CloseHandle(h);
            return None;
        }
        let ring = Ring::from_raw(view.Value as *mut u8);
        if !existed || !ring.is_valid() {
            ring.init();
        }
        Some(Shm { _h: h, ring })
    }
}

// ---------------------------------------------------------------- akış

/// MF olay kuyruğu iş parçacığı-güvenlidir (MFCreateEventQueue).
struct SendQ(IMFMediaEventQueue);
unsafe impl Send for SendQ {}

struct StreamState {
    started: bool,
    pending: u32,
    w: u32,
    h: u32,
    shutdown: bool,
}

#[implement(IMFMediaStream2)]
struct Stream {
    queue: IMFMediaEventQueue,
    desc: IMFStreamDescriptor,
    source: Mutex<Option<IMFMediaSource>>,
    st: Arc<(Mutex<StreamState>, Condvar)>,
    shm: Option<Arc<Shm>>,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl Stream {
    fn new(desc: IMFStreamDescriptor, shm: Option<Arc<Shm>>) -> Result<Self> {
        Ok(Self {
            queue: unsafe { MFCreateEventQueue()? },
            desc,
            source: Mutex::new(None),
            st: Arc::new((Mutex::new(StreamState { started: false, pending: 0, w: MODES[0].0, h: MODES[0].1, shutdown: false }), Condvar::new())),
            shm,
            worker: Mutex::new(None),
        })
    }

    /// Geçerli ortam türünden boyutu okur.
    fn current_size(&self) -> (u32, u32) {
        unsafe {
            if let Ok(h) = self.desc.GetMediaTypeHandler() {
                if let Ok(t) = h.GetCurrentMediaType() {
                    if let Ok(v) = t.GetUINT64(&MF_MT_FRAME_SIZE) {
                        return ((v >> 32) as u32, v as u32);
                    }
                }
            }
        }
        MODES[0]
    }

    fn start(&self) {
        let (w, h) = self.current_size();
        {
            let mut g = self.st.0.lock().unwrap();
            g.started = true;
            g.pending = 0;
            g.w = w;
            g.h = h;
        }
        if let Some(shm) = &self.shm {
            shm.ring.set_want(w, h);
        }
        let mut worker = self.worker.lock().unwrap();
        if worker.is_none() {
            let st = self.st.clone();
            let shm = self.shm.clone();
            let q = SendQ(self.queue.clone());
            *worker = Some(std::thread::spawn(move || {
                let q = q;
                worker_loop(st, shm, q.0)
            }));
        }
        unsafe {
            let _ = self.queue.QueueEventParamVar(MEStreamStarted.0 as u32, &GUID::zeroed(), S_OK, std::ptr::null());
        }
    }

    fn stop(&self) {
        self.st.0.lock().unwrap().started = false;
        unsafe {
            let _ = self.queue.QueueEventParamVar(MEStreamStopped.0 as u32, &GUID::zeroed(), S_OK, std::ptr::null());
        }
    }

    fn shutdown(&self) {
        {
            let mut g = self.st.0.lock().unwrap();
            g.shutdown = true;
            g.started = false;
        }
        self.st.1.notify_all();
        if let Some(j) = self.worker.lock().unwrap().take() {
            let _ = j.join();
        }
        unsafe {
            let _ = self.queue.Shutdown();
        }
        *self.source.lock().unwrap() = None; // döngüsel başvuruyu kır
    }
}

/// İstek geldikçe (RequestSample) kare hızında örnek üretir: halkadaki en yeni kare, yoksa/bayatsa yer tutucu.
fn worker_loop(st: Arc<(Mutex<StreamState>, Condvar)>, shm: Option<Arc<Shm>>, queue: IMFMediaEventQueue) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let interval = Duration::from_micros(1_000_000 / FPS as u64);
    let mut next = Instant::now();
    let mut last_seq = 0u64;
    let mut cur: Vec<u8> = Vec::new(); // son gösterilen kare (hedef boyutta)
    let mut raw: Vec<u8> = Vec::new();
    let mut placeholder: Vec<u8> = Vec::new();
    let mut ph_dim = (0u32, 0u32);
    let (lock, cv) = &*st;
    loop {
        let (w, h, token_count) = {
            let mut g = lock.lock().unwrap();
            while !g.shutdown && (!g.started || g.pending == 0) {
                g = cv.wait(g).unwrap();
            }
            if g.shutdown {
                return;
            }
            g.pending -= 1;
            (g.w, g.h, g.pending)
        };
        let _ = token_count;
        // kare hızı: ardışık örnekler arası en az interval
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        }
        next = Instant::now() + interval;

        let need = (w * h * 3 / 2) as usize;
        if cur.len() != need {
            cur = vec![0; need];
        }
        let mut have_live = false;
        if let Some(shm) = &shm {
            let ring = &shm.ring;
            let tick = unsafe { GetTickCount64() };
            ring.touch_consumer(tick);
            ring.set_want(w, h);
            if !ring.is_stale(tick, STALE_MS) {
                if let Some(info) = ring.read_latest(last_seq, &mut raw) {
                    last_seq = info.seq;
                    frames::scale_nv12(&raw, info.w as usize, info.h as usize, &mut cur, w as usize, h as usize);
                }
                have_live = true; // taze yazıcı var: yeni kare gelmediyse son kare tekrarlanır
            }
        }
        if !have_live {
            if ph_dim != (w, h) {
                placeholder = frames::placeholder_nv12(w as usize, h as usize);
                ph_dim = (w, h);
            }
            cur.copy_from_slice(&placeholder);
            last_seq = 0;
        }

        unsafe {
            let make = || -> Result<IMFSample> {
                let buf = MFCreateMemoryBuffer(need as u32)?;
                let mut p = std::ptr::null_mut();
                buf.Lock(&mut p, None, None)?;
                std::ptr::copy_nonoverlapping(cur.as_ptr(), p, need);
                buf.Unlock()?;
                buf.SetCurrentLength(need as u32)?;
                let s = MFCreateSample()?;
                s.AddBuffer(&buf)?;
                s.SetSampleTime(MFGetSystemTime())?;
                s.SetSampleDuration(10_000_000 / FPS as i64)?;
                Ok(s)
            };
            if let Ok(sample) = make() {
                let _ = queue.QueueEventParamUnk(MEMediaSample.0 as u32, &GUID::zeroed(), S_OK, &sample);
            }
        }
    }
}

impl IMFMediaEventGenerator_Impl for Stream_Impl {
    fn GetEvent(&self, flags: MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS) -> Result<IMFMediaEvent> {
        unsafe { self.queue.GetEvent(flags.0 as u32) }
    }
    fn BeginGetEvent(&self, cb: Ref<IMFAsyncCallback>, state: Ref<IUnknown>) -> Result<()> {
        unsafe { self.queue.BeginGetEvent(cb.as_ref(), state.as_ref()) }
    }
    fn EndGetEvent(&self, res: Ref<IMFAsyncResult>) -> Result<IMFMediaEvent> {
        unsafe { self.queue.EndGetEvent(res.as_ref()) }
    }
    fn QueueEvent(&self, met: u32, ext: *const GUID, hr: HRESULT, val: *const windows::Win32::System::Com::StructuredStorage::PROPVARIANT) -> Result<()> {
        unsafe { self.queue.QueueEventParamVar(met, ext, hr, val) }
    }
}

impl IMFMediaStream_Impl for Stream_Impl {
    fn GetMediaSource(&self) -> Result<IMFMediaSource> {
        self.source.lock().unwrap().clone().ok_or_else(|| Error::from(MF_E_SHUTDOWN))
    }
    fn GetStreamDescriptor(&self) -> Result<IMFStreamDescriptor> {
        Ok(self.desc.clone())
    }
    fn RequestSample(&self, _token: Ref<IUnknown>) -> Result<()> {
        let mut g = self.st.0.lock().unwrap();
        if g.shutdown {
            return Err(MF_E_SHUTDOWN.into());
        }
        g.pending = (g.pending + 1).min(4);
        drop(g);
        self.st.1.notify_all();
        Ok(())
    }
}

impl IMFMediaStream2_Impl for Stream_Impl {
    fn SetStreamState(&self, value: MF_STREAM_STATE) -> Result<()> {
        match value {
            MF_STREAM_STATE_RUNNING => self.start(),
            MF_STREAM_STATE_STOPPED | MF_STREAM_STATE_PAUSED => self.stop(),
            _ => {}
        }
        Ok(())
    }
    fn GetStreamState(&self) -> Result<MF_STREAM_STATE> {
        Ok(if self.st.0.lock().unwrap().started { MF_STREAM_STATE_RUNNING } else { MF_STREAM_STATE_STOPPED })
    }
}

// ---------------------------------------------------------------- kaynak

#[implement(IMFMediaSourceEx, IMFGetService, IKsControl)]
struct Source {
    queue: IMFMediaEventQueue,
    pd: IMFPresentationDescriptor,
    stream: ComObject<Stream>,
    attrs: IMFAttributes,
    stream_attrs: IMFAttributes,
    state: Mutex<(bool /*started*/, bool /*shutdown*/)>,
}

unsafe fn video_type(w: u32, h: u32) -> Result<IMFMediaType> {
    unsafe {
        let t = MFCreateMediaType()?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        t.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        t.SetUINT64(&MF_MT_FRAME_SIZE, pack(w, h))?;
        t.SetUINT64(&MF_MT_FRAME_RATE, pack(FPS, 1))?;
        t.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))?;
        t.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        t.SetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT, 1)?;
        t.SetUINT32(&MF_MT_FIXED_SIZE_SAMPLES, 1)?;
        t.SetUINT32(&MF_MT_DEFAULT_STRIDE, w)?;
        t.SetUINT32(&MF_MT_SAMPLE_SIZE, w * h * 3 / 2)?;
        t.SetUINT32(&MF_MT_AVG_BITRATE, w * h * 12 / 10 * FPS / 10)?;
        Ok(t)
    }
}

impl Source {
    fn create(shm: Option<Arc<Shm>>) -> Result<ComObject<Source>> {
        unsafe {
            let mut types = Vec::new();
            for (w, h) in MODES {
                types.push(video_type(w, h)?);
            }
            let opt: Vec<Option<IMFMediaType>> = types.iter().cloned().map(Some).collect();
            let sd = MFCreateStreamDescriptor(0, &opt)?;
            sd.GetMediaTypeHandler()?.SetCurrentMediaType(&types[0])?;
            // Frame Server'ın akışı sınıflandırması için gerekli öznitelikler
            sd.SetGUID(&MF_DEVICESTREAM_STREAM_CATEGORY, &PINNAME_VIDEO_CAPTURE)?;
            sd.SetUINT32(&MF_DEVICESTREAM_STREAM_ID, 0)?;
            sd.SetUINT32(&MF_DEVICESTREAM_FRAMESERVER_SHARED, 1)?;
            sd.SetUINT32(&MF_DEVICESTREAM_ATTRIBUTE_FRAMESOURCE_TYPES, MFFrameSourceTypes_Color.0 as u32)?;
            let pd = MFCreatePresentationDescriptor(Some(&[Some(sd.clone())]))?;
            pd.SelectStream(0)?;

            let attrs = {
                let mut a = None;
                MFCreateAttributes(&mut a, 2)?;
                a.ok_or_else(Error::empty)?
            };
            let stream_attrs = {
                let mut a = None;
                MFCreateAttributes(&mut a, 4)?;
                let a: IMFAttributes = a.ok_or_else(Error::empty)?;
                a.SetGUID(&MF_DEVICESTREAM_STREAM_CATEGORY, &PINNAME_VIDEO_CAPTURE)?;
                a.SetUINT32(&MF_DEVICESTREAM_STREAM_ID, 0)?;
                a.SetUINT32(&MF_DEVICESTREAM_FRAMESERVER_SHARED, 1)?;
                a.SetUINT32(&MF_DEVICESTREAM_ATTRIBUTE_FRAMESOURCE_TYPES, MFFrameSourceTypes_Color.0 as u32)?;
                a
            };
            let stream = ComObject::new(Stream::new(sd, shm)?);
            let src = ComObject::new(Source {
                queue: MFCreateEventQueue()?,
                pd,
                stream: stream.clone(),
                attrs,
                stream_attrs,
                state: Mutex::new((false, false)),
            });
            let ex: IMFMediaSourceEx = src.to_interface();
            let as_source: IMFMediaSource = ex.cast()?;
            *stream.source.lock().unwrap() = Some(as_source);
            Ok(src)
        }
    }
}

impl IMFMediaEventGenerator_Impl for Source_Impl {
    fn GetEvent(&self, flags: MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS) -> Result<IMFMediaEvent> {
        unsafe { self.queue.GetEvent(flags.0 as u32) }
    }
    fn BeginGetEvent(&self, cb: Ref<IMFAsyncCallback>, state: Ref<IUnknown>) -> Result<()> {
        unsafe { self.queue.BeginGetEvent(cb.as_ref(), state.as_ref()) }
    }
    fn EndGetEvent(&self, res: Ref<IMFAsyncResult>) -> Result<IMFMediaEvent> {
        unsafe { self.queue.EndGetEvent(res.as_ref()) }
    }
    fn QueueEvent(&self, met: u32, ext: *const GUID, hr: HRESULT, val: *const windows::Win32::System::Com::StructuredStorage::PROPVARIANT) -> Result<()> {
        unsafe { self.queue.QueueEventParamVar(met, ext, hr, val) }
    }
}

impl IMFMediaSource_Impl for Source_Impl {
    fn GetCharacteristics(&self) -> Result<u32> {
        Ok(MFMEDIASOURCE_IS_LIVE.0 as u32)
    }
    fn CreatePresentationDescriptor(&self) -> Result<IMFPresentationDescriptor> {
        if self.state.lock().unwrap().1 {
            return Err(MF_E_SHUTDOWN.into());
        }
        unsafe { self.pd.Clone() }
    }
    fn Start(
        &self,
        _pd: Ref<IMFPresentationDescriptor>,
        _time_format: *const GUID,
        _start: *const windows::Win32::System::Com::StructuredStorage::PROPVARIANT,
    ) -> Result<()> {
        {
            let mut s = self.state.lock().unwrap();
            if s.1 {
                return Err(MF_E_SHUTDOWN.into());
            }
            s.0 = true;
        }
        unsafe {
            let s2: IMFMediaStream2 = self.stream.to_interface();
            let stream_if: IMFMediaStream = s2.cast()?;
            self.queue.QueueEventParamUnk(MENewStream.0 as u32, &GUID::zeroed(), S_OK, &stream_if)?;
            self.stream.start();
            self.queue.QueueEventParamVar(MESourceStarted.0 as u32, &GUID::zeroed(), S_OK, std::ptr::null())?;
        }
        Ok(())
    }
    fn Stop(&self) -> Result<()> {
        self.state.lock().unwrap().0 = false;
        self.stream.stop();
        unsafe { self.queue.QueueEventParamVar(MESourceStopped.0 as u32, &GUID::zeroed(), S_OK, std::ptr::null()) }
    }
    fn Pause(&self) -> Result<()> {
        Err(MF_E_INVALID_STATE_TRANSITION.into())
    }
    fn Shutdown(&self) -> Result<()> {
        {
            let mut s = self.state.lock().unwrap();
            if s.1 {
                return Ok(());
            }
            s.1 = true;
        }
        self.stream.shutdown();
        unsafe { self.queue.Shutdown() }
    }
}

impl IMFMediaSourceEx_Impl for Source_Impl {
    fn GetSourceAttributes(&self) -> Result<IMFAttributes> {
        Ok(self.attrs.clone())
    }
    fn GetStreamAttributes(&self, _id: u32) -> Result<IMFAttributes> {
        Ok(self.stream_attrs.clone())
    }
    fn SetD3DManager(&self, _m: Ref<IUnknown>) -> Result<()> {
        Ok(()) // CPU örnekleri üretiyoruz; D3D yöneticisi kullanılmaz
    }
}

impl IMFGetService_Impl for Source_Impl {
    fn GetService(&self, _service: *const GUID, _riid: *const GUID, _ppv: *mut *mut c_void) -> Result<()> {
        Err(MF_E_UNSUPPORTED_SERVICE.into())
    }
}

impl IKsControl_Impl for Source_Impl {
    fn KsProperty(&self, _p: *const KSIDENTIFIER, _pl: u32, _data: *mut c_void, _dl: u32, _ret: *mut u32) -> Result<()> {
        Err(HRESULT::from_win32(ERROR_SET_NOT_FOUND.0).into())
    }
    fn KsMethod(&self, _m: *const KSIDENTIFIER, _ml: u32, _data: *mut c_void, _dl: u32, _ret: *mut u32) -> Result<()> {
        Err(HRESULT::from_win32(ERROR_SET_NOT_FOUND.0).into())
    }
    fn KsEvent(&self, _e: *const KSIDENTIFIER, _el: u32, _data: *mut c_void, _dl: u32, _ret: *mut u32) -> Result<()> {
        Err(HRESULT::from_win32(ERROR_SET_NOT_FOUND.0).into())
    }
}

// ---------------------------------------------------------------- etkinleştirici (IMFActivate)

#[implement(IMFActivate)]
struct Activate {
    attrs: IMFAttributes,
    source: Mutex<Option<ComObject<Source>>>,
    shm: Mutex<Option<Arc<Shm>>>,
}

impl Activate {
    fn new() -> Result<Self> {
        let mut a = None;
        unsafe { MFCreateAttributes(&mut a, 4)? };
        Ok(Self { attrs: a.ok_or_else(Error::empty)?, source: Mutex::new(None), shm: Mutex::new(None) })
    }
}

impl IMFActivate_Impl for Activate_Impl {
    fn ActivateObject(&self, riid: *const GUID, ppv: *mut *mut c_void) -> Result<()> {
        let shm = {
            let mut g = self.shm.lock().unwrap();
            if g.is_none() {
                *g = open_shm().map(Arc::new);
            }
            g.clone()
        };
        let src = Source::create(shm)?;
        let unk: IUnknown = src.to_interface();
        *self.source.lock().unwrap() = Some(src);
        unsafe { unk.query(riid, ppv).ok() }
    }
    fn ShutdownObject(&self) -> Result<()> {
        if let Some(s) = self.source.lock().unwrap().take() {
            let ex: IMFMediaSourceEx = s.to_interface();
            let src: IMFMediaSource = ex.cast()?;
            unsafe { src.Shutdown()? };
        }
        Ok(())
    }
    fn DetachObject(&self) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
}

// IMFAttributes: iç öznitelik deposuna devret
impl IMFAttributes_Impl for Activate_Impl {
    fn GetItem(&self, k: *const GUID, v: *mut windows::Win32::System::Com::StructuredStorage::PROPVARIANT) -> Result<()> {
        unsafe { self.attrs.GetItem(k, Some(v)) }
    }
    fn GetItemType(&self, k: *const GUID) -> Result<MF_ATTRIBUTE_TYPE> {
        unsafe { self.attrs.GetItemType(k) }
    }
    fn CompareItem(&self, k: *const GUID, v: *const windows::Win32::System::Com::StructuredStorage::PROPVARIANT) -> Result<BOOL> {
        unsafe { self.attrs.CompareItem(k, v) }
    }
    fn Compare(&self, theirs: Ref<IMFAttributes>, t: MF_ATTRIBUTES_MATCH_TYPE) -> Result<BOOL> {
        unsafe { self.attrs.Compare(theirs.as_ref(), t) }
    }
    fn GetUINT32(&self, k: *const GUID) -> Result<u32> {
        unsafe { self.attrs.GetUINT32(k) }
    }
    fn GetUINT64(&self, k: *const GUID) -> Result<u64> {
        unsafe { self.attrs.GetUINT64(k) }
    }
    fn GetDouble(&self, k: *const GUID) -> Result<f64> {
        unsafe { self.attrs.GetDouble(k) }
    }
    fn GetGUID(&self, k: *const GUID) -> Result<GUID> {
        unsafe { self.attrs.GetGUID(k) }
    }
    fn GetStringLength(&self, k: *const GUID) -> Result<u32> {
        unsafe { self.attrs.GetStringLength(k) }
    }
    fn GetString(&self, k: *const GUID, buf: PWSTR, cch: u32, out: *mut u32) -> Result<()> {
        unsafe { self.attrs.GetString(k, std::slice::from_raw_parts_mut(buf.0, cch as usize), Some(out)) }
    }
    fn GetAllocatedString(&self, k: *const GUID, pp: *mut PWSTR, cch: *mut u32) -> Result<()> {
        unsafe { self.attrs.GetAllocatedString(k, pp, cch) }
    }
    fn GetBlobSize(&self, k: *const GUID) -> Result<u32> {
        unsafe { self.attrs.GetBlobSize(k) }
    }
    fn GetBlob(&self, k: *const GUID, buf: *mut u8, cb: u32, out: *mut u32) -> Result<()> {
        unsafe { self.attrs.GetBlob(k, std::slice::from_raw_parts_mut(buf, cb as usize), Some(out)) }
    }
    fn GetAllocatedBlob(&self, k: *const GUID, pp: *mut *mut u8, cb: *mut u32) -> Result<()> {
        unsafe { self.attrs.GetAllocatedBlob(k, pp, cb) }
    }
    fn GetUnknown(&self, k: *const GUID, riid: *const GUID, ppv: *mut *mut c_void) -> Result<()> {
        unsafe {
            let u: IUnknown = self.attrs.GetUnknown(k)?;
            u.query(riid, ppv).ok()
        }
    }
    fn SetItem(&self, k: *const GUID, v: *const windows::Win32::System::Com::StructuredStorage::PROPVARIANT) -> Result<()> {
        unsafe { self.attrs.SetItem(k, v) }
    }
    fn DeleteItem(&self, k: *const GUID) -> Result<()> {
        unsafe { self.attrs.DeleteItem(k) }
    }
    fn DeleteAllItems(&self) -> Result<()> {
        unsafe { self.attrs.DeleteAllItems() }
    }
    fn SetUINT32(&self, k: *const GUID, v: u32) -> Result<()> {
        unsafe { self.attrs.SetUINT32(k, v) }
    }
    fn SetUINT64(&self, k: *const GUID, v: u64) -> Result<()> {
        unsafe { self.attrs.SetUINT64(k, v) }
    }
    fn SetDouble(&self, k: *const GUID, v: f64) -> Result<()> {
        unsafe { self.attrs.SetDouble(k, v) }
    }
    fn SetGUID(&self, k: *const GUID, v: *const GUID) -> Result<()> {
        unsafe { self.attrs.SetGUID(k, v) }
    }
    fn SetString(&self, k: *const GUID, v: &PCWSTR) -> Result<()> {
        unsafe { self.attrs.SetString(k, *v) }
    }
    fn SetBlob(&self, k: *const GUID, buf: *const u8, cb: u32) -> Result<()> {
        unsafe { self.attrs.SetBlob(k, std::slice::from_raw_parts(buf, cb as usize)) }
    }
    fn SetUnknown(&self, k: *const GUID, u: Ref<IUnknown>) -> Result<()> {
        unsafe { self.attrs.SetUnknown(k, u.as_ref()) }
    }
    fn LockStore(&self) -> Result<()> {
        unsafe { self.attrs.LockStore() }
    }
    fn UnlockStore(&self) -> Result<()> {
        unsafe { self.attrs.UnlockStore() }
    }
    fn GetCount(&self) -> Result<u32> {
        unsafe { self.attrs.GetCount() }
    }
    fn GetItemByIndex(&self, i: u32, k: *mut GUID, v: *mut windows::Win32::System::Com::StructuredStorage::PROPVARIANT) -> Result<()> {
        unsafe { self.attrs.GetItemByIndex(i, k, Some(v)) }
    }
    fn CopyAllItems(&self, dest: Ref<IMFAttributes>) -> Result<()> {
        unsafe { self.attrs.CopyAllItems(dest.as_ref()) }
    }
}

// ---------------------------------------------------------------- COM giriş noktaları

#[implement(IClassFactory)]
struct Factory;

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(&self, outer: Ref<IUnknown>, riid: *const GUID, ppv: *mut *mut c_void) -> Result<()> {
        if !outer.is_null() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let act: IMFActivate = Activate::new()?.into();
        unsafe { act.query(riid, ppv).ok() }
    }
    fn LockServer(&self, _lock: BOOL) -> Result<()> {
        Ok(())
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(rclsid: *const GUID, riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT {
    unsafe {
        if rclsid.is_null() || *rclsid != CLSID_TBRIDGE_VCAM {
            return CLASS_E_CLASSNOTAVAILABLE;
        }
        let f: IClassFactory = Factory.into();
        f.query(riid, ppv)
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    S_FALSE // Frame Server kaynağı yaşarken DLL bellekte kalır (basit ve güvenli)
}
