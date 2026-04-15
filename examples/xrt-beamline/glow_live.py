#!/usr/bin/env python
"""3D beamline viewer using xrtGlow.

The DCM is modeled as two explicit crystals so xrtGlow can draw both
reflections instead of collapsing them into one ``double_reflect`` step.
"""

import sys
import numpy as np

import xrt.backends.raycing as raycing
import xrt.backends.raycing.sources as rs
import xrt.backends.raycing.oes as roe
import xrt.backends.raycing.screens as rsc
import xrt.backends.raycing.materials as rm
import xrt.backends.raycing.run as rr
import xrt.gui.xrtGlow as xrtglow

try:
    from xrt.gui.xrtGlow import qt
except ImportError:
    from PyQt5 import QtWidgets as qt

DCM_ENERGY = 12398.42 / (2 * 3.1356 * np.sin(np.radians(14.31)))
NRAYS = 10000

si1 = rm.CrystalSi(hkl=(1, 1, 1), tK=297.15)
si2 = rm.CrystalSi(hkl=(1, 1, 1), tK=297.15)
mir_si = rm.Material('Si', rho=2.33, kind='mirror')
DCM_FIXED_OFFSET = 15.0


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


def build_beamline():
    bl = raycing.BeamLine(alignE=DCM_ENERGY)

    bl.src = rs.GeometricSource(
        bl, name='Source', center=[0, 0, 0], nrays=NRAYS,
        distx='normal', dx=0.3, distz='normal', dz=0.02,
        distxprime='normal', dxprime=50e-6,
        distzprime='normal', dzprime=20e-6,
        distE='flat',
        energies=(DCM_ENERGY * 0.99, DCM_ENERGY * 1.01),
    )

    bl.dcm1 = roe.OE(
        bl, name='DCM1', center=[0, 25000, 0],
        material=(si1,), pitch='auto',
        limPhysX=(-30, 30), limPhysY=(-15, 15),
    )

    # Let xrt auto-align the second crystal to the outgoing beam from DCM1.
    # A y-shift by the fixed exit offset reproduces the visible DCM dogleg
    # while exposing both reflections to xrtGlow.
    bl.dcm2 = roe.OE(
        bl, name='DCM2', center=['auto', 25000 + DCM_FIXED_OFFSET, 'auto'],
        material=(si2,), pitch='auto', positionRoll=np.pi,
        limPhysX=(-30, 30), limPhysY=(-15, 15),
    )

    bl.hfm = roe.BentFlatMirror(
        bl, name='HFM', center=['auto', 27000, 'auto'],
        material=(mir_si,), pitch=3e-3, R=3272727,
        positionRoll=np.pi / 2,
        limPhysX=(-50, 50), limPhysY=(-600, 600),
    )

    bl.vfm = roe.BentFlatMirror(
        bl, name='VFM', center=['auto', 30000, 'auto'],
        material=(mir_si,), pitch=3e-3, R=1818182,
        positionRoll=np.pi,
        limPhysX=(-50, 50), limPhysY=(-400, 400),
    )

    bl.sample = rsc.Screen(
        bl, name='Sample', center=['auto', 33000, 'auto'])

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
    bl.prepare_flow()
    return outDict

rr.run_process = run_process


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
    glow.setWindowTitle("XRT Beamline")
    glow.show()

    sys.exit(app.exec_())


if __name__ == '__main__':
    main()
