use anyhow::Result;

pub struct LuaRuntime;

impl LuaRuntime {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    pub fn execute(&self, script: &str) -> Result<()> {
        tracing::debug!("Executing Lua script: {} bytes", script.len());
        Ok(())
    }
}

impl Default for LuaRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to initialize LuaRuntime")
    }
}
