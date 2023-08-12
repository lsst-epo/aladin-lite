use crate::{
    async_task::{BuildCatalogIndex, ParseTableTask, TaskExecutor, TaskResult, TaskType},
    camera::CameraViewPort,
    downloader::Downloader,
    grid::ProjetedGrid,
    healpix::coverage::HEALPixCoverage,
    inertia::Inertia,
    math::{
        self,
        angle::{Angle, ArcDeg, ToAngle},
        lonlat::{LonLat, LonLatT},
    },
    renderable::Layers,
    renderable::{
        catalog::Manager, coverage::MOCRenderer, line::RasterizedLineRenderer, ImageCfg, Renderer,
    },
    shader::ShaderManager,
    tile_fetcher::TileFetcherQueue,
    time::DeltaTime,
};

use wasm_bindgen::prelude::*;

use al_core::colormap::{Colormap, Colormaps};
use al_core::WebGlContext;

use super::coosys;
use crate::Abort;
use al_api::{
    coo_system::CooSystem,
    grid::GridCfg,
    hips::{FITSCfg, HiPSCfg, ImageMetadata},
};
use cgmath::Vector4;
use fitsrs::{fits::AsyncFits, hdu::extension::AsyncXtensionHDU};
use wasm_bindgen_futures::JsFuture;

use web_sys::WebGl2RenderingContext;

use std::cell::RefCell;
use std::rc::Rc;

use std::collections::HashSet;

use crate::renderable::final_pass::RenderPass;
use al_core::FrameBufferObject;

use al_api::image::ImageParams;

pub struct App {
    pub gl: WebGlContext,

    //ui: GuiRef,
    shaders: ShaderManager,
    camera: CameraViewPort,

    downloader: Downloader,
    tile_fetcher: TileFetcherQueue,
    layers: Layers,

    time_start_blending: Time,
    request_redraw: bool,
    rendering: bool,

    // The grid renderable
    grid: ProjetedGrid,
    // The moc renderable
    moc: MOCRenderer,
    // Catalog manager
    manager: Manager,

    // Task executor
    exec: Rc<RefCell<TaskExecutor>>,

    inertia: Option<Inertia>,
    disable_inertia: Rc<RefCell<bool>>,
    dist_dragging: f32,
    time_start_dragging: Time,
    time_mouse_high_vel: Time,
    dragging: bool,

    prev_cam_position: Vector3<f64>,
    //prev_center: Vector3<f64>,
    out_of_fov: bool,
    //tasks_finished: bool,
    catalog_loaded: bool,
    start_time_frame: Time,
    last_time_request_for_new_tiles: Time,
    request_for_new_tiles: bool,

    _final_rendering_pass: RenderPass,
    _fbo_view: FrameBufferObject,
    _fbo_ui: FrameBufferObject,
    line_renderer: RasterizedLineRenderer,

    colormaps: Colormaps,

    projection: ProjectionType,

    // Async data receivers
    fits_send: async_channel::Sender<ImageCfg>,
    fits_recv: async_channel::Receiver<ImageCfg>,

    ack_send: async_channel::Sender<ImageParams>,
    ack_recv: async_channel::Receiver<ImageParams>,

    // callbacks
    callback_position_changed: js_sys::Function,
}

use cgmath::{Vector2, Vector3};
use futures::{io::BufReader, stream::StreamExt}; // for `next`

use crate::math::projection::*;
pub const BLENDING_ANIM_DURATION: DeltaTime = DeltaTime::from_millis(200.0); // in ms
                                                                             //use crate::buffer::Tile;
use crate::time::Time;
use cgmath::InnerSpace;

use crate::downloader::query;
use crate::downloader::request;
use al_api::resources::Resources;

impl App {
    pub fn new(
        gl: &WebGlContext,
        mut shaders: ShaderManager,
        resources: Resources,
        // Callbacks
        callback_position_changed: js_sys::Function,
    ) -> Result<Self, JsValue> {
        let gl = gl.clone();
        let exec = Rc::new(RefCell::new(TaskExecutor::new()));

        let projection = ProjectionType::Sin(mapproj::zenithal::sin::Sin);
        gl.blend_func_separate(
            WebGl2RenderingContext::SRC_ALPHA,
            WebGl2RenderingContext::ONE,
            WebGl2RenderingContext::ONE,
            WebGl2RenderingContext::ONE,
        );
        // TODO: https://caniuse.com/?search=scissor is not supported for safari <= 14.1
        // When it will be supported nearly everywhere, we will need to uncomment this line to
        // enable it
        //gl.enable(WebGl2RenderingContext::SCISSOR_TEST);
        gl.enable(WebGl2RenderingContext::CULL_FACE);
        gl.cull_face(WebGl2RenderingContext::BACK);

        // The tile buffer responsible for the tile requests
        let downloader = Downloader::new();

        let camera = CameraViewPort::new(&gl, CooSystem::ICRS, &projection);
        let screen_size = &camera.get_screen_size();

        let _fbo_view =
            FrameBufferObject::new(&gl, screen_size.x as usize, screen_size.y as usize)?;
        let _fbo_ui = FrameBufferObject::new(&gl, screen_size.x as usize, screen_size.y as usize)?;

        // The surveys storing the textures of the resolved tiles
        let layers = Layers::new(&gl, &projection)?;

        let time_start_blending = Time::now();

        // Catalog definition
        let manager = Manager::new(&gl, &mut shaders, &camera, &resources)?;

        // Grid definition
        let grid = ProjetedGrid::new()?;

        // Variable storing the location to move to
        let inertia = None;
        let disable_inertia = Rc::new(RefCell::new(false));

        //let tasks_finished = false;
        let request_redraw = false;
        let rendering = true;
        let prev_cam_position = camera.get_center().truncate();
        //let prev_center = Vector3::new(0.0, 1.0, 0.0);
        let out_of_fov = false;
        let catalog_loaded = false;

        let colormaps = Colormaps::new(&gl)?;

        let _final_rendering_pass = RenderPass::new(&gl)?;
        let tile_fetcher = TileFetcherQueue::new();

        //let ui = Gui::new(aladin_div_name, &gl)?;
        let start_time_frame = Time::now();
        let last_time_request_for_new_tiles = Time::now();

        let request_for_new_tiles = true;

        let moc = MOCRenderer::new()?;
        gl.clear_color(0.15, 0.15, 0.15, 1.0);

        let (fits_send, fits_recv) = async_channel::unbounded::<ImageCfg>();
        let (ack_send, ack_recv) = async_channel::unbounded::<ImageParams>();

        let line_renderer = RasterizedLineRenderer::new(&gl)?;

        let dist_dragging = 0.0;
        let time_start_dragging = Time::now();
        let dragging = false;
        let time_mouse_high_vel = Time::now();

        Ok(App {
            gl,
            start_time_frame,
            //ui,
            shaders,

            camera,

            last_time_request_for_new_tiles,
            request_for_new_tiles,
            downloader,
            layers,

            time_start_blending,
            rendering,
            request_redraw,
            // The grid renderable
            grid,
            // MOCs renderable
            moc,
            // The catalog renderable
            manager,
            exec,
            //prev_center,
            _fbo_view,
            _fbo_ui,
            _final_rendering_pass,

            line_renderer,

            // inertia
            inertia,
            disable_inertia,
            dist_dragging,
            time_start_dragging,
            time_mouse_high_vel,
            dragging,

            prev_cam_position,
            out_of_fov,

            //tasks_finished,
            catalog_loaded,

            tile_fetcher,

            colormaps,
            projection,

            fits_send,
            fits_recv,
            ack_send,
            ack_recv,

            callback_position_changed,
        })
    }

    fn look_for_new_tiles(&mut self) -> Result<(), JsValue> {
        // Move the views of the different active surveys
        self.tile_fetcher.clear();
        // Loop over the surveys
        for survey in self.layers.values_mut_hips() {
            let cfg = survey.get_config();
            let hips_url = cfg.get_root_url().to_string();
            let format = cfg.get_format();
            let min_tile_depth = cfg.delta_depth().max(cfg.get_min_depth_tile());
            let mut ancestors = HashSet::new();

            if let Some(tiles_iter) = survey.look_for_new_tiles(&mut self.camera) {
                for tile_cell in tiles_iter.into_iter() {
                    self.tile_fetcher.append(
                        query::Tile::new(&tile_cell, hips_url.clone(), format),
                        &mut self.downloader,
                    );

                    if tile_cell.depth() >= min_tile_depth + 3 {
                        let ancestor_tile_cell = tile_cell.ancestor(3);
                        ancestors.insert(ancestor_tile_cell);
                    }
                    //let ancestor_next_tile_cell = next_tile_cell.ancestor(3);
                    //if !survey.contains_tile(&ancestor_tile_cell) {
                    //self.tile_fetcher.append(
                    //    query::Tile::new(&ancestor_tile_cell, hips_url.clone(), format),
                    //    &mut self.downloader,
                    //);
                    //}
                    //if ancestor_tile_cell != ancestor_next_tile_cell {

                    //}
                }
            }
            // Request for ancestor
            for ancestor in ancestors {
                if !survey.update_priority_tile(&ancestor) {
                    self.tile_fetcher.append(
                        query::Tile::new(&ancestor, hips_url.clone(), format),
                        &mut self.downloader,
                    );
                }
            }
        }

        Ok(())
    }

    // Run async tasks:
    // - parsing catalogs
    // - copying textures to GPU
    // Return true when a task is complete. This always lead
    // to a redraw of aladin lite
    /*fn run_tasks(&mut self, dt: DeltaTime) -> Result<HashSet<Tile>, JsValue> {
        let tasks_time = (dt.0 * 0.5).min(8.3);
        let results = self.exec.borrow_mut().run(tasks_time);
        self.tasks_finished = !results.is_empty();

        // Retrieve back all the tiles that have been
        // copied to the GPU
        // This is important for the tile buffer to know which
        // requests can be reused to query more tiles
        let mut tiles_available = HashSet::new();
        for result in results {
            match result {
                TaskResult::TableParsed {
                    name,
                    sources,
                    colormap,
                } => {
                    self.manager.add_catalog(
                        name,
                        sources,
                        colormap,
                        &mut self.shaders,
                        &self.camera,
                        self.surveys.get_view().unwrap_abort(),
                    );
                    self.catalog_loaded = true;
                    self.request_redraw = true;
                }
                TaskResult::TileSentToGPU { tile } => {
                    tiles_available.insert(tile);
                }
            }
        }

        Ok(tiles_available)
    }*/
    /*fn run_tasks(&mut self, dt: DeltaTime) -> Result<(), JsValue> {
        let tasks_time = (dt.0 * 0.5).min(8.3);
        let results = self.exec.borrow_mut().run(tasks_time);
        self.tasks_finished = !results.is_empty();

        // Retrieve back all the tiles that have been
        // copied to the GPU
        // This is important for the tile buffer to know which
        // requests can be reused to query more tiles
        for result in results {
            match result {
                TaskResult::TableParsed {
                    name,
                    sources,
                    colormap,
                } => {
                    self.manager.add_catalog(
                        name,
                        sources,
                        colormap,
                        &mut self.shaders,
                        &self.camera,
                        self.surveys.get_view().unwrap_abort(),
                    );
                    self.catalog_loaded = true;
                    self.request_redraw = true;
                } //TaskResult::TileSentToGPU { tile } => todo!()
            }
        }

        Ok(())
    }*/
}

use crate::downloader::request::Resource;
use al_api::cell::HEALPixCellProjeted;

use crate::downloader::request::tile::Tile;
use crate::healpix::cell::HEALPixCell;
use al_api::color::ColorRGB;

impl App {
    pub(crate) fn set_background_color(&mut self, color: ColorRGB) {
        self.layers.set_background_color(color);
        self.request_redraw = true;
    }

    pub(crate) fn get_visible_cells(&self, depth: u8) -> Box<[HEALPixCellProjeted]> {
        // Convert the camera frame vertices to icrs before doing the moc
        let coverage = crate::camera::build_fov_coverage(
            depth,
            self.camera.get_field_of_view(),
            self.camera.get_center(),
            self.camera.get_coo_system(),
            CooSystem::ICRS,
            &self.projection,
        );

        let cells: Vec<_> = coverage
            .flatten_to_fixed_depth_cells()
            .filter_map(|ipix| {
                // this cell is defined in ICRS
                let cell = HEALPixCell(depth, ipix);

                let v = cell.vertices();
                let proj2screen = |(lon, lat): &(f64, f64)| -> Option<[f64; 2]> {
                    // 1. convert to xyzw
                    let xyzw = crate::math::lonlat::radec_to_xyzw(Angle(*lon), Angle(*lat));
                    // 2. get it back to the camera frame system
                    let xyzw = crate::coosys::apply_coo_system(
                        CooSystem::ICRS,
                        self.camera.get_coo_system(),
                        &xyzw,
                    );

                    // 3. project on screen
                    if let Some(p) = self.projection.model_to_clip_space(&xyzw, &self.camera) {
                        Some([p.x, p.y])
                    } else {
                        None
                    }
                };

                if let (Some(c1), Some(c2), Some(c3), Some(c4)) = (
                    proj2screen(&v[0]),
                    proj2screen(&v[1]),
                    proj2screen(&v[2]),
                    proj2screen(&v[3]),
                ) {
                    let c: [[f64; 2]; 4] = [c1, c2, c3, c4];

                    let mut j = c.len() - 1;
                    for i in 0..c.len() {
                        if crate::math::vector::dist2(&c[j], &c[i]) > 0.04 {
                            return None;
                        }

                        j = i;
                    }

                    let v1 = crate::clip_to_screen_space(&c[0].into(), &self.camera);
                    let v2 = crate::clip_to_screen_space(&c[1].into(), &self.camera);
                    let v3 = crate::clip_to_screen_space(&c[2].into(), &self.camera);
                    let v4 = crate::clip_to_screen_space(&c[3].into(), &self.camera);

                    let vx = [v1[0], v2[0], v3[0], v4[0]];
                    let vy = [v1[1], v2[1], v3[1], v4[1]];

                    Some(HEALPixCellProjeted { ipix, vx, vy })
                } else {
                    None
                }
            })
            .collect();

        cells.into_boxed_slice()
    }

    pub(crate) fn is_catalog_loaded(&self) -> bool {
        self.catalog_loaded
    }

    pub(crate) fn is_ready(&self) -> Result<bool, JsValue> {
        let res = self.layers.is_ready();

        Ok(res)
    }

    /*pub(crate) fn get_moc(&self, cfg: &al_api::moc::MOC) -> Option<&HEALPixCoverage> {
        self.moc.get(cfg)
    }*/

    pub(crate) fn add_moc(
        &mut self,
        cfg: al_api::moc::MOC,
        moc: HEALPixCoverage,
    ) -> Result<(), JsValue> {
        self.moc
            .push_back(moc, cfg, &mut self.camera, &self.projection);
        self.request_redraw = true;

        Ok(())
    }

    pub(crate) fn remove_moc(&mut self, cfg: &al_api::moc::MOC) -> Result<(), JsValue> {
        self.moc
            .remove(cfg, &mut self.camera, &self.projection)
            .ok_or_else(|| JsValue::from_str("MOC not found"))?;

        self.request_redraw = true;

        Ok(())
    }

    pub(crate) fn set_moc_cfg(&mut self, cfg: al_api::moc::MOC) -> Result<(), JsValue> {
        self.moc
            .set_cfg(
                cfg,
                &mut self.camera,
                &self.projection,
                &mut self.line_renderer,
            )
            .ok_or_else(|| JsValue::from_str("MOC not found"))?;
        self.request_redraw = true;

        Ok(())
    }

    pub(crate) fn set_callback_position_changed(&mut self, callback: js_sys::Function) {
        self.callback_position_changed = callback;
    }

    pub(crate) fn update(&mut self, _dt: DeltaTime) -> Result<bool, JsValue> {
        self.start_time_frame = Time::now();

        //let available_tiles = self.run_tasks(dt)?;
        if let Some(inertia) = self.inertia.as_mut() {
            inertia.apply(&mut self.camera, &self.projection);
            // Always request for new tiles while moving
            self.request_for_new_tiles = true;

            // The threshold stopping criteria must be dependant
            // of the zoom level, in this case the initial angular distance
            // speed
            let thresh_speed = inertia.get_start_ampl() * 1e-3;
            let cur_speed = inertia.get_cur_speed();

            // Create the javascript object to pass to the callback
            let args: js_sys::Object = js_sys::Object::new();
            let center = self.camera.get_center().lonlat();
            js_sys::Reflect::set(
                &args,
                &"ra".into(),
                &JsValue::from_f64(center.lon().to_degrees()),
            )?;
            js_sys::Reflect::set(
                &args,
                &"dec".into(),
                &JsValue::from_f64(center.lat().to_degrees()),
            )?;
            js_sys::Reflect::set(&args, &"dragging".into(), &JsValue::from_bool(false))?;
            // Position has changed, we call the callback
            self.callback_position_changed
                .call1(&JsValue::null(), &args)?;

            if cur_speed < thresh_speed {
                self.inertia = None;
            }
        }

        // The rendering is done following these different situations:
        // - the camera has moved
        let has_camera_moved = self.camera.has_moved();

        //let has_camera_recently_moved =
        //    ;
        let _has_camera_zoomed = self.camera.has_zoomed();
        {
            // Newly available tiles must lead to
            // 1. Surveys must be aware of the new available tiles
            //self.surveys.set_available_tiles(&available_tiles);
            // 2. Get the resolved tiles and push them to the image surveys
            /*let is_there_new_available_tiles = self
            .downloader
            .get_resolved_tiles(/*&available_tiles, */&mut self.surveys);*/
            let rscs_received = self.downloader.get_received_resources();

            let _num_tile_handled = 0;
            let _tile_copied = false;
            for rsc in rscs_received {
                match rsc {
                    Resource::Tile(tile) => {
                        if !has_camera_moved {
                            if let Some(survey) =
                                self.layers.get_mut_hips_from_url(&tile.get_hips_url())
                            {
                                let cfg = survey.get_config();

                                if cfg.get_format() == tile.format {
                                    let delta_depth = cfg.delta_depth();
                                    let fov_coverage = self.camera.get_cov(cfg.get_frame());
                                    let included_or_near_coverage = tile
                                        .cell()
                                        .get_texture_cell(delta_depth)
                                        .get_tile_cells(delta_depth)
                                        .any(|neighbor_tile_cell| {
                                            fov_coverage.intersects_cell(&neighbor_tile_cell)
                                        });

                                    let is_tile_root = tile.cell().depth() == delta_depth;
                                    let _depth = tile.cell().depth();
                                    //al_core::info!("is root tile", depth, is_tile_root);
                                    // do not perform tex_sub costly GPU calls while the camera is zooming
                                    if is_tile_root || included_or_near_coverage {
                                        let is_missing = tile.missing();
                                        /*self.tile_fetcher.notify_tile(
                                            &tile,
                                            true,
                                            false,
                                            &mut self.downloader,
                                        );*/
                                        let Tile {
                                            cell,
                                            image,
                                            time_req,
                                            ..
                                        } = tile;

                                        let image = if is_missing {
                                            // Otherwise we push nothing, it is probably the case where:
                                            // - an request error occured on a valid tile
                                            // - the tile is not present, e.g. chandra HiPS have not the 0, 1 and 2 order tiles
                                            None
                                        } else {
                                            Some(image)
                                        };

                                        survey.add_tile(&cell, image, time_req)?;

                                        self.request_redraw = true;
                                        //} else {
                                        //    self.downloader.delay_rsc(Resource::Tile(tile));
                                        //}
                                        //}
                                        self.time_start_blending = Time::now();
                                        //self.tile_fetcher.notify(1, &mut self.downloader);
                                    }
                                }
                            }
                        } else {
                            //self.tile_fetcher
                            //    .notify_tile(&tile, false, true, &mut self.downloader);
                            self.downloader.delay_rsc(Resource::Tile(tile));
                        }
                    }
                    Resource::Allsky(allsky) => {
                        let hips_url = allsky.get_hips_url();

                        if let Some(survey) = self.layers.get_mut_hips_from_url(hips_url) {
                            let is_missing = allsky.missing();
                            if is_missing {
                                // The allsky image is missing so we donwload all the tiles contained into
                                // the 0's cell
                                let cfg = survey.get_config();
                                let _delta_depth = cfg.delta_depth();
                                let hips_url = cfg.get_root_url();
                                let format = cfg.get_format();
                                for texture_cell in crate::healpix::cell::ALLSKY_HPX_CELLS_D0 {
                                    for cell in texture_cell.get_tile_cells(cfg.delta_depth()) {
                                        let query =
                                            query::Tile::new(&cell, hips_url.to_string(), format);
                                        self.tile_fetcher
                                            .append_base_tile(query, &mut self.downloader);
                                    }
                                }
                            } else {
                                // tell the survey to not download tiles which order is <= 3 because the allsky
                                // give them already
                                survey.add_allsky(allsky)?;
                                // Once received ask for redraw
                                self.request_redraw = true;
                            }
                        }
                    }
                    Resource::PixelMetadata(metadata) => {
                        if let Some(hips) = self.layers.get_mut_hips_from_url(&metadata.hips_url) {
                            let mut cfg = hips.get_config_mut();

                            if let Some(metadata) = *metadata.value.lock().unwrap_abort() {
                                cfg.blank = metadata.blank;
                                cfg.offset = metadata.offset;
                                cfg.scale = metadata.scale;
                            }
                        }
                    }
                    Resource::Moc(moc) => {
                        let moc_url = moc.get_url();
                        let url = &moc_url[..moc_url.find("/Moc.fits").unwrap_abort()];
                        if let Some(hips) = self.layers.get_mut_hips_from_url(url) {
                            let request::moc::Moc { moc, .. } = moc;

                            if let Some(moc) = &*moc.lock().unwrap_abort() {
                                hips.set_moc(moc.clone());

                                self.request_for_new_tiles = true;
                                self.request_redraw = true;
                            };
                        }
                    }
                }
            }

            // We fetch when we does not move
            let has_not_moved_recently =
                (Time::now() - self.camera.get_time_of_last_move()) > DeltaTime(100.0);
            if has_not_moved_recently && self.inertia.is_none() {
                // Triggers the fetching of new queued tiles
                self.tile_fetcher.notify(&mut self.downloader);
            }
        }

        // The update from the camera
        self.layers.update(&mut self.camera, &self.projection);

        if self.request_for_new_tiles
            && Time::now() - self.last_time_request_for_new_tiles > DeltaTime::from(200.0)
        {
            self.look_for_new_tiles()?;

            self.request_for_new_tiles = false;
            self.last_time_request_for_new_tiles = Time::now();
        }

        // - there is at least one tile in its blending phase
        let blending_anim_occuring =
            (Time::now() - self.time_start_blending) < BLENDING_ANIM_DURATION;

        let start_fading = self.layers.values_hips().any(|hips| {
            if let Some(start_time) = hips.get_ready_time() {
                Time::now() - *start_time < BLENDING_ANIM_DURATION
            } else {
                false
            }
        });

        // Finally update the camera that reset the flag camera changed
        //if has_camera_moved {
        // Catalogues update
        /*if let Some(view) = self.layers.get_view() {
            self.manager.update(&self.camera, view);
        }*/
        //}

        // Check for async retrieval
        if let Ok(fits) = self.fits_recv.try_recv() {
            let params = fits.get_params();
            self.layers
                .add_image_fits(fits, &mut self.camera, &self.projection)?;
            self.request_redraw = true;

            // Send the ack to the js promise so that she finished
            let ack_send = self.ack_send.clone();
            wasm_bindgen_futures::spawn_local(async move {
                ack_send.send(params).await.unwrap_throw();
            })
        }

        self.rendering =
            blending_anim_occuring | has_camera_moved | self.request_redraw | start_fading;
        self.request_redraw = false;

        self.draw(false)?;

        Ok(has_camera_moved)
    }

    pub(crate) fn reset_north_orientation(&mut self) {
        // Reset the rotation around the center if there is one
        self.camera
            .set_rotation_around_center(Angle(0.0), &self.projection);
        // Reset the camera position to its current position
        // this will keep the current position but reset the orientation
        // so that the north pole is at the top of the center.
        self.set_center(&self.get_center());
    }

    pub(crate) fn read_pixel(&self, pos: &Vector2<f64>, layer: &str) -> Result<JsValue, JsValue> {
        if let Some(lonlat) = self.screen_to_world(pos) {
            if let Some(survey) = self.layers.get_hips_from_layer(layer) {
                survey.read_pixel(&lonlat, &self.camera)
            } else if let Some(_image) = self.layers.get_image_from_layer(layer) {
                Err(JsValue::from_str("TODO: read pixel value"))
            } else {
                Err(JsValue::from_str("Survey not found"))
            }
        } else {
            Err(JsValue::from_str(&"position is out of projection"))
        }
    }

    pub(crate) fn draw(&mut self, force_render: bool) -> Result<(), JsValue> {
        /*let scene_redraw = self.rendering | force_render;
        let mut ui = self.ui.lock();

        if scene_redraw {
            let shaders = &mut self.shaders;
            let gl = self.gl.clone();
            let camera = &self.camera;

            let grid = &mut self.grid;
            let layers = &mut self.layers;
            let catalogs = &self.manager;
            let colormaps = &self.colormaps;
            let fbo_view = &self.fbo_view;

            fbo_view.draw_onto(
                move || {
                    // Render the scene
                    gl.clear_color(0.00, 0.00, 0.00, 1.0);
                    gl.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);

                    layers.draw(camera, shaders, colormaps);

                    // Draw the catalog
                    catalogs.draw(&gl, shaders, camera, colormaps, fbo_view)?;

                    grid.draw(camera, shaders)?;

                    Ok(())
                },
                None,
            )?;

            // Reset the flags about the user action
            self.camera.reset();
        }

        let gl = self.gl.clone();

        let ui_redraw = ui.redraw_needed();
        if ui_redraw {
            let dpi  = self.camera.get_dpi();

            self.fbo_ui.draw_onto(move || {
                ui.draw(&gl, dpi)?;

                Ok(())
            }, None)?;
        }

        // If neither of the scene or the ui has been redraw then do nothing
        // otherwise, redraw both fbos on the screen
        if scene_redraw || ui_redraw {
            self.final_rendering_pass.draw_on_screen(&self.fbo_view);
            self.final_rendering_pass.draw_on_screen(&self.fbo_ui);
        }

        self.layers.reset_frame();*/

        let scene_redraw = self.rendering | force_render;
        //let mut ui = self.ui.lock();
        //let ui_redraw = ui.redraw_needed();
        //if scene_redraw || ui_redraw {
        if scene_redraw {
            //let catalogs = &self.manager;
            // Render the scene
            // Clear all the screen first (only the region set by the scissor)
            self.gl
                .clear(web_sys::WebGl2RenderingContext::COLOR_BUFFER_BIT);

            self.layers.draw(
                &mut self.camera,
                &mut self.shaders,
                &self.colormaps,
                &self.projection,
            )?;

            // Draw the catalog
            //let fbo_view = &self.fbo_view;
            //catalogs.draw(&gl, shaders, camera, colormaps, fbo_view)?;
            //catalogs.draw(&gl, shaders, camera, colormaps, None, self.projection)?;
            self.line_renderer.begin();
            //Time::measure_perf("moc draw", || {
            self.moc.draw(
                &mut self.shaders,
                &mut self.camera,
                &self.projection,
                &mut self.line_renderer,
            );

            //    Ok(())
            //})?;

            self.grid.draw(
                &self.camera,
                &mut self.shaders,
                &self.projection,
                &mut self.line_renderer,
            )?;
            self.line_renderer.end();

            self.line_renderer.draw(&self.camera)?;
            //let dpi  = self.camera.get_dpi();
            //ui.draw(&gl, dpi)?;

            // Reset the flags about the user action
            self.camera.reset();

            /*if self.rendering {
                self.layers.reset_frame();
                self.moc.reset_frame();
            }*/
        }

        Ok(())
    }

    pub(crate) fn remove_layer(&mut self, layer: &str) -> Result<(), JsValue> {
        self.layers
            .remove_layer(layer, &mut self.camera, &self.projection)?;

        self.request_redraw = true;

        Ok(())
    }

    pub(crate) fn rename_layer(&mut self, layer: &str, new_layer: &str) -> Result<(), JsValue> {
        self.layers.rename_layer(&layer, &new_layer)
    }

    pub(crate) fn swap_layers(
        &mut self,
        first_layer: &str,
        second_layer: &str,
    ) -> Result<(), JsValue> {
        self.layers.swap_layers(first_layer, second_layer)?;

        self.request_redraw = true;

        Ok(())
    }

    pub(crate) fn add_image_survey(&mut self, hips_cfg: HiPSCfg) -> Result<(), JsValue> {
        let hips =
            self.layers
                .add_image_survey(&self.gl, hips_cfg, &mut self.camera, &self.projection)?;
        self.tile_fetcher
            .launch_starting_hips_requests(hips, &mut self.downloader);

        // Once its added, request the tiles in the view (unless the viewer is at depth 0)
        self.request_for_new_tiles = true;
        self.request_redraw = true;
        //self.grid.update(&self.camera, &self.projection);

        Ok(())
    }

    pub(crate) fn add_image_fits(&mut self, cfg: FITSCfg) -> Result<js_sys::Promise, JsValue> {
        let FITSCfg { layer, url, meta } = cfg;
        let gl = self.gl.clone();

        let fits_sender = self.fits_send.clone();
        let ack_recv = self.ack_recv.clone();
        // Stop the current inertia
        self.inertia = None;
        // And disable it while the fits has not been loaded
        let disable_inertia = self.disable_inertia.clone();
        *(disable_inertia.borrow_mut()) = true;

        let fut = async move {
            use crate::renderable::image::Image;
            use futures::future::Either;
            use futures::TryStreamExt;
            use js_sys::Uint8Array;
            use wasm_streams::ReadableStream;
            use web_sys::window;
            use web_sys::Response;
            use web_sys::{Request, RequestInit, RequestMode};

            let mut opts = RequestInit::new();
            opts.method("GET");
            opts.mode(RequestMode::Cors);

            let window = window().unwrap();
            let request = Request::new_with_str_and_init(&url, &opts)?;

            let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
            let resp: Response = resp_value.dyn_into()?;

            // Get the response's body as a JS ReadableStream
            let raw_body = resp.body().unwrap();
            let body = ReadableStream::from_raw(raw_body.dyn_into()?);

            // Convert the JS ReadableStream to a Rust stream
            let bytes_reader = match body.try_into_async_read() {
                Ok(async_read) => Either::Left(async_read),
                Err((_err, body)) => Either::Right(
                    body.into_stream()
                        .map_ok(|js_value| {
                            js_value.dyn_into::<Uint8Array>().unwrap_throw().to_vec()
                        })
                        .map_err(|_js_error| {
                            std::io::Error::new(std::io::ErrorKind::Other, "failed to read")
                        })
                        .into_async_read(),
                ),
            };

            let mut reader = BufReader::new(bytes_reader);

            let AsyncFits { mut hdu } = AsyncFits::from_reader(&mut reader)
                .await
                .map_err(|e| JsValue::from_str(&format!("Fits file parsing: reason: {}", e)))?;

            let mut hdu_ext_idx = 0;
            let mut images_params = vec![];

            match Image::from_fits_hdu_async(&gl, &mut hdu.0).await {
                Ok(image) => {
                    let layer_ext = layer.clone();
                    let url_ext = url.clone();

                    let fits = ImageCfg {
                        image: image,
                        layer: layer_ext,
                        url: url_ext,
                        meta: meta.clone(),
                    };

                    fits_sender.send(fits).await.unwrap();

                    // Wait for the ack here
                    let image_params = ack_recv
                        .recv()
                        .await
                        .map_err(|_| JsValue::from_str("Problem receiving fits"))?;

                    images_params.push(image_params);

                    let mut hdu_ext = hdu.next().await;

                    // Continue parsing the file extensions here
                    while let Ok(Some(mut xhdu)) = hdu_ext {
                        match &mut xhdu {
                            AsyncXtensionHDU::Image(xhdu_img) => {
                                match Image::from_fits_hdu_async(&gl, xhdu_img).await {
                                    Ok(image) => {
                                        let layer_ext =
                                            layer.clone() + "_ext_" + &format!("{hdu_ext_idx}");
                                        let url_ext =
                                            url.clone() + "_ext_" + &format!("{hdu_ext_idx}");

                                        let fits_ext = ImageCfg {
                                            image: image,
                                            layer: layer_ext,
                                            url: url_ext,
                                            meta: meta.clone(),
                                        };

                                        fits_sender.send(fits_ext).await.unwrap();

                                        let image_params = ack_recv.recv().await.map_err(|_| {
                                            JsValue::from_str("Problem receving fits")
                                        })?;

                                        images_params.push(image_params);
                                    }
                                    Err(error) => {
                                        al_core::log::console_warn(&
                                            format!("The extension {hdu_ext_idx} has not been parsed, reason:")
                                        );

                                        al_core::log::console_warn(error);
                                    }
                                }
                            }
                            _ => {
                                al_core::log::console_warn(&
                                    format!("The extension {hdu_ext_idx} is a BinTable/AsciiTable and is thus discarded")
                                );
                            }
                        }

                        hdu_ext_idx += 1;

                        hdu_ext = xhdu.next().await;
                    }
                }
                Err(error) => {
                    al_core::log::console_warn(error);

                    let mut hdu_ext = hdu.next().await;

                    while let Ok(Some(mut xhdu)) = hdu_ext {
                        match &mut xhdu {
                            AsyncXtensionHDU::Image(xhdu_img) => {
                                match Image::from_fits_hdu_async(&gl, xhdu_img).await {
                                    Ok(image) => {
                                        let layer_ext =
                                            layer.clone() + "_ext_" + &format!("{hdu_ext_idx}");
                                        let url_ext =
                                            url.clone() + "_ext_" + &format!("{hdu_ext_idx}");

                                        let fits_ext = ImageCfg {
                                            image: image,
                                            layer: layer_ext,
                                            url: url_ext,
                                            meta: meta.clone(),
                                        };

                                        fits_sender.send(fits_ext).await.unwrap();

                                        let image_params = ack_recv.recv().await.map_err(|_| {
                                            JsValue::from_str("Problem receving fits")
                                        })?;

                                        images_params.push(image_params);
                                    }
                                    Err(error) => {
                                        al_core::log::console_warn(&
                                            format!("The extension {hdu_ext_idx} has not been parsed, reason:")
                                        );

                                        al_core::log::console_warn(error);
                                    }
                                }
                            }
                            _ => {
                                al_core::log::console_warn(&
                                    format!("The extension {hdu_ext_idx} is a BinTable/AsciiTable and is thus discarded")
                                );
                            }
                        }

                        hdu_ext_idx += 1;

                        hdu_ext = xhdu.next().await;
                    }
                }
            }

            if !images_params.is_empty() {
                serde_wasm_bindgen::to_value(&images_params).map_err(|e| e.into())
            } else {
                Err(JsValue::from_str("The fits could not be parsed"))
            }
        };

        let reenable_inertia = Closure::new(move || {
            // renable inertia again
            *(disable_inertia.borrow_mut()) = false;
        });

        let promise = wasm_bindgen_futures::future_to_promise(fut)
            // Reenable inertia independantly from whether the
            // fits has been correctly parsed or not
            .finally(&reenable_inertia);

        // forget the closure, it is not very proper to do this as
        // it won't be deallocated
        reenable_inertia.forget();

        Ok(promise)
    }

    pub(crate) fn get_layer_cfg(&self, layer: &str) -> Result<ImageMetadata, JsValue> {
        self.layers.get_layer_cfg(layer)
    }

    pub(crate) fn set_hips_url(
        &mut self,
        past_url: String,
        new_url: String,
    ) -> Result<(), JsValue> {
        self.layers.set_survey_url(past_url, new_url.clone())?;

        let hips = self.layers.get_hips_from_url(&new_url).unwrap_abort();
        // Relaunch the base tiles for the survey to be ready with the new url
        self.tile_fetcher
            .launch_starting_hips_requests(hips, &mut self.downloader);

        Ok(())
    }

    pub(crate) fn set_image_survey_color_cfg(
        &mut self,
        layer: String,
        meta: ImageMetadata,
    ) -> Result<(), JsValue> {
        let old_meta = self.layers.get_layer_cfg(&layer)?;
        // Set the new meta
        let new_img_fmt = meta.img_format;
        self.layers
            .set_layer_cfg(layer.clone(), meta, &mut self.camera, &self.projection)?;

        if old_meta.img_format != new_img_fmt {
            // The image format has been changed
            let hips = self
                .layers
                .get_mut_hips_from_layer(&layer)
                .ok_or_else(|| JsValue::from_str("Layer not found"))?;
            hips.set_img_format(new_img_fmt)?;

            // Relaunch the base tiles for the survey to be ready with the new url
            self.tile_fetcher
                .launch_starting_hips_requests(hips, &mut self.downloader);

            // Once its added, request the tiles in the view (unless the viewer is at depth 0)
            self.request_for_new_tiles = true;
        }

        self.request_redraw = true;

        Ok(())
    }

    // Width and height given are in pixels
    pub(crate) fn set_projection(&mut self, projection: ProjectionType) -> Result<(), JsValue> {
        self.projection = projection;

        // Recompute the ndc_to_clip
        self.camera.set_projection(&self.projection);
        // Recompute clip zoom factor
        self.layers.set_projection(&self.projection)?;

        self.request_for_new_tiles = true;
        self.request_redraw = true;

        Ok(())
    }

    pub(crate) fn get_max_fov(&self) -> f64 {
        self.projection.aperture_start()
    }

    pub(crate) fn get_longitude_reversed(&self) -> bool {
        self.camera.get_longitude_reversed()
    }

    pub(crate) fn add_catalog(&mut self, name: String, table: JsValue, _colormap: String) {
        let mut exec_ref = self.exec.borrow_mut();
        let table = table;

        exec_ref
            .spawner()
            .spawn(TaskType::ParseTableTask, async move {
                let mut stream = ParseTableTask::<[f32; 2]>::new(table);
                let mut results: Vec<LonLatT<f32>> = vec![];

                while let Some(item) = stream.next().await {
                    results.push(LonLatT::new(item[0].to_angle(), item[1].to_angle()));
                }

                let mut stream_sort = BuildCatalogIndex::new(results);
                while stream_sort.next().await.is_some() {}

                // The stream is finished, we get the sorted sources
                let results = stream_sort.sources;

                TaskResult::TableParsed {
                    name,
                    sources: results.into_boxed_slice(),
                }
            });
    }

    pub(crate) fn resize(&mut self, width: f32, height: f32) {
        self.camera.set_screen_size(width, height, &self.projection);
        self.camera
            .set_aperture(self.camera.get_aperture(), &self.projection);
        // resize the view fbo
        //self.fbo_view.resize(w as usize, h as usize);
        // resize the ui fbo
        //self.fbo_ui.resize(w as usize, h as usize);

        // launch the new tile requests
        self.request_for_new_tiles = true;
        self.manager.set_kernel_size(&self.camera);

        self.request_redraw = true;
    }

    pub(crate) fn set_survey_url(
        &mut self,
        past_url: String,
        new_url: String,
    ) -> Result<(), JsValue> {
        self.layers.set_survey_url(past_url, new_url)
    }

    pub(crate) fn set_catalog_opacity(
        &mut self,
        name: String,
        opacity: f32,
    ) -> Result<(), JsValue> {
        let catalog = self.manager.get_mut_catalog(&name).map_err(|e| {
            let err: JsValue = e.into();
            err
        })?;
        catalog.set_alpha(opacity);

        self.request_redraw = true;

        Ok(())
    }

    pub(crate) fn set_kernel_strength(
        &mut self,
        name: String,
        strength: f32,
    ) -> Result<(), JsValue> {
        let catalog = self.manager.get_mut_catalog(&name).map_err(|e| {
            let err: JsValue = e.into();
            err
        })?;
        catalog.set_strength(strength);

        self.request_redraw = true;

        Ok(())
    }

    pub(crate) fn set_grid_cfg(&mut self, cfg: GridCfg) -> Result<(), JsValue> {
        self.grid.set_cfg(cfg, &self.camera, &self.projection)?;
        self.request_redraw = true;

        Ok(())
    }

    pub(crate) fn set_coo_system(&mut self, coo_system: CooSystem) {
        self.camera.set_coo_system(coo_system, &self.projection);
        self.request_for_new_tiles = true;

        self.request_redraw = true;
    }

    pub(crate) fn world_to_screen(&self, ra: f64, dec: f64) -> Option<Vector2<f64>> {
        let lonlat = LonLatT::new(ArcDeg(ra).into(), ArcDeg(dec).into());
        let model_pos_xyz = lonlat.vector();

        self.projection
            .view_to_screen_space(&model_pos_xyz, &self.camera)
    }

    pub(crate) fn screen_to_world(&self, pos: &Vector2<f64>) -> Option<LonLatT<f64>> {
        // Select the HiPS layer rendered lastly
        self.projection
            .screen_to_model_space(pos, &self.camera)
            .map(|model_pos| model_pos.lonlat())
    }

    pub(crate) fn screen_to_clip(&self, pos: &Vector2<f64>) -> Vector2<f64> {
        // Select the HiPS layer rendered lastly
        crate::math::projection::screen_to_clip_space(pos, &self.camera)
    }

    pub(crate) fn view_to_icrs_coosys(&self, lonlat: &LonLatT<f64>) -> LonLatT<f64> {
        let icrs_pos: Vector4<_> = lonlat.vector();
        let view_system = self.camera.get_coo_system();
        let (ra, dec) = math::lonlat::xyzw_to_radec(&coosys::apply_coo_system(
            view_system,
            CooSystem::ICRS,
            &icrs_pos,
        ));

        LonLatT::new(ra, dec)
    }

    pub(crate) fn set_center(&mut self, lonlat: &LonLatT<f64>) {
        self.prev_cam_position = self.camera.get_center().truncate();

        self.camera
            .set_center(lonlat, CooSystem::ICRS, &self.projection);
        self.request_for_new_tiles = true;

        // And stop the current inertia as well if there is one
        self.inertia = None;
    }

    pub(crate) fn move_mouse(&mut self, s1x: f32, s1y: f32, s2x: f32, s2y: f32) {
        if self.dragging {
            let from_mouse_pos = [s1x, s1y];
            let to_mouse_pos = [s2x, s2y];
            let dx = crate::math::vector::dist2(&from_mouse_pos, &to_mouse_pos).sqrt();
            self.dist_dragging += dx;

            let dv = dx / (Time::now() - self.camera.get_time_of_last_move()).as_secs();

            if dv > 10000.0 {
                self.time_mouse_high_vel = Time::now();
            }
        }
    }

    pub(crate) fn press_left_button_mouse(&mut self, _sx: f32, _sy: f32) {
        self.dist_dragging = 0.0;
        self.time_start_dragging = Time::now();
        self.dragging = true;

        self.inertia = None;
        self.request_for_new_tiles = true;
        self.out_of_fov = false;
    }

    pub(crate) fn release_left_button_mouse(&mut self, sx: f32, sy: f32) {
        self.request_for_new_tiles = true;

        self.dragging = false;
        let _cur_mouse_pos = [sx, sy];

        // Check whether the center has moved
        // between the pressing and releasing
        // of the left button.
        //
        // Do not start inerting if:
        // * the mouse has not moved
        // * the mouse is out of the projection
        // * the mouse has not been moved since a certain
        //   amount of time

        //debug!(now);
        //debug!(time_of_last_move);
        if self.out_of_fov {
            return;
        }

        let inertia_disabled: bool = *(self.disable_inertia.borrow_mut());
        if inertia_disabled {
            return;
        }

        if self.dist_dragging == 0.0 {
            return;
        }

        let now = Time::now();
        let dragging_duration = (now - self.time_start_dragging).as_secs();
        let dragging_vel = self.dist_dragging / dragging_duration;

        let _dist_dragging = self.dist_dragging;
        // Detect if there has been a recent acceleration
        // It is also possible that the dragging time is too short and if it is the case, trigger the inertia
        let recent_acceleration = (Time::now() - self.time_mouse_high_vel).as_secs() < 0.1
            || (Time::now() - self.time_start_dragging).as_secs() < 0.1;

        if dragging_vel < 3000.0 && !recent_acceleration {
            return;
        }

        // Start inertia here
        // Angular distance between the previous and current
        // center position
        let center = self.camera.get_center().truncate();
        let axis = self.prev_cam_position.cross(center).normalize();

        //let delta_time = ((now - time_of_last_move).0 as f64).max(1.0);
        let delta_angle = math::vector::angle3(&self.prev_cam_position, &center).to_radians();
        let ampl = delta_angle * (dragging_vel as f64) * 5e-3;
        //let ampl = (dragging_vel * 0.01) as f64;

        self.inertia = Some(Inertia::new(ampl.to_radians(), axis))
    }

    pub(crate) fn rotate_around_center(&mut self, theta: ArcDeg<f64>) {
        self.camera
            .set_rotation_around_center(theta.into(), &self.projection);
        // New tiles can be needed and some tiles can be removed
        self.request_for_new_tiles = true;

        self.request_redraw = true;
    }

    pub(crate) fn get_rotation_around_center(&self) -> &Angle<f64> {
        self.camera.get_rotation_around_center()
    }

    pub(crate) fn set_fov(&mut self, fov: Angle<f64>) {
        // For the moment, no animation is triggered.
        // The fov is directly set
        self.camera.set_aperture(fov, &self.projection);
        self.request_for_new_tiles = true;
        self.request_redraw = true;
    }

    /*pub(crate) fn project_line(&self, lon1: f64, lat1: f64, lon2: f64, lat2: f64) -> Vec<Vector2<f64>> {
        let v1: Vector3<f64> = LonLatT::new(ArcDeg(lon1).into(), ArcDeg(lat1).into()).vector();
        let v2: Vector3<f64> = LonLatT::new(ArcDeg(lon2).into(), ArcDeg(lat2).into()).vector();

        line::project_along_great_circles(&v1, &v2, &self.camera, self.projection)
    }*/

    pub(crate) fn go_from_to(&mut self, s1x: f64, s1y: f64, s2x: f64, s2y: f64) {
        // Select the HiPS layer rendered lastly
        if let (Some(w1), Some(w2)) = (
            self.projection
                .screen_to_model_space(&Vector2::new(s1x, s1y), &self.camera),
            self.projection
                .screen_to_model_space(&Vector2::new(s2x, s2y), &self.camera),
        ) {
            let prev_pos = w1.truncate();
            //let cur_pos = w1.truncate();
            let cur_pos = w2.truncate();
            //let next_pos = w2.truncate();
            if prev_pos != cur_pos {
                /* 1. Rotate by computing the angle between the last and current position */

                // Apply the rotation to the camera to
                // go from the current pos to the next position
                let axis = prev_pos.cross(cur_pos).normalize();

                let d = math::vector::angle3(&prev_pos, &cur_pos);

                self.prev_cam_position = self.camera.get_center().truncate();
                self.camera.rotate(&(-axis), d, &self.projection);

                /* 2. Or just set the center to the current position */
                //self.set_center(&cur_pos.lonlat());

                self.request_for_new_tiles = true;
            }
        } else {
            self.out_of_fov = true;
        }
    }

    pub(crate) fn add_cmap(&mut self, label: String, cmap: Colormap) -> Result<(), JsValue> {
        self.colormaps.add_cmap(label, cmap)
    }

    // Accessors
    pub(crate) fn get_center(&self) -> LonLatT<f64> {
        self.camera.get_center().lonlat()
    }

    pub(crate) fn get_norder(&self) -> i32 {
        self.camera.get_tile_depth() as i32
    }

    pub(crate) fn get_clip_zoom_factor(&self) -> f64 {
        self.camera.get_clip_zoom_factor()
    }

    pub(crate) fn get_fov(&self) -> f64 {
        let deg: ArcDeg<f64> = self.camera.get_aperture().into();
        deg.0
    }

    pub(crate) fn get_colormaps(&self) -> &Colormaps {
        &self.colormaps
    }

    pub(crate) fn get_gl_canvas(&self) -> Option<js_sys::Object> {
        self.gl.canvas()
    }

    pub(crate) fn is_rendering(&self) -> bool {
        self.rendering
    }
}
