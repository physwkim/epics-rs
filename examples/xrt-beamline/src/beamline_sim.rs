//! XRT beamline simulation: Undulator → DCM Si(111) → HFM → VFM → Screen.
//!
//! Builds a beamline from current motor positions, runs ray tracing,
//! and returns the screen capture (2D intensity) plus beam statistics.

use xrt_core::consts::CH;
use xrt_materials::crystal::CrystalGeometry;
use xrt_materials::crystal_variants::CrystalSi;
use xrt_materials::data::ScatteringTable;
use xrt_materials::material::{Material, MaterialKind};
use xrt_oes::beamline::{Beamline, BeamlineOutput, OeParamsBuilder};
use xrt_oes::crystal_oe::CrystalOpticalElement;
use xrt_oes::material_oe::MaterialOpticalElement;
use xrt_oes::screen::{Screen, ScreenCapture};
use xrt_oes::surfaces::flat::FlatSurface;
use xrt_oes::surfaces::bent_flat::BentFlatSurface;
use xrt_sources::distributions::{EnergyDist, SpatialDist};
use xrt_sources::geometric::GeometricSource;

/// Beamline geometry constants (distances in mm).
pub struct BeamlineGeometry {
    /// Source to DCM distance [mm]
    pub source_to_dcm: f64,
    /// DCM crystal gap [mm]
    pub dcm_gap: f64,
    /// DCM to HFM distance [mm]
    pub dcm_to_hfm: f64,
    /// HFM to VFM distance [mm]
    pub hfm_to_vfm: f64,
    /// VFM to sample distance [mm]
    pub vfm_to_sample: f64,
}

impl Default for BeamlineGeometry {
    fn default() -> Self {
        Self {
            source_to_dcm: 25_000.0,  // 25 m
            dcm_gap: 15.0,            // 15 mm between crystals
            dcm_to_hfm: 2_000.0,      // 2 m
            hfm_to_vfm: 3_000.0,      // 3 m
            vfm_to_sample: 3_000.0,   // 3 m
        }
    }
}

/// Undulator parameters for energy calculation.
pub struct UndulatorConfig {
    /// Electron beam energy [GeV]
    pub electron_energy: f64,
    /// Undulator period [mm]
    pub period: f64,
    /// Maximum K parameter (at minimum gap)
    pub k_max: f64,
    /// Minimum gap [mm]
    pub gap_min: f64,
    /// Gap coefficient for exponential decay
    pub gap_coeff: f64,
}

impl Default for UndulatorConfig {
    fn default() -> Self {
        Self {
            electron_energy: 3.0,  // 3 GeV
            period: 10.0,          // 10 mm
            k_max: 2.5,
            gap_min: 5.0,          // 5 mm
            gap_coeff: std::f64::consts::PI,
        }
    }
}

impl UndulatorConfig {
    /// Calculate K parameter from gap.
    ///
    /// K = K_max * exp(-π * gap / period)  (simplified Halbach model)
    pub fn k_from_gap(&self, gap: f64) -> f64 {
        self.k_max * (-self.gap_coeff * gap / self.period).exp()
    }

    /// Calculate fundamental energy [eV] from gap [mm].
    ///
    /// E₁(keV) = 0.9496 * E_e²(GeV) / (λ_u(cm) * (1 + K²/2))
    /// E₁(eV)  = 9496 * E_e²(GeV) / (λ_u(mm) * (1 + K²/2))
    pub fn energy_from_gap(&self, gap: f64) -> f64 {
        let k = self.k_from_gap(gap);
        9496.0 * self.electron_energy * self.electron_energy / (self.period * (1.0 + k * k / 2.0))
    }
}

/// All motor positions for one simulation step.
#[derive(Debug, Clone)]
pub struct MotorPositions {
    // Undulator
    pub und_gap: f64,    // mm
    pub und_x: f64,      // mm
    pub und_z: f64,      // mm

    // DCM
    pub dcm_theta: f64,  // degrees
    pub dcm_theta2: f64, // arcsec offset
    pub dcm_y: f64,      // mm (crystal gap)
    pub dcm_chi1: f64,   // mrad
    pub dcm_chi2: f64,   // mrad
    pub dcm_z: f64,      // mm (translation)

    // HFM (Horizontally Focusing Mirror)
    pub hfm_pitch: f64,   // mrad (grazing angle)
    pub hfm_roll: f64,    // mrad
    pub hfm_yaw: f64,     // mrad
    pub hfm_x: f64,       // mm
    pub hfm_y: f64,       // mm
    pub hfm_z: f64,       // mm
    pub hfm_r_major: f64, // mm (meridional bending radius)
    pub hfm_r_minor: f64, // mm (sagittal radius)

    // VFM (Vertically Focusing Mirror)
    pub vfm_pitch: f64,   // mrad
    pub vfm_roll: f64,    // mrad
    pub vfm_yaw: f64,     // mrad
    pub vfm_x: f64,       // mm
    pub vfm_y: f64,       // mm
    pub vfm_z: f64,       // mm
    pub vfm_r_major: f64, // mm (bending radius)
    pub vfm_r_minor: f64, // mm (sagittal radius)
}

impl Default for MotorPositions {
    fn default() -> Self {
        Self {
            und_gap: 6.1,  // 8 keV at period=10mm
            und_x: 0.0,
            und_z: 0.0,
            dcm_theta: 14.31, // Si(111) Bragg angle for 8 keV
            dcm_theta2: 0.0,
            dcm_y: 15.0,
            dcm_chi1: 0.0,
            dcm_chi2: 0.0,
            dcm_z: 0.0,
            hfm_pitch: 3.0,
            hfm_roll: 0.0,
            hfm_yaw: 0.0,
            hfm_x: 0.0,
            hfm_y: 0.0,
            hfm_z: 0.0,
            // Coddington: R = 2*p*q / (sin(α)*(p+q))
            // HFM: p=27m, q=6m, α=3mrad → R=3.27km
            hfm_r_major: 3_272_727.0,
            hfm_r_minor: 1e9,          // no sagittal focusing
            vfm_pitch: 3.0,
            vfm_roll: 0.0,
            vfm_yaw: 0.0,
            vfm_x: 0.0,
            vfm_y: 0.0,
            vfm_z: 0.0,
            // VFM: p=30m, q=3m, α=3mrad → R=1.82km
            vfm_r_major: 1_818_182.0,
            vfm_r_minor: 1e9,          // no sagittal focusing
        }
    }
}

/// Simulation configuration.
pub struct SimConfig {
    pub nrays: usize,
    pub geometry: BeamlineGeometry,
    pub undulator: UndulatorConfig,
    /// Screen half-width [mm]
    pub screen_dx: f64,
    /// Screen half-height [mm]
    pub screen_dz: f64,
    /// Screen bins in x
    pub screen_nx: usize,
    /// Screen bins in z
    pub screen_nz: usize,
    /// Source divergence σ_x [rad]
    pub source_div_x: f64,
    /// Source divergence σ_z [rad]
    pub source_div_z: f64,
    /// Source size σ_x [mm]
    pub source_size_x: f64,
    /// Source size σ_z [mm]
    pub source_size_z: f64,
    /// Energy bandwidth ΔE/E for the source
    pub energy_bandwidth: f64,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            nrays: 5000,
            geometry: BeamlineGeometry::default(),
            undulator: UndulatorConfig::default(),
            screen_dx: 0.256,  // ±0.256 mm → 0.5 µm/pixel at 1024 bins
            screen_dz: 0.256,
            screen_nx: 1024,
            screen_nz: 1024,
            source_div_x: 50e-6,   // 50 µrad
            source_div_z: 20e-6,   // 20 µrad
            source_size_x: 0.3,    // 0.3 mm
            source_size_z: 0.02,   // 0.02 mm
            energy_bandwidth: 0.02, // 2% ΔE/E
        }
    }
}

/// Results from one simulation step.
pub struct SimResult {
    pub capture: ScreenCapture,
    pub beamline_output: BeamlineOutput,
    /// Photon energy at source [eV]
    pub source_energy: f64,
    /// DCM selected energy [eV]
    pub dcm_energy: f64,
}

/// Run one simulation step with the given motor positions.
pub fn simulate(config: &SimConfig, motors: &MotorPositions) -> SimResult {
    let geo = &config.geometry;

    // 1. Calculate photon energy from undulator gap
    let source_energy = config.undulator.energy_from_gap(motors.und_gap);

    // DCM selected energy from Bragg angle: E = hc / (2d·sin(θ))
    let theta_rad = motors.dcm_theta.to_radians();
    let si_d = 3.1356;  // Si(111) d-spacing [Å]
    let dcm_energy = CH / (2.0 * si_d * theta_rad.sin());

    // 2. Create source
    let de = source_energy * config.energy_bandwidth;
    let source = GeometricSource {
        center: [motors.und_x, 0.0, motors.und_z],
        nrays: config.nrays,
        dist_x: SpatialDist::Normal(config.source_size_x),
        dist_z: SpatialDist::Normal(config.source_size_z),
        dist_xprime: SpatialDist::Normal(config.source_div_x),
        dist_zprime: SpatialDist::Normal(config.source_div_z),
        dist_e: EnergyDist::Flat(source_energy - de / 2.0, source_energy + de / 2.0),
        ..Default::default()
    };
    let mut beam = source.shine();

    // 3. Drift to DCM
    beam.propagate(geo.source_to_dcm);

    // 4. Build DCM crystals
    let table = ScatteringTable::ChantlerTotal;

    // Create Si(111) crystal 1
    let si1 = CrystalSi::new(
        [1, 1, 1],
        297.15,
        CrystalGeometry::BraggReflected,
        1.0,
        None,
        0.0,
        table,
    );

    // Create Si(111) crystal 2
    let si2 = CrystalSi::new(
        [1, 1, 1],
        297.15,
        CrystalGeometry::BraggReflected,
        1.0,
        None,
        0.0,
        table,
    );

    // If crystal creation fails, fall back to geometric-only simulation
    let bl = match (si1, si2) {
        (Ok(crystal1), Ok(crystal2)) => {
            let theta2_rad = theta_rad + motors.dcm_theta2 / 3600.0_f64 * std::f64::consts::PI / 180.0;

            let xtal1_oe = CrystalOpticalElement::new(
                FlatSurface,
                OeParamsBuilder::new()
                    .center(0.0, geo.source_to_dcm, 0.0)
                    .pitch(theta_rad)
                    .roll(motors.dcm_chi1 * 1e-3)
                    .build(),
                crystal1.base.clone(),
            );

            let xtal2_oe = CrystalOpticalElement::new(
                FlatSurface,
                OeParamsBuilder::new()
                    .center(0.0, geo.source_to_dcm + motors.dcm_y, 0.0)
                    .pitch(-theta2_rad)
                    .roll(motors.dcm_chi2 * 1e-3)
                    .build(),
                crystal2.base.clone(),
            );

            let bl = Beamline::new()
                .add_crystal("DCM_1", xtal1_oe, Box::new(crystal1))
                .drift(motors.dcm_y)
                .add_crystal("DCM_2", xtal2_oe, Box::new(crystal2));

            bl
        }
        _ => {
            // Fallback: geometric flat mirrors for DCM
            let m1 = xrt_oes::oe::OpticalElement::new(
                FlatSurface,
                OeParamsBuilder::new()
                    .center(0.0, geo.source_to_dcm, 0.0)
                    .pitch(theta_rad)
                    .roll(motors.dcm_chi1 * 1e-3)
                    .build(),
            );
            let m2 = xrt_oes::oe::OpticalElement::new(
                FlatSurface,
                OeParamsBuilder::new()
                    .center(0.0, geo.source_to_dcm + motors.dcm_y, 0.0)
                    .pitch(-theta_rad)
                    .roll(motors.dcm_chi2 * 1e-3)
                    .build(),
            );
            let bl = Beamline::new()
                .add("DCM_1", m1)
                .drift(motors.dcm_y)
                .add("DCM_2", m2);

            bl
        }
    };

    // 5. Drift DCM → HFM
    let hfm_y = geo.source_to_dcm + motors.dcm_y + geo.dcm_to_hfm + motors.hfm_z;
    let bl = bl.drift(geo.dcm_to_hfm);

    // 6. HFM (Horizontally Focusing Mirror) with Si coating
    //    position_roll = π/2 rotates mirror to deflect horizontally
    let si_mirror = Material::new(
        &["Si"],
        None,
        2.33,
        MaterialKind::Mirror,
        None,
        ScatteringTable::ChantlerTotal,
    );

    let bl = match &si_mirror {
        Ok(mat) => {
            let mut hfm_params = OeParamsBuilder::new()
                .center(motors.hfm_x, hfm_y, motors.hfm_y)
                .pitch(motors.hfm_pitch * 1e-3)
                .roll(motors.hfm_roll * 1e-3)
                .yaw(motors.hfm_yaw * 1e-3)
                .build();
            hfm_params.position_roll = std::f64::consts::FRAC_PI_2;

            let hfm = MaterialOpticalElement::new(
                BentFlatSurface::new(motors.hfm_r_major, 0.0),
                hfm_params,
                mat.clone(),
            );
            bl.add_material("HFM", hfm)
        }
        Err(_) => {
            let mut hfm_params = OeParamsBuilder::new()
                .center(motors.hfm_x, hfm_y, motors.hfm_y)
                .pitch(motors.hfm_pitch * 1e-3)
                .build();
            hfm_params.position_roll = std::f64::consts::FRAC_PI_2;

            let hfm = xrt_oes::oe::OpticalElement::new(
                BentFlatSurface::new(motors.hfm_r_major, 0.0),
                hfm_params,
            );
            bl.add("HFM", hfm)
        }
    };

    // 7. Drift HFM → VFM
    let vfm_y = hfm_y + geo.hfm_to_vfm + motors.vfm_z;
    let bl = bl.drift(geo.hfm_to_vfm);

    // 8. VFM (Vertically Focusing Mirror) with Si coating
    let bl = match &si_mirror {
        Ok(mat) => {
            let vfm = MaterialOpticalElement::new(
                BentFlatSurface::new(motors.vfm_r_major, 0.0),
                OeParamsBuilder::new()
                    .center(motors.vfm_x, vfm_y, motors.vfm_y)
                    .pitch(motors.vfm_pitch * 1e-3)
                    .roll(motors.vfm_roll * 1e-3)
                    .yaw(motors.vfm_yaw * 1e-3)
                    .build(),
                mat.clone(),
            );
            bl.add_material("VFM", vfm)
        }
        Err(_) => {
            let vfm = xrt_oes::oe::OpticalElement::new(
                BentFlatSurface::new(motors.vfm_r_major, 0.0),
                OeParamsBuilder::new()
                    .center(motors.vfm_x, vfm_y, motors.vfm_y)
                    .pitch(motors.vfm_pitch * 1e-3)
                    .roll(motors.vfm_roll * 1e-3)
                    .yaw(motors.vfm_yaw * 1e-3)
                    .build(),
            );
            bl.add("VFM", vfm)
        }
    };

    // 9. Drift VFM → Sample
    let bl = bl.drift(geo.vfm_to_sample);

    // 10. Propagate
    let beamline_output = bl.propagate(&mut beam);

    // 11. Capture on screen at sample position
    //     Account for HFM horizontal deflection and VFM vertical deflection
    let hfm_defl = 2.0 * motors.hfm_pitch * 1e-3; // rad
    let vfm_defl = 2.0 * motors.vfm_pitch * 1e-3;  // rad
    let sample_y = vfm_y + geo.vfm_to_sample;
    let sample_x = hfm_defl * (geo.hfm_to_vfm + geo.vfm_to_sample);
    let sample_z = motors.dcm_y - vfm_defl * geo.vfm_to_sample;
    let screen = Screen::new(
        [sample_x, sample_y, sample_z],
        config.screen_dx,
        config.screen_dz,
        config.screen_nx,
        config.screen_nz,
    );
    let capture = screen.capture(&beam);

    SimResult {
        capture,
        beamline_output,
        source_energy,
        dcm_energy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_undulator_energy() {
        let und = UndulatorConfig::default();
        // At 15mm gap with default params
        let e = und.energy_from_gap(15.0);
        assert!(e > 1000.0 && e < 50000.0, "energy={e} eV should be reasonable");
    }

    #[test]
    fn test_simulate_default() {
        let config = SimConfig {
            nrays: 500,
            ..Default::default()
        };
        let motors = MotorPositions::default();
        let result = simulate(&config, &motors);
        assert!(result.source_energy > 0.0);
        assert!(result.dcm_energy > 0.0);
    }
}
