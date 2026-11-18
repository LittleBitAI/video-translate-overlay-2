# 개발 안내

앱을 직접 빌드하거나 고치는 사람을 위한 문서입니다. 앱 사용법은 [README](../README.md)에 있습니다.

## 소스에서 실행하기

필요한 도구는 Node.js 24, Rust stable, CMake, C/C++ 컴파일러입니다. 실제 받아쓰기까지 돌리려면 libclang도 필요합니다.

- Windows: Visual Studio Build Tools의 C++ 빌드 도구와 Windows SDK를 설치합니다.
- macOS: Xcode Command Line Tools를 씁니다.
- macOS·Linux에는 API 키 보안 저장소가 없어서 실제 키를 저장할 수 없습니다.

저장소 루트에서 의존성을 설치합니다.

```sh
npm ci --prefer-offline --no-audit --no-fund
```

### Windows 데스크톱 앱

1. `LIBCLANG_PATH`를 `libclang.dll`이 있는 폴더로 지정합니다.
2. [Vulkan SDK](https://vulkan.lunarg.com/sdk/home)를 설치합니다. Windows에서 받아쓰기를 빌드하면(앱과 브라우저 실행 모두) 그래픽카드용 Vulkan 백엔드가 함께 빌드되기 때문입니다. 설치하면 `VULKAN_SDK`가 지정됩니다. 실행할 때는 그래픽카드가 있으면 그래픽카드로, 없으면 CPU로 받아씁니다.
3. 배포자가 앱을 빌드할 때는 기존 데스크톱 OAuth 자격 증명 JSON이 `artifacts/google-oauth/desktop-client.json`에 있거나, 그 경로가 `VTO_GOOGLE_DESKTOP_CLIENT_JSON`에 지정돼 있어야 합니다. [Google 등록 안내](design/google-auth-setup.md)를 따르세요. 설치한 앱의 사용자는 바로 로그인하면 되고, 자격 증명이나 클라이언트 ID를 넣지 않습니다. Android 빌드에는 이 파일이 필요 없습니다.

```powershell
$env:LIBCLANG_PATH = "C:/path/to/clang/native"
cargo run --release -p video-translate-overlay --features app --bin video-translate-overlay
```

이미 받아 둔 모델을 쓰려면 `VTO_WHISPER_MODEL`에 모델 파일 경로를 지정합니다. 지정하지 않으면 첫 작업에서 내려받습니다.

### 브라우저에서 실행

브라우저 버전은 실행한 PC의 로컬 서버를 씁니다. Android APK와는 따로 돌아갑니다.

Windows PowerShell:

```powershell
$env:PORT = "8080"
$env:APP_DATA_DIR = "$PWD/data"
$env:LIBCLANG_PATH = "C:/path/to/clang/native"
npm run live
```

macOS·Linux에서 화면과 작업 흐름만 볼 때는 대체 데이터를 씁니다. Windows에서도 쓸 수 있습니다.

```sh
PORT=8080 APP_DATA_DIR=./data npm run live -- --stand-ins
```

- 주소는 `http://127.0.0.1:8080/`, 설정은 `/preferences`입니다. 서버는 같은 PC의 접속만 받습니다.
- `APP_DATA_DIR`에 SQLite, 오디오, 모델을 저장합니다.
- `LIBCLANG_PATH` 없이 실행하면 받아쓰기 단계에서 작업이 실패합니다.
- `--stand-ins`는 오디오, 받아쓰기, Gemini를 가짜로 바꾸는 화면 확인용 모드라 실제 번역은 안 됩니다.
- 브라우저 버전은 유튜브 임베드와 바로 재생할 수 있는 미디어 주소를 지원합니다. 일반 사이트의 영상 감지는 앱 안 브라우저를 씁니다.

## Android APK 빌드

PC는 APK를 만들 때만 필요합니다. 설치한 뒤에는 휴대폰 안의 Rust 코어와 화면만 씁니다. 자세한 환경 설정은 [Tauri 사전 준비](https://tauri.app/start/prerequisites/#android)를 참고하세요.

| 도구·변수 | 설정 |
| --- | --- |
| JDK | 17, `JAVA_HOME` 지정 |
| Android SDK | Platform 36, Build Tools 36.0.0, Platform Tools 설치. `ANDROID_HOME` 지정 |
| Android NDK | 27.2.12479018, `NDK_HOME` 지정 |
| Ninja | `PATH`에 추가 |
| libclang | `LIBCLANG_PATH` 지정 |
| Rust 대상 | `aarch64-linux-android` |
| Tauri CLI | 2.x |

저장소 루트에서 실행합니다.

```sh
rustup target add aarch64-linux-android
cargo install tauri-cli --version "^2" --locked
npm ci --prefer-offline --no-audit --no-fund
cargo tauri android build --apk --target aarch64 --features app
```

이 명령은 배포용(release) APK를 `src-tauri/gen/android/app/build/outputs/apk/` 아래에 만듭니다. 설치하려면 서명이 필요합니다. 배포 서명은 [Tauri Android 서명](https://tauri.app/distribute/sign/android/)을 참고하세요.

개발용 서명으로 바로 설치할 APK가 필요하면 아래 명령을 씁니다. 개발 서버 없이 앱 화면을 담아 만듭니다.

```sh
cargo tauri android build --debug --apk --target aarch64 --features app
```

Windows에서는 네이티브 라이브러리 빌드는 끝났는데 심볼릭 링크를 만들지 못해 실패할 수 있습니다. 개발자 모드를 켜거나, 라이브러리를 직접 복사해 패키징합니다. ARM64 배포용 빌드라면 다음과 같습니다.

```powershell
$jni = "src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a"
New-Item -ItemType Directory -Force $jni | Out-Null
Copy-Item "target/aarch64-linux-android/release/libvto.so" $jni
Copy-Item "$env:NDK_HOME/toolchains/llvm/prebuilt/windows-x86_64/sysroot/usr/lib/aarch64-linux-android/libc++_shared.so" $jni
Push-Location src-tauri/gen/android
./gradlew.bat assembleArm64Release -x rustBuildArm64Release
Pop-Location
```

이 명령도 서명 전 APK를 만듭니다. 디버그 빌드라면 라이브러리 경로의 `release`를 `debug`로, Gradle 작업의 `Release`를 `Debug`로 바꿉니다.

## 릴리스

승인된 설치 파일은 GitHub Releases로만 배포합니다. `artifacts/`의 로컬 빌드 결과는 커밋하지 않습니다. Android 배포 APK는 기존 설치본과 같은 개발 서명 키를 계속 쓰고, 스토어 서명은 별개입니다. 승인, 패키징, 서명 유지, 첫 공개 확인 절차는 [릴리스 절차](releases/procedure.md)에, 버전별 변경 내용은 [GitHub Releases](https://github.com/LittleBitAI/video-translate-overlay-2/releases)에 있습니다.

## 테스트

저장소의 완료 검사는 아래 명령 전체이고, CI도 같은 명령을 돌립니다. PowerShell은 `&&`를 지원하는 7 이상을 쓰세요.

```sh
npm ci --prefer-offline --no-audit --no-fund && node scripts/contract-tests.mjs artifacts/junit.xml && npm run test:ui && npm run build
```

| 검사 | 확인하는 것 |
| --- | --- |
| `scripts/contract-tests.mjs` | 실제 앱 코어의 계약 테스트, Rust 단위 테스트, Chromium 오버레이 테스트. 결과는 `artifacts/junit.xml` |
| `npm run test:ui` | 첫 화면·설정의 상태와 사용자 동작 |
| `npm run build` | Svelte 화면의 배포용 빌드 |

계약 테스트는 외부 서비스와 기기 기능 일부를 가짜로 바꿔 돌립니다. 통과해도 실제 Gemini 응답이나 유튜브 접속이 성공한다는 보장은 아닙니다.

## 버전별 검증 기록(0.1.1~0.1.6)

### 0.1.6

전체 완료 검사 통과(테스트 199개 성공, 3개 건너뜀). 번역 요청당 대상 한 조각, 이전 원문만 참고하는 문맥, 잘못된 응답 번호 거부, 원문·시각·기존 비용을 유지한 재번역, 페이지당 자막 레이어 하나를 추가로 검증했습니다.

- 문제 영상 `vSCYL8ySh9g`: Android 앱에서 받아쓴 원문 189개를 실제 Gemini API로 번역했습니다. 재번역 전후 원문·시각이 모두 같고, 8분 57초에는 채팅 화면 설명 대신 토큰 수 설명이 배정됐습니다. 최종 방식의 재번역 사용량은 코드의 단가·환율 기준 약 55원입니다. 영상 하나의 결과이며 실제 청구액이나 S21 실기기 동작을 보장하지 않습니다.
- Android 14의 0.1.6 디버그 APK에서 실제 유튜브 페이지의 8분 57초 자막, 중복 없는 레이어, 재번역 전후 작업 ID 변경, 가로 전체화면 자막 유지를 확인했습니다. 기기 재번역 검사는 위 실제 API 결과를 저장한 응답 서버를 썼고, 실제 Gemini 호출은 따로 검증했습니다. 배포용 ARM64 APK는 기존과 같은 키로 서명했고, PC 테스트 서버 설정을 담지 않았습니다.
- 2026년 11월 13일 사용자가 갤럭시 S21에서 0.1.6을 직접 실행해 자막 싱크가 나아졌고 정상 동작한다고 확인했습니다. 사용자 실사용 확인이며, 개발 환경에서 S21의 속도·발열을 잰 결과는 아닙니다.

### 0.1.5

전체 완료 검사 통과(테스트 194개 성공, 2개 건너뜀).
- 화면 검사: 휴대폰 384×854, PC 1280×720에서 버튼과 자막이 잘리지 않는지 확인합니다.
- 로그인 회귀 검사: 설정의 계정 연결, 다음 영상에서 로그인 화면 생략, 앱 재시작, 로그인 뒤 영상 주소 복원, 오래된 응답 무시를 확인합니다.
- 오디오 회귀 검사: 브라우저 세션으로 받은 주소 전달, 다른 영상·광고 길이·기본이 아닌 언어 트랙·라이브 제외, 잘못된 주소와 네트워크 실패 처리를 확인합니다.
- 전체화면: Android 브라우저 엔진이 전체화면 요청을 바로 취소하던 문제를 고쳤습니다. 앱이 전체화면 뷰를 띄우고 가로로 돌리며, 나올 때 화면 방향과 시스템 바를 되돌립니다. `전체화면` 버튼은 자막이 포함된 플레이어 전체를 키웁니다. Android 14 에뮬레이터의 0.1.5 디버그 APK에서 실제 유튜브 페이지와 미리 저장한 한국어 자막으로 진입·화면복귀·재진입·뒤로가기를 확인했습니다. 가로 866×412에서 자막이 영상 안에 보였고, 세로로 돌아온 뒤에도 같은 주소·완료 작업 ID·자막이 유지됐습니다. 실제 S21에서 회전·전체화면을 돌려 본 검사는 아닙니다.
- ARM64 배포용 APK는 빌드·서명·패키지 검증을 마쳤습니다. PC 없이 혼자 도는 것은 같은 소스의 x86_64 디버그 APK로 확인했습니다. Android 14 에뮬레이터에서 PC 앱·웹 서버·ADB 포트 전달 없이, Wi-Fi와 모바일 데이터를 끈 채 첫 화면과 설정 조회를 확인했습니다. 개발 환경에서 S21 Ultra로 직접 확인한 적은 없습니다.

### 0.1.4

Android 14 x86_64 에뮬레이터에서 공개 유튜브 영상 `jNQXAC9IVRw`(약 19초)로 쟀습니다. 받아쓰기 단계가 0.1.3 디버그 APK에서 약 42.7초, 0.1.4에서 약 3.1초였습니다. 진행 상태를 0.5초마다 읽은 각 1회 측정이며, 모델 로딩은 포함하고 다운로드·Gemini 번역은 뺐습니다. 새 모델 59,707,625바이트를 앱이 내려받아 검증한 뒤 썼고, 받아쓴 3개 구간과 번역 단계 진입을 SQLite에서 확인했습니다. 짧은 기술 음성 검사는 통과했지만, 일반 유튜브 샘플에서 일부 단어를 잘못 알아들었습니다. 기존 앱 데이터를 유지한 업데이트로 검사했고, 예전 큰 모델 파일도 남았습니다. 실제 S21의 속도·발열·배터리와 15분짜리 영상 전체 성능은 재지 않았습니다.

### 0.1.3

익명 유튜브 요청이 봇 확인에 막혀 오디오를 받지 못하던 문제를 고쳤습니다. 앱 브라우저의 세션으로 플레이어 API를 요청하고, 받은 서명된 오디오 주소를 Rust 작업에 저장합니다. 다운로드와 받아쓰기는 페이지 밖에서 계속됩니다. Android 14 에뮬레이터에서 공개 영상 `jNQXAC9IVRw`로 영상감지 → AAC 309,288바이트 다운로드 → 약 19초 오디오 확인 → 실제 whisper.cpp 받아쓰기 → 번역 단계 진입까지 확인했고, 받아쓴 3개 구간을 SQLite에서 확인했습니다. 검증용 Gemini 주소를 연결하지 않아 실제 번역은 하지 않았습니다. 실제 Google 로그인 계정과 S21 Ultra의 ARM64 실행은 검증하지 않았습니다.

### 0.1.2

유튜브의 Trusted Types 보안 정책 때문에 영상감지 버튼을 만들지 못하던 문제를 고쳤습니다. HTML 문자열 대신 DOM으로 버튼을 만듭니다. 같은 보안 정책을 건 회귀 검사와, 실제 모바일 유튜브 페이지에서 버튼 표시·클릭·API 키 안내 응답을 확인했습니다. Android 14 에뮬레이터에서 확인했고, ARM64 배포본과 같은 오버레이 코드를 씁니다. 실제 영상 번역까지 해 본 검사는 아닙니다.

### 0.1.1

같은 소스의 x86_64 디버그 APK에서 테스트 쿠키 저장 → 앱 내리기 → 프로세스 종료 → 새 프로세스에서 쿠키 다시 읽기 순서로 확인했습니다. 앱의 로그아웃 버튼으로 쿠키가 지워지는 것도 확인했습니다. 실제 Google 계정 인증은 검증하지 않았습니다.


## 코드와 설계 문서

| 경로 | 내용 |
| --- | --- |
| `src/ui/` | 첫 화면과 설정(Svelte) |
| `src/overlay/` | 영상감지 버튼, 진행 상태, 자막 표시 |
| `src-tauri/src/` | Rust 코어: 다운로드, 받아쓰기, 번역, 저장소 |
| `src-tauri/gen/android/` | Android 앱, 키 저장소, 백그라운드 서비스 |
| `contract-tests/`, `tests/` | 계약·오버레이·화면 테스트 |
| [백엔드 설계](design/backend.md) | 요구사항과 처리 흐름 |
| [화면 설계](design/frontend.md) | 화면 구성과 상태 |
| [백엔드 구현 안내](../src-tauri/README.md) | 파일별 역할과 플랫폼별 제약 |

## 받아쓰기 모델 비교

PC 배포용 빌드에서 같은 14.8초 기술 음성(음성 구간 4개)을 설정마다 2번씩 받아썼습니다. 모델 로딩을 뺀 받아쓰기 시간입니다.

| 모델·설정 | 시간 |
| --- | --- |
| `base`, Flash Attention 끔 | 5.0~5.5초 |
| `base`, Flash Attention 켬 | 6.0초 |
| `small`, Flash Attention 켬 | 19.3~19.6초 |
| `large-v3-turbo`, 기존 beam 설정 | 66.9~87.3초 |

모든 비교에서 `deep learning`, `gradient descent`, `transformer`, `attention`을 알아들었고, 작업을 바꿔 버퍼를 다시 쓴 뒤에도 언어와 시각 검사를 통과했습니다. 짧은 합성 음성이라 실제 강연의 정확도나 S21의 속도를 보장하지는 않습니다.

다시 재려면 `VTO_WHISPER_BENCH_MODELS`에 세 모델이 든 폴더를, `VTO_WHISPER_BENCH_AUDIO`에 위 기술 용어가 들어간 WAV를 지정하고 아래 검사를 실행합니다. `VTO_WHISPER_BENCH_RESULT`를 지정하면 결과를 JSON으로도 저장합니다. libclang이 필요합니다.

```sh
cargo test --release -p video-translate-overlay --features whisper --lib whisper::tests::compare_mobile_models_on_technical_speech -- --ignored --nocapture
```


## 휴대폰 받아쓰기 모델을 줄인 이유(0.1.4)

0.1.4에서 Android의 받아쓰기 계산량을 줄였습니다. 기존 작업의 음성 구간과 자막 시각은 그대로이고, 업데이트해도 끝난 구간부터 이어 갑니다. 예전 모델 파일을 자동으로 지우지는 않습니다.

| 항목 | 바뀐 내용 |
| --- | --- |
| 모델 | `large-v3-turbo q5_0` 574MB → 다국어 `base q5_1` 60MB. 파일 크기 약 90% 감소 |
| 후보 탐색 | beam 5개 → greedy 1개 |
| 반복 계산 | 온도를 바꿔 최대 6번까지 다시 하던 자동 재처리를 뺌 |
| 추론 메모리 | 음성 구간마다 새로 만들던 상태·계산 버퍼를 다시 씀. 이전 구간의 글은 넘기지 않음 |
| CPU | 스레드 최대 8개 → 4개 |
| 언어·시각 | 첫 구간에서 감지한 언어를 유지. 입력 길이를 넘는 끝 시각은 입력 끝으로 자름 |

`base`는 큰 모델보다 억양, 잡음, 기술 용어에 약할 수 있습니다. [whisper.cpp Android 예제](https://github.com/ggml-org/whisper.cpp/tree/master/examples/whisper.android)도 `tiny`·`base` 모델을 권합니다. Flash Attention은 같은 CPU 비교에서 더 느려서 켜지 않았습니다. 휴대폰의 GPU·NPU 가속과 고급 ARM 명령어 강제 사용도 하지 않았습니다. [Android CPU 기능 안내](https://developer.android.com/ndk/guides/cpu-features)대로 지원 여부를 확인해야 하고, S21 실기기에서 드라이버·명령어·발열을 검증한 기록이 없기 때문입니다. 기존 ARM NEON과 배포용 최적화 빌드는 그대로 씁니다.
