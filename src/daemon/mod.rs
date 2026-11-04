//! 데몬 — 로컬 읽기 API (moai-5n6m).
//!
//! 이 모듈의 `rpc` 는 JSON-RPC 2.0 **메시지 층**이다. 전송(Unix 소켓의 줄, 나중의 HTTP)을 모르고
//! `cmd`·`store` 의 어떤 타입도 이름 대지 않는다 — 명령을 메서드에 묶는 것은 뒤의 멤버 몫이다.
//!
//! `moai daemon` 멤버(moai-5n6m.x8t)가 이것을 부르기 전까지는 아무도 쓰지 않아 dead_code 가 선다.
#![allow(dead_code)]

pub mod rpc;
