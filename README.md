# video-translate-overlay-2

외국어로 된 딥러닝·기술 강연과 강의 영상을 한국어 자막으로 보는 앱입니다.
영상 위 오버레이에서 영상감지 버튼을 누르면 오디오를 내려받아 기기 안의 모델(whisper.cpp)이 말을 글로 옮기고(write), gemini-3.1-flash-lite가 그 글을 번역해 말과 같은 때에 뜨는 자막을 만든 뒤 알림을 보냅니다.
작업은 앱을 백그라운드로 보내도 이어지고, 비용은 1시간짜리 영상에 1,000원 안팎입니다.

화면은 둘입니다. 첫 화면 `/`에서 주소를 넣어 영상을 열고 영상감지, 진행 상태, 자막을 보고, `/preferences`에서 Gemini 개인키를 넣거나 지우고 유튜브에서 로그아웃합니다.
설계는 [`docs/design/`](docs/design/)에, 백엔드 파일별 설명과 안드로이드 빌드는 [`src-tauri/README.md`](src-tauri/README.md)에 있습니다.

## 준비

필요한 것은 Node.js 24, Rust(stable, rustup), CMake(오디오 디코더 libopus를 소스에서 빌드), C/C++ 컴파일러입니다.
도구를 깐 뒤 저장소 루트에서 npm 패키지를 받습니다.

Windows (PowerShell):

```powershell
winget install --id OpenJS.NodeJS --exact
winget install --id Rustlang.Rustup --exact
winget install --id Kitware.CMake --exact
winget install --id Microsoft.VisualStudio.2022.BuildTools --exact --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
# 새 터미널을 열고 저장소 루트에서:
npm ci --prefer-offline --no-audit --no-fund
```

macOS (Terminal):

```sh
xcode-select --install
brew install node cmake
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# 새 터미널을 열고 저장소 루트에서:
npm ci --prefer-offline --no-audit --no-fund
```

Rust 크레이트와 테스트용 Chromium은 처음 빌드하거나 테스트할 때 저절로 받습니다.

## 실행

`npm run live`가 화면을 빌드하고 서버(`vto-web`)를 빌드해 띄웁니다.
`PORT`로 포트를, `APP_DATA_DIR`로 데이터 폴더(SQLite, 내려받은 오디오, 모델)를 정합니다.

Windows (PowerShell):

```powershell
$env:PORT = "8080"; $env:APP_DATA_DIR = "$PWD\data"; npm run live
```

macOS:

```sh
PORT=8080 APP_DATA_DIR=./data npm run live
```

그러면 `http://127.0.0.1:8080/`(첫 화면)과 `http://127.0.0.1:8080/preferences`(설정)가 열립니다. 서버는 이 기기(127.0.0.1)에서 오는 요청만 받습니다.

- 기기 안의 모델까지 넣으려면 `LIBCLANG_PATH`를 libclang이 있는 폴더로 정한 뒤 실행합니다(예: `pip install libclang`). 없으면 작업이 write 단계에서 멈춥니다.
- `npm run live -- --stand-ins`는 Gemini, 유튜브 오디오, 모델을 가짜로 바꿔, 개인키 없이 모든 화면 상태를 둘러보게 합니다. 실제 번역에는 쓰지 마세요.
- macOS에는 아직 개인키 저장소가 없어서, `--stand-ins` 없이는 개인키 저장이 거절됩니다.
- Windows 데스크톱 앱(영상 창 위의 오버레이와 알림 포함)은 `LIBCLANG_PATH`를 정한 뒤 `cargo run --release -p video-translate-overlay --features app --bin video-translate-overlay`로 띄웁니다.

## 테스트

```sh
npm ci --prefer-offline --no-audit --no-fund && node scripts/contract-tests.mjs artifacts/junit.xml && npm run test:ui && npm run build
```

이 한 줄이 CI(`.github/workflows/ci.yml`)에서도 도는 검사입니다.

- `scripts/contract-tests.mjs`: 백엔드 계약 테스트(`contract-tests/`)와 Rust 단위 테스트를 cargo-nextest로 돌립니다. 계약 테스트는 실제 앱 코어(명령, SQLite, 작업 흐름)를 `vto-contract-host`로 띄우고 기기 쪽 부품만 가짜로 바꿔 확인합니다. 이어서 오버레이 테스트(`tests/overlay/`)를 Chromium에서 Vitest로 돌리고, 결과를 `artifacts/junit.xml` 하나로 모읍니다.
- `npm run test:ui`: 두 화면(`tests/ui/`)의 상태(비었을 때, 불러오는 중, 오류, 찼을 때)와 동작을 Chromium에서 확인합니다.
- `npm run build`: 화면을 `dist/`로 빌드합니다.

## 라이선스

Apache-2.0 ([`LICENSE`](LICENSE))
