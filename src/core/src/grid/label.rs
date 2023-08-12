

use crate::math::PI;
use cgmath::Vector3;
use crate::ProjectionType;
use crate::CameraViewPort;
use crate::LonLatT;
use cgmath::InnerSpace;

use crate::math::angle::SerializeFmt;
use crate::math::TWICE_PI;
use crate::grid::XYScreen;
use crate::math::lonlat::LonLat;

use crate::math::angle::ToAngle;
use core::ops::Range;
use cgmath::Vector2;

const OFF_TANGENT: f64 = 35.0;
const OFF_BI_TANGENT: f64 = 5.0;

pub enum LabelOptions {
    Centered,
    OnSide,
}

#[derive(Debug)]
pub struct Label {
    // The position
    pub position: XYScreen,
    // the string content
    pub content: String,
    // in radians
    pub rot: f64,
}
impl Label {
    pub fn from_meridian(
        lon: f64,
        lat: &Range<f64>,
        options: LabelOptions,
        camera: &CameraViewPort,
        projection: &ProjectionType,
        fmt: &SerializeFmt
    ) -> Option<Self> {
        let fov = camera.get_field_of_view();
        let d = if fov.contains_north_pole() {
            Vector3::new(0.0, 1.0, 0.0)
        } else if fov.contains_south_pole() {
            Vector3::new(0.0, -1.0, 0.0)
        } else {
            Vector3::new(0.0, 1.0, 0.0)
        };

        let lonlat = match options {
            LabelOptions::Centered => {
                let mut lat = camera.get_center().lat().to_radians();
                if lat.abs() > 70.0_f64.to_radians() {
                    lat = lat.signum() * 70.0_f64.to_radians();
                }

                LonLatT::new(lon.to_angle(), lat.to_angle())
            }
            LabelOptions::OnSide => LonLatT::new(lon.to_angle(), lat.start.to_angle())     
        };

        let m1: Vector3<_> = lonlat.vector();
        let m2 = (m1 + d * 1e-3).normalize();

        //let s1 = projection.model_to_screen_space(&(system.to_icrs_j2000::<f64>() * m1), camera, reversed_longitude)?;
        let d1 = projection.model_to_screen_space(&m1.extend(1.0), camera)?;
        let d2 = projection.model_to_screen_space(&m2.extend(1.0), camera)?;

        //let s2 = projection.model_to_screen_space(&(system.to_icrs_j2000::<f64>() * m2), camera, reversed_longitude)?;
        let dt = (d2 - d1).normalize();
        let db = Vector2::new(dt.y.abs(), dt.x.abs());

        let mut lon = m1.lon().to_radians();
        if lon < 0.0 {
            lon += TWICE_PI;
        }

        let content = fmt.to_string(lon.to_angle());
        let position = if !fov.is_allsky() {
            d1 + OFF_TANGENT * dt - OFF_BI_TANGENT * db
        } else {
            d1
        };

        // rot is between -PI and +PI
        let rot = dt.y.signum() * dt.x.acos();

        Some(Label {
            position,
            content,
            rot,
        })
    }

    pub fn from_parallel(
        lat: f64,
        lon: &Range<f64>,
        options: LabelOptions,
        camera: &CameraViewPort,
        projection: &ProjectionType,
    ) -> Option<Self> {
        let lonlat = match options {
            LabelOptions::Centered => {
                let lon = camera.get_center().lon();
                LonLatT::new(lon, lat.to_angle())
            }
            LabelOptions::OnSide => LonLatT::new(lon.start.to_angle(), lat.to_angle())     
        };

        let m1: Vector3<_> = lonlat.vector();

        let mut t = Vector3::new(-m1.z, 0.0, m1.x).normalize();
        let center = camera.get_center().truncate();

        let dot_t_center = center.dot(t);
        if dot_t_center.abs() < 1e-4 {
            t = -t;
        } else {
            t = dot_t_center.signum() * t;
        }

        let m2 = (m1 + t * 1e-3).normalize();

        let d1 = projection.model_to_screen_space(&m1.extend(1.0), camera)?;
        let d2 = projection.model_to_screen_space(&m2.extend(1.0), camera)?;

        let dt = (d2 - d1).normalize();
        let db = Vector2::new(dt.y.abs(), dt.x.abs());

        let content = SerializeFmt::DMS.to_string(lonlat.lat());

        let fov = camera.get_field_of_view();
        let position = if !fov.is_allsky() && !fov.contains_pole() {
            d1 + OFF_TANGENT * dt - OFF_BI_TANGENT * db
        } else {
            d1
        };

        // rot is between -PI and +PI
        let rot = dt.y.signum() * dt.x.acos() + PI;


        Some(Label {
            position,
            content,
            rot,
        })
    }
}
