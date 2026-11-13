# 영상 번역 오버레이

외국어 강연·강의 영상을 한국어 자막으로 보는 앱입니다. 앱 안에서 영상을 열고 `영상감지`를 누르면 음성을 인식하고 번역합니다. 처리가 끝나면 알림을 보내고, 재생 위치에 맞춰 자막을 표시합니다.

Android 앱은 PC 없이 실행합니다. 화면, 처리 코어, 음성 인식 엔진을 APK에 포함합니다. PC 프로그램이나 개발 서버를 켤 필요가 없습니다. 영상 다운로드, 최초 모델 다운로드, Gemini 번역에는 인터넷이 필요합니다.

## Android에서 시작하기

Android 7.0 이상, ARM64 기기가 대상입니다. 갤럭시 S21 시리즈도 이 규격에 해당합니다.

1. [ARM64 APK 0.1.6](artifacts/video-translate-overlay-0.1.6-arm64.apk)를 휴대폰에 옮겨 설치합니다. 기존 앱은 지우지 않고 업데이트합니다. 설치 화면에서 요청하면 해당 출처의 앱 설치를 허용합니다.
2. 앱을 열고 `설정`에서 본인 Gemini API 키를 저장합니다. 번역 비용은 키 소유자에게 청구됩니다.
3. 유튜브 계정을 사용할 경우 `설정 → 유튜브 계정 연결`에서 한 번 로그인합니다. 이후 영상은 같은 앱 브라우저의 세션을 사용합니다. 로그인 없이도 영상을 열 수 있습니다.
4. 첫 화면에서 영상 주소를 열고, 영상 위 `영상감지`를 누릅니다. 처음에는 약 60MB 음성 인식 모델을 내려받습니다. 0.1.4로 업데이트한 뒤에도 새 모델을 한 번 받습니다. 모델 외에 오디오와 작업 자료를 저장할 공간도 필요합니다.
5. 자막 준비 알림을 누르고 영상을 재생합니다. 자막은 전체 처리가 끝난 뒤 표시합니다. 영상 위 앱의 `전체화면` 버튼을 누르면 영상과 자막이 가로 전체화면으로 전환됩니다. `화면복귀`나 휴대폰 뒤로가기로 나옵니다. 완료한 자막은 다시 열 때 그대로 표시합니다.

0.1.5까지의 버전에서 만든 자막의 내용이 앞뒤로 밀리면 해당 영상에서 `다시 번역`을 누릅니다. 받아쓰기와 시각은 유지하고 번역만 새로 만듭니다. 추가 API 비용이 발생하며, 이전 번역 비용까지 합쳐 영상별 한도를 확인합니다. 앱 업데이트만으로 기존 자막을 자동 변경하지 않습니다.

APK 링크는 이 작업 폴더의 빌드 결과입니다. `artifacts/`는 Git에 포함하지 않으므로 저장소를 새로 받으면 직접 빌드해야 합니다. 이 APK는 개발용 키로 서명한 release 설치본이며, 크기는 약 24MB입니다. 스토어 배포용 서명은 별도입니다.

## 처리 방식과 지원 범위

| 항목 | 현재 동작 |
| --- | --- |
| 음성 인식 | Android는 whisper.cpp의 다국어 `base q5_1`, PC는 `large-v3-turbo q5_0` 실행 |
| 한국어 번역 | 인식한 텍스트를 `gemini-3.1-flash-lite`에 전송. 용어표를 먼저 만들어 묶음 간 용어 유지 |
| 처리 순서 | 오디오 다운로드 → 음성 인식 → 번역 → 완료 알림 |
| 백그라운드 | Android 포그라운드 서비스와 진행 알림으로 작업 유지. 저장한 진행 지점에서 재시작 |
| 자막 재생 | 일시 정지와 탐색 위치 반영. 광고로 판단한 구간에서는 숨김 |
| 다시 보기 | 같은 영상의 완성된 자막 재사용. 재번역 비용 없음 |
| 영상 소스 | 유튜브와 다운로드 가능한 일반 영상·오디오, 지원 형식의 HLS |
| 지원 제외 | 라이브 방송, DRM 영상, DASH, 목소리 변경 |

오버레이는 앱 안에서 연 영상 위에 표시합니다. 다른 앱의 화면 위에 자막을 띄우는 기능은 없습니다.

앱을 백그라운드로 보내도 처리하도록 구현했습니다. 강제 종료나 운영체제의 프로세스 종료 뒤에는 앱을 다시 열어야 이어집니다. 실제 휴대폰의 처리 속도와 배터리 사용량은 측정하지 않았습니다.

유튜브 오디오는 앱 브라우저의 세션으로 주소를 받은 뒤 Rust 코어가 내려받습니다. 실제 공개 영상의 다운로드와 기기 내 음성 인식을 확인했습니다. 사이트 변경, 로그인 제한, 다운로드 차단에 따라 작업이 거절될 수 있습니다. Google이 앱 내 로그인을 막으면 건너뛰기를 사용할 수 있습니다.

## 모바일 받아쓰기 최적화

0.1.4는 Android의 받아쓰기 연산량을 줄입니다. 기존 작업의 저장된 음성 구간과 자막 시각을 유지하며, 업데이트해도 완료한 구간부터 이어갑니다. 기존 모델 파일을 자동 삭제하지는 않습니다.

| 항목 | Android 0.1.4 동작 |
| --- | --- |
| 모델 | `large-v3-turbo q5_0` 574MB → 다국어 `base q5_1` 60MB. 파일 크기 약 90% 감소 |
| 후보 탐색 | beam 5개 → greedy 1개 |
| 반복 계산 | 온도를 바꿔 최대 6번 시도하던 자동 재처리 제외 |
| 추론 메모리 | 음성 구간마다 새로 만들던 상태·계산 버퍼 재사용. 이전 작업의 텍스트는 넘기지 않음 |
| CPU | 최대 4개 스레드. 기존 최대 8개에서 축소 |
| 언어·시각 | 첫 구간에서 감지한 언어 유지. 모델이 입력 길이를 넘겨 준 끝 시각은 입력 끝으로 제한 |

기존 모델도 이미 양자화돼 있었습니다. 이번 변경의 핵심은 모델 축소와 탐색·반복 비용 감소입니다. [whisper.cpp Android 예제](https://github.com/ggml-org/whisper.cpp/tree/master/examples/whisper.android)도 `tiny/base` 모델을 권장합니다. `base`는 큰 모델보다 악센트·잡음·기술 용어 인식 정확도가 낮아질 수 있습니다.

Flash Attention은 같은 CPU 음성 비교에서 더 느려 적용하지 않았습니다. GPU·NPU 가속과 고급 ARM 명령어를 강제로 켜는 방식도 적용하지 않았습니다. [Android CPU 기능 안내](https://developer.android.com/ndk/guides/cpu-features)에 따라 지원 여부 확인이 필요하며, S21 실기기에서 드라이버·명령어·발열을 검증한 기록은 없습니다. 기존 ARM NEON과 release 최적화 빌드는 사용합니다.

PC release 비교는 같은 14.8초 기술 음성, 음성 구간 4개, 각 설정 2회로 진행했습니다. 모델 로딩을 제외한 받아쓰기 시간입니다. `base`는 Flash Attention 없이 5.0~5.5초, 켰을 때 6.0초였습니다. `small`과 Flash Attention은 19.3~19.6초, `large-v3-turbo`와 기존 beam 설정은 66.9~87.3초였습니다. 모든 비교에서 `deep learning`, `gradient descent`, `transformer`, `attention`을 인식했고, 작업을 바꿔 버퍼를 재사용한 뒤에도 언어와 시각 검사를 통과했습니다. 짧은 합성 음성 비교이므로 실제 강연의 정확도나 S21 속도를 보장하지 않습니다.

비교를 다시 하려면 `VTO_WHISPER_BENCH_MODELS`에 세 모델이 있는 폴더, `VTO_WHISPER_BENCH_AUDIO`에 위 기술 용어가 포함된 WAV를 지정하고 다음 검사를 실행합니다. `VTO_WHISPER_BENCH_RESULT`를 지정하면 JSON 결과도 저장합니다. libclang이 필요합니다.

```sh
cargo test --release -p video-translate-overlay --features whisper --lib whisper::tests::compare_mobile_models_on_technical_speech -- --ignored --nocapture
```

## 비용과 개인정보

번역은 자막 한 조각씩 요청합니다. 앞 문장은 문법·부정을 해석하는 참고로만 보내고, 뒤 문장은 보내지 않습니다. 여러 조각을 합쳐 번역하면서 뒤 문장의 내용이 앞 시각에 들어가던 문제를 막기 위한 방식입니다. 요청 수가 늘어 번역 시간과 비용이 증가할 수 있습니다. 모든 요청에서 같은 용어표와 비용 제한을 사용합니다.

음성 인식은 기기에서 처리합니다. Gemini에는 번역할 텍스트를 보냅니다. 영상·모델 다운로드에 드는 데이터 요금은 별도입니다.

앱은 API 사용량으로 비용을 추정하고, 영상 1시간당 1,000원 기준을 넘으면 추가 번역 요청을 중단합니다. 이는 코드에 설정한 단가와 환율에 따른 제한입니다. 실제 청구액을 보장하지 않으며, 이미 보낸 요청의 비용은 발생할 수 있습니다.

API 키는 Android Keystore로 암호화해 저장합니다. Windows 앱은 자격 증명 관리자를 사용합니다. 설정 화면에는 끝 네 글자만 표시하고, 키 삭제도 지원합니다. PC와 휴대폰의 키·작업·자막은 각각 저장하며 자동 동기화하지 않습니다.

유튜브 로그인은 Google 화면에서 진행합니다. 로그인 쿠키는 앱의 WebView 프로필에 저장하며, Android에서 앱을 백그라운드로 보낼 때 디스크에 반영합니다. 로그인 후 영상으로 돌아오면 로그인 화면 대신 영상 주소를 기억합니다. 설정의 `유튜브 로그아웃`은 쿠키와 사이트 자료를 지웁니다.

휴대폰의 유튜브 앱·Chrome과는 별도 세션입니다. 휴대폰에 등록된 Google 계정을 자동으로 가져오는 기능은 없습니다. Google이 앱 내 로그인을 차단하면 계정을 연결할 수 없습니다. 앱은 Google 비밀번호를 받거나 저장하지 않습니다.

## 소스에서 실행하기

기본 도구는 Node.js 24, Rust stable, CMake, C/C++ 컴파일러입니다. 실제 음성 인식까지 실행하려면 libclang도 필요합니다.

Windows에서는 Visual Studio Build Tools의 C++ 빌드 도구와 Windows SDK를 설치합니다. macOS에서는 Xcode Command Line Tools를 사용합니다. macOS·Linux에는 API 키 보안 저장소가 구현되지 않아 실제 키 저장을 지원하지 않습니다.

저장소 루트에서 의존성을 설치합니다.

```sh
npm ci --prefer-offline --no-audit --no-fund
```

### Windows 데스크톱 앱

PowerShell에서 `LIBCLANG_PATH`를 `libclang.dll`이 있는 폴더로 지정한 뒤 실행합니다.

```powershell
$env:LIBCLANG_PATH = "C:/path/to/clang/native"
cargo run --release -p video-translate-overlay --features app --bin video-translate-overlay
```

설치된 모델을 사용하려면 `VTO_WHISPER_MODEL`에 모델 파일 경로를 지정합니다. 지정하지 않으면 첫 작업에서 모델을 내려받습니다.

### 브라우저 실행

브라우저 버전은 실행한 PC의 로컬 서버를 사용합니다. Android APK와 별도 실행 방식입니다.

Windows PowerShell:

```powershell
$env:PORT = "8080"
$env:APP_DATA_DIR = "$PWD/data"
$env:LIBCLANG_PATH = "C:/path/to/clang/native"
npm run live
```

macOS·Linux에서 화면과 작업 흐름만 확인하려면 대체 데이터를 사용합니다.

```sh
PORT=8080 APP_DATA_DIR=./data npm run live -- --stand-ins
```

접속 주소는 `http://127.0.0.1:8080/`, 설정은 `/preferences`입니다. 서버는 같은 기기의 접속만 받습니다. `APP_DATA_DIR`에는 SQLite, 오디오, 모델을 저장합니다.

`LIBCLANG_PATH` 없이 실행하면 실제 음성 인식 단계에서 작업이 실패합니다. `--stand-ins`는 오디오·음성 인식·Gemini를 대체하는 화면 확인용 모드입니다. 실제 번역에는 사용할 수 없습니다. Windows에서도 `npm run live -- --stand-ins`로 실행할 수 있습니다.

브라우저 버전은 유튜브 임베드와 직접 재생 가능한 미디어 주소를 지원합니다. 일반 사이트의 영상 감지는 앱 내 브라우저를 사용합니다.

## Android APK 빌드

PC는 APK를 빌드할 때만 필요합니다. 설치 후에는 휴대폰 안의 Rust 코어와 화면을 사용합니다. 세부 환경 설정은 [Tauri 사전 준비](https://tauri.app/start/prerequisites/#android)를 참고합니다.

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

기본 명령은 release APK를 만듭니다. 출력은 `src-tauri/gen/android/app/build/outputs/apk/` 아래에 생성됩니다. 설치하려면 서명이 필요합니다. 배포 서명 설정은 [Tauri Android 서명](https://tauri.app/distribute/sign/android/)을 참고합니다.

개발용 서명으로 바로 설치할 APK가 필요하면 다음 명령을 사용합니다. 개발 서버 연결 없이 앱 자체 화면을 포함합니다.

```sh
cargo tauri android build --debug --apk --target aarch64 --features app
```

Windows에서 네이티브 라이브러리 빌드는 끝났지만 심볼릭 링크 생성에 실패할 수 있습니다. 개발자 모드를 사용하거나 라이브러리를 복사해 패키징합니다. ARM64 release 빌드의 명령은 다음과 같습니다.

```powershell
$jni = "src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a"
New-Item -ItemType Directory -Force $jni | Out-Null
Copy-Item "target/aarch64-linux-android/release/libvto.so" $jni
Copy-Item "$env:NDK_HOME/toolchains/llvm/prebuilt/windows-x86_64/sysroot/usr/lib/aarch64-linux-android/libc++_shared.so" $jni
Push-Location src-tauri/gen/android
./gradlew.bat assembleArm64Release -x rustBuildArm64Release
Pop-Location
```

이 명령도 서명 전 APK를 만듭니다. 디버그 빌드에 적용하려면 라이브러리 경로의 `release`를 `debug`로, Gradle 작업의 `Release`를 `Debug`로 바꿉니다.

## 테스트와 확인 범위

저장소 완료 검사는 아래 전체 명령입니다. CI도 같은 명령을 실행합니다. PowerShell은 `&&`를 지원하는 7 이상을 사용합니다.

```sh
npm ci --prefer-offline --no-audit --no-fund && node scripts/contract-tests.mjs artifacts/junit.xml && npm run test:ui && npm run build
```

| 검사 | 확인 대상 |
| --- | --- |
| `scripts/contract-tests.mjs` | 실제 앱 코어의 계약 테스트, Rust 단위 테스트, Chromium 오버레이 테스트. 결과는 `artifacts/junit.xml` |
| `npm run test:ui` | 첫 화면·설정의 상태와 사용자 동작 |
| `npm run build` | Svelte 화면의 배포 빌드 |

계약 테스트는 외부 서비스와 기기 기능 일부를 대체합니다. 통과 결과가 실제 Gemini 응답이나 유튜브 접속 성공을 보장하지는 않습니다.

0.1.6은 전체 완료 검사를 통과했습니다. 테스트 199개 성공, 3개 건너뜀입니다. 번역 요청당 대상 한 조각, 이전 원문만 참고하는 문맥, 잘못된 응답 번호 거부, 원문·시각·기존 비용을 유지한 재번역, 페이지당 자막 레이어 하나를 추가로 검증했습니다.

문제 영상 `vSCYL8ySh9g`는 Android 앱에서 받아쓴 원문 189개를 실제 Gemini API로 번역했습니다. 재번역 전후 원문·시각이 모두 같고, 8분 57초에는 채팅 화면 설명 대신 토큰 수 설명이 배정됐습니다. 최종 방식의 재번역 사용량은 코드의 단가·환율 기준 약 55원입니다. 단일 영상의 결과이며 실제 청구액이나 S21 실기기 동작을 보장하지 않습니다.

Android 14의 0.1.6 debug APK에서는 실제 유튜브 페이지의 8분 57초 자막, 중복 없는 레이어, 재번역 전후 작업 ID 변경, 가로 전체화면 자막 유지를 확인했습니다. 기기 재번역 검사는 위 실제 API 결과를 저장한 응답 서버를 사용했고, 실제 Gemini 호출은 별도로 검증했습니다. 배포용 ARM64 release APK는 기존과 같은 키로 서명했으며, PC 테스트 서버 설정을 포함하지 않습니다.

2026년 11월 13일 사용자가 갤럭시 S21의 모바일 0.1.6을 직접 실행해 자막 싱크가 개선됐고 정상 동작한다고 확인했습니다. 사용자 실검증 결과이며, 개발 환경에서 S21의 속도·발열을 직접 측정한 결과는 아닙니다.

0.1.5는 전체 완료 검사를 통과했습니다. 테스트 194개 성공, 2개 건너뜀입니다. 모바일 화면 검사는 384×854, PC 화면 검사는 1280×720에서 버튼과 자막이 잘리지 않는지 확인합니다. 로그인 회귀 검사는 설정의 계정 연결, 다음 영상에서 로그인 화면 생략, 앱 재시작, 로그인 후 영상 주소 복원, 오래된 응답 무시를 확인합니다. 오디오 회귀 검사는 브라우저 세션으로 받은 주소 전달, 다른 영상·광고 길이·비기본 언어 트랙·라이브 제외, 잘못된 주소와 네트워크 실패 처리를 확인합니다. 전체화면 회귀 검사는 플레이어와 자막을 함께 확대하고, 가로·세로 전환 뒤에도 자막을 유지하는지 확인합니다. 실제 모델 비교 검사는 0.1.4에서 별도로 실행해 통과했습니다.

ARM64 release APK는 빌드·서명·패키지 검증을 마쳤습니다. PC 독립 실행은 같은 소스의 x86_64 debug APK로 확인했습니다. Android 14 에뮬레이터에서 PC 앱·웹 서버·ADB 포트 전달 없이 Wi-Fi와 모바일 데이터를 끄고 첫 화면과 설정 조회를 검증했습니다. 개발 환경의 S21 Ultra 직접 검증은 없으며, S21 사용자 확인 범위는 위 실검증 기록과 같습니다.

0.1.1의 Android 검사는 같은 소스의 x86_64 debug APK에서 테스트 쿠키 저장 → 앱 백그라운드 전환 → 프로세스 종료 → 새 프로세스에서 쿠키 재조회 순서로 진행했습니다. 앱의 로그아웃 버튼으로 쿠키가 삭제되는 것도 확인했습니다. 실제 Google 계정 인증은 검증하지 않았습니다.

0.1.2는 유튜브의 Trusted Types 보안 정책 때문에 영상감지 버튼 생성이 실패하던 문제를 수정했습니다. HTML 문자열 대신 DOM으로 버튼을 만듭니다. 같은 보안 정책을 적용한 회귀 검사와 실제 모바일 유튜브 페이지에서 버튼 표시·클릭·앱의 API 키 안내 응답을 확인했습니다. 실행 검증은 Android 14 에뮬레이터이며, ARM64 배포본과 같은 오버레이 코드를 사용했습니다. 실제 영상 번역까지 진행한 검사는 아닙니다.

0.1.3은 익명 네이티브 유튜브 요청이 봇 확인에 막혀 오디오를 받지 못하던 문제를 수정했습니다. 앱 브라우저의 세션으로 플레이어 API를 요청하고, 받은 서명된 오디오 주소를 Rust 작업에 저장합니다. 다운로드·음성 인식은 페이지 밖에서 계속 실행합니다. Android 14 에뮬레이터의 0.1.3 APK에서 공개 영상 `jNQXAC9IVRw`의 영상감지 → AAC 309,288바이트 다운로드 → 약 19초 오디오 확인 → 실제 whisper.cpp 음성 인식 → 번역 단계 진입까지 확인했습니다. 음성 인식 결과 3개 구간을 SQLite에서 확인했습니다. 검증용 Gemini 주소는 연결하지 않아 실제 Gemini 번역은 수행하지 않았습니다. 실제 Google 로그인 계정과 S21 Ultra의 ARM64 실행은 검증하지 않았습니다.

0.1.4의 Android 실행 검사는 같은 Android 14 x86_64 에뮬레이터와 같은 공개 유튜브 영상 `jNQXAC9IVRw` 약 19초를 사용했습니다. 0.1.3 debug APK의 받아쓰기 단계는 약 42.7초, 0.1.4 debug APK는 약 3.1초였습니다. 진행 상태를 0.5초 간격으로 읽은 각 1회 측정이며, 모델 로딩은 포함하고 다운로드·Gemini 번역은 제외합니다. 새 모델 59,707,625바이트를 앱이 내려받아 검증한 뒤 사용했고, 인식 결과 3개 구간과 번역 단계 진입을 SQLite에서 확인했습니다. 짧은 기술 음성 검사는 통과했지만 일반 유튜브 샘플에서 일부 단어 오인식도 확인했습니다. 기존 앱 데이터를 유지한 업데이트로 검사했으며, 이전 큰 모델 파일도 남았습니다. ARM64 release APK는 빌드·서명·패키지 검증을 마쳤으나 실제 S21 속도·발열·배터리와 15분 전체 영상 성능은 측정하지 않았습니다.

0.1.5는 Android 브라우저 엔진이 전체화면 요청을 즉시 취소하던 문제를 수정했습니다. 앱이 전체화면 뷰를 표시하고 가로 방향으로 전환하며, 종료 시 화면 방향과 시스템 바를 복원합니다. 앱의 `전체화면` 버튼은 영상만 따로 확대하지 않고 자막이 포함된 플레이어를 확대합니다. Android 14 에뮬레이터의 0.1.5 debug APK에서 실제 유튜브 페이지와 미리 저장한 한국어 자막 테스트 자료로 진입·화면복귀·재진입·뒤로가기를 검증했습니다. 가로 866×412에서 자막이 영상 안에 표시되고, 세로 복귀 후에도 같은 URL·완료 작업 ID·자막이 유지됐습니다. ARM64 release APK는 빌드·서명·패키지 검증을 마쳤습니다. 실제 S21에서 회전·전체화면을 실행한 검사는 아닙니다.

## 코드와 설계 문서

| 경로 | 내용 |
| --- | --- |
| `src/ui/` | 첫 화면과 설정. Svelte |
| `src/overlay/` | 영상 감지 버튼, 진행 상태, 자막 표시 |
| `src-tauri/src/` | Rust 코어, 다운로드, 음성 인식, 번역, 저장소 |
| `src-tauri/gen/android/` | Android 앱, 키 저장소, 백그라운드 서비스 |
| `contract-tests/`, `tests/` | 계약·오버레이·화면 테스트 |
| [백엔드 설계](docs/design/backend.md) | 요구사항과 처리 흐름 |
| [화면 설계](docs/design/frontend.md) | 화면 구성과 상태 |
| [백엔드 구현 안내](src-tauri/README.md) | 파일별 역할과 플랫폼별 제약 |

라이선스: [Apache-2.0](LICENSE)
