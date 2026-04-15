#!/usr/bin/env python
"""3D beamline viewer using xrtGlow.

The DCM is modeled as two explicit crystals so xrtGlow can draw both
reflections instead of collapsing them into one ``double_reflect`` step.
"""

import sys
import numpy as np
import matplotlib
matplotlib.use('Qt5Agg')
import matplotlib.pyplot as plt
from matplotlib.backends.backend_qt5agg import FigureCanvasQTAgg

import xrt.backends.raycing as raycing
import xrt.backends.raycing.sources as rs
import xrt.backends.raycing.oes as roe
import xrt.backends.raycing.screens as rsc
import xrt.backends.raycing.materials as rm
import xrt.backends.raycing.run as rr
import xrt.gui.xrtGlow as xrtglow

try:
    import epics
except ImportError:
    epics = None

try:
    from xrt.gui.xrtGlow import qt
    qtcore = qt
except ImportError:
    from PyQt5 import QtWidgets as qt
    from PyQt5 import QtCore as qtcore

NRAYS = 10000
Y_DCM = 25000.0
Y_HFM = 27000.0
Y_VFM = 30000.0
Y_SAMPLE = 33000.0

si1 = rm.CrystalSi(hkl=(1, 1, 1), tK=297.15)
si2 = rm.CrystalSi(hkl=(1, 1, 1), tK=297.15)
mir_si = rm.Material('Si', rho=2.33, kind='mirror')
DCM_FIXED_OFFSET = 15.0

DEFAULT_MOTORS = {
    'und_gap': 6.1,
    'und_x': 0.0,
    'und_z': 0.0,
    'dcm_theta': 14.31,
    'dcm_theta2': 0.0,
    'dcm_y': 15.0,
    'dcm_chi1': 0.0,
    'dcm_chi2': 0.0,
    'dcm_z': 0.0,
    'hfm_pitch': 3.0,
    'hfm_roll': 0.0,
    'hfm_yaw': 0.0,
    'hfm_x': 0.0,
    'hfm_y': 0.0,
    'hfm_z': 0.0,
    'hfm_rmaj': 3272727.0,
    'hfm_rmin': 1e9,
    'vfm_pitch': 3.0,
    'vfm_roll': 0.0,
    'vfm_yaw': 0.0,
    'vfm_x': 0.0,
    'vfm_y': 0.0,
    'vfm_z': 0.0,
    'vfm_rmaj': 1818182.0,
    'vfm_rmin': 1e9,
}

PV_NAMES = {
    key: f'bl:{key.replace("_", ":")}.RBV' for key in DEFAULT_MOTORS
}


def dcm_energy_from_theta(theta_deg):
    return 12398.42 / (2 * 3.1356 * np.sin(np.radians(theta_deg)))


def maybe_auto(value, tol=1e-12):
    return 'auto' if abs(value) <= tol else value


def glow_only_good(beam, extra_good=None):
    """Return a beam copy that only keeps rays xrtGlow should display."""
    out = rs.Beam(copyFrom=beam)
    good = (out.state == 1) | (out.state == 2)
    if extra_good is not None:
        good &= extra_good(out)
    bad = ~good
    if np.any(bad):
        out.state[bad] = 0
        for field in ('x', 'y', 'z', 'a', 'b', 'c'):
            arr = getattr(out, field)
            arr[bad] = np.nan
    return out


def build_beamline(motors=None):
    motors = dict(DEFAULT_MOTORS if motors is None else motors)
    dcm_energy = dcm_energy_from_theta(motors['dcm_theta'])
    theta1 = np.radians(motors['dcm_theta'])
    theta2 = theta1 + np.radians(motors['dcm_theta2'] / 3600.0)

    bl = raycing.BeamLine(alignE=dcm_energy)
    bl.motors = motors

    bl.src = rs.GeometricSource(
        bl, name='Source',
        center=[motors['und_x'], 0.0, motors['und_z']], nrays=NRAYS,
        distx='normal', dx=0.3, distz='normal', dz=0.02,
        distxprime='normal', dxprime=50e-6,
        distzprime='normal', dzprime=20e-6,
        distE='flat',
        energies=(dcm_energy * 0.99, dcm_energy * 1.01),
    )

    bl.dcm1 = roe.OE(
        bl, name='DCM1', center=[0, Y_DCM, motors['dcm_z']],
        material=(si1,), pitch=theta1, roll=motors['dcm_chi1'] * 1e-3,
        limPhysX=(-30, 30), limPhysY=(-15, 15),
    )

    bl.dcm2 = roe.OE(
        bl, name='DCM2',
        center=['auto', Y_DCM + motors['dcm_y'], 'auto'],
        material=(si2,), pitch=-theta2, roll=motors['dcm_chi2'] * 1e-3,
        positionRoll=np.pi,
        limPhysX=(-30, 30), limPhysY=(-15, 15),
    )

    bl.hfm = roe.BentFlatMirror(
        bl, name='HFM',
        center=[maybe_auto(motors['hfm_x']),
                Y_HFM + motors['hfm_z'],
                maybe_auto(motors['hfm_y'])],
        material=(mir_si,), pitch=motors['hfm_pitch'] * 1e-3,
        roll=motors['hfm_roll'] * 1e-3,
        yaw=motors['hfm_yaw'] * 1e-3,
        R=motors['hfm_rmaj'], positionRoll=np.pi / 2,
        limPhysX=(-50, 50), limPhysY=(-600, 600),
    )

    bl.vfm = roe.BentFlatMirror(
        bl, name='VFM',
        center=[maybe_auto(motors['vfm_x']),
                Y_VFM + motors['vfm_z'],
                maybe_auto(motors['vfm_y'])],
        material=(mir_si,), pitch=motors['vfm_pitch'] * 1e-3,
        roll=motors['vfm_roll'] * 1e-3,
        yaw=motors['vfm_yaw'] * 1e-3,
        R=motors['vfm_rmaj'], positionRoll=np.pi,
        limPhysX=(-50, 50), limPhysY=(-400, 400),
    )

    bl.sample = rsc.Screen(
        bl, name='Sample', center=['auto', Y_SAMPLE, 'auto'])

    return bl


def run_process(bl):
    beam = bl.src.shine()
    bd1, bl1 = bl.dcm1.reflect(beam)
    bg, bl2 = bl.dcm2.reflect(bd1)
    bh, bhl = bl.hfm.reflect(bg)
    bv, bvl = bl.vfm.reflect(bh)
    bs = bl.sample.expose(bv)

    # Keep the real beams for propagation, but give xrtGlow cleaned copies so
    # non-intersecting/lost rays do not show up as stray beam branches.
    beam_g = glow_only_good(beam)
    bd1_g = glow_only_good(bd1)
    bg_g = glow_only_good(bg)
    bh_g = glow_only_good(bh, extra_good=lambda b: b.a > 2e-3)
    bv_g = glow_only_good(bv, extra_good=lambda b: (b.a > 2e-3) & (b.c < -2e-3))
    bs_g = glow_only_good(bs, extra_good=lambda b: (b.a > 2e-3) & (b.c < -2e-3))

    outDict = {
        'beam': beam_g, 'bd1': bd1_g, 'bg': bg_g, 'bl1': bl1, 'bl2': bl2,
        'bh': bh_g, 'bhl': bhl, 'bv': bv_g, 'bvl': bvl, 'bs': bs_g,
    }
    bl._raw_sample_beam = bs  # unfiltered beam for 2D profile
    bl.prepare_flow()
    return outDict

rr.run_process = run_process


class SampleView(qt.QWidget):
    """Real-time 2D beam profile at sample position."""

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setWindowTitle("Sample Beam Profile")
        self.resize(500, 500)

        self.fig, self.axes = plt.subplots(2, 2, figsize=(6, 6),
            gridspec_kw={'width_ratios': [3, 1], 'height_ratios': [1, 3],
                         'hspace': 0.05, 'wspace': 0.05})
        self.ax_main = self.axes[1, 0]
        self.ax_xproj = self.axes[0, 0]
        self.ax_zproj = self.axes[1, 1]
        self.axes[0, 1].set_visible(False)

        self.ax_xproj.tick_params(labelbottom=False)
        self.ax_zproj.tick_params(labelleft=False)

        self.canvas = FigureCanvasQTAgg(self.fig)
        layout = qt.QVBoxLayout(self)
        self.info_label = qt.QLabel("")
        self.info_label.setStyleSheet("font-family: monospace; font-size: 11px;")
        layout.addWidget(self.info_label)
        layout.addWidget(self.canvas)

    def update_beam(self, beam, energy=0.0):
        good = beam.state == 1
        if good.sum() == 0:
            self.info_label.setText("No good rays at sample")
            return

        x = beam.x[good] * 1e3  # mm → µm
        z = beam.z[good] * 1e3

        # Robust outlier removal using MAD (median absolute deviation)
        def mad_filter(arr, threshold=5.0):
            med = np.median(arr)
            mad = np.median(np.abs(arr - med))
            if mad < 0.01:
                mad = 0.01
            return np.abs(arr - med) < threshold * mad * 1.4826

        mask = mad_filter(x) & mad_filter(z)
        x, z = x[mask], z[mask]
        n_good = len(x)
        if n_good == 0:
            self.info_label.setText("No good rays after filtering")
            return

        # Center on beam and auto-range based on IQR
        cx, cz = np.median(x), np.median(z)
        x = x - cx  # center the data
        z = z - cz
        iqr_x = np.percentile(np.abs(x), 95)
        iqr_z = np.percentile(np.abs(z), 95)
        xlim = max(iqr_x * 2, 5)  # at least ±5 µm
        zlim = max(iqr_z * 2, 5)

        # 2D histogram
        bins = 128
        H, xe, ze = np.histogram2d(x, z, bins=bins,
                                    range=[[-xlim, xlim], [-zlim, zlim]])

        self.ax_main.cla()
        self.ax_main.imshow(H.T, origin='lower', aspect='auto',
                            extent=[-xlim, xlim, -zlim, zlim],
                            cmap='inferno', interpolation='nearest')
        self.ax_main.set_xlabel(u'x (µm)')
        self.ax_main.set_ylabel(u'z (µm)')

        # X projection
        self.ax_xproj.cla()
        xc = 0.5 * (xe[:-1] + xe[1:])
        self.ax_xproj.fill_between(xc, H.sum(axis=1), color='C0', alpha=0.7)
        self.ax_xproj.set_xlim(-xlim, xlim)
        self.ax_xproj.set_ylabel('counts')

        # Z projection
        self.ax_zproj.cla()
        zc = 0.5 * (ze[:-1] + ze[1:])
        self.ax_zproj.fill_betweenx(zc, H.sum(axis=0), color='C1', alpha=0.7)
        self.ax_zproj.set_ylim(-zlim, zlim)
        self.ax_zproj.set_xlabel('counts')

        # FWHM
        def fwhm(proj, centers):
            peak = proj.max()
            if peak <= 0:
                return 0.0
            half = peak * 0.5
            above = centers[proj >= half]
            return float(above[-1] - above[0]) if len(above) >= 2 else 0.0

        fwhm_x = fwhm(H.sum(axis=1), xc)
        fwhm_z = fwhm(H.sum(axis=0), zc)

        self.info_label.setText(
            f"E={energy:.1f} eV  |  N_good={n_good}  |  "
            f"FWHM x={fwhm_x:.1f} µm  z={fwhm_z:.1f} µm  |  "
            f"center x={cx:.1f} z={cz:.1f} µm"
        )

        self.canvas.draw_idle()


class PvUpdater(qtcore.QObject):
    refresh_requested = qtcore.pyqtSignal()

    def __init__(self, glow, sample_view=None, debounce_ms=50):
        super().__init__(glow)
        self.glow = glow
        self.sample_view = sample_view
        self.last = None
        self.pvs = {}
        self.debounce_ms = debounce_ms
        self.pending = False
        self.pending_changes = set()
        self.timer = qtcore.QTimer(self)
        self.timer.setSingleShot(True)
        self.timer.timeout.connect(self.refresh)
        self.refresh_requested.connect(self.schedule_refresh)
        if epics is not None:
            self.pvs = {
                key: epics.PV(
                    name,
                    auto_monitor=True,
                    connection_timeout=0.05,
                    connection_callback=lambda conn=None, pvname=None, key=key, **kw:
                        self.on_connection_event(key, conn=conn, pvname=pvname, **kw),
                )
                for key, name in PV_NAMES.items()
            }
            for key, pv in self.pvs.items():
                pv.add_callback(callback=lambda key=key, **kw: self.on_pv_change(key, **kw))

    def snapshot(self):
        motors = dict(DEFAULT_MOTORS)
        if not self.pvs:
            return motors
        for key, pv in self.pvs.items():
            try:
                value = pv.get(timeout=0.01, use_monitor=True)
            except Exception:
                value = None
            if value is not None:
                motors[key] = float(value)
        return motors

    def log_differences_vs_defaults(self):
        if not self.pvs:
            print("EPICS PV support unavailable. Using internal default motor values only.")
            return
        pv_motors = self.snapshot()
        diffs = [
            key for key in DEFAULT_MOTORS
            if abs(pv_motors[key] - DEFAULT_MOTORS[key]) > 1e-9
        ]
        if not diffs:
            print("PV/default comparison: all current PV values match internal defaults.")
            return
        print("PV/default comparison:")
        for key in diffs:
            print(
                f"  {PV_NAMES[key]}: "
                f"default={DEFAULT_MOTORS[key]:.6g}, "
                f"pv={pv_motors[key]:.6g}"
            )

    def on_connection_event(self, key, conn=None, **_kwargs):
        pass

    def on_pv_change(self, key, **_kwargs):
        self.pending_changes.add(key)
        self.refresh_requested.emit()

    def schedule_refresh(self):
        if self.pending:
            return
        self.pending = True
        self.timer.start(self.debounce_ms)

    def refresh(self):
        self.pending = False
        motors = self.snapshot()
        changed_keys = []
        if self.last is not None:
            changed_keys = [
                key for key in DEFAULT_MOTORS
                if abs(motors[key] - self.last[key]) > 1e-9
            ]
            if changed_keys:
                print("PV update detected:")
                for key in changed_keys:
                    print(
                        f"  {PV_NAMES[key]}: "
                        f"{self.last[key]:.6g} -> {motors[key]:.6g}"
                    )
                print(
                    f"Recalculating beamline for {len(changed_keys)} motor update(s)..."
                )
        self.last = motors
        self.pending_changes.clear()

        bl = build_beamline(motors)
        outDict = run_process(bl)
        ray_path = bl.export_to_glow()
        self.glow.bl = bl
        self.glow.updateOEsList(ray_path)
        self.glow.customGlWidget.glDraw()

        energy = dcm_energy_from_theta(motors['dcm_theta'])
        self.glow.setWindowTitle(f"XRT Beamline  E={energy:.1f} eV")

        # Update 2D sample view with unfiltered beam
        if self.sample_view is not None:
            bs = getattr(bl, '_raw_sample_beam', None)
            if bs is not None:
                self.sample_view.update_beam(bs, energy=energy)


def main():
    app = qt.QApplication(sys.argv)

    bl = build_beamline()
    print("Running ray tracing...")
    run_process(bl)

    print("Exporting to glow...")
    arrayOfRays = bl.export_to_glow()

    print(f"rayPath: {len(arrayOfRays[0])} segments, "
          f"beams: {len(arrayOfRays[1])}, "
          f"oes: {list(arrayOfRays[2].keys())}")

    glow = xrtglow.xrtGlow(arrayOfRays)
    glow.bl = bl
    glow.setWindowTitle("XRT Beamline")

    sample_view = SampleView()
    sample_view.show()

    glow.pv_updater = PvUpdater(glow, sample_view=sample_view)
    glow.pv_updater.log_differences_vs_defaults()
    glow.pv_updater.refresh()
    glow.show()

    sys.exit(app.exec_())


if __name__ == '__main__':
    main()
