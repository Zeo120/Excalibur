//! `excalibur-script`: QuickJS (DOM/Web automation) and Lua (mlua fuzzer hooks) dual scripting engine.

pub mod js;
pub mod lua;

pub use js::JsRuntime;
pub use lua::LuaRuntime;
