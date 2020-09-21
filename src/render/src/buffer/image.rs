use crate::core::Texture2DArray;
pub trait Image {
    fn tex_sub_image_3d(&self,
        // The texture array
        textures: &Texture2DArray,
        // An offset to write the image in the texture array
        offset: &Vector3<i32>
    );

    // The size of the image
    fn get_size(&self) -> &Vector2<i32>;

    //fn get_cutoff_values(&self) -> Option<(f32, f32)>;
}

#[derive(Debug)]
pub struct TileArrayBuffer<T: ArrayBuffer> {
    buf: T,
    size: Vector2<i32>
}

impl<T> TileArrayBuffer<T>
where T: ArrayBuffer {
    pub fn new(buf: &[T::Item], width: i32, num_channels: i32) -> Self {
        let size_buf = width * width * num_channels;
        assert_eq!(size_buf, buf.len() as i32);
        let buf = T::new(buf);
        let size = Vector2::new(width, width);
        Self { buf, size }
    }

    pub(super) fn blank(width: i32, num_channels: i32, blank_value: T::Item) -> Self {
        let size_buf = width * width * num_channels;
        let buf = T::empty(size_buf as u32, blank_value);

        let size = Vector2::new(width, width);

        Self { buf, size }
    }

    // Compute the 1- and 99- percentile of the tile pixel values
    pub(super) fn get_cutoff_values(&self) -> (T::Item, T::Item) {
        let mut sorted_values: Vec<T::Item> = self.buf.to_vec();
        sorted_values.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());

        let len = sorted_values.len() as f32;
        let idx1 = (0.01 * len) as usize;
        let idx2 = (0.99 * len) as usize;
    
        let (v1, v2) = (sorted_values[idx1], sorted_values[idx2]);
        //crate::log(&format!("cutoff: {:?} {:?}", v1, v2));
        (v1, v2)
    }
}

pub trait ArrayBuffer: AsRef<js_sys::Object> {
    type Item: std::cmp::PartialOrd + Clone + Copy + std::fmt::Debug;
    fn new(buf: &[Self::Item]) -> Self;
    fn empty(size: u32, blank_value: Self::Item) -> Self;
    fn to_vec(&self) -> Vec<Self::Item>;
}
#[derive(Debug)]
pub struct ArrayU8(js_sys::Uint8Array);
impl AsRef<js_sys::Object> for ArrayU8 {
    fn as_ref(&self) -> &js_sys::Object { self.0.as_ref() }
}

impl ArrayBuffer for ArrayU8 {
    type Item = u8;

    fn new(buf: &[Self::Item]) -> Self {
        ArrayU8(buf.into())
    }

    fn empty(size: u32, blank_value: Self::Item) -> Self {
        let uint8_arr = js_sys::Uint8Array::new_with_length(size).fill(blank_value, 0, size);
        let array = ArrayU8(uint8_arr);
        array
    }

    fn to_vec(&self) -> Vec<Self::Item> {
        self.0.to_vec()
    }
}
#[derive(Debug)]
pub struct ArrayI16(js_sys::Int16Array);
impl AsRef<js_sys::Object> for ArrayI16 {
    fn as_ref(&self) -> &js_sys::Object { self.0.as_ref() }
}

impl ArrayBuffer for ArrayI16 {
    type Item = i16;
    fn new(buf: &[Self::Item]) -> Self {
        ArrayI16(buf.into())
    }

    fn empty(size: u32, blank_value: Self::Item) -> Self {
        let int16_arr = js_sys::Int16Array::new_with_length(size).fill(blank_value, 0, size);
        let array = ArrayI16(int16_arr);
        array
    }

    fn to_vec(&self) -> Vec<Self::Item> {
        self.0.to_vec()
    }
}
#[derive(Debug)]
pub struct ArrayI32(js_sys::Int32Array);
impl AsRef<js_sys::Object> for ArrayI32 {
    fn as_ref(&self) -> &js_sys::Object { self.0.as_ref() }
}
impl ArrayBuffer for ArrayI32 {
    type Item = i32;

    fn new(buf: &[Self::Item]) -> Self {
        ArrayI32(buf.into())
    }

    fn empty(size: u32, blank_value: Self::Item) -> Self {
        let int32_arr = js_sys::Int32Array::new_with_length(size).fill(blank_value, 0, size);
        let array = ArrayI32(int32_arr);
        array
    }

    fn to_vec(&self) -> Vec<Self::Item> {
        self.0.to_vec()
    }
}
#[derive(Debug)]
pub struct ArrayF32(js_sys::Float32Array);
impl AsRef<js_sys::Object> for ArrayF32 {
    fn as_ref(&self) -> &js_sys::Object { self.0.as_ref() }
}

impl ArrayBuffer for ArrayF32 {
    type Item = f32;

    fn new(buf: &[Self::Item]) -> Self {
        ArrayF32(buf.into())
    }
    fn empty(size: u32, blank_value: Self::Item) -> Self {
        let f32_arr = js_sys::Float32Array::new_with_length(size).fill(blank_value, 0, size);
        let array = ArrayF32(f32_arr);
        array
    }

    fn to_vec(&self) -> Vec<Self::Item> {
        self.0.to_vec()
    }
}

use super::TileArrayBufferImage;
impl Image for TileArrayBufferImage {
    fn tex_sub_image_3d(&self,
        // The texture array
        textures: &Texture2DArray,
        // An offset to write the image in the texture array
        offset: &Vector3<i32>
    ) {
        match &self {
            TileArrayBufferImage::U8(b) => textures.bind()
                .tex_sub_image_3d_with_opt_array_buffer_view(
                    offset.x,
                    offset.y,
                    offset.z,
                    b.size.x,
                    b.size.y,
                    Some(b.buf.as_ref()),
                ),
            TileArrayBufferImage::I16(b) => textures.bind()
                .tex_sub_image_3d_with_opt_array_buffer_view(
                    offset.x,
                    offset.y,
                    offset.z,
                    b.size.x,
                    b.size.y,
                    Some(b.buf.as_ref()),
                ),
            TileArrayBufferImage::I32(b) => textures.bind()
                .tex_sub_image_3d_with_opt_array_buffer_view(
                    offset.x,
                    offset.y,
                    offset.z,
                    b.size.x,
                    b.size.y,
                    Some(b.buf.as_ref()),
                ),
            TileArrayBufferImage::F32(b) => textures.bind()
                .tex_sub_image_3d_with_opt_array_buffer_view(
                    offset.x,
                    offset.y,
                    offset.z,
                    b.size.x,
                    b.size.y,
                    Some(b.buf.as_ref()),
                ),
            _ => unimplemented!()
        }
    }

    // The size of the image
    fn get_size(&self) -> &Vector2<i32> {
        match &self {
            TileArrayBufferImage::U8(b) => &b.size,
            TileArrayBufferImage::I16(b) => &b.size,
            TileArrayBufferImage::I32(b) => &b.size,
            TileArrayBufferImage::F32(b) => &b.size,
            _ => unimplemented!()
        }
    }

    fn get_cutoff_values(&self) -> Option<(f32, f32)> {
        match &self {
            TileArrayBufferImage::U8(b) => {
                let values = b.get_cutoff_values();
                Some((values.0 as f32, values.1 as f32))
            },
            TileArrayBufferImage::I16(b) => {
                let values = b.get_cutoff_values();
                Some((values.0 as f32, values.1 as f32))
            },
            TileArrayBufferImage::I32(b) => {
                let values = b.get_cutoff_values();
                Some((values.0 as f32, values.1 as f32))
            },
            TileArrayBufferImage::F32(b) => {
                let values = b.get_cutoff_values();
                Some(values)
            },
            _ => unimplemented!()
        }
    }
}

impl Image for Rc<TileArrayBufferImage> {
    fn tex_sub_image_3d(&self,
        // The texture array
        textures: &Texture2DArray,
        // An offset to write the image in the texture array
        offset: &Vector3<i32>
    ) {
        let tile: &TileArrayBufferImage = &**self;
        match &tile {
            &TileArrayBufferImage::U8(b) => textures.bind()
                .tex_sub_image_3d_with_opt_array_buffer_view(
                    offset.x,
                    offset.y,
                    offset.z,
                    b.size.x,
                    b.size.y,
                    Some(b.buf.as_ref()),
                ),
            &TileArrayBufferImage::I16(b) => textures.bind()
                .tex_sub_image_3d_with_opt_array_buffer_view(
                    offset.x,
                    offset.y,
                    offset.z,
                    b.size.x,
                    b.size.y,
                    Some(b.buf.as_ref()),
                ),
            &TileArrayBufferImage::I32(b) => textures.bind()
                .tex_sub_image_3d_with_opt_array_buffer_view(
                    offset.x,
                    offset.y,
                    offset.z,
                    b.size.x,
                    b.size.y,
                    Some(b.buf.as_ref()),
                ),
            &TileArrayBufferImage::F32(b) => textures.bind()
                .tex_sub_image_3d_with_opt_array_buffer_view(
                    offset.x,
                    offset.y,
                    offset.z,
                    b.size.x,
                    b.size.y,
                    Some(b.buf.as_ref()),
                ),
            _ => unimplemented!()
        }
    }

    // The size of the image
    fn get_size(&self) -> &Vector2<i32> {
        let tile: &TileArrayBufferImage = &**self;
        match &tile {
            &TileArrayBufferImage::U8(b) => &b.size,
            &TileArrayBufferImage::I16(b) => &b.size,
            &TileArrayBufferImage::I32(b) => &b.size,
            &TileArrayBufferImage::F32(b) => &b.size,
            _ => unimplemented!()
        }
    }

    fn get_cutoff_values(&self) -> Option<(f32, f32)> {
        let tile: &TileArrayBufferImage = &**self;
        match &tile {
            &TileArrayBufferImage::U8(b) => {
                let values = b.get_cutoff_values();
                Some((values.0 as f32, values.1 as f32))
            },
            &TileArrayBufferImage::I16(b) => {
                let values = b.get_cutoff_values();
                Some((values.0 as f32, values.1 as f32))
            },
            &TileArrayBufferImage::I32(b) => {
                let values = b.get_cutoff_values();
                Some((values.0 as f32, values.1 as f32))
            },
            &TileArrayBufferImage::F32(b) => {
                let values = b.get_cutoff_values();
                Some(values)
            },
            _ => unimplemented!()
        }
    }
}

use crate::{
    healpix_cell::HEALPixCell,
    HiPSConfig,
    utils,
    time::Time
};
use std::cell::Cell;
use std::rc::Rc;
use js_sys::Function;

enum ImageRequestType {
    FITS(FITSImageRequest),
    Compressed(CompressedImageRequest),
}

impl ImageRequestType {
    fn send(&self, success: Option<&Function>, fail: Option<&Function>, url: &str) {
        match self {
            ImageRequestType::FITS(req) => req.send(success, fail, url),
            ImageRequestType::Compressed(req) => req.send(success, fail, url)
        }
    }

    fn image(&mut self, config: &mut HiPSConfig) -> RetrievedImageType {
        match self {
            ImageRequestType::FITS(req) => RetrievedImageType::FITSImage(req.image(config)),
            ImageRequestType::Compressed(req) => RetrievedImageType::CompressedImage(req.image(config))
        }
    }
}

impl Image for RetrievedImageType {
    fn tex_sub_image_3d(&self,
        // The texture array
        textures: &Texture2DArray,
        // An offset to write the image in the texture array
        offset: &Vector3<i32>
    ) {
        match self {
            RetrievedImageType::CompressedImage(img) => img.tex_sub_image_3d(textures, offset),
            RetrievedImageType::FITSImage(img) => img.tex_sub_image_3d(textures, offset)
        }
    }

    // The size of the image
    fn get_size(&self) -> &Vector2<i32> {
        match self {
            RetrievedImageType::CompressedImage(img) => img.get_size(),
            RetrievedImageType::FITSImage(img) => img.get_size()
        }
    }

    /*fn get_cutoff_values(&self) -> std::option::Option<(f32, f32)> {
        None
    }*/
}

impl From<FITSImageRequest> for ImageRequestType {
    fn from(req: FITSImageRequest) -> Self {
        ImageRequestType::FITS(req)
    }
}
impl From<CompressedImageRequest> for ImageRequestType {
    fn from(req: CompressedImageRequest) -> Self {
        ImageRequestType::Compressed(req)
    }
}

pub trait ImageRequest {
    type RetrievedImageType: Image + 'static;

    fn new() -> Self;
    fn send(&self, success: Option<&Function>, fail: Option<&Function>, url: &str);
    fn image(&mut self, config: &mut HiPSConfig) -> Self::RetrievedImageType;
}

pub struct TileRequest {
    // Is none when it is available for downloading a new fits
    // or image tile
    req: ImageRequestType,
    time_request: Time,
    // Flag telling if the tile has been copied so that
    // the HtmlImageElement can be reused to download another tile
    ready: bool,
    resolved: Rc<Cell<ResolvedStatus>>,
    cell: HEALPixCell,
    closures: [Closure<dyn FnMut(&web_sys::Event,)>; 2],
}
#[derive(Clone, Copy)]
#[derive(PartialEq)]
pub enum ResolvedStatus {
    NotResolved,
    Missing,
    Found
}
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;

use super::ImageSurvey;
impl TileRequest {
    pub fn new() -> Self {
        // By default, all the requests are parametrized to load
        // compressed image requests
        let req = ImageRequestType::Compressed(CompressedImageRequest::new());

        // By default, we say the tile is available to be reused
        let resolved = Rc::new(Cell::new(ResolvedStatus::NotResolved));
        let cell = HEALPixCell(0, 13);
        let closures = [
            Closure::wrap(Box::new(|_events: &web_sys::Event| {}) as Box<dyn FnMut(&web_sys::Event,)>),
            Closure::wrap(Box::new(|_events: &web_sys::Event| {}) as Box<dyn FnMut(&web_sys::Event,)>)
        ];
        let ready = true;
        let time_request = Time::now();
        Self { req, resolved, ready, cell, closures, time_request }
    }

    pub fn send(&mut self, cell: &HEALPixCell, survey: &ImageSurvey) {
        assert!(self.is_ready());
        // Change the type of request if necessary in function 
        // of the image survey
        self.req = match (self.req, survey.get_downloaded_tiles_format()) {
            (ImageRequestType::FITS(_), FormatImageType::JPG) => {
                ImageRequestType::Compressed(CompressedImageRequest::new())
            },
            (ImageRequestType::FITS(_), FormatImageType::PNG) => {
                ImageRequestType::Compressed(CompressedImageRequest::new())
            },
            (ImageRequestType::Compressed(_), FormatImageType::FITS(_)) => {
                ImageRequestType::FITS(FITSImageRequest::new())
            },
            _ => self.req
        };

        self.cell = *cell;
        self.ready = false;

        let url = {
            let HEALPixCell(depth, idx) = cell;

            let dir_idx = (idx / 10000) * 10000;

            let survey_conf = survey.config();
            let url = format!("{}/Norder{}/Dir{}/Npix{}.{}",
                survey_conf.root_url.to_string(),
                depth.to_string(),
                dir_idx.to_string(),
                idx.to_string(),
                survey_conf.get_ext_file()
            );
    
            url
        };
        
        let success = {
            let resolved = self.resolved.clone();
            Closure::wrap(Box::new(move |_: &web_sys::Event| {
                resolved.set(ResolvedStatus::Found);
            }) as Box<dyn FnMut(&web_sys::Event,)>)
        };

        let fail = {
            let resolved = self.resolved.clone();
            Closure::wrap(Box::new(move |_: &web_sys::Event| {
                resolved.set(ResolvedStatus::Missing);
            }) as Box<dyn FnMut(&web_sys::Event,)>)
        };

        self.resolved.set(ResolvedStatus::NotResolved);

        self.req.send(
            Some(success.as_ref().unchecked_ref()),
            Some(fail.as_ref().unchecked_ref()),
            &url
        );

        self.closures = [success, fail];
        self.time_request = Time::now();
    }

    pub fn get_cell(&self) -> &HEALPixCell {
        &self.cell
    }

    pub fn get_time_request(&self) -> Time {
        self.time_request
    }

    pub fn is_resolved(&self) -> bool {
        let resolved = self.resolve_status();
        resolved == ResolvedStatus::Found || resolved == ResolvedStatus::Missing
    }

    pub fn is_ready(&self) -> bool {
        self.ready
    }

    pub fn set_ready(&mut self) {
        self.ready = true;
    }

    pub fn clear(&mut self) {
        self.req.send(None, None, "");
        self.ready = true;
        self.resolved.set(ResolvedStatus::NotResolved);
        self.closures = [
            Closure::wrap(Box::new(|_events: &web_sys::Event| {}) as Box<dyn FnMut(&web_sys::Event,)>),
            Closure::wrap(Box::new(|_events: &web_sys::Event| {}) as Box<dyn FnMut(&web_sys::Event,)>)
        ];
        self.cell = HEALPixCell(0, 13);
        self.time_request = Time::now();
    }

    pub fn resolve_status(&self) -> ResolvedStatus {
        self.resolved.get()
    }

    pub fn get_image(&mut self, config: &mut HiPSConfig) -> RetrievedImageType {
        assert!(self.is_resolved());
        self.req.image(config)
    }
}

enum RetrievedImageType {
    FITSImage(TileArrayBufferImage),
    CompressedImage(TileHTMLImage)
}

pub struct CompressedImageRequest {
    image: web_sys::HtmlImageElement,
}

impl ImageRequest for CompressedImageRequest {
    type RetrievedImageType = TileHTMLImage;

    fn new() -> Self {
        let image = web_sys::HtmlImageElement::new().unwrap();
        image.set_cross_origin(Some("")); 

        Self { image }
    }

    fn send(&self, success: Option<&Function>, fail: Option<&Function>, url: &str) {
        self.image.set_src(&url);
        self.image.set_onload(success);
        self.image.set_onerror(fail);
    }

    fn image(&mut self, config: &mut HiPSConfig) -> Self::RetrievedImageType {
        let width = self.image.width() as i32;
        let height = self.image.height() as i32;

        let size = Vector2::new(width, height);
        TileHTMLImage {
            size,
            image: self.image.clone()
        }
    }
}

use web_sys::XmlHttpRequest;
pub struct FITSImageRequest {
    image: XmlHttpRequest,
}
use web_sys::XmlHttpRequestResponseType;
use fitsreader::{Fits, DataType};
use fitsreader::{FITSHeaderKeyword, FITSKeywordValue};
impl ImageRequest for FITSImageRequest {
    type RetrievedImageType = TileArrayBufferImage;

    fn new() -> Self {
        let image = XmlHttpRequest::new().unwrap();
        image.set_response_type(XmlHttpRequestResponseType::Arraybuffer);

        Self { image }
    }

    fn send(&self, success: Option<&Function>, fail: Option<&Function>, url: &str) {
        self.image.open_with_async("GET", url, true);
        self.image.set_onload(success);
        self.image.set_onerror(fail);

        crate::log(&format!("url {:?}", url));
        self.image.send().unwrap();
    }

    fn image(&mut self, config: &mut HiPSConfig) -> Self::RetrievedImageType {
        // We know at this point the request is resolved
        let array_buf = js_sys::Uint8Array::new(
            self.image.response().unwrap().as_ref()
        );

        let bytes = &array_buf.to_vec();
        let Fits { data, header } = Fits::from_bytes_slice(bytes).unwrap();

        let format = &config.format();
        let width = config.get_tile_size();
        let num_channels = format.get_num_channels() as i32;

        let img = match data {
            DataType::U8(data) => {
                TileArrayBufferImage::U8(TileArrayBuffer::<ArrayU8>::new(&data.0, width, num_channels))
            },
            DataType::I16(data) => {
                TileArrayBufferImage::I16(TileArrayBuffer::<ArrayI16>::new(&data.0, width, num_channels))
            },
            DataType::I32(data) => {
                TileArrayBufferImage::I32(TileArrayBuffer::<ArrayI32>::new(&data.0, width, num_channels))
            },
            DataType::F32(data) => {
                TileArrayBufferImage::F32(TileArrayBuffer::<ArrayF32>::new(&data.0, width, num_channels))
            },
            _ => unreachable!()
        };

        let bscale = if let Some(FITSHeaderKeyword::Other { value, .. } ) = header.get("BSCALE") {
            if let FITSKeywordValue::FloatingPoint(bscale) = value {
                *bscale as f32
            } else {
                1.0
            }
        } else {
            1.0
        };
        let bzero = if let Some(FITSHeaderKeyword::Other { value, .. } ) = header.get("BZERO") {
            if let FITSKeywordValue::FloatingPoint(bzero) = value {
                *bzero as f32
            } else {
                0.0
            }
        } else {
            0.0
        };
        config.set_bscale_bzero(bscale, bzero);
        if !config.is_blank_value() {
            let blank = if let Some(FITSHeaderKeyword::Other { value, .. } ) = header.get("BLANK") {
                if let FITSKeywordValue::FloatingPoint(blank) = value {
                    *blank as f32
                } else {
                    std::f32::MIN
                }
            } else {
                std::f32::MIN
            };
            config.set_blank_value(blank);
        }

        img
    }
}

impl Default for TileRequest {
    fn default() -> Self {
        RequestTile::new()
    }
}

pub struct TileHTMLImage {
    image: web_sys::HtmlImageElement,
    size: Vector2<i32>,
}
use cgmath::{Vector3, Vector2};
impl Image for TileHTMLImage {
    fn tex_sub_image_3d(&self,
        // The texture array
        textures: &Texture2DArray,
        // An offset to write the image in the texture array
        offset: &Vector3<i32>
    ) {
        let size = self.get_size();

        textures.bind()
            .tex_sub_image_3d_with_html_image_element(
                offset.x,
                offset.y,
                offset.z,
                size.x,
                size.y,
                &self.image,
            );
    }

    // The size of the image
    fn get_size(&self) -> &Vector2<i32> {
        &self.size
    }

    /*fn get_cutoff_values(&self) -> std::option::Option<(f32, f32)> {
        None
    }*/
}

impl Drop for TileRequest {
    fn drop(&mut self) {
        crate::log("Drop image!");
    }
}
