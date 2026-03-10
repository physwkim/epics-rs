epicsEnvSet("PREFIX", "mini:")

# ===== Beam current parameters =====
epicsEnvSet("BEAM_OFFSET",    "500.0")
epicsEnvSet("BEAM_AMPLITUDE", "25.0")
epicsEnvSet("BEAM_PERIOD",    "4.0")
epicsEnvSet("BEAM_UPDATE_MS", "100")

# ===== Motor parameters =====
epicsEnvSet("MOTOR_VELO",    "1.0")
epicsEnvSet("MOTOR_ACCL",    "0.5")
epicsEnvSet("MOTOR_HLM",     "100.0")
epicsEnvSet("MOTOR_LLM",     "-100.0")
epicsEnvSet("MOTOR_MRES",    "0.001")
epicsEnvSet("MOTOR_POLL_MS", "100")

# ===== MovingDot detector parameters =====
epicsEnvSet("DOT_SIZE_X",       "640")
epicsEnvSet("DOT_SIZE_Y",       "480")
epicsEnvSet("DOT_MAX_MEMORY",   "50000000")
epicsEnvSet("DOT_SIGMA_X",      "50.0")
epicsEnvSet("DOT_SIGMA_Y",      "25.0")
epicsEnvSet("DOT_BACKGROUND",   "1000.0")
epicsEnvSet("DOT_N_PER_I_PER_S","200.0")

# Configure all beamline components
miniBeamlineConfig()

# Load beam current
dbLoadRecords("$(MINI_BEAMLINE)/db/beam_current.template", "P=$(PREFIX)")

# Load point detectors
dbLoadRecords("$(MINI_BEAMLINE)/db/point_detector.template", "P=$(PREFIX),R=ph:,MTR=ph:mtr,DTYP=asynPointDet_PH")
dbLoadRecords("$(MINI_BEAMLINE)/db/point_detector.template", "P=$(PREFIX),R=edge:,MTR=edge:mtr,DTYP=asynPointDet_EDGE")
dbLoadRecords("$(MINI_BEAMLINE)/db/point_detector.template", "P=$(PREFIX),R=slit:,MTR=slit:mtr,DTYP=asynPointDet_SLIT")

# Load moving dot detector
dbLoadRecords("$(MINI_BEAMLINE)/db/moving_dot.template", "P=$(PREFIX),R=dot:cam:,PORT=DOT,DTYP=asynMovingDot")
