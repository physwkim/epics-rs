# Mini Beamline IOC

[caproto](https://github.com/caproto/caproto)의
[`ioc_examples/mini_beamline.py`](https://github.com/caproto/caproto/blob/master/caproto/ioc_examples/mini_beamline.py)에서
아이디어를 얻어 epics-rs로 재구현한 빔라인 시뮬레이션 IOC입니다.

시뮬레이션된 빔 전류, 1D 포인트 검출기 3개, 2D 영역 검출기 1개, 시뮬레이션 모터 5개를 제공합니다.

## 장치 구성

### Beam Current (빔 전류)

사인파로 진동하는 빔 전류를 시뮬레이션합니다.

```
I(t) = OFFSET + AMPLITUDE * sin(2*pi*t / PERIOD)
```

백그라운드 스레드에서 주기적으로 업데이트되며 I/O Intr 스캔으로 클라이언트에 전달됩니다.

### Point Detectors (1D 검출기)

모터 위치와 빔 전류를 입력으로 받아 스칼라 검출값을 계산합니다. 3종류:

| 이름 | 모드 | 수식 | 기본 sigma | 기본 center |
|------|------|------|-----------|-------------|
| PinHole | Gaussian 피크 | `N * I * exp * e^(-(mtr-center)^2 / 2*sigma^2)` | 5.0 | 0.0 |
| Edge | Error function | `N * I * exp * erfc((center-mtr) / sigma) / 2` | 2.5 | 5.0 |
| Slit | 이중 Error function | `N * I * exp * (erfc((mtr-center)/sigma) - erfc((mtr+center)/sigma)) / 2` | 2.5 | 7.5 |

모터 RBV가 변할 때마다 CP 링크를 통해 자동으로 검출값이 재계산됩니다.

### MovingDot (2D 영역 검출기)

2축 모터(X, Y) 위치에 따라 2D Gaussian 스폿이 이동하는 이미지를 생성하는 영역 검출기입니다.
ADDriver 패턴을 따르며, Single/Multiple/Continuous 이미지 모드를 지원합니다.

- Gaussian 스폿: `sigma_x=50, sigma_y=25` (픽셀 단위, 설정 가능)
- 배경 노이즈: `Poisson(lambda=1000)`
- 셔터 닫힘 시: 배경 노이즈만 출력 (다크 프레임)

### Motors (시뮬레이션 모터)

`motor-rs`의 `SimMotor`를 사용한 5개의 full MotorRecord입니다.

| PV | 용도 |
|----|------|
| `mini:ph:mtr` | PinHole 검출기 모터 |
| `mini:edge:mtr` | Edge 검출기 모터 |
| `mini:slit:mtr` | Slit 검출기 모터 |
| `mini:dot:mtrx` | MovingDot X축 모터 |
| `mini:dot:mtry` | MovingDot Y축 모터 |

## PV 목록

### Beam Current

| PV | 타입 | 설명 |
|----|------|------|
| `mini:current` | ai | 빔 전류 (mA, I/O Intr) |

### Point Detectors

`R` = `ph:`, `edge:`, `slit:` 각각에 대해:

| PV | 타입 | 설명 |
|----|------|------|
| `mini:{R}MotorPos` | ao | 모터 위치 (모터 RBV에서 CP 링크) |
| `mini:{R}BeamCurrent` | ao | 빔 전류 (mini:current에서 CP 링크) |
| `mini:{R}ExposureTime` | ao | 노출 시간 (s, 사용자 설정) |
| `mini:{R}ExposureTime_RBV` | ai | 노출 시간 리드백 |
| `mini:{R}DetValue_RBV` | ai | 검출값 (I/O Intr) |
| `mini:{R}DetSigma` | ao | 검출기 sigma (사용자 설정) |
| `mini:{R}DetSigma_RBV` | ai | sigma 리드백 |
| `mini:{R}DetCenter` | ao | 검출기 center (사용자 설정) |
| `mini:{R}DetCenter_RBV` | ai | center 리드백 |

### MovingDot Area Detector

| PV | 타입 | 설명 |
|----|------|------|
| `mini:dot:cam:Acquire` | bo | 이미지 수집 시작/정지 |
| `mini:dot:cam:Acquire_RBV` | bi | 수집 상태 리드백 |
| `mini:dot:cam:ImageMode` | mbbo | Single / Multiple / Continuous |
| `mini:dot:cam:ImageMode_RBV` | mbbi | 이미지 모드 리드백 |
| `mini:dot:cam:NumImages` | longout | Multiple 모드 이미지 수 |
| `mini:dot:cam:NumImages_RBV` | longin | 이미지 수 리드백 |
| `mini:dot:cam:NumImagesCounter_RBV` | longin | 현재까지 수집된 이미지 수 |
| `mini:dot:cam:AcquireTime` | ao | 노출 시간 (s) |
| `mini:dot:cam:AcquireTime_RBV` | ai | 노출 시간 리드백 |
| `mini:dot:cam:AcquirePeriod` | ao | 수집 주기 (s) |
| `mini:dot:cam:AcquirePeriod_RBV` | ai | 수집 주기 리드백 |
| `mini:dot:cam:DetectorState_RBV` | mbbi | 검출기 상태 (Idle/Acquire/...) |
| `mini:dot:cam:AcquireBusy_RBV` | bi | 수집 중 여부 |
| `mini:dot:cam:ArrayCounter` | longout | 프레임 카운터 (리셋용) |
| `mini:dot:cam:ArrayCounter_RBV` | longin | 프레임 카운터 리드백 |
| `mini:dot:cam:ArrayCallbacks` | bo | NDArray 콜백 활성화 |
| `mini:dot:cam:ArrayCallbacks_RBV` | bi | 콜백 상태 리드백 |
| `mini:dot:cam:MaxSizeX_RBV` | longin | 최대 이미지 너비 |
| `mini:dot:cam:MaxSizeY_RBV` | longin | 최대 이미지 높이 |
| `mini:dot:cam:SizeX` | longout | 이미지 너비 |
| `mini:dot:cam:SizeX_RBV` | longin | 너비 리드백 |
| `mini:dot:cam:SizeY` | longout | 이미지 높이 |
| `mini:dot:cam:SizeY_RBV` | longin | 높이 리드백 |
| `mini:dot:cam:MotorXPos` | ao | X 모터 위치 (dot:mtrx RBV CP) |
| `mini:dot:cam:MotorXPos_RBV` | ai | X 모터 위치 리드백 |
| `mini:dot:cam:MotorYPos` | ao | Y 모터 위치 (dot:mtry RBV CP) |
| `mini:dot:cam:MotorYPos_RBV` | ai | Y 모터 위치 리드백 |
| `mini:dot:cam:BeamCurrent` | ao | 빔 전류 (mini:current CP) |
| `mini:dot:cam:BeamCurrent_RBV` | ai | 빔 전류 리드백 |
| `mini:dot:cam:ShutterOpen` | bo | 셔터 열림/닫힘 |
| `mini:dot:cam:ShutterOpen_RBV` | bi | 셔터 상태 리드백 |
| `mini:dot:cam:Manufacturer_RBV` | stringin | 제조사 ("Mini Beamline") |
| `mini:dot:cam:Model_RBV` | stringin | 모델명 ("Moving Dot") |

## 설정

`ioc/st.cmd` 파일에서 `epicsEnvSet`으로 모든 시뮬레이션 파라미터를 IOC 실행 전에 변경할 수 있습니다.
`miniBeamlineConfig()` 호출 이전에 설정해야 합니다.

### Beam Current 설정

| 변수 | 기본값 | 설명 |
|------|--------|------|
| `BEAM_OFFSET` | 500.0 | DC 오프셋 (mA) |
| `BEAM_AMPLITUDE` | 25.0 | 진폭 (mA) |
| `BEAM_PERIOD` | 4.0 | 진동 주기 (s) |
| `BEAM_UPDATE_MS` | 100 | 업데이트 간격 (ms) |

### Motor 설정

모든 모터에 공통 적용됩니다.

| 변수 | 기본값 | 설명 |
|------|--------|------|
| `MOTOR_VELO` | 1.0 | 이동 속도 |
| `MOTOR_ACCL` | 0.5 | 가속 시간 (s) |
| `MOTOR_HLM` | 100.0 | 상한 리미트 |
| `MOTOR_LLM` | -100.0 | 하한 리미트 |
| `MOTOR_MRES` | 0.001 | 모터 분해능 |
| `MOTOR_POLL_MS` | 100 | 폴링 간격 (ms) |

### MovingDot 설정

| 변수 | 기본값 | 설명 |
|------|--------|------|
| `DOT_SIZE_X` | 640 | 이미지 너비 (px) |
| `DOT_SIZE_Y` | 480 | 이미지 높이 (px) |
| `DOT_MAX_MEMORY` | 50000000 | NDArray 풀 최대 메모리 (bytes) |
| `DOT_SIGMA_X` | 50.0 | Gaussian 스폿 X sigma (px) |
| `DOT_SIGMA_Y` | 25.0 | Gaussian 스폿 Y sigma (px) |
| `DOT_BACKGROUND` | 1000.0 | 배경 노이즈 (Poisson lambda) |
| `DOT_N_PER_I_PER_S` | 200.0 | 빔 전류당 초당 광자 수 |

### 설정 예시

```
# ioc/st.cmd — 고속 빔, 작은 이미지로 테스트
epicsEnvSet("BEAM_PERIOD",    "1.0")
epicsEnvSet("BEAM_AMPLITUDE", "50.0")
epicsEnvSet("DOT_SIZE_X",     "128")
epicsEnvSet("DOT_SIZE_Y",     "96")
epicsEnvSet("MOTOR_VELO",     "10.0")
```

## 빌드 및 실행

```bash
# Release 빌드 (최적화)
cargo build --release -p mini-beamline --features ioc

# 실행
./target/release/mini_ioc examples/mini-beamline/ioc/st.cmd
```

CA 서버 포트는 환경변수 `EPICS_CA_SERVER_PORT`로 변경할 수 있습니다 (기본값: 5064).

```bash
EPICS_CA_SERVER_PORT=5065 ./target/release/mini_ioc examples/mini-beamline/ioc/st.cmd
```

### 동작 확인

```bash
# 빔 전류 모니터링
camonitor mini:current

# 모터 이동 후 검출값 확인
caput mini:ph:mtr 0
camonitor mini:ph:DetValue_RBV

# 모터를 중심에서 멀리 이동 — 검출값 감소
caput mini:ph:mtr 20

# MovingDot 이미지 수집
caput mini:dot:cam:ArrayCallbacks 1
caput mini:dot:cam:ImageMode 0        # Single
caput mini:dot:cam:AcquireTime 0.1
caput mini:dot:cam:Acquire 1
caget mini:dot:cam:ArrayCounter_RBV
```
