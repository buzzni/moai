// 윈도 릴리스 바이너리(moai.exe)에 PE 버전 정보(VERSIONINFO)를 넣는다 (moai-uwzg.kpx).
// 탐색기의 속성·"자세히" 탭과 `(Get-Item moai.exe).VersionInfo` 가 이것을 읽는다.
//
// **의존성을 들이지 않는다.** `winresource`·`embed-resource` 는 윈도 호스트에서 rc.exe(또는
// windres)를 불러야 하고 Cargo.lock 에 크레이트가 느는데, 우리가 넣는 것은 VS_VERSIONINFO
// 하나뿐이다. 그 구조체를 `build/versioninfo.rs` 가 바이트로 써서 `.res` 로 내면 link.exe(와
// lld-link)가 그대로 받는다 — .res 는 CPU 와 무관하니 x86_64 와 aarch64 가 같은 길이고,
// 리눅스에서 바이트 배치를 시험할 수 있다(`tests/versioninfo.rs`).
//
// 가르는 것은 호스트가 아니라 **타깃**이다(CARGO_CFG_TARGET_*): 리눅스에서 윈도로 교차
// 빌드해도 맞게 돌고, 유닉스 타깃에서는 아무것도 안 한다. 코드 서명은 여기 없다 — 따로 선다.
#[path = "build/versioninfo.rs"]
mod versioninfo;

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build/versioninfo.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        return;
    }
    let get = |k: &str| env::var(k).unwrap_or_default();
    let num = |k: &str| get(k).parse::<u16>().unwrap_or(0);
    let version = get("CARGO_PKG_VERSION");
    let copyright = format!("{} License - {}", get("CARGO_PKG_LICENSE"), get("CARGO_PKG_REPOSITORY"));
    let strings = [
        ("CompanyName", "buzzni"),
        ("FileDescription", "moai issue tracker"),
        ("FileVersion", version.as_str()),
        ("InternalName", "moai"),
        ("LegalCopyright", copyright.as_str()),
        ("OriginalFilename", "moai.exe"),
        ("ProductName", "moai"),
        ("ProductVersion", version.as_str()),
    ];
    let data = versioninfo::version_info(&versioninfo::Info {
        version: (num("CARGO_PKG_VERSION_MAJOR"), num("CARGO_PKG_VERSION_MINOR"), num("CARGO_PKG_VERSION_PATCH")),
        strings: &strings,
    });
    let path = PathBuf::from(get("OUT_DIR")).join("moai-version.res");
    fs::write(&path, versioninfo::res_file(&data)).expect("moai-version.res 를 못 썼다");
    // bins 에만 건다 — 시험 실행 파일에 같은 리소스가 겹쳐 들 까닭이 없다.
    println!("cargo:rustc-link-arg-bins={}", path.display());
}
