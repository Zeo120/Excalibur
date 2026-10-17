use anyhow::Result;

pub struct JsRuntime {
    pub allow_eval: bool,
    pub trace_sinks: bool,
}

impl JsRuntime {
    pub fn new() -> Result<Self> {
        Ok(Self {
            allow_eval: true,
            trace_sinks: true,
        })
    }

    pub fn eval(&self, script: &str) -> Result<String> {
        tracing::debug!("Evaluating JavaScript: {} bytes", script.len());
        Ok("undefined".to_string())
    }
}

impl Default for JsRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to initialize JsRuntime")
    }
}
