# ophyd test IOC — provides PVs expected by ophyd's test suite
# Replaces Docker-based epics-services-for-ophyd

# ===== Simulated Motors =====
# 6 motors matching ophyd test expectations
# simMotorCreate(port, lowLimit, highLimit, pollMs)
simMotorCreate("mtr1", -100, 100, 100)
simMotorCreate("mtr2", -100, 100, 100)
simMotorCreate("mtr3", -100, 100, 100)
simMotorCreate("mtr4", -100, 100, 100)
simMotorCreate("mtr5", -100, 100, 100)
simMotorCreate("mtr6", -100, 100, 100)

dbLoadRecords("$(MOTOR)/motor.template", "P=XF:31IDA-OP{Tbl-Ax:,M=X1}Mtr,PORT=mtr1,VELO=1.0,ACCL=0.5,MRES=0.001")
dbLoadRecords("$(MOTOR)/motor.template", "P=XF:31IDA-OP{Tbl-Ax:,M=X2}Mtr,PORT=mtr2,VELO=1.0,ACCL=0.5,MRES=0.001")
dbLoadRecords("$(MOTOR)/motor.template", "P=XF:31IDA-OP{Tbl-Ax:,M=X3}Mtr,PORT=mtr3,VELO=1.0,ACCL=0.5,MRES=0.001")
dbLoadRecords("$(MOTOR)/motor.template", "P=XF:31IDA-OP{Tbl-Ax:,M=X4}Mtr,PORT=mtr4,VELO=1.0,ACCL=0.5,MRES=0.001")
dbLoadRecords("$(MOTOR)/motor.template", "P=XF:31IDA-OP{Tbl-Ax:,M=X5}Mtr,PORT=mtr5,VELO=1.0,ACCL=0.5,MRES=0.001")
dbLoadRecords("$(MOTOR)/motor.template", "P=XF:31IDA-OP{Tbl-Ax:,M=X6}Mtr,PORT=mtr6,VELO=1.0,ACCL=0.5,MRES=0.001")

# Also load sim: prefix motors used by some tests
simMotorCreate("sim_mtr1", -100, 100, 100)
simMotorCreate("sim_mtr2", -100, 100, 100)
dbLoadRecords("$(MOTOR)/motor.template", "P=sim:,M=mtr1,PORT=sim_mtr1,VELO=1.0,ACCL=0.5,MRES=0.001")
dbLoadRecords("$(MOTOR)/motor.template", "P=sim:,M=mtr2,PORT=sim_mtr2,VELO=1.0,ACCL=0.5,MRES=0.001")

# Fake motor prefix used in tests
simMotorCreate("fake_mtr", -100, 100, 100)
dbLoadRecords("$(MOTOR)/motor.template", "P=XF:31IDA-OP{Tbl-Ax:,M=FakeMtr},PORT=fake_mtr,VELO=1.0,ACCL=0.5,MRES=0.001")

# ===== Sensors =====
# 6 sensors matching ophyd test expectations
dbLoadRecords("$(OPHYD_TEST_IOC)/db/sensor.template", "P=XF:31IDA-BI{Dev:1},R=E-I")
dbLoadRecords("$(OPHYD_TEST_IOC)/db/sensor.template", "P=XF:31IDA-BI{Dev:2},R=E-I")
dbLoadRecords("$(OPHYD_TEST_IOC)/db/sensor.template", "P=XF:31IDA-BI{Dev:3},R=E-I")
dbLoadRecords("$(OPHYD_TEST_IOC)/db/sensor.template", "P=XF:31IDA-BI{Dev:4},R=E-I")
dbLoadRecords("$(OPHYD_TEST_IOC)/db/sensor.template", "P=XF:31IDA-BI{Dev:5},R=E-I")
dbLoadRecords("$(OPHYD_TEST_IOC)/db/sensor.template", "P=XF:31IDA-BI{Dev:6},R=E-I")

# ===== SimDetector AreaDetector =====
simDetectorConfig("SIM", 640, 480, 50000000)

# Load SimDetector records with XF:31IDA-BI{Cam:Tbl}: prefix
epicsEnvSet("EPICS_DB_INCLUDE_PATH", "$(ADSIMDETECTOR)/db:$(ADCORE)/db")
dbLoadRecords("$(ADSIMDETECTOR)/db/simDetector.template", "P=XF:31IDA-BI{Cam:Tbl}:,R=cam1:,PORT=SIM,ADDR=0,TIMEOUT=1")

# Load standard areaDetector plugins
epicsEnvSet("PREFIX", "XF:31IDA-BI{Cam:Tbl}:")
epicsEnvSet("PORT",       "SIM")
epicsEnvSet("QSIZE",      "20")
epicsEnvSet("XSIZE",      "640")
epicsEnvSet("YSIZE",      "480")
epicsEnvSet("NCHANS",     "2048")
epicsEnvSet("CBUFFS",     "500")
epicsEnvSet("NELEMENTS",  "307200")
epicsEnvSet("FTVL",       "SHORT")
epicsEnvSet("TYPE",       "Int16")
< $(ADCORE)/ioc/commonPlugins.cmd

# Also load with ADSIM: prefix for ophyd test compatibility.
# Reuse the same plugin ports — only load record templates, no Configure commands.
epicsEnvSet("PREFIX", "ADSIM:")
dbLoadRecords("$(ADSIMDETECTOR)/db/simDetector.template", "P=ADSIM:,R=cam1:,PORT=SIM,ADDR=0,TIMEOUT=1")
dbLoadRecords("NDStdArrays.template", "P=$(PREFIX),R=image1:,PORT=IMAGE1,NDARRAY_PORT=$(PORT),TYPE=$(TYPE),FTVL=$(FTVL),NELEMENTS=$(NELEMENTS)")
dbLoadRecords("NDFile.template", "P=$(PREFIX),R=netCDF1:,PORT=FileNetCDF1,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDFileTIFF.template", "P=$(PREFIX),R=TIFF1:,PORT=FileTIFF1,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDFileJPEG.template", "P=$(PREFIX),R=JPEG1:,PORT=FileJPEG1,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDFile.template", "P=$(PREFIX),R=Nexus1:,PORT=FileNexus1,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDFileHDF5.template", "P=$(PREFIX),R=HDF1:,PORT=FileHDF1,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDROI.template", "P=$(PREFIX),R=ROI1:,PORT=ROI1,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDROI.template", "P=$(PREFIX),R=ROI2:,PORT=ROI2,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDROI.template", "P=$(PREFIX),R=ROI3:,PORT=ROI3,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDROI.template", "P=$(PREFIX),R=ROI4:,PORT=ROI4,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDROIStat8.template", "P=$(PREFIX),R=ROIStat1:,PORT=ROISTAT1,NCHANS=$(NCHANS),NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDProcess.template", "P=$(PREFIX),R=Proc1:,PORT=PROC1,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDStats.template", "P=$(PREFIX),R=Stats1:,PORT=STATS1,NCHANS=$(NCHANS),XSIZE=$(XSIZE),YSIZE=$(YSIZE),HIST_SIZE=256,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDStats.template", "P=$(PREFIX),R=Stats2:,PORT=STATS2,NCHANS=$(NCHANS),XSIZE=$(XSIZE),YSIZE=$(YSIZE),HIST_SIZE=256,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDStats.template", "P=$(PREFIX),R=Stats3:,PORT=STATS3,NCHANS=$(NCHANS),XSIZE=$(XSIZE),YSIZE=$(YSIZE),HIST_SIZE=256,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDStats.template", "P=$(PREFIX),R=Stats4:,PORT=STATS4,NCHANS=$(NCHANS),XSIZE=$(XSIZE),YSIZE=$(YSIZE),HIST_SIZE=256,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDStats.template", "P=$(PREFIX),R=Stats5:,PORT=STATS5,NCHANS=$(NCHANS),XSIZE=$(XSIZE),YSIZE=$(YSIZE),HIST_SIZE=256,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDTransform.template", "P=$(PREFIX),R=Trans1:,PORT=TRANS1,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDOverlay.template", "P=$(PREFIX),R=Over1:,PORT=OVER1,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDColorConvert.template", "P=$(PREFIX),R=CC1:,PORT=CC1,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDColorConvert.template", "P=$(PREFIX),R=CC2:,PORT=CC2,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDCircularBuff.template", "P=$(PREFIX),R=CB1:,PORT=CB1,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDAttribute.template", "P=$(PREFIX),R=Attr1:,PORT=ATTR1,NCHANS=$(NCHANS),NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDFFT.template", "P=$(PREFIX),R=FFT1:,PORT=FFT1,NCHANS=$(NCHANS),NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDCodec.template", "P=$(PREFIX),R=Codec1:,PORT=CODEC1,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDCodec.template", "P=$(PREFIX),R=Codec2:,PORT=CODEC2,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDBadPixel.template", "P=$(PREFIX),R=BadPix1:,PORT=BADPIX1,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDScatter.template", "P=$(PREFIX),R=Scatter1:,PORT=SCATTER1,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDGather.template", "P=$(PREFIX),R=Gather1:,PORT=GATHER1,NDARRAY_PORT=$(PORT)")
dbLoadRecords("NDPva.template", "P=$(PREFIX),R=Pva1:,PORT=PVA1,NDARRAY_PORT=$(PORT)")
