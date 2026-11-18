//! Transverse Mercator on the WGS84 ellipsoid (Snyder 1987, pp. 60-61),
//! centred on the data with a scale factor of 1. Over a city or a county the
//! distortion is negligible at line-printer resolution.

const A: f64 = 6_378_137.0;
const F: f64 = 1.0 / 298.257_223_563;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransverseMercator {
    pub lon0: f64,
    pub lat0: f64,
}

fn meridian_arc(phi: f64) -> f64 {
    let e2 = F * (2.0 - F);
    let (e4, e6) = (e2 * e2, e2 * e2 * e2);
    A * ((1.0 - e2 / 4.0 - 3.0 * e4 / 64.0 - 5.0 * e6 / 256.0) * phi
        - (3.0 * e2 / 8.0 + 3.0 * e4 / 32.0 + 45.0 * e6 / 1024.0) * (2.0 * phi).sin()
        + (15.0 * e4 / 256.0 + 45.0 * e6 / 1024.0) * (4.0 * phi).sin()
        - (35.0 * e6 / 3072.0) * (6.0 * phi).sin())
}

impl TransverseMercator {
    /// Eastings and northings in metres from the projection's origin.
    pub fn forward(&self, lon: f64, lat: f64) -> (f64, f64) {
        let e2 = F * (2.0 - F);
        let ep2 = e2 / (1.0 - e2);
        let phi = lat.to_radians();
        let (s, c) = phi.sin_cos();
        let n = A / (1.0 - e2 * s * s).sqrt();
        let t = (s / c).powi(2);
        let cc = ep2 * c * c;
        let a = (lon - self.lon0).to_radians() * c;
        let x = n
            * (a + (1.0 - t + cc) * a.powi(3) / 6.0
                + (5.0 - 18.0 * t + t * t + 72.0 * cc - 58.0 * ep2) * a.powi(5) / 120.0);
        let y = meridian_arc(phi) - meridian_arc(self.lat0.to_radians())
            + n * (s / c)
                * (a * a / 2.0
                    + (5.0 - t + 9.0 * cc + 4.0 * cc * cc) * a.powi(4) / 24.0
                    + (61.0 - 58.0 * t + t * t + 600.0 * cc - 330.0 * ep2) * a.powi(6) / 720.0);
        (x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_and_axes() {
        let tm = TransverseMercator {
            lon0: -0.1,
            lat0: 51.5,
        };
        assert_eq!(tm.forward(-0.1, 51.5), (0.0, 0.0));
        // Along the central meridian, y is the meridian arc (half a degree
        // north of 51.5 N, per PROJ).
        let (x, y) = tm.forward(-0.1, 52.0);
        assert!(x.abs() < 1e-6);
        assert!((y - 55_631.298).abs() < 0.01, "{y}");
        // Across: a parallel's radius is N cos(phi).
        let (x, _) = tm.forward(0.0, 51.5);
        let e2 = F * (2.0 - F);
        let s = 51.5f64.to_radians().sin();
        let expect =
            A / (1.0 - e2 * s * s).sqrt() * 51.5f64.to_radians().cos() * 0.1f64.to_radians();
        assert!((x - expect).abs() < 0.01, "{x} {expect}");
        // Symmetric about the central meridian.
        let (xw, yw) = tm.forward(-0.3, 51.6);
        let (xe, ye) = tm.forward(0.1, 51.6);
        assert!((xw + xe).abs() < 1e-6 && (yw - ye).abs() < 1e-6);
    }

    #[test]
    fn matches_proj() {
        // PROJ 9 (pyproj 3.8), +proj=tmerc +lat_0=51.5 +lon_0=-0.1 +k=1
        // +ellps=WGS84.
        let tm = TransverseMercator {
            lon0: -0.1,
            lat0: 51.5,
        };
        let (x, y) = tm.forward(0.3, 51.9);
        assert!((x - 27_532.323).abs() < 0.01, "{x}");
        assert!((y - 44_580.288).abs() < 0.01, "{y}");
        let (x, y) = tm.forward(0.0, 51.5);
        assert!((x - 6_944.051).abs() < 0.01 && (y - 4.742).abs() < 0.01);
    }

    #[test]
    fn matches_utm() {
        // UTM zone 30 (EPSG:32630, via PROJ): 1 W, 51 N is easting
        // 640,333.30 and northing 5,651,728.68 (k0 = 0.9996, false easting
        // 500 km).
        let tm = TransverseMercator {
            lon0: -3.0,
            lat0: 0.0,
        };
        let (x, y) = tm.forward(-1.0, 51.0);
        assert!((x * 0.9996 + 500_000.0 - 640_333.296).abs() < 0.01, "{x}");
        assert!((y * 0.9996 - 5_651_728.683).abs() < 0.01, "{y}");
    }
}
