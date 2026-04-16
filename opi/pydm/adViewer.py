"""
AD Viewer — side-by-side CA (image1) + PVA (Pva1) image viewer.

Run:
    pydm examples/sim-detector/opi/pydm/adViewer.py --macro "P=SIM1:,R=cam1:,IMAGE=image1:,PVA=Pva1:,PVA_PV=SIM1:Pva1:Image"

Macros (all optional, defaults shown):
    P       — camera PV prefix            (SIM1:)
    R       — camera record suffix        (cam1:)
    IMAGE   — CA image plugin prefix      (image1:)
    PVA     — PVA plugin prefix           (Pva1:)
    PVA_PV  — PVA-published image PV name (SIM1:Pva1:Image)

Controls:
    - Start / Stop acquisition (cam1:Acquire)
    - Enable / disable image1 plugin       (image1:EnableCallbacks)
    - Enable / disable Pva1 plugin         (Pva1:EnableCallbacks)
    - Live CA viewer (ca://...image1:ArrayData)
    - Live PVA viewer (pva://...Pva1:Image)
"""

from typing import Optional

from pydm import Display
from pydm.widgets import (
    PyDMImageView,
    PyDMLabel,
    PyDMLineEdit,
    PyDMPushButton,
    PyDMShellCommand,
)
from qtpy.QtCore import Qt
from qtpy.QtWidgets import (
    QCheckBox,
    QGridLayout,
    QGroupBox,
    QHBoxLayout,
    QLabel,
    QVBoxLayout,
    QWidget,
)


# ──────────────────────────────────────────────────────────────────────
# Helper: a PyDM checkbox that drives an asynBo-style EnableCallbacks PV.
# PyDM doesn't ship PyDMCheckbox, so we use QCheckBox + PyDMPushButton pair.
# ──────────────────────────────────────────────────────────────────────
class EnableToggle(QWidget):
    """Checkbox that writes 1/0 to `{prefix}EnableCallbacks` and reflects
    `{prefix}EnableCallbacks_RBV` via a side label."""

    def __init__(self, label: str, write_pv: str, rbv_pv: str, parent=None):
        super().__init__(parent)
        row = QHBoxLayout(self)
        row.setContentsMargins(0, 0, 0, 0)

        # Invisible pushbuttons do the actual writes; checkbox just clicks them.
        self._on = PyDMPushButton(
            init_channel=write_pv, pressValue=1, label="Enable"
        )
        self._off = PyDMPushButton(
            init_channel=write_pv, pressValue=0, label="Disable"
        )
        self._on.setVisible(False)
        self._off.setVisible(False)

        self._cb = QCheckBox(label)
        self._cb.toggled.connect(self._toggled)
        row.addWidget(self._cb)

        rbv = PyDMLabel(init_channel=rbv_pv)
        rbv.setMinimumWidth(60)
        rbv.setAlignment(Qt.AlignCenter)
        row.addWidget(rbv)
        row.addStretch(1)

        row.addWidget(self._on)
        row.addWidget(self._off)

    def _toggled(self, checked: bool):
        (self._on if checked else self._off).sendValue()


class ADViewer(Display):
    def __init__(self, parent=None, args=None, macros=None):
        super().__init__(parent=parent, args=args, macros=macros)

        m = macros or {}
        prefix = m.get("P", "SIM1:") + m.get("R", "cam1:")
        image_prefix = m.get("P", "SIM1:") + m.get("IMAGE", "image1:")
        pva_prefix = m.get("P", "SIM1:") + m.get("PVA", "Pva1:")
        pva_published = m.get("PVA_PV", f"{m.get('P', 'SIM1:')}Pva1:Image")

        self.setWindowTitle(f"AD Viewer — {prefix}")

        root = QVBoxLayout(self)
        root.setContentsMargins(8, 8, 8, 8)
        root.setSpacing(8)

        # ── Controls row ──────────────────────────────────────────────
        controls = QGroupBox("Controls")
        cgrid = QGridLayout(controls)

        cgrid.addWidget(QLabel("Acquire:"), 0, 0)
        start = PyDMPushButton(
            init_channel=f"ca://{prefix}Acquire", pressValue=1, label="Start"
        )
        stop = PyDMPushButton(
            init_channel=f"ca://{prefix}Acquire", pressValue=0, label="Stop"
        )
        acquire_rbv = PyDMLabel(init_channel=f"ca://{prefix}Acquire_RBV")
        acquire_rbv.setMinimumWidth(80)
        acquire_rbv.setAlignment(Qt.AlignCenter)
        cgrid.addWidget(start, 0, 1)
        cgrid.addWidget(stop, 0, 2)
        cgrid.addWidget(QLabel("State:"), 0, 3)
        cgrid.addWidget(acquire_rbv, 0, 4)

        cgrid.addWidget(QLabel("Exposure (s):"), 1, 0)
        exp = PyDMLineEdit(init_channel=f"ca://{prefix}AcquireTime")
        exp.setMinimumWidth(100)
        cgrid.addWidget(exp, 1, 1)
        exp_rbv = PyDMLabel(init_channel=f"ca://{prefix}AcquireTime_RBV")
        cgrid.addWidget(exp_rbv, 1, 2)

        cgrid.addWidget(QLabel("Rate (Hz):"), 1, 3)
        rate_rbv = PyDMLabel(init_channel=f"ca://{prefix}ArrayRate_RBV")
        cgrid.addWidget(rate_rbv, 1, 4)

        # Plugin enables
        cgrid.addWidget(
            EnableToggle(
                "image1 (CA)",
                write_pv=f"ca://{image_prefix}EnableCallbacks",
                rbv_pv=f"ca://{image_prefix}EnableCallbacks_RBV",
            ),
            2,
            0,
            1,
            5,
        )
        cgrid.addWidget(
            EnableToggle(
                "Pva1 (PVA)",
                write_pv=f"ca://{pva_prefix}EnableCallbacks",
                rbv_pv=f"ca://{pva_prefix}EnableCallbacks_RBV",
            ),
            3,
            0,
            1,
            5,
        )

        root.addWidget(controls)

        # ── Image viewers row ─────────────────────────────────────────
        viewers = QHBoxLayout()
        viewers.setSpacing(8)

        viewers.addWidget(
            self._image_panel(
                title=f"image1 (CA) — ca://{image_prefix}ArrayData",
                image_channel=f"ca://{image_prefix}ArrayData",
                width_channel=f"ca://{image_prefix}ArraySize0_RBV",
                height_channel=f"ca://{image_prefix}ArraySize1_RBV",
            )
        )
        viewers.addWidget(
            self._image_panel(
                title=f"Pva1 (PVA) — pva://{pva_published}",
                image_channel=f"pva://{pva_published}",
                width_channel=None,  # PVA carries dimensions in NDArray struct
                height_channel=None,
            )
        )
        root.addLayout(viewers, stretch=1)

        # Bottom: quick iocsh/edm/pvmonitor shortcuts (optional, nice to have)
        tools = QHBoxLayout()
        tools.addStretch(1)
        tools.addWidget(
            PyDMShellCommand(
                command=f"caget {prefix}ArrayCounter_RBV", title="caget counter"
            )
        )
        tools.addWidget(
            PyDMShellCommand(
                command=f"pvget {pva_published}", title="pvget Pva1"
            )
        )
        root.addLayout(tools)

    def _image_panel(
        self,
        title: str,
        image_channel: str,
        width_channel: Optional[str],
        height_channel: Optional[str],
    ) -> QWidget:
        box = QGroupBox(title)
        layout = QVBoxLayout(box)

        view = PyDMImageView(image_channel=image_channel)
        if width_channel:
            view.widthChannel = width_channel
        if height_channel:
            # PyDMImageView uses reading_order + shape from width+height
            pass
        view.readingOrder = PyDMImageView.Clike
        view.setMinimumSize(400, 300)

        # Unhide pyqtgraph's built-in histogram / LUT / ROI / menu widgets.
        # PyDMImageView hides these by default; surfacing them gives the user
        # in-view controls for data range (levels), color map, ROI, and
        # auto-range toggles.
        for attr in ("histogram", "roiBtn", "menuBtn"):
            w = getattr(view.ui, attr, None)
            if w is not None:
                w.show()

        layout.addWidget(view)

        meta = QLabel()
        meta.setAlignment(Qt.AlignCenter)
        layout.addWidget(meta)

        return box

    def ui_filepath(self):
        # Display built entirely in Python; no .ui file.
        return None

    def ui_filename(self):
        return None
