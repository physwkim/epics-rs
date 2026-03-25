# random-signals

4채널 랜덤 시그널 IOC — 10ms (100Hz) 주기로 업데이트되는 PV 4개를 생성합니다.

## PV 목록

| PV 이름 | 설명 |
|---|---|
| `BLM:001:SA:A` | 채널 A 랜덤 시그널 (-10.0 ~ 10.0) |
| `BLM:001:SA:B` | 채널 B 랜덤 시그널 (-10.0 ~ 10.0) |
| `BLM:001:SA:C` | 채널 C 랜덤 시그널 (-10.0 ~ 10.0) |
| `BLM:001:SA:D` | 채널 D 랜덤 시그널 (-10.0 ~ 10.0) |

## 실행

```bash
cargo run --release -p random-signals
```

CA 서버 포트를 변경하려면:

```bash
EPICS_CA_SERVER_PORT=5065 cargo run --release -p random-signals
```

## blm_sum.db와 함께 사용

`blm_sum.db`는 4채널의 합을 계산하는 calc 레코드입니다.

```
dbLoadRecords("blm_sum.db", "P=BLM:001:, ChA=SA:A, ChB=SA:B, ChC=SA:C, ChD=SA:D")
```
