use std::ffi::{CStr, CString};
use std::mem::MaybeUninit;
use std::ptr;

use anyhow::{Result, bail};
use ndi::internal::bindings::{
    NDIlib_audio_frame_v3_t, NDIlib_audio_frame_v3_t__bindgen_ty_1, NDIlib_find_create_t,
    NDIlib_find_create_v2, NDIlib_find_destroy, NDIlib_find_get_current_sources,
    NDIlib_find_instance_t, NDIlib_frame_type_e_NDIlib_frame_type_audio,
    NDIlib_frame_type_e_NDIlib_frame_type_error, NDIlib_frame_type_e_NDIlib_frame_type_metadata,
    NDIlib_frame_type_e_NDIlib_frame_type_video, NDIlib_metadata_frame_t, NDIlib_recv_bandwidth_e,
    NDIlib_recv_bandwidth_e_NDIlib_recv_bandwidth_audio_only,
    NDIlib_recv_bandwidth_e_NDIlib_recv_bandwidth_highest,
    NDIlib_recv_bandwidth_e_NDIlib_recv_bandwidth_lowest,
    NDIlib_recv_bandwidth_e_NDIlib_recv_bandwidth_metadata_only, NDIlib_recv_capture_v3,
    NDIlib_recv_color_format_e, NDIlib_recv_color_format_e_NDIlib_recv_color_format_BGRX_BGRA,
    NDIlib_recv_color_format_e_NDIlib_recv_color_format_RGBX_RGBA,
    NDIlib_recv_color_format_e_NDIlib_recv_color_format_UYVY_BGRA,
    NDIlib_recv_color_format_e_NDIlib_recv_color_format_UYVY_RGBA,
    NDIlib_recv_color_format_e_NDIlib_recv_color_format_best,
    NDIlib_recv_color_format_e_NDIlib_recv_color_format_fastest, NDIlib_recv_create_v3,
    NDIlib_recv_create_v3_t, NDIlib_recv_destroy, NDIlib_recv_free_audio_v3,
    NDIlib_recv_free_metadata, NDIlib_recv_free_video_v2, NDIlib_recv_instance_t,
    NDIlib_send_create, NDIlib_send_create_t, NDIlib_send_destroy, NDIlib_send_get_no_connections,
    NDIlib_send_instance_t, NDIlib_send_send_audio_v3, NDIlib_send_send_video_v2, NDIlib_source_t,
    NDIlib_source_t__bindgen_ty_1, NDIlib_video_frame_v2_t,
};

use crate::config::{Bandwidth, ColorFormat};

pub struct Finder {
    instance: NDIlib_find_instance_t,
}

unsafe impl Send for Finder {}

impl Finder {
    pub fn new() -> Result<Self> {
        let settings = NDIlib_find_create_t {
            show_local_sources: true,
            p_groups: ptr::null(),
            p_extra_ips: ptr::null(),
        };
        let instance = unsafe { NDIlib_find_create_v2(&settings) };
        if instance.is_null() {
            bail!("failed to create NDI finder");
        }
        Ok(Self { instance })
    }

    pub fn sources(&self) -> Vec<Source> {
        let mut count = 0u32;
        let pointer = unsafe { NDIlib_find_get_current_sources(self.instance, &mut count) };
        if pointer.is_null() || count == 0 {
            return Vec::new();
        }

        let mut sources = Vec::with_capacity(count as usize);
        for index in 0..count {
            let raw = unsafe { &*pointer.add(index as usize) };
            if let Some(source) = Source::from_raw(raw) {
                sources.push(source);
            }
        }
        sources
    }
}

impl Drop for Finder {
    fn drop(&mut self) {
        unsafe { NDIlib_find_destroy(self.instance) };
    }
}

#[derive(Debug, Clone)]
pub struct Source {
    pub name: String,
    address: Option<String>,
}

impl Source {
    fn from_raw(raw: &NDIlib_source_t) -> Option<Self> {
        let name = if raw.p_ndi_name.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(raw.p_ndi_name) }
                .to_string_lossy()
                .into_owned()
        };

        let address_pointer = unsafe { raw.__bindgen_anon_1.p_url_address };
        let address = if address_pointer.is_null() {
            None
        } else {
            Some(
                unsafe { CStr::from_ptr(address_pointer) }
                    .to_string_lossy()
                    .into_owned(),
            )
        };

        if name.is_empty() && address.is_none() {
            None
        } else {
            Some(Self { name, address })
        }
    }
}

pub struct Recv {
    instance: NDIlib_recv_instance_t,
}

unsafe impl Send for Recv {}

impl Recv {
    pub fn new(source: &Source, color: ColorFormat, bandwidth: Bandwidth) -> Result<Self> {
        let name = CString::new(source.name.clone())?;
        let address = source
            .address
            .as_ref()
            .map(|value| CString::new(value.clone()))
            .transpose()?;
        let source_raw = build_source(name.as_ptr(), address.as_deref());

        let receiver_name = CString::new("otome")?;
        let settings = NDIlib_recv_create_v3_t {
            source_to_connect_to: source_raw,
            color_format: map_color_format(color),
            bandwidth: map_bandwidth(bandwidth),
            allow_video_fields: true,
            p_ndi_recv_name: receiver_name.as_ptr(),
        };

        let instance = unsafe { NDIlib_recv_create_v3(&settings) };
        if instance.is_null() {
            bail!("failed to create NDI receiver");
        }
        Ok(Self { instance })
    }

    pub fn capture(&self, timeout_ms: u32) -> Captured {
        let mut video = MaybeUninit::<NDIlib_video_frame_v2_t>::uninit();
        let mut audio = MaybeUninit::<NDIlib_audio_frame_v3_t>::uninit();
        let mut metadata = MaybeUninit::<NDIlib_metadata_frame_t>::uninit();

        let frame_type = unsafe {
            NDIlib_recv_capture_v3(
                self.instance,
                video.as_mut_ptr(),
                audio.as_mut_ptr(),
                metadata.as_mut_ptr(),
                timeout_ms,
            )
        };

        #[allow(non_upper_case_globals)]
        match frame_type {
            NDIlib_frame_type_e_NDIlib_frame_type_video => Captured::Video(VideoFrame {
                instance: self.instance,
                raw: unsafe { video.assume_init() },
            }),
            NDIlib_frame_type_e_NDIlib_frame_type_audio => Captured::Audio(AudioFrame {
                instance: self.instance,
                raw: unsafe { audio.assume_init() },
            }),
            NDIlib_frame_type_e_NDIlib_frame_type_metadata => {
                let raw = unsafe { metadata.assume_init() };
                unsafe { NDIlib_recv_free_metadata(self.instance, &raw) };
                Captured::None
            }
            NDIlib_frame_type_e_NDIlib_frame_type_error => Captured::Error,
            _ => Captured::None,
        }
    }
}

impl Drop for Recv {
    fn drop(&mut self) {
        unsafe { NDIlib_recv_destroy(self.instance) };
    }
}

pub enum Captured {
    Video(VideoFrame),
    Audio(AudioFrame),
    None,
    Error,
}

pub struct VideoFrame {
    instance: NDIlib_recv_instance_t,
    raw: NDIlib_video_frame_v2_t,
}

impl VideoFrame {
    pub fn raw(&self) -> &NDIlib_video_frame_v2_t {
        &self.raw
    }
}

impl Drop for VideoFrame {
    fn drop(&mut self) {
        unsafe { NDIlib_recv_free_video_v2(self.instance, &self.raw) };
    }
}

pub struct AudioFrame {
    instance: NDIlib_recv_instance_t,
    raw: NDIlib_audio_frame_v3_t,
}

impl AudioFrame {
    pub fn raw(&self) -> &NDIlib_audio_frame_v3_t {
        &self.raw
    }
}

impl Drop for AudioFrame {
    fn drop(&mut self) {
        unsafe { NDIlib_recv_free_audio_v3(self.instance, &self.raw) };
    }
}

pub struct Sender {
    instance: NDIlib_send_instance_t,
}

unsafe impl Send for Sender {}

impl Sender {
    pub fn new(
        name: &str,
        groups: Option<&str>,
        clock_video: bool,
        clock_audio: bool,
    ) -> Result<Self> {
        let name = CString::new(name)?;
        let groups = groups
            .filter(|value| !value.trim().is_empty())
            .map(CString::new)
            .transpose()?;

        let settings = NDIlib_send_create_t {
            p_ndi_name: name.as_ptr(),
            p_groups: groups.as_ref().map_or(ptr::null(), |value| value.as_ptr()),
            clock_video,
            clock_audio,
        };

        let instance = unsafe { NDIlib_send_create(&settings) };
        if instance.is_null() {
            bail!("failed to create NDI sender");
        }
        Ok(Self { instance })
    }

    pub fn send_video(&self, frame: &NDIlib_video_frame_v2_t) {
        unsafe { NDIlib_send_send_video_v2(self.instance, frame) };
    }

    pub fn send_audio(&self, frame: &NDIlib_audio_frame_v3_t) {
        unsafe { NDIlib_send_send_audio_v3(self.instance, frame) };
    }

    pub fn send_silence(&self, source: &NDIlib_audio_frame_v3_t) {
        let channels = source.no_channels;
        let samples = source.no_samples;
        if channels <= 0 || samples <= 0 {
            return;
        }

        let Some(sample_count) = (channels as usize).checked_mul(samples as usize) else {
            return;
        };
        let Some(stride) = (samples as usize).checked_mul(std::mem::size_of::<f32>()) else {
            return;
        };
        if stride > i32::MAX as usize {
            return;
        }

        let mut buffer = vec![0.0f32; sample_count];
        let frame = NDIlib_audio_frame_v3_t {
            sample_rate: source.sample_rate,
            no_channels: channels,
            no_samples: samples,
            timecode: source.timecode,
            FourCC: source.FourCC,
            p_data: buffer.as_mut_ptr().cast(),
            __bindgen_anon_1: NDIlib_audio_frame_v3_t__bindgen_ty_1 {
                channel_stride_in_bytes: stride as i32,
            },
            p_metadata: ptr::null(),
            timestamp: source.timestamp,
        };
        self.send_audio(&frame);
    }

    pub fn connections(&self) -> u32 {
        unsafe { NDIlib_send_get_no_connections(self.instance, 0).max(0) as u32 }
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        unsafe { NDIlib_send_destroy(self.instance) };
    }
}

fn build_source(name: *const std::os::raw::c_char, address: Option<&CStr>) -> NDIlib_source_t {
    let union = match address {
        Some(value) if value.to_string_lossy().contains("://") => NDIlib_source_t__bindgen_ty_1 {
            p_url_address: value.as_ptr(),
        },
        Some(value) => NDIlib_source_t__bindgen_ty_1 {
            p_ip_address: value.as_ptr(),
        },
        None => NDIlib_source_t__bindgen_ty_1 {
            p_ip_address: ptr::null(),
        },
    };

    NDIlib_source_t {
        p_ndi_name: name,
        __bindgen_anon_1: union,
    }
}

fn map_color_format(format: ColorFormat) -> NDIlib_recv_color_format_e {
    match format {
        ColorFormat::BgrxBgra => NDIlib_recv_color_format_e_NDIlib_recv_color_format_BGRX_BGRA,
        ColorFormat::UyvyBgra => NDIlib_recv_color_format_e_NDIlib_recv_color_format_UYVY_BGRA,
        ColorFormat::RgbxRgba => NDIlib_recv_color_format_e_NDIlib_recv_color_format_RGBX_RGBA,
        ColorFormat::UyvyRgba => NDIlib_recv_color_format_e_NDIlib_recv_color_format_UYVY_RGBA,
        ColorFormat::Fastest => NDIlib_recv_color_format_e_NDIlib_recv_color_format_fastest,
        ColorFormat::Best => NDIlib_recv_color_format_e_NDIlib_recv_color_format_best,
    }
}

fn map_bandwidth(bandwidth: Bandwidth) -> NDIlib_recv_bandwidth_e {
    match bandwidth {
        Bandwidth::Highest => NDIlib_recv_bandwidth_e_NDIlib_recv_bandwidth_highest,
        Bandwidth::Lowest => NDIlib_recv_bandwidth_e_NDIlib_recv_bandwidth_lowest,
        Bandwidth::AudioOnly => NDIlib_recv_bandwidth_e_NDIlib_recv_bandwidth_audio_only,
        Bandwidth::MetadataOnly => NDIlib_recv_bandwidth_e_NDIlib_recv_bandwidth_metadata_only,
    }
}
